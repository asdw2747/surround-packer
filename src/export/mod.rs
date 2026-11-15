pub mod dvd;
pub mod ffmpeg;
pub mod flac;
pub mod wav;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::mixdown::{self, Mix, MixSnapshot};
use crate::model::{ChannelLayout, MAX_CHANNELS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Wav24,
    Flac24,
    Ac3,
    Eac3,
    Dts,
}

impl Format {
    pub const ALL: [Self; 5] = [Self::Wav24, Self::Flac24, Self::Ac3, Self::Eac3, Self::Dts];
    pub const DEFAULT: [Self; 5] = [Self::Wav24, Self::Flac24, Self::Ac3, Self::Eac3, Self::Dts];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wav24 => "24-bit PCM WAV",
            Self::Flac24 => "24-bit FLAC",
            Self::Ac3 => "Dolby Digital (AC3)",
            Self::Eac3 => "Dolby Digital Plus (E-AC3)",
            Self::Dts => "DTS 5.1",
        }
    }

    pub const fn short(self) -> &'static str {
        match self {
            Self::Wav24 => "WAV",
            Self::Flac24 => "FLAC",
            Self::Ac3 => "AC3",
            Self::Eac3 => "E-AC3",
            Self::Dts => "DTS",
        }
    }

    pub const fn requires_ffmpeg(self) -> bool {
        matches!(self, Self::Ac3 | Self::Eac3 | Self::Dts)
    }

    pub const fn uses_container(self) -> bool {
        matches!(self, Self::Ac3 | Self::Eac3 | Self::Dts)
    }

    pub const fn tooltip(self) -> &'static str {
        match self {
            Self::Wav24 | Self::Flac24 => "Encoded natively in Rust; no ffmpeg required.",
            _ => "Requires ffmpeg on PATH.",
        }
    }

    pub const fn codec(self) -> Option<ffmpeg::Codec> {
        match self {
            Self::Ac3 => Some(ffmpeg::Codec::Ac3),
            Self::Eac3 => Some(ffmpeg::Codec::Eac3),
            Self::Dts => Some(ffmpeg::Codec::Dts),
            Self::Wav24 | Self::Flac24 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerChoice {
    #[default]
    Mka,
    Raw,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportSettings {
    pub output_dir: PathBuf,
    pub base_name: String,
    pub formats: Vec<Format>,
    pub dts_bitrate: u32,
    pub container: ContainerChoice,
    pub artist: String,
    pub title: String,
    pub album: String,
    #[serde(default)]
    pub allow_eac3_51_fallback: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            output_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            base_name: "SurroundPacker 5.1 Mix".to_string(),
            formats: Format::DEFAULT.to_vec(),
            dts_bitrate: ffmpeg::DTS_BITRATE_HIGH,
            container: ContainerChoice::Mka,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            allow_eac3_51_fallback: false,
        }
    }
}

impl ExportSettings {
    pub fn sanitized_base_name(&self) -> String {
        let mut name = self.base_name.trim().to_string();
        if name.is_empty() {
            name = "SurroundPacker 5.1 Mix".to_string();
        }
        for invalid in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
            name = name.replace(invalid, "_");
        }
        name.trim_matches(['.', ' ']).to_string()
    }

    pub fn output_path(&self, format: Format) -> Result<PathBuf> {
        let base = self.sanitized_base_name();
        let extension = match format {
            Format::Wav24 => "wav".to_string(),
            Format::Flac24 => "flac".to_string(),
            _ => ffmpeg::output_extension(
                format.codec().expect("ffmpeg format"),
                match self.container {
                    ContainerChoice::Mka => ffmpeg::Container::Mka,
                    ContainerChoice::Raw => ffmpeg::Container::Raw,
                },
            )
            .to_string(),
        };
        let file_name = match format {
            Format::Wav24 | Format::Flac24 => format!("{base}.{extension}"),
            _ => format!("{base} - {}.{extension}", format.short()),
        };
        Ok(self.output_dir.join(file_name))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExportReport {
    pub outputs: Vec<PathBuf>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub elapsed_secs: f64,
}

#[derive(Clone, Debug)]
pub enum ExportEvent {
    Progress {
        fraction: f32,
        stage: String,
    },
    Peaks {
        pre_db: [f32; MAX_CHANNELS],
        post_db: [f32; MAX_CHANNELS],
        limiter_active: bool,
        gate_reduction_db: [f32; MAX_CHANNELS],
    },
    Log(String),
    Done(ExportReport),
    Failed(String),
}

fn send_progress(tx: &Sender<ExportEvent>, fraction: f32, stage: &str) {
    let _ = tx.send(ExportEvent::Progress {
        fraction: fraction.clamp(0.0, 1.0),
        stage: stage.to_string(),
    });
}

fn send_log(tx: &Sender<ExportEvent>, message: impl Into<String>) {
    let _ = tx.send(ExportEvent::Log(message.into()));
}

fn temp_wav_path(base: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "surround-packer-{}-{}-{stamp}.wav",
        std::process::id(),
        base.replace([' ', '/', '\\'], "_")
    ))
}

