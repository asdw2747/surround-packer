use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::audio::{decode_audio, probe_audio, AudioProbe};
use crate::decode::{canonicalize_51, source_layout};
use crate::dsp::{self, LIMITER_CEILING_DB};
use crate::export::dvd::{self, DvdAudioCodec, DvdTools, DvdTrack, DVD_R_CAPACITY_MB};
use crate::export::{ffmpeg, wav};
use crate::model::ChannelLayout;

pub use crate::decode::SourceChannelOrder;

pub const DVD_RATE: u32 = 48_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchStatus {
    Ready,
    Converting,
    Done(PathBuf),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct BatchTrack {
    pub id: u64,
    pub path: PathBuf,
    pub display_name: String,
    pub probe: Option<AudioProbe>,
    pub status: BatchStatus,
}

impl BatchTrack {
    pub fn new(id: u64, path: PathBuf) -> Self {
        let display_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("track")
            .to_string();
        let probe = probe_audio(&path).ok();
        let status = match &probe {
            Some(info) if matches!(info.channels, 6 | 8) && info.sample_rate > 0 => {
                BatchStatus::Ready
            }
            Some(info) => BatchStatus::Error(format!(
                "unsupported source: {} channels at {} Hz (need 5.1 or 7.1)",
                info.channels, info.sample_rate
            )),
            None => BatchStatus::Error("could not read audio metadata".to_string()),
        };
        Self {
            id,
            path,
            display_name,
            probe,
            status,
        }
    }

    pub fn layout(&self) -> Option<ChannelLayout> {
        self.probe
            .as_ref()
            .and_then(|probe| ChannelLayout::from_channel_count(probe.channels))
    }

    pub fn layout_label(&self) -> &'static str {
        self.layout().map_or("—", ChannelLayout::short_label)
    }

    pub fn sample_rate_label(&self) -> String {
        self.probe
            .as_ref()
            .map(|probe| format!("{:.1} kHz", probe.sample_rate as f32 / 1000.0))
            .unwrap_or_else(|| "—".to_string())
    }

    pub fn duration_label(&self) -> String {
        self.probe
            .as_ref()
            .and_then(|probe| probe.duration_secs())
            .map(|seconds| {
                let total = seconds.round() as u64;
                format!("{}:{:02}", total / 60, total % 60)
            })
            .unwrap_or_else(|| "—".to_string())
    }
}

#[derive(Debug)]
pub struct BatchState {
    pub tracks: Vec<BatchTrack>,
    pub output_dir: PathBuf,
    pub album_title: String,
    pub codec: DvdAudioCodec,
    pub source_order: SourceChannelOrder,
    pub converting: bool,
    pub progress: f32,
    pub stage: String,
    pub last_outputs: Vec<PathBuf>,
    pub last_video_ts: Option<PathBuf>,
    pub last_iso: Option<PathBuf>,
    pub last_errors: Vec<String>,
    pub log: Vec<String>,
    pub(crate) rx: Option<Receiver<BatchEvent>>,
    pub(crate) next_id: u64,
    cancel: Arc<AtomicBool>,
}

impl Default for BatchState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            output_dir: PathBuf::new(),
            album_title: "SurroundPacker Audio-DVD".to_string(),
            codec: DvdAudioCodec::Ac3Dts1509,
            source_order: SourceChannelOrder::Smpte51,
            converting: false,
            progress: 0.0,
            stage: String::new(),
            last_outputs: Vec::new(),
            last_video_ts: None,
            last_iso: None,
            last_errors: Vec::new(),
            log: Vec::new(),
            rx: None,
            next_id: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BatchEvent {
    Progress {
        fraction: f32,
        stage: String,
    },
    Log(String),
    TrackStarted {
        id: u64,
    },
    TrackDone {
        id: u64,
        output: PathBuf,
    },
    TrackError {
        id: u64,
        message: String,
    },
    Done {
        outputs: Vec<PathBuf>,
        video_ts: Option<PathBuf>,
        iso: Option<PathBuf>,
        errors: Vec<String>,
        warnings: Vec<String>,
    },
    Failed(String),
    Cancelled,
}

impl BatchState {
    pub fn new(output_dir: PathBuf, album_title: impl Into<String>) -> Self {
        Self {
            output_dir,
            album_title: album_title.into(),
            ..Self::default()
        }
    }

