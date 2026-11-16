use std::path::Path;

use anyhow::{bail, Context, Result};
use libflac_rs::{Encoder, EncoderConfig};

use crate::model::ChannelLayout;

/// libFLAC compression level 5 balances size and speed for 24-bit multichannel.
const FLAC_COMPRESSION_LEVEL: u32 = 5;
const FLAC_BLOCK_SIZE: u32 = 4_096;

pub const FLAC_CHANNEL_MASK_COMMENT: &str = "WAVEFORMATEXTENSIBLE_CHANNEL_MASK=0x003F";
pub const FLAC_7_1_CHANNEL_MASK_COMMENT: &str = "WAVEFORMATEXTENSIBLE_CHANNEL_MASK=0x063F";

#[derive(Clone, Debug)]
pub struct FlacTags {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub encoder: String,
}

impl Default for FlacTags {
    fn default() -> Self {
        Self {
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            encoder: format!("SurroundPacker {}", env!("CARGO_PKG_VERSION")),
        }
    }
}

fn push_le_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_comment(buffer: &mut Vec<u8>, comment: &str) {
    let bytes = comment.as_bytes();
    push_le_u32(buffer, bytes.len().min(u32::MAX as usize) as u32);
    buffer.extend_from_slice(bytes);
}

/// Build a FLAC VORBIS_COMMENT block payload with the 5.1 mask and metadata tags.
pub fn vorbis_comment_payload(tags: &FlacTags, layout: ChannelLayout) -> Vec<u8> {
    let mask_comment = if layout.is_71() {
        FLAC_7_1_CHANNEL_MASK_COMMENT
    } else {
        FLAC_CHANNEL_MASK_COMMENT
    };
    let vendor = tags.encoder.as_str();
    let mut comments = vec![mask_comment.to_string()];
    for (key, value) in [
        ("ARTIST", &tags.artist),
        ("TITLE", &tags.title),
        ("ALBUM", &tags.album),
        ("ENCODER", &tags.encoder),
    ] {
        let value = value.trim();
        if !value.is_empty() {
            comments.push(format!("{key}={value}"));
        }
    }

    let mut payload = Vec::new();
    push_le_u32(&mut payload, vendor.len().min(u32::MAX as usize) as u32);
    payload.extend_from_slice(vendor.as_bytes());
    push_le_u32(&mut payload, comments.len() as u32);
    for comment in comments {
        push_comment(&mut payload, &comment);
    }
    payload
}

fn to_pcm24(sample: f32) -> i32 {
    (sample.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32
}

/// Encode a 24-bit, 6-channel FLAC file natively.
pub fn write_flac24<F>(
    path: &Path,
    layout: ChannelLayout,
    sample_rate: u32,
    channels: &[Vec<f32>],
    tags: &FlacTags,
    mut progress: F,
) -> Result<()>
where
    F: FnMut(f32),
{
    if channels.len() != layout.count() {
        bail!(
            "layout {} needs {} channels, got {}",
            layout.label(),
            layout.count(),
            channels.len()
        );
    }
    let frames = channels[0].len();
    if channels.iter().any(|channel| channel.len() != frames) {
        bail!("channel lengths differ while encoding FLAC");
    }
    if frames == 0 {
        bail!("cannot encode an empty FLAC stream");
    }

    progress(0.05);
    let mut interleaved = Vec::with_capacity(frames * channels.len());
    for frame in 0..frames {
        for channel in channels {
            interleaved.push(to_pcm24(channel[frame]));
        }
    }
    progress(0.2);

    let encoder = Encoder::new(
        EncoderConfig::new(layout.count() as u32, 24, sample_rate)
            .with_block_size(FLAC_BLOCK_SIZE)
            .with_compression_level(FLAC_COMPRESSION_LEVEL)
            .with_md5(true),
    );
    let stream = encoder.encode(&interleaved);
    progress(0.85);

    // libFLAC's default Vorbis block only carries a vendor string. Replace it
    // with the channel mask and user tags while leaving STREAMINFO and frames
    // untouched, including the audio MD5.
    let payload = vorbis_comment_payload(tags, layout);
    let stream = replace_vorbis_comment(stream, &payload)?;
    std::fs::write(path, stream).with_context(|| format!("write {}", path.display()))?;
    progress(1.0);
    Ok(())
}

/// Replace the first VORBIS_COMMENT block while preserving all other metadata
/// and the audio frames.
fn replace_vorbis_comment(stream: Vec<u8>, payload: &[u8]) -> Result<Vec<u8>> {
    if stream.len() < 4 || &stream[..4] != b"fLaC" {
        bail!("FLAC encoder did not produce a valid stream");
    }
    if payload.len() > 0x00FF_FFFF {
        bail!("FLAC Vorbis comment block is too large");
    }

    let mut offset = 4usize;
    let mut metadata: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut last = false;
    while !last {
        if offset + 4 > stream.len() {
            bail!("truncated FLAC metadata");
        }
        let header = stream[offset];
        last = header & 0x80 != 0;
        let block_type = header & 0x7F;
        let length = ((stream[offset + 1] as usize) << 16)
            | ((stream[offset + 2] as usize) << 8)
            | stream[offset + 3] as usize;
        offset += 4;
        if offset + length > stream.len() {
            bail!("truncated FLAC metadata block");
        }
        metadata.push((block_type, stream[offset..offset + length].to_vec()));
        offset += length;
    }
    let audio = &stream[offset..];

    let mut output = Vec::with_capacity(stream.len() + payload.len() + 4);
    output.extend_from_slice(b"fLaC");
    for (block_type, data) in metadata.iter().filter(|(block_type, _)| *block_type != 4) {
        output.push(*block_type);
        output.extend_from_slice(&(data.len() as u32).to_be_bytes()[1..]);
        output.extend_from_slice(data);
    }
    output.push(0x80 | 4);
    output.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]);
    output.extend_from_slice(payload);
    output.extend_from_slice(audio);
    Ok(output)
}

