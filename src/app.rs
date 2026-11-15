use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Context, Rect};

use crate::audio::{is_batch_supported_path, is_supported_path, load_stem, Stem};
use crate::audition::Audition;
use crate::batch::BatchState;
use crate::dsp::{DspSettings, PeakReport};
use crate::export::dvd::DvdTools;
use crate::export::{self, ExportEvent, ExportReport, ExportSettings, Format};
use crate::mixdown::{self, MixSnapshot};
use crate::model::ChannelLayout;
use crate::routing::{
    apply_preset, Channel, PanLaw, Preset, Route, RoutingTemplate, Slot, StemRef, CHANNEL_COUNT,
};
use crate::ui::{panels, speaker_room, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTab {
    StemPacker,
    AudioDvd,
}

impl AppTab {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StemPacker => "Stem Packer",
            Self::AudioDvd => "Audio-DVD Builder",
        }
    }
}

enum LoadEvent {
    Loaded { stem: Stem, target: Option<Channel> },
    Failed { path: PathBuf, error: String },
}

pub struct SurroundPackerApp {
    ctx: Context,
    pub stems: Vec<Arc<Stem>>,
    next_stem_id: u64,
    pub slots: [Slot; CHANNEL_COUNT],
    pub layout: ChannelLayout,
    pub tab: AppTab,
    pub batch: BatchState,
    pub audition: Audition,
    pub dsp: DspSettings,
    pub pan_law: PanLaw,
    pub ffmpeg: export::ffmpeg::FfmpegInfo,
    pub dvd_tools: DvdTools,
    pub export_settings: ExportSettings,
    pub exporting: bool,
    pub export_progress: f32,
    pub export_stage: String,
    pub peaks: Option<PeakReport>,
    pub status: String,
    pub logs: VecDeque<String>,
    pub template_name: String,
    pub editor_channel: Option<Channel>,
    pub last_report: Option<ExportReport>,
    pub slot_rects: [Rect; CHANNEL_COUNT],
    pub(crate) loading: usize,
    load_tx: Sender<LoadEvent>,
    load_rx: Receiver<LoadEvent>,
    ffmpeg_rx: Option<Receiver<export::ffmpeg::FfmpegInfo>>,
    dvd_tools_rx: Option<Receiver<DvdTools>>,
    pub(crate) analysis_rx: Option<Receiver<Result<PeakReport, String>>>,
    analysis_dirty: bool,
    last_edit: Instant,
    export_rx: Option<Receiver<ExportEvent>>,
    pending_preset: Option<Preset>,
}