    pub fn add_paths(&mut self, paths: &[PathBuf]) -> usize {
        let mut added = 0;
        for path in paths {
            let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            if !crate::audio::is_batch_supported_path(&path) {
                self.log
                    .push(format!("Skipped unsupported file: {}", path.display()));
                continue;
            }
            if self.tracks.iter().any(|track| track.path == path) {
                continue;
            }
            self.next_id += 1;
            self.tracks.push(BatchTrack::new(self.next_id, path));
            added += 1;
        }
        added
    }

    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.tracks.len() {
            self.tracks.swap(index, index - 1);
        }
    }

    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.tracks.len() {
            self.tracks.swap(index, index + 1);
        }
    }

    pub fn total_duration_secs(&self) -> f64 {
        self.tracks
            .iter()
            .filter_map(|track| track.probe.as_ref()?.duration_secs())
            .sum()
    }

    pub fn dvd_tracks(&self) -> Vec<DvdTrack> {
        self.tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| matches!(track.status, BatchStatus::Ready | BatchStatus::Done(_)))
            .map(|(index, track)| DvdTrack {
                title: track.display_name.clone(),
                mpg_name: output_mpg_name(index, track),
                duration_secs: track
                    .probe
                    .as_ref()
                    .and_then(|probe| probe.duration_secs())
                    .unwrap_or(0.0),
            })
            .collect()
    }

    pub fn estimated_size_mb(&self) -> f64 {
        dvd::estimated_dvd_mb(&self.dvd_tracks(), self.codec)
    }

    pub fn over_capacity(&self) -> bool {
        self.estimated_size_mb() > DVD_R_CAPACITY_MB
    }

    pub fn can_build(&self) -> bool {
        !self.converting
            && !self.tracks.is_empty()
            && self
                .tracks
                .iter()
                .any(|track| matches!(track.status, BatchStatus::Ready))
    }

    pub fn start_build(&mut self, tools: DvdTools) {
        if !self.can_build() {
            return;
        }
        let tracks = self.tracks.clone();
        let output_dir = self.output_dir.clone();
        let disc_title = self.album_title.clone();
        let codec = self.codec;
        self.cancel.store(false, Ordering::Relaxed);
        self.converting = true;
        self.progress = 0.0;
        self.stage = "Preparing Audio-DVD build".to_string();
        self.last_outputs.clear();
        self.last_video_ts = None;
        self.last_iso = None;
        self.last_errors.clear();
        self.rx = Some(spawn_dvd_build(
            tracks,
            output_dir,
            disc_title,
            codec,
            self.source_order,
            tools,
            self.cancel.clone(),
        ));
    }

    pub fn cancel_build(&mut self) {
        if self.converting {
            self.cancel.store(true, Ordering::Relaxed);
            self.stage = "Cancelling Audio-DVD build".to_string();
            ffmpeg::terminate_active_ffmpeg();
        }
    }

    pub fn poll(&mut self) -> Vec<String> {
        let mut updates = Vec::new();
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok(BatchEvent::Progress { fraction, stage }) => {
                        self.progress = fraction;
                        self.stage = stage.clone();
                        updates.push(stage);
                    }
                    Ok(BatchEvent::Log(message)) => {
                        self.log.push(message.clone());
                        updates.push(message);
                    }
                    Ok(BatchEvent::TrackStarted { id }) => {
                        if let Some(track) = self.tracks.iter_mut().find(|track| track.id == id) {
                            track.status = BatchStatus::Converting;
                        }
                    }
                    Ok(BatchEvent::TrackDone { id, output }) => {
                        if let Some(track) = self.tracks.iter_mut().find(|track| track.id == id) {
                            track.status = BatchStatus::Done(output);
                        }
                    }
                    Ok(BatchEvent::TrackError { id, message }) => {
                        if let Some(track) = self.tracks.iter_mut().find(|track| track.id == id) {
                            track.status = BatchStatus::Error(message);
                        }
                    }
                    Ok(BatchEvent::Done {
                        outputs,
                        video_ts,
                        iso,
                        errors,
                        warnings,
                    }) => {
                        self.converting = false;
                        self.progress = 1.0;
                        self.stage = "Audio-DVD build complete".to_string();
                        self.last_outputs = outputs;
                        self.last_video_ts = video_ts;
                        self.last_iso = iso;
                        self.last_errors = errors;
                        for warning in warnings {
                            self.log.push(format!("Warning: {warning}"));
                        }
                        self.rx = None;
                        updates.push("Audio-DVD build complete".to_string());
                        break;
                    }
                    Ok(BatchEvent::Failed(message)) => {
                        self.converting = false;
                        self.stage = "Audio-DVD build failed".to_string();
                        self.last_errors.push(message.clone());
                        self.log.push(format!("Error: {message}"));
                        self.rx = None;
                        updates.push(message);
                        break;
                    }
                    Ok(BatchEvent::Cancelled) => {
                        self.converting = false;
                        self.stage = "Audio-DVD build cancelled".to_string();
                        for track in &mut self.tracks {
                            if matches!(track.status, BatchStatus::Converting) {
                                track.status = BatchStatus::Ready;
                            }
                        }
                        self.log.push("Build cancelled by user.".to_string());
                        self.rx = None;
                        updates.push("Audio-DVD build cancelled".to_string());
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.converting = false;
                        self.stage = "DVD worker stopped".to_string();
                        self.last_errors
                            .push("DVD worker stopped unexpectedly".to_string());
                        self.rx = None;
                        break;
                    }
                }
            }
        }
        updates
    }
}

