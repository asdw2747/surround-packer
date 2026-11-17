use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dsp::{
    DspSettings, GATE_DEFAULT_RELEASE_MS, GATE_DEFAULT_THRESHOLD_DB, GATE_RELEASE_MAX_MS,
    GATE_RELEASE_MIN_MS, GATE_THRESHOLD_MAX_DB, GATE_THRESHOLD_MIN_DB,
};

pub const CHANNEL_COUNT: usize = MAX_CHANNELS;
pub const MAX_ROUTES_PER_CHANNEL: usize = 12;

pub use crate::model::{Channel, ChannelLayout, MAX_CHANNELS};

impl Channel {
    pub const fn default_mode(self) -> ChannelMode {
        match self {
            Self::Fl | Self::Bl | Self::Sl => ChannelMode::StereoL,
            Self::Fr | Self::Br | Self::Sr => ChannelMode::StereoR,
            Self::Fc | Self::Lfe => ChannelMode::MonoSum,
        }
    }
}

/// How a (possibly stereo) source stem is reduced to one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelMode {
    StereoL,
    StereoR,
    MonoSum,
    LeftOnly,
    RightOnly,
}

impl ChannelMode {
    pub const ALL: [Self; 5] = [
        Self::StereoL,
        Self::StereoR,
        Self::MonoSum,
        Self::LeftOnly,
        Self::RightOnly,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::StereoL => "Stereo L",
            Self::StereoR => "Stereo R",
            Self::MonoSum => "Mono Sum",
            Self::LeftOnly => "Left Only",
            Self::RightOnly => "Right Only",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::StereoL => "Use the left channel (mono sources use their only channel).",
            Self::StereoR => "Use the right channel (mono sources use their only channel).",
            Self::MonoSum => "Sum left and right with the selected pan law.",
            Self::LeftOnly => "Force the left channel, even for multichannel sources.",
            Self::RightOnly => "Force the right channel, even for multichannel sources.",
        }
    }

    /// Resolve a zero-copy channel tap for this source.
    pub fn tap(self, source_channels: usize, pan_law: PanLaw) -> Option<SourceTap> {
        if source_channels == 0 {
            return None;
        }
        match self {
            Self::StereoL | Self::LeftOnly => Some(SourceTap {
                first: 0,
                first_gain: 1.0,
                second: None,
                second_gain: 0.0,
            }),
            Self::StereoR | Self::RightOnly => Some(SourceTap {
                first: usize::from(source_channels > 1),
                first_gain: 1.0,
                second: None,
                second_gain: 0.0,
            }),
            Self::MonoSum => {
                if source_channels > 1 {
                    let factor = pan_law.factor();
                    Some(SourceTap {
                        first: 0,
                        first_gain: factor,
                        second: Some(1),
                        second_gain: factor,
                    })
                } else {
                    Some(SourceTap {
                        first: 0,
                        first_gain: 1.0,
                        second: None,
                        second_gain: 0.0,
                    })
                }
            }
        }
    }
}

/// A resolved source tap. `second_gain` is only used for mono summing.
#[derive(Clone, Copy, Debug)]
pub struct SourceTap {
    pub first: usize,
    pub first_gain: f32,
    pub second: Option<usize>,
    pub second_gain: f32,
}