/// Parse Vorbis comments from a FLAC file. Exposed for tests and diagnostics.
pub fn read_flac_comments(data: &[u8]) -> Result<Vec<String>> {
    if data.len() < 4 || &data[..4] != b"fLaC" {
        bail!("not a FLAC stream");
    }
    let mut offset = 4usize;
    while offset + 4 <= data.len() {
        let header = data[offset];
        let last = header & 0x80 != 0;
        let block_type = header & 0x7F;
        let length = ((data[offset + 1] as usize) << 16)
            | ((data[offset + 2] as usize) << 8)
            | data[offset + 3] as usize;
        offset += 4;
        if offset + length > data.len() {
            bail!("truncated FLAC metadata block");
        }
        if block_type == 4 {
            let payload = &data[offset..offset + length];
            let mut cursor = 0usize;
            if payload.len() < 4 {
                bail!("truncated Vorbis comment");
            }
            let vendor_len =
                u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + vendor_len + 4 > payload.len() {
                bail!("truncated Vorbis vendor string");
            }
            cursor += vendor_len;
            let count =
                u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let mut comments = Vec::with_capacity(count);
            for _ in 0..count {
                if cursor + 4 > payload.len() {
                    bail!("truncated Vorbis comment length");
                }
                let len =
                    u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4;
                if cursor + len > payload.len() {
                    bail!("truncated Vorbis comment value");
                }
                comments.push(String::from_utf8_lossy(&payload[cursor..cursor + len]).into_owned());
                cursor += len;
            }
            return Ok(comments);
        }
        offset += length;
        if last {
            break;
        }
    }
    bail!("no Vorbis comment metadata block found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorbis_comment_contains_mask_and_tags() {
        let tags = FlacTags {
            artist: "Test Artist".into(),
            title: "Test Title".into(),
            album: "Test Album".into(),
            ..FlacTags::default()
        };
        let payload = vorbis_comment_payload(&tags, ChannelLayout::Surround51);
        let text = String::from_utf8_lossy(&payload);
        assert!(text.contains(FLAC_CHANNEL_MASK_COMMENT));
        assert!(text.contains("ARTIST=Test Artist"));
        assert!(text.contains("TITLE=Test Title"));
        assert!(text.contains("ALBUM=Test Album"));
        assert!(text.contains("ENCODER=SurroundPacker"));
    }

    fn test_signal_seconds(seconds: f32) -> Vec<Vec<f32>> {
        let frames = (48_000.0 * seconds) as usize;
        let frequencies = [220.0, 277.18, 329.63, 55.0, 440.0, 523.25];
        frequencies
            .iter()
            .map(|frequency| {
                (0..frames)
                    .map(|frame| {
                        let t = frame as f32 / 48_000.0;
                        let fundamental = 0.2 * (2.0 * std::f32::consts::PI * frequency * t).sin();
                        let harmonic =
                            0.05 * (2.0 * std::f32::consts::PI * frequency * 2.0 * t).sin();
                        fundamental + harmonic
                    })
                    .collect()
            })
            .collect()
    }

    fn expected_interleaved(channels: &[Vec<f32>]) -> Vec<i32> {
        let mut expected = Vec::with_capacity(channels[0].len() * channels.len());
        for frame in 0..channels[0].len() {
            for channel in channels {
                expected.push(to_pcm24(channel[frame]));
            }
        }
        expected
    }

    #[test]
    fn ten_seconds_of_24_bit_six_channel_flac_compresses_and_is_bit_exact() {
        let channels = test_signal_seconds(10.0);
        let expected = expected_interleaved(&channels);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("compressed.flac");
        write_flac24(
            &path,
            ChannelLayout::Surround51,
            48_000,
            &channels,
            &FlacTags::default(),
            |_| {},
        )
        .unwrap();

        let data = std::fs::read(&path).unwrap();
        assert!(
            data.len() < 8_000_000,
            "10 s 24-bit 5.1 FLAC is {} bytes; compression is not active",
            data.len()
        );
        assert!(
            data.len() < expected.len() * 3,
            "FLAC is larger than the raw 24-bit PCM payload"
        );

        let decoded = libflac_rs::decode(&data).expect("decode FLAC");
        assert_eq!(decoded.channels, 6);
        assert_eq!(decoded.bits_per_sample, 24);
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.total_samples, 480_000);
        assert!(decoded.md5_ok, "FLAC audio MD5 mismatch");
        assert_eq!(decoded.interleaved, expected);

        let comments = read_flac_comments(&data).unwrap();
        assert!(comments
            .iter()
            .any(|comment| comment == FLAC_CHANNEL_MASK_COMMENT));
    }

    #[test]
    fn noise_to_silence_transition_stays_compressed_and_lossless() {
        let frames = 48_000usize;
        let mut state = 123_456u32;
        let mut channels = Vec::new();
        for _ in 0..6 {
            let mut channel = Vec::with_capacity(frames);
            for frame in 0..frames {
                if frame < frames / 2 {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    channel.push(state as f32 / u32::MAX as f32 * 2.0 - 1.0);
                } else {
                    channel.push(0.0);
                }
            }
            channels.push(channel);
        }
        let expected = expected_interleaved(&channels);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("transition.flac");
        write_flac24(
            &path,
            ChannelLayout::Surround51,
            48_000,
            &channels,
            &FlacTags::default(),
            |_| {},
        )
        .unwrap();

        let data = std::fs::read(&path).unwrap();
        assert!(
            data.len() < 1_000_000,
            "noise-to-silence block produced {} bytes",
            data.len()
        );
        let decoded = libflac_rs::decode(&data).expect("decode FLAC");
        assert!(decoded.md5_ok);
        assert_eq!(decoded.interleaved, expected);
    }

    #[test]
    fn encodes_24_bit_six_channel_flac_with_metadata() {
        let mut channels: Vec<Vec<f32>> = (0..6).map(|_| vec![0.0; 512]).collect();
        for (index, channel) in channels.iter_mut().enumerate() {
            for (frame, sample) in channel.iter_mut().enumerate() {
                *sample = ((frame + index * 7) as f32 * 0.001).sin() * 0.1;
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.flac");
        let tags = FlacTags {
            artist: "SurroundPacker".into(),
            title: "Unit Test".into(),
            album: "Tests".into(),
            ..FlacTags::default()
        };
        write_flac24(
            &path,
            ChannelLayout::Surround51,
            48_000,
            &channels,
            &tags,
            |_| {},
        )
        .unwrap();
        let data = std::fs::read(&path).unwrap();
        assert_eq!(&data[..4], b"fLaC");
        let comments = read_flac_comments(&data).unwrap();
        assert!(comments
            .iter()
            .any(|comment| comment == FLAC_CHANNEL_MASK_COMMENT));
        assert!(comments.iter().any(|comment| comment == "TITLE=Unit Test"));
        assert!(comments
            .iter()
            .any(|comment| comment == "ARTIST=SurroundPacker"));
    }
}