fn safe_stem(name: &str) -> String {
    let filtered: String = name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect();
    let trimmed = filtered.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "track".to_string()
    } else {
        trimmed.to_string()
    }
}

fn output_mpg_name(index: usize, track: &BatchTrack) -> String {
    format!("{:02} - {}.mpg", index + 1, safe_stem(&track.display_name))
}

fn temp_wav_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "surround-packer-dvd-{}-{stamp}.wav",
        std::process::id()
    ))
}

fn prepare_audio(
    track: &mut BatchTrack,
    source_order: SourceChannelOrder,
) -> Result<(PathBuf, f64)> {
    let decoded = decode_audio(&track.path)?;
    let source_channels = decoded.channels.len();
    if source_layout(source_channels).is_none() {
        anyhow::bail!("source has {source_channels} channels; expected 5.1 or 7.1");
    }
    let canonical = canonicalize_51(decoded.channels, source_order)?;
    let mut folded: Vec<Vec<f32>> = canonical.into_iter().take(6).collect();
    for plane in &mut folded {
        if decoded.sample_rate != DVD_RATE {
            *plane = crate::audio::resample_plane(plane, decoded.sample_rate, DVD_RATE);
        }
    }
    dsp::limit_channels(&mut folded, LIMITER_CEILING_DB);
    let duration = folded
        .first()
        .map_or(0.0, |plane| plane.len() as f64 / DVD_RATE as f64);
    let path = temp_wav_path();
    wav::write_wav24(&path, ChannelLayout::Surround51, DVD_RATE, &folded, |_| {})?;
    Ok((path, duration))
}