impl SourceTap {
    pub fn read(self, planes: &[Vec<f32>], frame: usize) -> f32 {
        let mut value = planes
            .get(self.first)
            .and_then(|plane| plane.get(frame))
            .copied()
            .unwrap_or(0.0)
            * self.first_gain;
        if let Some(second) = self.second {
            value += planes
                .get(second)
                .and_then(|plane| plane.get(frame))
                .copied()
                .unwrap_or(0.0)
                * self.second_gain;
        }
        value
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanLaw {
    #[default]
    Minus3,
    Minus6,
}

impl PanLaw {
    pub const fn factor(self) -> f32 {
        match self {
            Self::Minus3 => std::f32::consts::FRAC_1_SQRT_2,
            Self::Minus6 => 0.5,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Minus3 => "-3 dB",
            Self::Minus6 => "-6 dB",
        }
    }
}

/// One stem feeding one speaker channel.
#[derive(Clone, Debug)]
pub struct Route {
    pub stem_id: u64,
    pub stem_path: PathBuf,
    pub mode: ChannelMode,
    pub gain_db: f32,
    pub mute: bool,
}

impl Route {
    pub fn new(stem_id: u64, stem_path: PathBuf, mode: ChannelMode, gain_db: f32) -> Self {
        Self {
            stem_id,
            stem_path,
            mode,
            gain_db,
            mute: false,
        }
    }

    pub fn display_name(&self) -> String {
        self.stem_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown stem")
            .to_string()
    }
}

/// A speaker channel. Multiple stems may be layered into the same speaker.
#[derive(Clone, Debug)]
pub struct Slot {
    pub channel: Channel,
    pub routes: Vec<Route>,
    pub gain_db: f32,
    pub mute: bool,
    pub solo: bool,
    pub gate_enabled: bool,
    pub gate_threshold_db: f32,
    pub gate_release_ms: f32,
}

impl Slot {
    pub fn new(channel: Channel) -> Self {
        Self {
            channel,
            routes: Vec::new(),
            gain_db: 0.0,
            mute: false,
            solo: false,
            gate_enabled: false,
            gate_threshold_db: GATE_DEFAULT_THRESHOLD_DB,
            gate_release_ms: GATE_DEFAULT_RELEASE_MS,
        }
    }

    pub fn empty_slots() -> [Self; CHANNEL_COUNT] {
        std::array::from_fn(|index| Self::new(Channel::ALL[index]))
    }

    pub fn clear(&mut self) {
        self.routes.clear();
        self.gain_db = 0.0;
        self.mute = false;
        self.solo = false;
        self.gate_enabled = false;
        self.gate_threshold_db = GATE_DEFAULT_THRESHOLD_DB;
        self.gate_release_ms = GATE_DEFAULT_RELEASE_MS;
    }

    pub fn add_route(&mut self, route: Route) {
        if self.routes.len() < MAX_ROUTES_PER_CHANNEL {
            self.routes.push(route);
        }
    }

    pub fn remove_route(&mut self, index: usize) {
        if index < self.routes.len() {
            self.routes.remove(index);
        }
    }

    pub fn primary(&self) -> Option<&Route> {
        self.routes.first()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemKind {
    LeadVocals,
    Vocals,
    BackgroundVocals,
    Drums,
    Bass,
    Melodic,
    Ambient,
    Other,
    Unknown,
}

impl StemKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::LeadVocals => "Lead Vocals",
            Self::Vocals => "Vocals",
            Self::BackgroundVocals => "Background Vocals",
            Self::Drums => "Drums",
            Self::Bass => "Bass",
            Self::Melodic => "Melodic",
            Self::Ambient => "Ambient",
            Self::Other => "Other",
            Self::Unknown => "Unknown",
        }
    }
}

/// Strip only a known audio extension. `Path::file_stem` is unsafe here because
/// song titles such as "(feat. Artist)" contain dots, and `StemRef` paths may
/// already have had their extension removed.
fn audio_stem(name: &str) -> &str {
    let file = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    if let Some((base, extension)) = file.rsplit_once('.') {
        let extension = extension.to_ascii_lowercase();
        if !base.is_empty()
            && matches!(
                extension.as_str(),
                "wav"
                    | "flac"
                    | "mp3"
                    | "ogg"
                    | "oga"
                    | "m4a"
                    | "aac"
                    | "aiff"
                    | "aif"
                    | "opus"
                    | "wv"
                    | "ape"
                    | "dts"
                    | "ac3"
                    | "eac3"
                    | "mka"
            )
        {
            return base;
        }
    }
    file
}

/// Normalize a tag or filename fragment into lowercase alphanumeric tokens.
fn role_tokens(text: &str) -> Vec<&str> {
    // The returned &str slices borrow from `text`; callers pass owned strings
    // that outlive the token vector.
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

fn normalize_lower(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect()
}

fn role_from_tokens(tokens: &[&str]) -> Option<StemKind> {
    let has = |candidates: &[&str]| tokens.iter().any(|token| candidates.contains(token));

    let vocals = has(&["vocals", "vocal", "vox", "voice", "voices", "acapella"]);
    let lead = has(&["lead"]);
    let lead_compound = has(&["leadvocal", "leadvocals"]);
    let background = has(&["background", "backing"]);
    let bgv = has(&["bgv", "bgvs"]);
    let harmony = has(&["harmony", "harmonies", "choir"]);

    if (vocals && lead) || lead_compound {
        Some(StemKind::LeadVocals)
    } else if (vocals && background) || bgv || harmony {
        Some(StemKind::BackgroundVocals)
    } else if vocals {
        Some(StemKind::Vocals)
    } else if has(&["drums", "drum", "percussion", "perc", "kick", "snare"]) {
        Some(StemKind::Drums)
    } else if has(&["bass", "sub", "subbass", "lowend"]) {
        Some(StemKind::Bass)
    } else if has(&["other", "instrumental", "instrument", "inst", "karaoke"]) {
        Some(StemKind::Other)
    } else {
        None
    }
}

fn parenthesized_groups(stem: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in stem.chars() {
        match ch {
            '(' => {
                depth += 1;
                if depth == 1 {
                    current.clear();
                }
            }
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    groups.push(current.clone());
                }
            }
            _ if depth > 0 => current.push(ch),
            _ => {}
        }
    }
    groups
}

