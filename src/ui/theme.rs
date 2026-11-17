use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Frame, Margin, Stroke, TextStyle, Theme,
};

use crate::routing::{Channel, StemKind};

pub const BG: Color32 = Color32::from_rgb(9, 12, 18);
pub const PANEL: Color32 = Color32::from_rgb(14, 19, 28);
pub const CARD: Color32 = Color32::from_rgb(20, 27, 39);
pub const CARD_HOVER: Color32 = Color32::from_rgb(25, 35, 50);
pub const CARD_ACTIVE: Color32 = Color32::from_rgb(18, 42, 45);
pub const BORDER: Color32 = Color32::from_rgb(42, 54, 72);
pub const ACCENT: Color32 = Color32::from_rgb(77, 208, 200);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(30, 104, 105);
pub const TEXT: Color32 = Color32::from_rgb(226, 234, 244);
pub const MUTED: Color32 = Color32::from_rgb(135, 150, 170);
pub const FAINT: Color32 = Color32::from_rgb(88, 102, 122);
pub const LFE_COLOR: Color32 = Color32::from_rgb(255, 171, 64);
pub const SURROUND_COLOR: Color32 = Color32::from_rgb(160, 140, 255);
pub const BACK_COLOR: Color32 = Color32::from_rgb(105, 170, 255);
pub const VOCALS_COLOR: Color32 = Color32::from_rgb(255, 196, 87);
pub const DRUMS_COLOR: Color32 = Color32::from_rgb(88, 170, 255);
pub const BASS_COLOR: Color32 = Color32::from_rgb(190, 120, 255);
pub const MELODIC_COLOR: Color32 = Color32::from_rgb(92, 214, 140);
pub const WARNING: Color32 = Color32::from_rgb(255, 184, 76);
pub const ERROR: Color32 = Color32::from_rgb(255, 104, 112);
pub const SUCCESS: Color32 = Color32::from_rgb(92, 214, 140);

pub fn apply(ctx: &Context) {
    ctx.set_theme(Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(8, 11, 16);
    visuals.faint_bg_color = Color32::from_rgb(24, 32, 44);
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 34, 47);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(23, 30, 42);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(34, 45, 61);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(30, 40, 55);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    ctx.set_visuals(visuals);

    ctx.style_mut_of(Theme::Dark, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(9.0, 5.0);
        style.spacing.slider_width = 120.0;
        style.spacing.interact_size.y = 22.0;
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(10.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(11.0, FontFamily::Monospace),
        );
    });
}

pub fn panel_frame() -> Frame {
    Frame::new()
        .fill(PANEL)
        .inner_margin(Margin::symmetric(12, 10))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn card_frame(active: bool) -> Frame {
    Frame::new()
        .fill(if active { CARD_ACTIVE } else { CARD })
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
        .stroke(Stroke::new(1.0, if active { ACCENT_DIM } else { BORDER }))
}

pub fn channel_color(channel: Channel) -> Color32 {
    match channel {
        Channel::Lfe => LFE_COLOR,
        Channel::Sl | Channel::Sr => SURROUND_COLOR,
        Channel::Bl | Channel::Br => BACK_COLOR,
        _ => ACCENT,
    }
}

pub fn kind_color(kind: &StemKind) -> Color32 {
    match kind {
        StemKind::LeadVocals | StemKind::Vocals | StemKind::BackingVocals => VOCALS_COLOR,
        StemKind::Drums => DRUMS_COLOR,
        StemKind::Bass => BASS_COLOR,
        StemKind::Melodic | StemKind::Other => MELODIC_COLOR,
        StemKind::Ambient => SURROUND_COLOR,
        StemKind::Instrumental => MUTED,
        StemKind::Unknown => FAINT,
    }
}

pub fn db_norm(db: f32) -> f32 {
    if !db.is_finite() {
        return 0.0;
    }
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

pub fn meter_color(db: f32) -> Color32 {
    if !db.is_finite() {
        FAINT
    } else if db > -0.3 {
        ERROR
    } else if db > -6.0 {
        WARNING
    } else if db > -18.0 {
        SUCCESS
    } else {
        ACCENT_DIM
    }
}

pub fn format_db(db: f32) -> String {
    if !db.is_finite() {
        "−∞".to_string()
    } else {
        format!("{db:+.1}")
    }
}

pub fn short_path(name: &str, max_chars: usize) -> String {
    let count = name.chars().count();
    if count <= max_chars {
        name.to_string()
    } else {
        let keep = max_chars.saturating_sub(1);
        let mut output: String = name.chars().take(keep).collect();
        output.push('…');
        output
    }
}