fn spawn_dvd_build(
    tracks: Vec<BatchTrack>,
    output_dir: PathBuf,
    disc_title: String,
    codec: DvdAudioCodec,
    source_order: SourceChannelOrder,
    tools: DvdTools,
    cancel: Arc<AtomicBool>,
) -> Receiver<BatchEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if !tools.ffmpeg.available {
            let _ = tx.send(BatchEvent::Failed(
                "ffmpeg is required to build Audio-DVD titles.".to_string(),
            ));
            return;
        }
        let workdir = output_dir.join("Audio-DVD");
        if let Err(error) = std::fs::create_dir_all(&workdir) {
            let _ = tx.send(BatchEvent::Failed(format!(
                "Could not create Audio-DVD folder: {error}"
            )));
            return;
        }

        let mut tracks = tracks;
        let total = tracks.len().max(1);
        let mut outputs = Vec::new();
        let mut dvd_tracks = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for (index, track) in tracks.iter_mut().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if matches!(track.status, BatchStatus::Error(_)) {
                if let BatchStatus::Error(message) = &track.status {
                    errors.push(format!("{}: {message}", track.display_name));
                }
                continue;
            }
            let id = track.id;
            let title = track.display_name.clone();
            let mpg_name = output_mpg_name(index, track);
            let mpg_path = workdir.join(&mpg_name);
            let base = index as f32 / total as f32;
            let span = 0.82 / total as f32;
            let _ = tx.send(BatchEvent::TrackStarted { id });
            let _ = tx.send(BatchEvent::Progress {
                fraction: base,
                stage: format!("Preparing {}", track.display_name),
            });

            let result = (|| -> Result<f64> {
                let (wav_path, duration) = prepare_audio(track, source_order)?;
                let mpg_result = dvd::build_title_card_mpg(
                    &wav_path,
                    &mpg_path,
                    index + 1,
                    &title,
                    codec,
                    duration,
                    &tools.mplex,
                    Some(&cancel),
                    |fraction| {
                        let stage = if fraction < 0.10 {
                            format!("Rendering title {}", track.display_name)
                        } else if fraction < 0.28 {
                            format!("Encoding audio {}", track.display_name)
                        } else {
                            format!("Muxing {}", track.display_name)
                        };
                        let _ = tx.send(BatchEvent::Progress {
                            fraction: base + span * fraction,
                            stage,
                        });
                    },
                );
                let _ = std::fs::remove_file(&wav_path);
                mpg_result?;
                Ok(duration)
            })();

            match result {
                Ok(duration) => {
                    track.status = BatchStatus::Done(mpg_path.clone());
                    let _ = tx.send(BatchEvent::TrackDone {
                        id,
                        output: mpg_path.clone(),
                    });
                    outputs.push(mpg_path);
                    dvd_tracks.push(DvdTrack {
                        title,
                        mpg_name,
                        duration_secs: duration,
                    });
                }
                Err(error) => {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let message = format!("{}: {error:#}", track.display_name);
                    track.status = BatchStatus::Error(message.clone());
                    errors.push(message.clone());
                    let _ = tx.send(BatchEvent::TrackError { id, message });
                }
            }
        }

        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(BatchEvent::Cancelled);
            return;
        }

        let mut video_ts = None;
        let mut iso = None;
        if !dvd_tracks.is_empty() {
            if !tools.mplex.available() {
                warnings.push(
                    "mplex (mjpegtools) not found; skipped VIDEO_TS and ISO. The .mpg files are playable but lack DVD navigation data."
                        .to_string(),
                );
            } else if tools.dvdauthor.available() {
                let _ = tx.send(BatchEvent::Progress {
                    fraction: 0.86,
                    stage: "Authoring VIDEO_TS".to_string(),
                });
                let xml_path = workdir.join("dvdauthor.xml");
                let iso_root = workdir.join("iso_root");
                let xml = dvd::build_dvdauthor_xml(&dvd_tracks);
                if iso_root.exists() {
                    let _ = std::fs::remove_dir_all(&iso_root);
                }
                let result = std::fs::create_dir_all(&iso_root)
                    .map_err(anyhow::Error::from)
                    .and_then(|_| std::fs::write(&xml_path, xml).map_err(anyhow::Error::from))
                    .and_then(|_| {
                        dvd::run_dvdauthor(&tools.dvdauthor, &workdir, &xml_path, &iso_root)
                    });
                match result {
                    Ok(()) => {
                        let candidate = iso_root.join("VIDEO_TS");
                        if candidate.is_dir() {
                            video_ts = Some(candidate);
                        } else {
                            warnings
                                .push("dvdauthor did not produce a VIDEO_TS folder".to_string());
                        }
                    }
                    Err(error) => {
                        warnings.push(format!("dvdauthor failed: {error:#}"));
                    }
                }
                if video_ts.is_some() && tools.iso.available() {
                    let _ = tx.send(BatchEvent::Progress {
                        fraction: 0.94,
                        stage: "Packaging Audio-DVD.iso".to_string(),
                    });
                    let iso_path = output_dir.join("Audio-DVD.iso");
                    match dvd::run_iso(&tools.iso, &iso_root, &iso_path) {
                        Ok(()) => iso = Some(iso_path),
                        Err(error) => warnings.push(format!("ISO packaging failed: {error:#}")),
                    }
                }
                if !tools.iso.available() {
                    warnings.push(
                        "No ISO authoring tool found (xorriso/mkisofs/genisoimage); skipped .iso."
                            .to_string(),
                    );
                }
            } else {
                warnings.push(
                    "dvdauthor not found; skipped VIDEO_TS and ISO. The .mpg files are still available."
                        .to_string(),
                );
            }
        }

        let _ = tx.send(BatchEvent::Progress {
            fraction: 1.0,
            stage: format!("{disc_title}: Audio-DVD build complete"),
        });
        let _ = tx.send(BatchEvent::Done {
            outputs,
            video_ts,
            iso,
            errors,
            warnings,
        });
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpg_names_are_ordered_and_safe() {
        let track = BatchTrack {
            id: 1,
            path: PathBuf::from("input.wav"),
            display_name: "A: Song / Title".to_string(),
            probe: None,
            status: BatchStatus::Ready,
        };
        assert_eq!(output_mpg_name(0, &track), "01 - A_ Song _ Title.mpg");
    }
}