fn trailing_role(stem: &str) -> Option<StemKind> {
    // Prefer the last parenthesized role tag. This ignores earlier tags such as
    // "(feat. Artist)" in song titles.
    for group in parenthesized_groups(stem).iter().rev() {
        let normalized = normalize_lower(group);
        let tokens = role_tokens(&normalized);
        if let Some(kind) = role_from_tokens(&tokens) {
            return Some(kind);
        }
    }

    // Otherwise accept a trailing suffix such as "Song - Lead Vocals" or
    // "Song_lead_vocals". Only the final three tokens are considered.
    let normalized = normalize_lower(stem);
    let tokens = role_tokens(&normalized);
    let start = tokens.len().saturating_sub(3);
    role_from_tokens(&tokens[start..])
}

/// Robust UVR5-style filename classification.
///
/// Roles are taken from the trailing parenthesized tag (or trailing suffix),
/// so an earlier title fragment such as "(feat. Artist)" cannot override the
/// actual UVR5 role.
pub fn classify_filename(name: &str) -> StemKind {
    let stem = audio_stem(name);

    if let Some(kind) = trailing_role(stem) {
        return kind;
    }

    // Broad fallback for non-UVR5 creative stems. Role words are intentionally
    // not considered here: they must come from the trailing tag.
    let normalized = normalize_lower(stem);
    let tokens = role_tokens(&normalized);
    let has = |candidates: &[&str]| tokens.iter().any(|token| candidates.contains(token));

    if has(&[
        "ambient",
        "atmos",
        "atmosphere",
        "pad",
        "pads",
        "strings",
        "string",
    ]) {
        StemKind::Ambient
    } else if has(&[
        "guitar",
        "guitars",
        "synth",
        "synthesizer",
        "piano",
        "keys",
        "keyboard",
        "melody",
        "melodic",
        "lead",
        "arp",
        "arpeggio",
    ]) {
        StemKind::Melodic
    } else {
        StemKind::Unknown
    }
}

#[derive(Clone, Debug)]
pub struct StemRef {
    pub id: u64,
    pub path: PathBuf,
}

impl StemRef {
    pub fn new(id: u64, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            path: path.into(),
        }
    }

    pub fn display_name(&self) -> String {
        self.path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown stem")
            .to_string()
    }

    pub fn kind(&self) -> StemKind {
        classify_filename(&self.display_name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    VocalSplit,
    VocalsDrumsFront,
    ClassicQuad,
}

impl Preset {
    pub const fn label(self) -> &'static str {
        match self {
            Self::VocalSplit => "5-Stem Vocal Split",
            Self::VocalsDrumsFront => "Vocals + Drums Front / Instruments Surround",
            Self::ClassicQuad => "Classic Quad + Center Vocal",
        }
    }
}

fn first_of(stems: &[StemRef], kind: StemKind) -> Option<&StemRef> {
    stems.iter().find(|stem| stem.kind() == kind)
}

fn add_route(
    slots: &mut [Slot; CHANNEL_COUNT],
    channel: Channel,
    stem: &StemRef,
    mode: ChannelMode,
    gain_db: f32,
) {
    slots[channel.index()].add_route(Route::new(stem.id, stem.path.clone(), mode, gain_db));
}

fn has_route_from(slots: &[Slot; CHANNEL_COUNT], channel: Channel, stem_id: u64) -> bool {
    slots[channel.index()]
        .routes
        .iter()
        .any(|route| route.stem_id == stem_id)
}

fn add_front(slots: &mut [Slot; CHANNEL_COUNT], stem: &StemRef, gain_db: f32) {
    add_route(slots, Channel::Fl, stem, ChannelMode::StereoL, gain_db);
    add_route(slots, Channel::Fr, stem, ChannelMode::StereoR, gain_db);
}