/// Run an export synchronously on a worker thread and stream progress through `tx`.
pub fn run_export(
    snapshot: &MixSnapshot,
    settings: &ExportSettings,
    formats: &[Format],
    tx: Sender<ExportEvent>,
) {
    let started = Instant::now();
    let result = run_export_inner(snapshot, settings, formats, &tx);

    match result {
        Ok(mut report) => {
            report.elapsed_secs = started.elapsed().as_secs_f64();
            let _ = tx.send(ExportEvent::Done(report));
        }
        Err(error) => {
            let _ = tx.send(ExportEvent::Failed(format!("{error:#}")));
        }
    }
}

fn ensure_temp_51(mix: &Mix, slot: &mut Option<PathBuf>, base: &str) -> Result<PathBuf> {
    if let Some(path) = slot {
        return Ok(path.clone());
    }
    let path = temp_wav_path(&format!("{base}-51"));
    let folded = if mix.layout.is_71() {
        mix.fold_to_5_1().to_vec()
    } else {
        mix.planes.clone()
    };
    wav::write_wav24(
        &path,
        ChannelLayout::Surround51,
        mix.sample_rate(),
        &folded,
        |_| {},
    )?;
    *slot = Some(path.clone());
    Ok(path)
}

fn ensure_temp_71(mix: &Mix, slot: &mut Option<PathBuf>, base: &str) -> Result<PathBuf> {
    if let Some(path) = slot {
        return Ok(path.clone());
    }
    let path = temp_wav_path(&format!("{base}-71"));
    wav::write_wav24(
        &path,
        ChannelLayout::Surround71,
        mix.sample_rate(),
        &mix.planes,
        |_| {},
    )?;
    *slot = Some(path.clone());
    Ok(path)
}