impl SurroundPackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_paths(cc, &[])
    }

    pub fn new_with_paths(cc: &eframe::CreationContext<'_>, paths: &[PathBuf]) -> Self {
        theme::apply(&cc.egui_ctx);

        let mut export_settings = ExportSettings::default();
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let music = PathBuf::from(home).join("Music");
            if music.is_dir() {
                export_settings.output_dir = music;
            }
        }
        export_settings.base_name = "SurroundPacker 5.1 Mix".to_string();

        let (load_tx, load_rx) = mpsc::channel();
        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            stems: Vec::new(),
            next_stem_id: 1,
            slots: Slot::empty_slots(),
            layout: ChannelLayout::default(),
            tab: AppTab::StemPacker,
            batch: BatchState::new(
                export_settings.output_dir.clone(),
                "SurroundPacker Audio-DVD",
            ),
            audition: Audition::default(),
            dsp: DspSettings::default(),
            pan_law: PanLaw::default(),
            ffmpeg: export::ffmpeg::FfmpegInfo::default(),
            dvd_tools: DvdTools::default(),
            export_settings,
            exporting: false,
            export_progress: 0.0,
            export_stage: String::new(),
            peaks: None,
            status: "Drop UVR5 stems onto the speakers, or load them from the library.".to_string(),
            logs: VecDeque::new(),
            template_name: "My Surround Template".to_string(),
            editor_channel: None,
            last_report: None,
            slot_rects: [Rect::NOTHING; CHANNEL_COUNT],
            loading: 0,
            load_tx,
            load_rx,
            ffmpeg_rx: None,
            dvd_tools_rx: None,
            analysis_rx: None,
            analysis_dirty: false,
            last_edit: Instant::now(),
            export_rx: None,
            pending_preset: None,
        };
        app.push_log(format!(
            "SurroundPacker {} ready",
            env!("CARGO_PKG_VERSION")
        ));
        app.refresh_ffmpeg();
        app.refresh_dvd_tools();
        for path in paths {
            app.enqueue_load(path.clone(), None);
        }
        app
    }

    pub fn snapshot(&self) -> MixSnapshot {
        let stems = self
            .stems
            .iter()
            .map(|stem| (stem.id, Arc::clone(stem)))
            .collect::<HashMap<_, _>>();
        MixSnapshot {
            stems,
            slots: self.slots.clone(),
            layout: self.layout,
            dsp: self.dsp.clone(),
            pan_law: self.pan_law,
        }
    }

    pub fn queue_preset(&mut self, preset: Preset) {
        if self.loading == 0 && !self.stems.is_empty() {
            self.apply_preset(preset);
        } else {
            self.pending_preset = Some(preset);
            self.set_status("Waiting for stems before applying the startup preset…");
        }
    }

    pub fn set_layout(&mut self, layout: ChannelLayout) {
        if self.layout == layout {
            return;
        }
        let old_default = self.layout.default_surround_delay_ms();
        if (self.dsp.surround_delay_ms - old_default).abs() < 0.01 {
            self.dsp.surround_delay_ms = layout.default_surround_delay_ms();
        }
        if layout.is_71() {
            self.dsp.back_delay_ms = self.dsp.back_delay_ms.clamp(0.0, 35.0);
            if self.dsp.back_delay_ms == 0.0 {
                self.dsp.back_delay_ms = layout.default_back_delay_ms();
            }
        }
        if self.export_settings.base_name == "SurroundPacker 5.1 Mix"
            || self.export_settings.base_name == "SurroundPacker 7.1 Mix"
        {
            self.export_settings.base_name = format!("SurroundPacker {} Mix", layout.short_label());
        }
        self.layout = layout;
        self.mark_dirty();
        self.set_status(format!("Channel layout: {}", layout.label()));
    }

    pub fn start_audition(&mut self) {
        if !self.snapshot().has_audible_routes() {
            self.set_status("Route at least one stem before auditioning.");
            return;
        }
        self.audition.render(self.snapshot());
        self.set_status("Rendering stereo headphone folddown…");
    }

    pub fn stop_audition(&mut self) {
        self.audition.stop();
        self.set_status("Audition stopped.");
    }

    pub fn mark_dirty(&mut self) {
        self.analysis_dirty = true;
        self.last_edit = Instant::now();
    }

    pub fn request_analysis_now(&mut self) {
        self.analysis_dirty = true;
        self.last_edit = Instant::now() - Duration::from_secs(2);
    }

    pub fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push_back(message.into());
        while self.logs.len() > 250 {
            self.logs.pop_front();
        }
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.push_log(message.clone());
        self.status = message;
    }

    pub fn refresh_ffmpeg(&mut self) {
        let (tx, rx) = mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(export::ffmpeg::probe_ffmpeg());
            ctx.request_repaint();
        });
        self.ffmpeg_rx = Some(rx);
    }

    pub fn refresh_dvd_tools(&mut self) {
        let (tx, rx) = mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(export::dvd::probe_dvd_tools());
            ctx.request_repaint();
        });
        self.dvd_tools_rx = Some(rx);
    }

    pub fn start_dvd_build(&mut self) {
        self.batch.start_build(self.dvd_tools.clone());
    }

    pub fn can_export(&self) -> bool {
        let formats = &self.export_settings.formats;
        let eac3_blocked = formats.contains(&Format::Eac3)
            && self.layout.is_71()
            && !self.ffmpeg.eac3_7_1
            && !self.export_settings.allow_eac3_51_fallback;
        !self.exporting
            && !formats.is_empty()
            && !eac3_blocked
            && self.snapshot().has_audible_routes()
            && (!formats.iter().any(|format| format.requires_ffmpeg()) || self.ffmpeg.available)
    }

    pub fn set_format_selected(&mut self, format: Format, selected: bool) {
        let contains = self.export_settings.formats.contains(&format);
        if selected && !contains {
            self.export_settings.formats.push(format);
        } else if !selected && contains {
            self.export_settings
                .formats
                .retain(|candidate| *candidate != format);
        }
        self.export_settings.formats.sort_by_key(|candidate| {
            Format::ALL
                .iter()
                .position(|known| known == candidate)
                .unwrap_or(usize::MAX)
        });
    }

    pub fn enqueue_load(&mut self, path: PathBuf, target: Option<Channel>) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if !is_supported_path(&path) {
            self.set_status(format!("Unsupported file type: {}", path.display()));
            return;
        }
        if let Some(existing) = self.stems.iter().find(|stem| stem.path == path) {
            let id = existing.id;
            if let Some(channel) = target {
                self.attach_stem_to_channel(id, channel);
            }
            return;
        }

        let id = self.next_stem_id;
        self.next_stem_id += 1;
        self.loading += 1;
        self.push_log(format!("Loading {}", path.display()));
        let tx = self.load_tx.clone();
        let thread_path = path.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let event = match load_stem(&thread_path, id) {
                Ok(stem) => LoadEvent::Loaded { stem, target },
                Err(error) => LoadEvent::Failed {
                    path: thread_path,
                    error: format!("{error:#}"),
                },
            };
            let _ = tx.send(event);
            ctx.request_repaint();
        });
    }

    pub fn attach_stem_to_channel(&mut self, stem_id: u64, channel: Channel) {
        let Some(stem) = self.stems.iter().find(|stem| stem.id == stem_id).cloned() else {
            return;
        };
        let slot = &mut self.slots[channel.index()];
        if !slot.routes.iter().any(|route| route.stem_id == stem_id) {
            slot.routes.push(Route::new(
                stem_id,
                stem.path.clone(),
                channel.default_mode(),
                0.0,
            ));
        }
        self.set_status(format!(
            "Routed {} to {}",
            stem.display_name,
            channel.short()
        ));
        self.mark_dirty();
    }

    pub fn remove_stem(&mut self, stem_id: u64) {
        self.stems.retain(|stem| stem.id != stem_id);
        for slot in &mut self.slots {
            slot.routes.retain(|route| route.stem_id != stem_id);
        }
        self.mark_dirty();
        self.set_status("Removed stem from the library and all speakers.");
    }

    pub fn clear_session(&mut self) {
        self.stems.clear();
        for slot in &mut self.slots {
            slot.clear();
        }
        self.peaks = None;
        self.analysis_dirty = false;
        self.set_status("Cleared stems and routing.");
    }

    pub fn apply_preset(&mut self, preset: Preset) {
        if self.stems.is_empty() {
            self.set_status("Load stems before applying a routing preset.");
            return;
        }
        let refs: Vec<StemRef> = self
            .stems
            .iter()
            .map(|stem| StemRef::new(stem.id, stem.path.clone()))
            .collect();
        self.slots = apply_preset(preset, &refs, self.layout);
        self.mark_dirty();
        self.set_status(format!("Applied preset: {}", preset.label()));
    }

    pub fn save_template(&mut self) {
        let default_name = format!(
            "{}.json",
            self.template_name.trim().replace(['/', '\\', ':'], "_")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SurroundPacker template", &["json"])
            .set_file_name(default_name)
            .set_directory(&self.export_settings.output_dir)
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_none() {
            path.with_extension("json")
        } else {
            path
        };
        let template = RoutingTemplate::from_slots(
            self.template_name.trim(),
            self.layout,
            &self.slots,
            self.pan_law,
            &self.dsp,
        );
        match serde_json::to_string_pretty(&template)
            .map_err(anyhow::Error::from)
            .and_then(|json| std::fs::write(&path, json).map_err(anyhow::Error::from))
        {
            Ok(()) => self.set_status(format!("Saved routing template {}", path.display())),
            Err(error) => {
                self.set_status(format!("Could not save template: {error}"));
                show_error_dialog("SurroundPacker — Template Error", &format!("{error:#}"));
            }
        }
    }

    pub fn load_template(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SurroundPacker template", &["json"])
            .set_directory(&self.export_settings.output_dir)
            .pick_file()
        else {
            return;
        };
        let result = (|| -> anyhow::Result<()> {
            let json = std::fs::read_to_string(&path)?;
            let template: RoutingTemplate = serde_json::from_str(&json)?;
            if template.format_version != RoutingTemplate::CURRENT_VERSION {
                anyhow::bail!(
                    "unsupported template version {} (expected {})",
                    template.format_version,
                    RoutingTemplate::CURRENT_VERSION
                );
            }

            let mut resolved: HashMap<PathBuf, (u64, PathBuf)> = HashMap::new();
            let mut missing_examples: Vec<String> = Vec::new();
            for template_slot in &template.slots {
                for route in &template_slot.routes {
                    if resolved.contains_key(&route.stem_path) {
                        continue;
                    }
                    let existing = self
                        .stems
                        .iter()
                        .find(|stem| stem.path == route.stem_path)
                        .or_else(|| {
                            let file_name = route.stem_path.file_name()?;
                            self.stems
                                .iter()
                                .find(|stem| stem.path.file_name() == Some(file_name))
                        });
                    if let Some(stem) = existing {
                        resolved.insert(route.stem_path.clone(), (stem.id, stem.path.clone()));
                        continue;
                    }
                    if route.stem_path.exists() {
                        let id = self.next_stem_id;
                        self.next_stem_id += 1;
                        match load_stem(&route.stem_path, id) {
                            Ok(stem) => {
                                let stem_path = stem.path.clone();
                                self.stems.push(Arc::new(stem));
                                resolved.insert(route.stem_path.clone(), (id, stem_path));
                            }
                            Err(error) => {
                                missing_examples
                                    .push(format!("{}: {error}", route.stem_path.display()));
                            }
                        }
                    } else {
                        missing_examples.push(format!("{} (not found)", route.stem_path.display()));
                    }
                }
            }

            let (slots, missing) = template.to_slots(|path| resolved.get(path).cloned());
            self.slots = slots;
            self.layout = template.layout;
            self.dsp = template.dsp.clone();
            self.pan_law = template.pan_law;
            self.template_name = template.name.clone();
            self.mark_dirty();
            self.set_status(format!(
                "Loaded template “{}” ({} unresolved route(s))",
                template.name, missing
            ));
            for example in missing_examples.iter().take(4) {
                self.push_log(format!("Template source unavailable: {example}"));
            }
            Ok(())
        })();

        if let Err(error) = result {
            self.set_status(format!("Could not load template: {error:#}"));
            show_error_dialog("SurroundPacker — Template Error", &format!("{error:#}"));
        }
    }

    pub fn start_export(&mut self) {
        if self.exporting {
            return;
        }
        let formats = self.export_settings.formats.clone();
        if formats.is_empty() {
            show_error_dialog(
                "SurroundPacker — Export",
                "Select at least one export format.",
            );
            return;
        }
        if !self.snapshot().has_audible_routes() {
            show_error_dialog(
                "SurroundPacker — Export",
                "Route at least one stem to an audible speaker before exporting.",
            );
            return;
        }
        if formats.iter().any(|format| format.requires_ffmpeg()) && !self.ffmpeg.available {
            show_error_dialog(
                "SurroundPacker — FFmpeg Required",
                "AC3, E-AC3, and DTS export require ffmpeg on PATH.\n\n24-bit WAV and 24-bit FLAC export work natively without ffmpeg. Deselect the ffmpeg formats or install ffmpeg, then press Refresh in the header.",
            );
            return;
        }
        if formats.contains(&Format::Eac3)
            && self.layout.is_71()
            && !self.ffmpeg.eac3_7_1
            && !self.export_settings.allow_eac3_51_fallback
        {
            show_error_dialog(
                "SurroundPacker — 7.1 E-AC3",
                "This FFmpeg build cannot encode discrete 7.1 E-AC3. Enable the 5.1 E-AC3 compatibility fold in the Export panel, or use 7.1 WAV/FLAC.",
            );
            return;
        }
        let snapshot = self.snapshot();
        self.exporting = true;
        self.export_progress = 0.0;
        self.export_stage = "Preparing export…".to_string();
        self.last_report = None;
        self.push_log(format!(
            "Export started: {} format(s), base name “{}”",
            formats.len(),
            self.export_settings.sanitized_base_name()
        ));
        self.export_rx = Some(export::spawn_export(
            snapshot,
            self.export_settings.clone(),
            formats,
        ));
    }

    fn poll_workers(&mut self) {
        self.audition.poll();
        if let Some(error) = self.audition.last_error.take() {
            self.set_status(format!("Audition: {error}"));
        }
        let batch_updates = self.batch.poll();
        for update in batch_updates {
            if update.contains("complete") || update.contains("failed") {
                self.set_status(update);
            }
        }
        if let Some(rx) = &self.ffmpeg_rx {
            match rx.try_recv() {
                Ok(info) => {
                    let available = info.available;
                    self.ffmpeg = info;
                    self.ffmpeg_rx = None;
                    if available {
                        self.push_log(format!("ffmpeg ready: {}", self.ffmpeg.version));
                    } else {
                        self.push_log(
                            "ffmpeg not found; AC3/E-AC3/DTS disabled. WAV and FLAC remain available.",
                        );
                        self.export_settings
                            .formats
                            .retain(|format| !format.requires_ffmpeg());
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.ffmpeg_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if let Some(rx) = &self.dvd_tools_rx {
            match rx.try_recv() {
                Ok(tools) => {
                    self.ffmpeg = tools.ffmpeg.clone();
                    self.dvd_tools = tools;
                    self.dvd_tools_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.dvd_tools_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        let load_events: Vec<LoadEvent> =
            std::iter::from_fn(|| self.load_rx.try_recv().ok()).collect();
        for event in load_events {
            match event {
                LoadEvent::Loaded { stem, target } => {
                    self.loading = self.loading.saturating_sub(1);
                    let id = stem.id;
                    let name = stem.display_name.clone();
                    let path = stem.path.clone();
                    self.stems.push(Arc::new(stem));
                    self.stems
                        .sort_by(|a, b| a.display_name.cmp(&b.display_name));
                    if let Some(channel) = target {
                        self.attach_stem_to_channel(id, channel);
                    } else {
                        self.set_status(format!("Loaded {name}"));
                    }
                    self.push_log(format!("Decoded {}", path.display()));
                    self.mark_dirty();
                }
                LoadEvent::Failed { path, error } => {
                    self.loading = self.loading.saturating_sub(1);
                    self.set_status(format!("Could not load {}: {error}", path.display()));
                }
            }
        }

        if self.loading == 0 {
            if let Some(preset) = self.pending_preset.take() {
                if !self.stems.is_empty() {
                    self.apply_preset(preset);
                }
            }
        }

        if let Some(rx) = &self.analysis_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.analysis_rx = None;
                    match result {
                        Ok(report) => self.peaks = Some(report),
                        Err(error) => {
                            self.set_status(format!("Peak analysis failed: {error}"));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.analysis_rx = None;
                    self.analysis_dirty = true;
                    self.set_status("Peak analysis worker stopped unexpectedly.");
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        let export_event = self.export_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(event) = export_event {
            match event {
                ExportEvent::Progress { fraction, stage } => {
                    self.export_progress = fraction;
                    self.export_stage = stage;
                }
                ExportEvent::Peaks {
                    pre_db,
                    post_db,
                    limiter_active,
                    gate_reduction_db,
                } => {
                    self.peaks = Some(PeakReport {
                        pre_db,
                        post_db,
                        limiter_active,
                        gate_reduction_db,
                    });
                }
                ExportEvent::Log(message) => self.push_log(message),
                ExportEvent::Done(report) => {
                    self.exporting = false;
                    self.export_rx = None;
                    self.export_progress = 1.0;
                    self.export_stage = "Export complete".to_string();
                    let summary = if report.outputs.is_empty() {
                        "Export failed: no files were written.".to_string()
                    } else if report.errors.is_empty() {
                        format!(
                            "Exported {} file(s) in {:.1}s",
                            report.outputs.len(),
                            report.elapsed_secs
                        )
                    } else {
                        format!(
                            "Exported {} file(s) with {} error(s)",
                            report.outputs.len(),
                            report.errors.len()
                        )
                    };
                    self.set_status(summary);
                    for path in &report.outputs {
                        self.push_log(format!("Output: {}", path.display()));
                    }
                    for warning in &report.warnings {
                        self.push_log(format!("Warning: {warning}"));
                    }
                    for error in &report.errors {
                        self.push_log(format!("Error: {error}"));
                    }
                    if !report.errors.is_empty() {
                        show_error_dialog(
                            "SurroundPacker — Export Warnings",
                            &report.errors.join("\n\n"),
                        );
                    }
                    self.last_report = Some(report);
                }
                ExportEvent::Failed(message) => {
                    self.exporting = false;
                    self.export_rx = None;
                    self.export_stage = "Export failed".to_string();
                    self.set_status(format!("Export failed: {message}"));
                    show_error_dialog("SurroundPacker — Export Failed", &message);
                }
            }
        }

        if let Some(report) = &self.last_report {
            if !report.outputs.is_empty() {
                // Keep the progress bar at 100% after a successful run.
                self.export_progress = 1.0;
            }
        }
    }

    fn maybe_start_analysis(&mut self) {
        if self.exporting || !self.analysis_dirty || self.analysis_rx.is_some() {
            return;
        }
        if self.last_edit.elapsed() < Duration::from_millis(350) {
            return;
        }
        let snapshot = self.snapshot();
        if !snapshot.has_audible_routes() {
            self.peaks = None;
            self.analysis_dirty = false;
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = mixdown::render(&snapshot, |_| {})
                .map(|mix| PeakReport {
                    pre_db: mix.pre_peaks_db,
                    post_db: mix.post_peaks_db,
                    limiter_active: mix.limiter_active,
                    gate_reduction_db: mix.gate_reduction_db,
                })
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.analysis_rx = Some(rx);
        self.analysis_dirty = false;
    }

    fn handle_os_drop(&mut self, ctx: &Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        if self.tab == AppTab::AudioDvd {
            let supported: Vec<PathBuf> = dropped
                .into_iter()
                .filter(|path| is_batch_supported_path(path))
                .collect();
            let added = self.batch.add_paths(&supported);
            self.set_status(format!("Added {} file(s) to the Audio-DVD queue.", added));
            return;
        }
        let pointer = ctx.input(|input| input.pointer.hover_pos());
        let target = pointer.and_then(|position| {
            Channel::ALL
                .iter()
                .find(|channel| self.slot_rects[channel.index()].contains(position))
                .copied()
        });
        let mut first = true;
        for path in dropped {
            let channel = if first { target } else { None };
            first = false;
            self.enqueue_load(path, channel);
        }
    }

    pub fn show_slot_editor(&mut self, ctx: &Context) {
        let Some(channel) = self.editor_channel else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{} · {} layers", channel.short(), channel.name()))
            .open(&mut open)
            .default_size([480.0, 430.0])
            .min_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Layer multiple sources into one speaker. The channel trim applies to the summed result.",
                    )
                    .size(10.0)
                    .color(theme::MUTED),
                );
                ui.separator();

                let slot = &mut self.slots[channel.index()];
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Channel trim").size(10.0));
                    ui.add(
                        egui::Slider::new(&mut slot.gain_db, -12.0..=12.0)
                            .suffix(" dB")
                            .fixed_decimals(1),
                    );
                    ui.checkbox(&mut slot.mute, "Mute");
                    ui.checkbox(&mut slot.solo, "Solo");
                });

                ui.separator();
                let mut remove_index = None;
                let mut changed = false;
                for (index, route) in slot.routes.iter_mut().enumerate() {
                    egui::Frame::new()
                        .fill(theme::CARD)
                        .corner_radius(8)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let name = route.display_name();
                                ui.label(
                                    egui::RichText::new(theme::short_path(&name, 34))
                                        .size(10.5)
                                        .color(theme::TEXT),
                                )
                                .on_hover_text(route.stem_path.display().to_string());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("Remove").clicked() {
                                        remove_index = Some(index);
                                    }
                                    if ui.checkbox(&mut route.mute, "Mute").changed() {
                                        changed = true;
                                    }
                                });
                            });
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(("edit-mode", channel.index(), index))
                                    .width(108.0)
                                    .selected_text(route.mode.label())
                                    .show_ui(ui, |ui| {
                                        for mode in crate::routing::ChannelMode::ALL {
                                            if ui
                                                .selectable_value(&mut route.mode, mode, mode.label())
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(&mut route.gain_db, -12.0..=12.0)
                                        .show_value(false),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{:+.1} dB", route.gain_db))
                                        .size(9.0)
                                        .color(theme::MUTED),
                                );
                            });
                        });
                }
                if let Some(index) = remove_index {
                    slot.remove_route(index);
                    changed = true;
                }

                let stem_choices: Vec<(u64, String)> = self
                    .stems
                    .iter()
                    .map(|stem| (stem.id, stem.display_name.clone()))
                    .collect();
                let mut add_stem = None;
                ui.menu_button("+ Add source to this speaker", |ui| {
                    for (id, name) in stem_choices {
                        if ui.button(name).clicked() {
                            add_stem = Some(id);
                            ui.close();
                        }
                    }
                });
                if let Some(id) = add_stem {
                    if let Some(stem) = self.stems.iter().find(|stem| stem.id == id) {
                        slot.routes.push(Route::new(
                            id,
                            stem.path.clone(),
                            channel.default_mode(),
                            0.0,
                        ));
                        changed = true;
                    }
                }

                if changed {
                    self.mark_dirty();
                }
            });
        if !open {
            self.editor_channel = None;
        }
    }
}