fn add_surround(slots: &mut [Slot; CHANNEL_COUNT], stem: &StemRef, gain_db: f32) {
    add_route(slots, Channel::Sl, stem, ChannelMode::StereoL, gain_db);
    add_route(slots, Channel::Sr, stem, ChannelMode::StereoR, gain_db);
}

fn add_back(slots: &mut [Slot; CHANNEL_COUNT], stem: &StemRef, gain_db: f32) {
    add_route(slots, Channel::Bl, stem, ChannelMode::StereoL, gain_db);
    add_route(slots, Channel::Br, stem, ChannelMode::StereoR, gain_db);
}

fn add_ambient(
    slots: &mut [Slot; CHANNEL_COUNT],
    layout: ChannelLayout,
    stem: &StemRef,
    gain_db: f32,
) {
    if layout.is_71() {
        add_back(slots, stem, gain_db);
    } else {
        add_surround(slots, stem, gain_db);
    }
}

fn fill_remaining(
    slots: &mut [Slot; CHANNEL_COUNT],
    layout: ChannelLayout,
    stem: &StemRef,
    gain_db: f32,
) {
    if slots[Channel::Fl.index()].is_empty() {
        add_front(slots, stem, gain_db);
    } else if slots[Channel::Sl.index()].is_empty() {
        add_surround(slots, stem, gain_db);
    } else if layout.is_71() && slots[Channel::Bl.index()].is_empty() {
        add_back(slots, stem, gain_db);
    }
}

/// Apply a one-click UVR5 routing preset for the selected layout.
pub fn apply_preset(
    preset: Preset,
    stems: &[StemRef],
    layout: ChannelLayout,
) -> [Slot; CHANNEL_COUNT] {
    let mut slots = Slot::empty_slots();

    let lead = first_of(stems, StemKind::LeadVocals);
    let combined_vocals = first_of(stems, StemKind::Vocals);
    let vocals = lead.or(combined_vocals);
    let backing = first_of(stems, StemKind::BackgroundVocals);
    let drums = first_of(stems, StemKind::Drums);
    let bass = first_of(stems, StemKind::Bass);
    let melodic = first_of(stems, StemKind::Melodic).or_else(|| first_of(stems, StemKind::Other));
    let ambient = first_of(stems, StemKind::Ambient);
    let unknown = first_of(stems, StemKind::Unknown);

    match preset {
        Preset::VocalSplit => {
            if let Some(vocals) = vocals {
                add_route(&mut slots, Channel::Fc, vocals, ChannelMode::MonoSum, 0.0);
                let slot = &mut slots[Channel::Fc.index()];
                slot.gate_enabled = true;
                slot.gate_threshold_db = GATE_DEFAULT_THRESHOLD_DB;
                slot.gate_release_ms = GATE_DEFAULT_RELEASE_MS;
            }
            if let Some(backing) = backing {
                add_surround(&mut slots, backing, 0.0);
            }
            if let Some(drums) = drums {
                add_front(&mut slots, drums, 0.0);
            }
            let other = first_of(stems, StemKind::Other);
            if let Some(other) = other {
                add_front(&mut slots, other, 0.0);
                add_surround(&mut slots, other, -4.5);
            }
            if let Some(bass) = bass {
                add_route(&mut slots, Channel::Lfe, bass, ChannelMode::MonoSum, 0.0);
            }
        }
        Preset::VocalsDrumsFront => {
            if let Some(drums) = drums {
                add_front(&mut slots, drums, 0.0);
                add_route(&mut slots, Channel::Lfe, drums, ChannelMode::MonoSum, -6.0);
            }
            if let Some(bass) = bass {
                add_route(&mut slots, Channel::Lfe, bass, ChannelMode::MonoSum, 0.0);
                if !has_route_from(&slots, Channel::Fl, bass.id) {
                    add_route(&mut slots, Channel::Fl, bass, ChannelMode::StereoL, -6.0);
                    add_route(&mut slots, Channel::Fr, bass, ChannelMode::StereoR, -6.0);
                }
            }
            if let Some(vocals) = vocals {
                add_route(&mut slots, Channel::Fc, vocals, ChannelMode::MonoSum, 0.0);
                add_route(&mut slots, Channel::Fl, vocals, ChannelMode::StereoL, -9.0);
                add_route(&mut slots, Channel::Fr, vocals, ChannelMode::StereoR, -9.0);
            }
            if let Some(melodic) = melodic {
                add_surround(&mut slots, melodic, 0.0);
            }
            if let Some(ambient) = ambient {
                add_ambient(&mut slots, layout, ambient, -3.0);
            }
            if let Some(backing) = backing {
                add_ambient(&mut slots, layout, backing, -6.0);
            }
            if let Some(unknown) = unknown {
                fill_remaining(&mut slots, layout, unknown, -3.0);
            }
        }
        Preset::ClassicQuad => {
            if let Some(vocals) = vocals {
                add_route(&mut slots, Channel::Fc, vocals, ChannelMode::MonoSum, 0.0);
            }
            if let Some(drums) = drums {
                add_front(&mut slots, drums, 0.0);
                add_route(&mut slots, Channel::Lfe, drums, ChannelMode::MonoSum, -6.0);
            }
            if let Some(bass) = bass {
                add_route(&mut slots, Channel::Lfe, bass, ChannelMode::MonoSum, 0.0);
                if !has_route_from(&slots, Channel::Fl, bass.id) {
                    add_route(&mut slots, Channel::Fl, bass, ChannelMode::StereoL, -9.0);
                    add_route(&mut slots, Channel::Fr, bass, ChannelMode::StereoR, -9.0);
                }
            }
            if let Some(melodic) = melodic {
                add_surround(&mut slots, melodic, 0.0);
            }
            if let Some(ambient) = ambient {
                add_ambient(&mut slots, layout, ambient, -3.0);
            }
            if let Some(backing) = backing {
                add_ambient(&mut slots, layout, backing, -6.0);
            }
            if let Some(unknown) = unknown {
                fill_remaining(&mut slots, layout, unknown, -3.0);
            }
        }
    }

    slots
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateRoute {
    pub stem_path: PathBuf,
    pub mode: ChannelMode,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default)]
    pub mute: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateSlot {
    pub channel: Channel,
    #[serde(default)]
    pub routes: Vec<TemplateRoute>,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub gate_enabled: bool,
    #[serde(default = "default_gate_threshold_db")]
    pub gate_threshold_db: f32,
    #[serde(default = "default_gate_release_ms")]
    pub gate_release_ms: f32,
}

