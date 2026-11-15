use std::path::Path;

use anyhow::{bail, Context, Result};
use flacenc::bitsink::ByteSink;
use flacenc::component::{BitRepr, MetadataBlockData};
use flacenc::config;
use flacenc::error::Verify;
use flacenc::source::MemSource;

use crate::model::ChannelLayout;

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

    let mut encoder = config::Encoder::default();
    encoder.block_size = 4_096;
    encoder.multithread = true;
    let encoder = encoder
        .into_verified()
        .map_err(|error| anyhow::anyhow!("invalid FLAC encoder configuration: {error:?}"))?;

    let source = MemSource::from_samples(&interleaved, layout.count(), 24, sample_rate as usize);
    let mut stream = flacenc::encode_with_fixed_block_size(&encoder, source, encoder.block_size)
        .map_err(|error| anyhow::anyhow!("FLAC encoding failed: {error:?}"))?;
    progress(0.85);

    stream.add_metadata_block(
        MetadataBlockData::new_unknown(4, &vorbis_comment_payload(tags, layout))
            .map_err(|error| anyhow::anyhow!("failed to add FLAC metadata: {error:?}"))?,
    );

    let mut sink = ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|error| anyhow::anyhow!("FLAC stream serialization failed: {error:?}"))?;
    std::fs::write(path, sink.as_slice()).with_context(|| format!("write {}", path.display()))?;
    progress(1.0);
    Ok(())
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
