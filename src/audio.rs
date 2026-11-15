use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::routing::{classify_filename, StemKind};

pub const PROJECT_RATE: u32 = 48_000;
pub const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "flac", "mp3", "ogg", "oga"];

#[derive(Clone, Debug)]
pub struct Stem {
    pub id: u64,
    pub path: PathBuf,
    pub display_name: String,
    pub source_sample_rate: u32,
    /// Planar f32 samples, already resampled to [`PROJECT_RATE`].
    pub channels: Vec<Vec<f32>>,
    pub waveform: Vec<f32>,
}

impl Stem {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / PROJECT_RATE as f64
    }

    pub fn kind(&self) -> StemKind {
        classify_filename(&self.display_name)
    }
}

pub fn is_supported_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let lower = extension.to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

pub fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[derive(Clone, Debug)]
pub struct AudioProbe {
    pub channels: usize,
    pub sample_rate: u32,
    pub frames: Option<u64>,
}

impl AudioProbe {
    pub fn duration_secs(&self) -> Option<f64> {
        let frames = self.frames?;
        (self.sample_rate > 0).then(|| frames as f64 / self.sample_rate as f64)
    }
}

#[derive(Clone, Debug)]
pub struct DecodedAudio {
    pub path: PathBuf,
    pub display_name: String,
    pub sample_rate: u32,
    /// Planar samples at the source sample rate.
    pub channels: Vec<Vec<f32>>,
}

impl DecodedAudio {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }
}

pub fn probe_audio(path: &Path) -> Result<AudioProbe> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("probe {}", path.display()))?;
    let track = probed
        .format
        .default_track()
        .ok_or_else(|| anyhow!("no decodable audio track in {}", path.display()))?;
    let params = &track.codec_params;
    Ok(AudioProbe {
        channels: params.channels.map_or(0, |channels| channels.count()),
        sample_rate: params.sample_rate.unwrap_or(0),
        frames: params.n_frames,
    })
}

pub fn decode_audio(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("probe {}", path.display()))?;
    let mut format = probed.format;

    let (track_id, mut decoder) = {
        let track = format
            .default_track()
            .ok_or_else(|| anyhow!("no decodable audio track in {}", path.display()))?;
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .with_context(|| format!("initialize decoder for {}", path.display()))?;
        (track.id, decoder)
    };

    let mut planes: Vec<Vec<f32>> = Vec::new();
    let mut source_sample_rate = 0u32;
    let mut sample_buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(error) => {
                return Err(anyhow!("read {}: {error}", path.display()));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                if planes.is_empty() {
                    source_sample_rate = spec.rate;
                    planes = (0..spec.channels.count()).map(|_| Vec::new()).collect();
                    sample_buffer = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let channels = planes.len();
                let mut buffer = sample_buffer
                    .take()
                    .ok_or_else(|| anyhow!("decoder buffer missing"))?;
                if buffer.capacity() < decoded.capacity() {
                    buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                }
                buffer.copy_interleaved_ref(decoded);
                for frame in buffer.samples().chunks_exact(channels) {
                    for (channel, plane) in planes.iter_mut().enumerate() {
                        plane.push(frame[channel]);
                    }
                }
                sample_buffer = Some(buffer);
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => {
                return Err(anyhow!("decode {}: {error}", path.display()));
            }
        }
    }

    if planes.is_empty() || planes[0].is_empty() {
        return Err(anyhow!("{} contains no audio samples", path.display()));
    }

    Ok(DecodedAudio {
        path: path.to_path_buf(),
        display_name: display_name(path),
        sample_rate: source_sample_rate,
        channels: planes,
    })
}

pub fn load_stem(path: &Path, id: u64) -> Result<Stem> {
    let decoded = decode_audio(path)?;
    let source_sample_rate = decoded.sample_rate;
    let mut planes = decoded.channels;
    if source_sample_rate != PROJECT_RATE {
        for plane in &mut planes {
            *plane = resample_plane(plane, source_sample_rate, PROJECT_RATE);
        }
    }

    let waveform = build_waveform(&planes);
    Ok(Stem {
        id,
        path: decoded.path,
        display_name: decoded.display_name,
        source_sample_rate,
        channels: planes,
        waveform,
    })
}