fn default_gate_threshold_db() -> f32 {
    GATE_DEFAULT_THRESHOLD_DB
}

fn default_gate_release_ms() -> f32 {
    GATE_DEFAULT_RELEASE_MS
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingTemplate {
    pub format_version: u32,
    pub name: String,
    #[serde(default)]
    pub layout: ChannelLayout,
    #[serde(default)]
    pub pan_law: PanLaw,
    #[serde(default)]
    pub dsp: DspSettings,
    pub slots: Vec<TemplateSlot>,
}

impl RoutingTemplate {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn from_slots(
        name: impl Into<String>,
        layout: ChannelLayout,
        slots: &[Slot; CHANNEL_COUNT],
        pan_law: PanLaw,
        dsp: &DspSettings,
    ) -> Self {
        let slots = slots
            .iter()
            .map(|slot| TemplateSlot {
                channel: slot.channel,
                routes: slot
                    .routes
                    .iter()
                    .map(|route| TemplateRoute {
                        stem_path: route.stem_path.clone(),
                        mode: route.mode,
                        gain_db: route.gain_db,
                        mute: route.mute,
                    })
                    .collect(),
                gain_db: slot.gain_db,
                mute: slot.mute,
                solo: slot.solo,
                gate_enabled: slot.gate_enabled,
                gate_threshold_db: slot.gate_threshold_db,
                gate_release_ms: slot.gate_release_ms,
            })
            .collect();
        Self {
            format_version: Self::CURRENT_VERSION,
            name: name.into(),
            layout,
            pan_law,
            dsp: dsp.clone(),
            slots,
        }
    }

    /// Convert the template into runtime slots, resolving paths through a callback.
    /// Returns the number of routes that could not be resolved.
    pub fn to_slots<F>(&self, mut resolve: F) -> ([Slot; CHANNEL_COUNT], usize)
    where
        F: FnMut(&Path) -> Option<(u64, PathBuf)>,
    {
        let mut slots = Slot::empty_slots();
        let mut missing = 0usize;

        for template_slot in &self.slots {
            let index = template_slot.channel.index();
            let slot = &mut slots[index];
            slot.gain_db = template_slot.gain_db.clamp(-12.0, 12.0);
            slot.mute = template_slot.mute;
            slot.solo = template_slot.solo;
            slot.gate_enabled = template_slot.gate_enabled;
            slot.gate_threshold_db = template_slot
                .gate_threshold_db
                .clamp(GATE_THRESHOLD_MIN_DB, GATE_THRESHOLD_MAX_DB);
            slot.gate_release_ms = template_slot
                .gate_release_ms
                .clamp(GATE_RELEASE_MIN_MS, GATE_RELEASE_MAX_MS);
            for route in &template_slot.routes {
                match resolve(&route.stem_path) {
                    Some((stem_id, path)) => slot.add_route(Route::new(
                        stem_id,
                        path,
                        route.mode,
                        route.gain_db.clamp(-12.0, 12.0),
                    )),
                    None => missing += 1,
                }
            }
        }

        (slots, missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_stems() -> Vec<StemRef> {
        vec![
            StemRef::new(7, "antonymph_(Vocals).flac"),
            StemRef::new(8, "antonymph_(Drums).flac"),
        ]
    }

    #[test]
    fn classifies_uvr5_fixture_names() {
        assert_eq!(
            classify_filename("antonymph_(Vocals).flac"),
            StemKind::Vocals
        );
        assert_eq!(classify_filename("antonymph_(Drums).flac"), StemKind::Drums);
        assert_eq!(classify_filename("Song (Bass).wav"), StemKind::Bass);
        assert_eq!(classify_filename("Song (Guitar).ogg"), StemKind::Melodic);
        assert_eq!(classify_filename("Song (Other).mp3"), StemKind::Other);
        assert_eq!(
            classify_filename("Song (Instrumental).flac"),
            StemKind::Other
        );
        assert_eq!(
            classify_filename("Song (Backing Vocals).flac"),
            StemKind::BackgroundVocals
        );
        assert_eq!(
            classify_filename("Song (Lead Vocals).flac"),
            StemKind::LeadVocals
        );
        assert_eq!(
            classify_filename("Song (lead_vocals).flac"),
            StemKind::LeadVocals
        );
        assert_eq!(
            classify_filename("Song (Background Vocals).flac"),
            StemKind::BackgroundVocals
        );
        assert_eq!(
            classify_filename("Song (backing_vocals).flac"),
            StemKind::BackgroundVocals
        );
        assert_eq!(
            classify_filename("Song (Harmony).flac"),
            StemKind::BackgroundVocals
        );
        assert_eq!(classify_filename("Song (Pad).flac"), StemKind::Ambient);
        assert_eq!(classify_filename("Song (Strings).flac"), StemKind::Ambient);
    }

    #[test]
    fn classifies_trailing_vocal_split_tags_after_title_metadata() {
        let prefix = "1_03_Unstoppable (feat. Artist)_";
        for (suffix, expected) in [
            ("(Lead Vocals)", StemKind::LeadVocals),
            ("(Background Vocals)", StemKind::BackgroundVocals),
            ("(Vocals)", StemKind::Vocals),
            ("(Drums)", StemKind::Drums),
            ("(Bass)", StemKind::Bass),
            ("(Other)", StemKind::Other),
        ] {
            assert_eq!(
                classify_filename(&format!("{prefix}{suffix}.wav")),
                expected,
                "failed to classify {suffix}"
            );
        }
        assert_eq!(
            classify_filename("1_03_Unstoppable (feat. Artist)_lead_vocals.wav"),
            StemKind::LeadVocals
        );
        assert_eq!(
            classify_filename("1_03_Unstoppable (feat. Artist)_background_vocals.wav"),
            StemKind::BackgroundVocals
        );
    }

    fn vocal_split_stems(include_combined: bool) -> Vec<StemRef> {
        let mut stems = vec![
            StemRef::new(1, "Song (Lead Vocals).flac"),
            StemRef::new(2, "Song (Background Vocals).flac"),
            StemRef::new(3, "Song (Drums).flac"),
            StemRef::new(4, "Song (Bass).flac"),
            StemRef::new(5, "Song (Other).flac"),
        ];
        if include_combined {
            stems.push(StemRef::new(6, "Song (Vocals).flac"));
        }
        stems
    }

    fn route_gain(slot: &Slot, stem_id: u64) -> Option<f32> {
        slot.routes
            .iter()
            .find(|route| route.stem_id == stem_id)
            .map(|route| route.gain_db)
    }

    #[test]
    fn vocal_split_preset_routes_lead_backing_drums_other_and_bass() {
        for layout in [ChannelLayout::Surround51, ChannelLayout::Surround71] {
            let slots = apply_preset(Preset::VocalSplit, &vocal_split_stems(false), layout);
            let fc = &slots[Channel::Fc.index()];
            assert_eq!(fc.routes.len(), 1);
            assert_eq!(fc.routes[0].stem_id, 1);
            assert!(fc.gate_enabled);
            assert_eq!(fc.gate_threshold_db, GATE_DEFAULT_THRESHOLD_DB);
            assert_eq!(fc.gate_release_ms, GATE_DEFAULT_RELEASE_MS);

            for channel in [Channel::Sl, Channel::Sr] {
                let slot = &slots[channel.index()];
                assert_eq!(route_gain(slot, 2), Some(0.0));
                assert_eq!(route_gain(slot, 5), Some(-4.5));
            }
            for channel in [Channel::Fl, Channel::Fr] {
                let slot = &slots[channel.index()];
                assert_eq!(route_gain(slot, 3), Some(0.0));
                assert_eq!(route_gain(slot, 5), Some(0.0));
            }
            assert_eq!(route_gain(&slots[Channel::Lfe.index()], 4), Some(0.0));
            assert!(slots[Channel::Bl.index()].is_empty());
            assert!(slots[Channel::Br.index()].is_empty());
        }
    }

    #[test]
    fn vocal_split_preset_skips_combined_vocals_when_lead_exists() {
        let slots = apply_preset(
            Preset::VocalSplit,
            &vocal_split_stems(true),
            ChannelLayout::Surround51,
        );
        assert_eq!(
            slots[Channel::Fc.index()]
                .routes
                .iter()
                .map(|route| route.stem_id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        for slot in &slots {
            assert!(
                slot.routes.iter().all(|route| route.stem_id != 6),
                "combined Vocals stem was still routed in {}",
                slot.channel.short()
            );
        }
    }

    #[test]
    fn quick_preset_vocal_split_handles_title_metadata_and_combined_vocals() {
        let prefix = "1_03_Unstoppable (feat. Artist)_";
        let stems = vec![
            StemRef::new(1, format!("{prefix}(Lead Vocals).wav")),
            StemRef::new(2, format!("{prefix}(Background Vocals).wav")),
            StemRef::new(3, format!("{prefix}(Vocals).wav")),
            StemRef::new(4, format!("{prefix}(Drums).wav")),
            StemRef::new(5, format!("{prefix}(Bass).wav")),
            StemRef::new(6, format!("{prefix}(Other).wav")),
        ];
        let slots = apply_preset(Preset::VocalSplit, &stems, ChannelLayout::Surround51);
        assert_eq!(route_gain(&slots[Channel::Fc.index()], 1), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Sl.index()], 2), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Sr.index()], 2), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Fl.index()], 4), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Fr.index()], 4), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Fl.index()], 6), Some(0.0));
        assert_eq!(route_gain(&slots[Channel::Sl.index()], 6), Some(-4.5));
        assert_eq!(route_gain(&slots[Channel::Lfe.index()], 5), Some(0.0));
        assert!(slots[Channel::Fc.index()].gate_enabled);
        for slot in &slots {
            assert!(
                slot.routes.iter().all(|route| route.stem_id != 3),
                "combined Vocals was routed in {}",
                slot.channel.short()
            );
        }
    }

    #[test]
    fn seven_one_preset_splits_lead_and_ambient() {
        let stems = vec![
            StemRef::new(1, "antonymph_(Vocals).flac"),
            StemRef::new(2, "antonymph_(Drums).flac"),
            StemRef::new(3, "Song (Guitar).flac"),
            StemRef::new(4, "Song (Pad).flac"),
            StemRef::new(5, "Song (Backing Vocals).flac"),
        ];
        let slots = apply_preset(Preset::VocalsDrumsFront, &stems, ChannelLayout::Surround71);
        assert!(slots[Channel::Sl.index()]
            .routes
            .iter()
            .any(|route| route.stem_id == 3));
        assert!(slots[Channel::Sr.index()]
            .routes
            .iter()
            .any(|route| route.stem_id == 3));
        assert!(slots[Channel::Bl.index()]
            .routes
            .iter()
            .any(|route| route.stem_id == 4));
        assert!(slots[Channel::Br.index()]
            .routes
            .iter()
            .any(|route| route.stem_id == 5));
    }

    #[test]
    fn vocals_drums_front_routing_uses_fc_and_layered_front() {
        let slots = apply_preset(
            Preset::VocalsDrumsFront,
            &fixture_stems(),
            ChannelLayout::Surround51,
        );
        let fc = &slots[Channel::Fc.index()];
        assert_eq!(fc.routes.len(), 1);
        assert_eq!(fc.routes[0].stem_id, 7);

        let fl = &slots[Channel::Fl.index()];
        assert!(fl
            .routes
            .iter()
            .any(|route| route.stem_id == 8 && route.gain_db == 0.0));
        assert!(fl
            .routes
            .iter()
            .any(|route| route.stem_id == 7 && route.gain_db == -9.0));

        let lfe = &slots[Channel::Lfe.index()];
        assert!(lfe.routes.iter().any(|route| route.stem_id == 8));
        assert!(!lfe.mute);
    }

    #[test]
    fn vocals_only_get_subtle_front_support() {
        let stems = vec![StemRef::new(1, "antonymph_(Vocals).flac")];
        let slots = apply_preset(Preset::VocalsDrumsFront, &stems, ChannelLayout::Surround51);
        assert_eq!(slots[Channel::Fc.index()].routes[0].stem_id, 1);
        assert_eq!(slots[Channel::Fl.index()].routes[0].gain_db, -9.0);
        assert_eq!(slots[Channel::Fr.index()].routes[0].gain_db, -9.0);
    }

    #[test]
    fn mono_sum_honors_minus_3_and_minus_6_db() {
        let planes = vec![vec![0.5f32], vec![0.25f32]];
        let minus3 = ChannelMode::MonoSum
            .tap(2, PanLaw::Minus3)
            .unwrap()
            .read(&planes, 0);
        let minus6 = ChannelMode::MonoSum
            .tap(2, PanLaw::Minus6)
            .unwrap()
            .read(&planes, 0);
        assert!((minus3 - 0.75 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((minus6 - 0.75 * 0.5).abs() < 1e-6);
        let mono = ChannelMode::MonoSum
            .tap(1, PanLaw::Minus6)
            .unwrap()
            .read(&[vec![0.5f32]], 0);
        assert!((mono - 0.5).abs() < 1e-6);
    }

    #[test]
    fn template_roundtrips_json() {
        let mut slots = Slot::empty_slots();
        slots[Channel::Fl.index()].add_route(Route::new(
            3,
            PathBuf::from("stem.flac"),
            ChannelMode::StereoL,
            -2.5,
        ));
        slots[Channel::Fl.index()].gain_db = 1.5;
        slots[Channel::Fl.index()].gate_enabled = true;
        slots[Channel::Fl.index()].gate_threshold_db = -40.0;
        slots[Channel::Fl.index()].gate_release_ms = 400.0;
        let template = RoutingTemplate::from_slots(
            "Test",
            ChannelLayout::Surround51,
            &slots,
            PanLaw::Minus6,
            &DspSettings::default(),
        );
        let json = serde_json::to_string_pretty(&template).unwrap();
        let decoded: RoutingTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "Test");
        assert_eq!(decoded.layout, ChannelLayout::Surround51);
        assert_eq!(decoded.pan_law, PanLaw::Minus6);
        assert_eq!(decoded.slots.len(), CHANNEL_COUNT);
        let (resolved, missing) = decoded
            .to_slots(|path| (path == Path::new("stem.flac")).then(|| (3, path.to_path_buf())));
        assert_eq!(missing, 0);
        assert_eq!(resolved[Channel::Fl.index()].routes[0].gain_db, -2.5);
        assert_eq!(resolved[Channel::Fl.index()].gain_db, 1.5);
        assert!(resolved[Channel::Fl.index()].gate_enabled);
        assert_eq!(resolved[Channel::Fl.index()].gate_threshold_db, -40.0);
        assert_eq!(resolved[Channel::Fl.index()].gate_release_ms, 400.0);
    }

    #[test]
    fn legacy_templates_load_gate_defaults() {
        let value = serde_json::json!({
            "format_version": 1,
            "name": "Legacy",
            "layout": "5.1",
            "slots": [{"channel": "fl"}]
        });
        let decoded: RoutingTemplate = serde_json::from_value(value).unwrap();
        let slot = &decoded.slots[0];
        assert!(!slot.gate_enabled);
        assert_eq!(slot.gate_threshold_db, GATE_DEFAULT_THRESHOLD_DB);
        assert_eq!(slot.gate_release_ms, GATE_DEFAULT_RELEASE_MS);
    }

    #[test]
    fn template_persists_seven_one_layout() {
        let slots = Slot::empty_slots();
        let template = RoutingTemplate::from_slots(
            "71",
            ChannelLayout::Surround71,
            &slots,
            PanLaw::Minus3,
            &DspSettings::default(),
        );
        let json = serde_json::to_string(&template).unwrap();
        let decoded: RoutingTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.layout, ChannelLayout::Surround71);
    }
}