impl Drop for SurroundPackerApp {
    fn drop(&mut self) {
        if self.batch.converting {
            self.batch.cancel_build();
        }
    }
}

impl eframe::App for SurroundPackerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_workers();
        self.handle_os_drop(&ctx);
        self.maybe_start_analysis();

        egui::Panel::top("surround-packer-header")
            .exact_size(60.0)
            .show(ui, |ui| panels::show_header(self, ui));

        egui::Panel::bottom("surround-packer-status")
            .exact_size(30.0)
            .show(ui, |ui| panels::show_status(self, ui));

        if self.tab == AppTab::AudioDvd {
            egui::Panel::bottom("surround-packer-batch-footer")
                .resizable(false)
                .exact_size(112.0)
                .show(ui, |ui| panels::show_batch_footer(self, ui));
        }

        if self.tab == AppTab::StemPacker {
            egui::Panel::left("surround-packer-stems")
                .resizable(false)
                .default_size(276.0)
                .show(ui, |ui| panels::show_left(self, ui));

            egui::Panel::right("surround-packer-export")
                .resizable(false)
                .default_size(352.0)
                .show(ui, |ui| panels::show_right(self, ui));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ui, |ui| {
                if self.tab == AppTab::AudioDvd {
                    panels::show_batch(self, ui);
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("SPEAKER ROOM")
                            .strong()
                            .size(11.0)
                            .color(theme::TEXT),
                    );
                    let order = self
                        .layout
                        .channels()
                        .iter()
                        .map(|channel| channel.short())
                        .collect::<Vec<_>>()
                        .join(" ");
                    ui.label(
                        egui::RichText::new(format!(
                            "SMPTE {} · {}",
                            self.layout.short_label(),
                            order
                        ))
                        .size(9.0)
                        .color(theme::FAINT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Drag stems onto a speaker")
                                .size(9.0)
                                .color(theme::FAINT),
                        );
                    });
                });

                let room = speaker_room::show(
                    ui,
                    self.layout,
                    &mut self.slots,
                    self.peaks.as_ref(),
                    &mut self.slot_rects,
                );
                if room.changed {
                    self.mark_dirty();
                }
                if let Some(channel) = room.editor {
                    self.editor_channel = Some(channel);
                }
                if let Some((channel, path)) = room.dropped {
                    if let Some(stem) = self.stems.iter().find(|stem| {
                        stem.path == path
                            || std::fs::canonicalize(&stem.path).ok()
                                == std::fs::canonicalize(&path).ok()
                    }) {
                        self.attach_stem_to_channel(stem.id, channel);
                    } else {
                        self.enqueue_load(path, Some(channel));
                    }
                }

                ui.add_space(2.0);
                let mut play = false;
                let mut stop = false;
                speaker_room::show_audition_controls(
                    ui,
                    self.layout,
                    self.audition.playing,
                    self.audition.rendering,
                    &mut play,
                    &mut stop,
                );
                if play {
                    self.start_audition();
                }
                if stop {
                    self.stop_audition();
                }
                ui.add_space(2.0);
                speaker_room::show_master_meters(ui, self.layout, self.peaks.as_ref());
            });

        if self.tab == AppTab::StemPacker {
            self.show_slot_editor(&ctx);
        }

        if self.exporting || self.analysis_rx.is_some() || self.loading > 0 || self.batch.converting
        {
            ctx.request_repaint_after(Duration::from_millis(120));
        } else if self.analysis_dirty {
            // Wake up once the debounce window has elapsed so meters refresh
            // even if the pointer never moves.
            ctx.request_repaint_after(Duration::from_millis(400));
        }
    }
}

pub fn show_error_dialog(title: &str, message: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