fn build_waveform(planes: &[Vec<f32>]) -> Vec<f32> {
    const BUCKETS: usize = 256;
    let frames = planes.first().map_or(0, Vec::len);
    if frames == 0 {
        return Vec::new();
    }
    let bucket_size = frames.div_ceil(BUCKETS).max(1);
    let mut waveform = Vec::with_capacity(BUCKETS);
    for bucket in 0..BUCKETS {
        let start = bucket * bucket_size;
        if start >= frames {
            break;
        }
        let end = ((bucket + 1) * bucket_size).min(frames);
        let mut peak = 0.0f32;
        for plane in planes.iter().take(2) {
            for sample in &plane[start..end] {
                peak = peak.max(sample.abs());
            }
        }
        waveform.push(peak);
    }
    waveform
}

fn sinc(value: f64) -> f64 {
    if value.abs() < 1e-12 {
        1.0
    } else {
        let x = std::f64::consts::PI * value;
        x.sin() / x
    }
}

fn blackman(value: f64) -> f64 {
    if value.abs() > 1.0 {
        return 0.0;
    }
    let x = std::f64::consts::PI * value;
    0.42 + 0.5 * x.cos() + 0.08 * (2.0 * x).cos()
}

/// Windowed-sinc resampling for one plane.
pub fn resample_plane(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source_rate == target_rate || input.is_empty() || source_rate == 0 || target_rate == 0 {
        return input.to_vec();
    }

    let ratio = target_rate as f64 / source_rate as f64;
    let output_len = ((input.len() as f64) * ratio).round().max(1.0) as usize;
    let mut output = Vec::with_capacity(output_len);

    const HALF_TAPS: i32 = 8;
    let cutoff = if target_rate < source_rate {
        0.95 * ratio
    } else {
        0.95
    };

    for output_index in 0..output_len {
        let source_position = output_index as f64 / ratio;
        let base = source_position.floor() as i64;
        let mut accumulator = 0.0f64;
        let mut weight_sum = 0.0f64;

        for tap in -HALF_TAPS..=HALF_TAPS {
            let source_index = base + i64::from(tap);
            if source_index < 0 || source_index as usize >= input.len() {
                continue;
            }
            let distance = source_position - source_index as f64;
            let window = blackman(distance / (HALF_TAPS as f64 + 1.0));
            let weight = sinc(distance * cutoff) * window;
            accumulator += input[source_index as usize] as f64 * weight;
            weight_sum += weight;
        }

        let sample = if weight_sum.abs() > 1e-9 {
            accumulator / weight_sum
        } else {
            let nearest = source_position.round().clamp(0.0, (input.len() - 1) as f64) as usize;
            input[nearest] as f64
        };
        output.push(sample as f32);
    }

    output
}

pub fn is_batch_supported_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "wav" | "flac"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_length_and_preserves_tone() {
        let source_rate = 44_100;
        let frames = source_rate as usize;
        let input: Vec<f32> = (0..frames)
            .map(|frame| {
                let t = frame as f32 / source_rate as f32;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
            })
            .collect();
        let output = resample_plane(&input, source_rate, 48_000);
        assert_eq!(output.len(), 48_000);
        let rms = (output[4_800..]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / (output.len() - 4_800) as f32)
            .sqrt();
        assert!(rms > 0.3 && rms < 0.4, "unexpected RMS {rms}");
    }

    #[test]
    fn waveform_has_expected_buckets() {
        let planes = vec![vec![0.0, 0.5, -1.0, 0.25]; 6];
        let waveform = build_waveform(&planes);
        assert!(!waveform.is_empty());
        assert!(waveform.iter().any(|peak| (*peak - 1.0).abs() < 1e-6));
    }
}