fn run_export_inner(
    snapshot: &MixSnapshot,
    settings: &ExportSettings,
    formats: &[Format],
    tx: &Sender<ExportEvent>,
) -> Result<ExportReport> {
    if formats.is_empty() {
        bail!("Select at least one export format.");
    }
    if !snapshot.has_audible_routes() {
        bail!("No stems are routed to audible speakers.");
    }

    let ffmpeg_info = ffmpeg::probe_ffmpeg();
    if formats.iter().any(|format| format.requires_ffmpeg()) && !ffmpeg_info.available {
        bail!(
            "AC3, E-AC3, and DTS export require ffmpeg on PATH. 24-bit WAV and FLAC work natively."
        );
    }

    std::fs::create_dir_all(&settings.output_dir)
        .with_context(|| format!("create output directory {}", settings.output_dir.display()))?;

    let mut report = ExportReport::default();
    let mut outputs = Vec::new();
    for format in formats {
        let path = settings.output_path(*format)?;
        if outputs.contains(&path) {
            bail!(
                "Two selected formats would write the same file: {}",
                path.display()
            );
        }
        outputs.push(path);
    }

    send_progress(tx, 0.01, "Preparing surround mix");
    let mix = mixdown::render(snapshot, |fraction| {
        send_progress(tx, 0.02 + fraction * 0.43, "Rendering surround mix");
    })?;
    let layout = mix.layout;
    let _ = tx.send(ExportEvent::Peaks {
        pre_db: mix.pre_peaks_db,
        post_db: mix.post_peaks_db,
        limiter_active: mix.limiter_active,
        gate_reduction_db: mix.gate_reduction_db,
    });

    send_log(
        tx,
        format!(
            "Rendered {} channel(s) ({}) {:.2}s at {} Hz; pre-limiter peak {:.2} dBFS, post {:.2} dBFS",
            layout.count(),
            layout.short_label(),
            mix.duration_secs(),
            mix.sample_rate(),
            mix.pre_peaks_db
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max),
            mix.post_peaks_db
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max)
        ),
    );

    let base = settings.sanitized_base_name();
    let mut temp_51: Option<PathBuf> = None;
    let mut temp_71: Option<PathBuf> = None;

    for (index, format) in formats.iter().enumerate() {
        let output = settings.output_path(*format)?;
        let base_fraction = 0.46 + (index as f32 / formats.len() as f32) * 0.50;
        let span = 0.50 / formats.len() as f32;

        match format {
            Format::Wav24 => {
                send_progress(tx, base_fraction, "Writing 24-bit WAV");
                wav::write_wav24(
                    &output,
                    layout,
                    mix.sample_rate(),
                    &mix.planes,
                    |fraction| {
                        send_progress(tx, base_fraction + fraction * span, "Writing 24-bit WAV");
                    },
                )?;
                report.outputs.push(output);
            }
            Format::Flac24 => {
                send_progress(tx, base_fraction, "Encoding 24-bit FLAC");
                let tags = flac::FlacTags {
                    artist: settings.artist.clone(),
                    title: if settings.title.trim().is_empty() {
                        settings.sanitized_base_name()
                    } else {
                        settings.title.clone()
                    },
                    album: settings.album.clone(),
                    encoder: format!("SurroundPacker {}", env!("CARGO_PKG_VERSION")),
                };
                flac::write_flac24(
                    &output,
                    layout,
                    mix.sample_rate(),
                    &mix.planes,
                    &tags,
                    |fraction| {
                        send_progress(tx, base_fraction + fraction * span, "Encoding 24-bit FLAC");
                    },
                )?;
                report.outputs.push(output);
            }
            Format::Ac3 | Format::Eac3 | Format::Dts => {
                let wants_71_eac3 = *format == Format::Eac3 && layout.is_71();
                if wants_71_eac3 && !ffmpeg_info.eac3_7_1 && !settings.allow_eac3_51_fallback {
                    let message = "E-AC3: this FFmpeg build cannot encode discrete 7.1 E-AC3. Enable the 5.1 compatibility fallback or switch to 7.1 WAV/FLAC.".to_string();
                    send_log(tx, &message);
                    report.errors.push(message);
                    continue;
                }

                let use_71_input = wants_71_eac3 && ffmpeg_info.eac3_7_1;
                let input = if use_71_input {
                    send_progress(tx, base_fraction, "Writing temporary 7.1 master");
                    ensure_temp_71(&mix, &mut temp_71, &base)?
                } else {
                    send_progress(tx, base_fraction, "Writing temporary 5.1 bus");
                    ensure_temp_51(&mix, &mut temp_51, &base)?
                };

                if layout.is_71() && !use_71_input {
                    report.warnings.push(format!(
                        "{}: 7.1 source folded to 5.1 (BL→SL, BR→SR at −3 dB).",
                        format.short()
                    ));
                }
                if wants_71_eac3 && !ffmpeg_info.eac3_7_1 {
                    report.warnings.push(
                        "E-AC3: local FFmpeg lacks discrete 7.1 encoding; wrote a 5.1 compatibility folddown."
                            .to_string(),
                    );
                }

                let metadata = vec![
                    ("title".to_string(), settings.title.clone()),
                    ("artist".to_string(), settings.artist.clone()),
                    ("album".to_string(), settings.album.clone()),
                ];
                send_progress(tx, base_fraction, &format!("Encoding {}", format.label()));
                let result = if use_71_input {
                    ffmpeg::encode_eac3_7_1_with_progress(
                        &input,
                        &output,
                        match settings.container {
                            ContainerChoice::Mka => ffmpeg::Container::Mka,
                            ContainerChoice::Raw => ffmpeg::Container::Raw,
                        },
                        mix.duration_secs(),
                        &metadata,
                        &mut |fraction| {
                            send_progress(
                                tx,
                                base_fraction + fraction * span,
                                &format!("Encoding {}", format.label()),
                            );
                        },
                    )
                } else {
                    ffmpeg::encode_with_progress(
                        &input,
                        &output,
                        format.codec().expect("ffmpeg codec"),
                        settings.dts_bitrate,
                        match settings.container {
                            ContainerChoice::Mka => ffmpeg::Container::Mka,
                            ContainerChoice::Raw => ffmpeg::Container::Raw,
                        },
                        mix.duration_secs(),
                        &metadata,
                        &mut |fraction| {
                            send_progress(
                                tx,
                                base_fraction + fraction * span,
                                &format!("Encoding {}", format.label()),
                            );
                        },
                    )
                };

                match result {
                    Ok(()) => {
                        send_log(tx, format!("Wrote {}", output.display()));
                        report.outputs.push(output);
                    }
                    Err(error) => {
                        let message = format!("{}: {error:#}", format.short());
                        send_log(tx, &message);
                        report.errors.push(message);
                    }
                }
            }
        }
    }

    for temp in [temp_51, temp_71].into_iter().flatten() {
        let _ = std::fs::remove_file(temp);
    }

    send_progress(tx, 1.0, "Export complete");
    Ok(report)
}

