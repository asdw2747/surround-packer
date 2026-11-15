use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::model::ChannelLayout;

pub const WAV_HEADER_LEN: usize = 68;
pub const SMPTE_5_1_CHANNEL_MASK: u32 = 0x003F;
pub const SMPTE_7_1_CHANNEL_MASK: u32 = 0x063F;

const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const PCM_SUBFORMAT: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

fn push_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

/// Build a 24-bit WAVEFORMATEXTENSIBLE header for the selected layout.
pub fn wav_header(layout: ChannelLayout, sample_rate: u32, frames: usize) -> Vec<u8> {
    let channel_count = layout.count();
    let data_size = frames.saturating_mul(channel_count).saturating_mul(3);
    let block_align = (channel_count * 3) as u16;
    let byte_rate = sample_rate.saturating_mul(channel_count as u32 * 3);
    let riff_size = (60u64 + data_size as u64).min(u32::MAX as u64) as u32;

    let mut header = Vec::with_capacity(WAV_HEADER_LEN);
    header.extend_from_slice(b"RIFF");
    push_u32(&mut header, riff_size);
    header.extend_from_slice(b"WAVE");

    header.extend_from_slice(b"fmt ");
    push_u32(&mut header, 40);
    push_u16(&mut header, FORMAT_EXTENSIBLE);
    push_u16(&mut header, channel_count as u16);
    push_u32(&mut header, sample_rate);
    push_u32(&mut header, byte_rate);
    push_u16(&mut header, block_align);
    push_u16(&mut header, 24);
    push_u16(&mut header, 22);
    push_u16(&mut header, 24);
    push_u32(&mut header, layout.mask());
    header.extend_from_slice(&PCM_SUBFORMAT);

    header.extend_from_slice(b"data");
    push_u32(&mut header, data_size as u32);
    debug_assert_eq!(header.len(), WAV_HEADER_LEN);
    header
}

/// Convert one f32 sample to a 3-byte little-endian 24-bit PCM sample.
pub fn to_i24_bytes(sample: f32) -> [u8; 3] {
    let value = (sample.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

fn interleave_chunk(channels: &[Vec<f32>], start: usize, frames: usize, buffer: &mut Vec<u8>) {
    buffer.clear();
    buffer.reserve(frames * channels.len() * 3);
    for frame in start..start + frames {
        for channel in channels {
            buffer.extend_from_slice(&to_i24_bytes(channel[frame]));
        }
    }
}

/// Write a 24-bit little-endian WAVEFORMATEXTENSIBLE 5.1 WAV file.
pub fn write_wav24<F>(
    path: &Path,
    layout: ChannelLayout,
    sample_rate: u32,
    channels: &[Vec<f32>],
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
        bail!("channel lengths differ while writing WAV");
    }
    if frames.saturating_mul(channels.len()).saturating_mul(3) > u32::MAX as usize {
        bail!("WAV file exceeds the 4 GiB RIFF limit");
    }

    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::with_capacity(1 << 20, file);
    writer.write_all(&wav_header(layout, sample_rate, frames))?;

    const CHUNK_FRAMES: usize = 8_192;
    let mut buffer = Vec::with_capacity(CHUNK_FRAMES * channels.len() * 3);
    let mut position = 0usize;
    while position < frames {
        let count = CHUNK_FRAMES.min(frames - position);
        interleave_chunk(channels, position, count, &mut buffer);
        writer.write_all(&buffer)?;
        position += count;
        progress(position as f32 / frames.max(1) as f32);
    }
    writer.flush()?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_24_bit_extensible_with_51_mask() {
        let header = wav_header(ChannelLayout::Surround51, 48_000, 1_000);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(header[16..20].try_into().unwrap()), 40);
        assert_eq!(
            u16::from_le_bytes(header[20..22].try_into().unwrap()),
            0xFFFE
        );
        assert_eq!(u16::from_le_bytes(header[22..24].try_into().unwrap()), 6);
        assert_eq!(
            u32::from_le_bytes(header[24..28].try_into().unwrap()),
            48_000
        );
        assert_eq!(
            u32::from_le_bytes(header[28..32].try_into().unwrap()),
            48_000 * 18
        );
        assert_eq!(u16::from_le_bytes(header[32..34].try_into().unwrap()), 18);
        assert_eq!(u16::from_le_bytes(header[34..36].try_into().unwrap()), 24);
        assert_eq!(u16::from_le_bytes(header[36..38].try_into().unwrap()), 22);
        assert_eq!(u16::from_le_bytes(header[38..40].try_into().unwrap()), 24);
        assert_eq!(
            u32::from_le_bytes(header[40..44].try_into().unwrap()),
            SMPTE_5_1_CHANNEL_MASK
        );
        assert_eq!(&header[44..60], &PCM_SUBFORMAT);
        assert_eq!(&header[60..64], b"data");
        assert_eq!(
            u32::from_le_bytes(header[64..68].try_into().unwrap()),
            (1_000 * 6 * 3) as u32
        );
    }

    #[test]
    fn header_is_24_bit_extensible_with_71_mask() {
        let header = wav_header(ChannelLayout::Surround71, 48_000, 1_000);
        assert_eq!(u16::from_le_bytes(header[22..24].try_into().unwrap()), 8);
        assert_eq!(u16::from_le_bytes(header[32..34].try_into().unwrap()), 24);
        assert_eq!(
            u32::from_le_bytes(header[28..32].try_into().unwrap()),
            48_000 * 24
        );
        assert_eq!(
            u32::from_le_bytes(header[40..44].try_into().unwrap()),
            SMPTE_7_1_CHANNEL_MASK
        );
        assert_eq!(
            u32::from_le_bytes(header[64..68].try_into().unwrap()),
            (1_000 * 8 * 3) as u32
        );
    }

    #[test]
    fn interleaving_is_smpte_51_and_71_order() {
        let channels_51: Vec<Vec<f32>> = (0..6)
            .map(|index| vec![(index + 1) as f32 / 8_388_607.0; 2])
            .collect();
        let mut buffer = Vec::new();
        interleave_chunk(&channels_51, 0, 1, &mut buffer);
        let values: Vec<i32> = buffer
            .chunks_exact(3)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
            .collect();
        assert_eq!(values, vec![1, 2, 3, 4, 5, 6]);

        let channels_71: Vec<Vec<f32>> = (0..8)
            .map(|index| vec![(index + 1) as f32 / 8_388_607.0; 2])
            .collect();
        interleave_chunk(&channels_71, 0, 1, &mut buffer);
        let values: Vec<i32> = buffer
            .chunks_exact(3)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
            .collect();
        assert_eq!(values, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test71.wav");
        write_wav24(
            &path,
            ChannelLayout::Surround71,
            48_000,
            &channels_71,
            |_| {},
        )
        .unwrap();
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len(), WAV_HEADER_LEN + 2 * 8 * 3);
        let first = &data[WAV_HEADER_LEN..WAV_HEADER_LEN + 24];
        let first_values: Vec<i32> = first
            .chunks_exact(3)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
            .collect();
        assert_eq!(first_values, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn pcm24_conversion_clips_and_rounds() {
        assert_eq!(to_i24_bytes(0.0), [0, 0, 0]);
        assert_eq!(to_i24_bytes(1.0), [0xFF, 0xFF, 0x7F]);
        assert_eq!(to_i24_bytes(-1.0), [0x01, 0x00, 0x80]);
        assert_eq!(to_i24_bytes(2.0), [0xFF, 0xFF, 0x7F]);
    }
}