/// Spawn an export worker.
pub fn spawn_export(
    snapshot: MixSnapshot,
    settings: ExportSettings,
    formats: Vec<Format>,
) -> Receiver<ExportEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        run_export(&snapshot, &settings, &formats, tx);
    });
    rx
}

pub fn format_peak(db: f32) -> String {
    if db.is_finite() {
        format!("{db:+.1} dBFS")
    } else {
        "−∞ dBFS".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_are_unique_and_safe() {
        let settings = ExportSettings {
            output_dir: PathBuf::from("/tmp/out"),
            base_name: "My: Mix".into(),
            formats: Format::DEFAULT.to_vec(),
            container: ContainerChoice::Mka,
            ..ExportSettings::default()
        };
        assert_eq!(settings.sanitized_base_name(), "My_ Mix");
        let paths: Vec<_> = Format::ALL
            .iter()
            .map(|format| settings.output_path(*format).unwrap())
            .collect();
        for (index, path) in paths.iter().enumerate() {
            assert!(!paths[..index].contains(path), "duplicate output {path:?}");
        }
        assert!(paths[0].ends_with("My_ Mix.wav"));
        assert!(paths[1].ends_with("My_ Mix.flac"));
        assert!(paths[2].ends_with("My_ Mix - AC3.mka"));
    }

    #[test]
    fn only_ffmpeg_formats_require_ffmpeg() {
        assert!(!Format::Wav24.requires_ffmpeg());
        assert!(!Format::Flac24.requires_ffmpeg());
        assert!(Format::Ac3.requires_ffmpeg());
        assert!(Format::Eac3.requires_ffmpeg());
        assert!(Format::Dts.requires_ffmpeg());
        assert!(!Format::Wav24.uses_container());
    }
}
