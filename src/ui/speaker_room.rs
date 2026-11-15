use std::path::PathBuf;

use egui::{
    Align, Button, Color32, ComboBox, Layout, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2,
};

use crate::dsp::{
    PeakReport, GATE_RELEASE_MAX_MS, GATE_RELEASE_MIN_MS, GATE_THRESHOLD_MAX_DB,
    GATE_THRESHOLD_MIN_DB,
};
use crate::model::{ChannelLayout, MAX_CHANNELS};
use crate::routing::{Channel, ChannelMode, Slot};
use crate::ui::theme;

pub struct RoomResponse {
    pub changed: bool,
    pub editor: Option<Channel>,
    pub dropped: Option<(Channel, PathBuf)>,
}

impl RoomResponse {
    fn new() -> Self {
        Self {
            changed: false,
            editor: None,
            dropped: None,
        }
    }
}

pub fn show(
    ui: &mut Ui,
    layout: ChannelLayout,
    slots: &mut [Slot; MAX_CHANNELS],
    peaks: Option<&PeakReport>,
    slot_rects: &mut [egui::Rect; MAX_CHANNELS],
) -> RoomResponse {
    let mut response = RoomResponse::new();
    let width = ui.available_width();
    let height = (ui.available_height() * if layout.is_71() { 0.62 } else { 0.70 })
        .clamp(if layout.is_71() { 480.0 } else { 430.0 }, 680.0);
    let (room, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(room, 18, Color32::from_rgb(13, 17, 25));
    painter.rect_stroke(
        room,
        18,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );

    // Room floor and perspective grid.
    let floor = room.shrink2(Vec2::new(18.0, 22.0));
    painter.rect_filled(floor, 14, Color32::from_rgb(16, 21, 31));
    for step in 1..5 {
        let y = floor.top() + floor.height() * step as f32 / 5.0;
        painter.line_segment(
            [
                egui::pos2(floor.left() + 12.0, y),
                egui::pos2(floor.right() - 12.0, y),
            ],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(90, 110, 140, 22)),
        );
    }

    let (card_width, card_height) = if layout.is_71() {
        (
            (room.width() * 0.215).clamp(126.0, 168.0),
            (room.height() * 0.235).clamp(118.0, 146.0),
        )
    } else {
        (
            (room.width() * 0.255).clamp(148.0, 205.0),
            (room.height() * 0.265).clamp(128.0, 158.0),
        )
    };
    let positions: &[(Channel, f32, f32)] = if layout.is_71() {
        &[
            (Channel::Fl, 0.16, 0.20),
            (Channel::Fr, 0.84, 0.20),
            (Channel::Fc, 0.50, 0.09),
            (Channel::Lfe, 0.50, 0.82),
            (Channel::Sl, 0.07, 0.50),
            (Channel::Sr, 0.93, 0.50),
            (Channel::Bl, 0.18, 0.76),
            (Channel::Br, 0.82, 0.76),
        ]
    } else {
        &[
            (Channel::Fl, 0.16, 0.22),
            (Channel::Fr, 0.84, 0.22),
            (Channel::Fc, 0.50, 0.13),
            (Channel::Lfe, 0.50, 0.84),
            (Channel::Sl, 0.12, 0.63),
            (Channel::Sr, 0.88, 0.63),
        ]
    };

    let mut card_rects = [egui::Rect::NOTHING; MAX_CHANNELS];
    for (channel, fx, fy) in positions {
        let center = room.min + Vec2::new(room.width() * fx, room.height() * fy);
        let half = Vec2::new(card_width * 0.5, card_height * 0.5);
        let min_x = (center.x - half.x).clamp(room.left() + 7.0, room.right() - card_width - 7.0);
        let min_y = (center.y - half.y).clamp(room.top() + 7.0, room.bottom() - card_height - 7.0);
        card_rects[channel.index()] =
            egui::Rect::from_min_size(egui::pos2(min_x, min_y), Vec2::new(card_width, card_height));
    }

    let pointer = ui.ctx().input(|input| input.pointer.hover_pos());
    let os_drag = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
    let internal_drag = egui::DragAndDrop::has_payload_of_type::<PathBuf>(ui.ctx());
    let drop_active = os_drag || internal_drag;
    let hovered_channel = if drop_active {
        pointer.and_then(|position| {
            layout
                .channels()
                .iter()
                .find(|channel| card_rects[channel.index()].contains(position))
                .copied()
        })
    } else {
        None
    };

    // Listening position and speaker connections.
    let listener = room.center() + Vec2::new(0.0, -6.0);
    for channel in layout.channels() {
        let index = channel.index();
        let color = theme::channel_color(*channel);
        let active = !slots[index].routes.is_empty() && !slots[index].mute;
        let alpha = if active { 55 } else { 22 };
        painter.line_segment(
            [listener, card_rects[index].center()],
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
            ),
        );
    }
    painter.circle_filled(
        listener,
        30.0,
        Color32::from_rgba_unmultiplied(77, 208, 200, 16),
    );
    painter.circle_stroke(
        listener,
        30.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(77, 208, 200, 60)),
    );
    painter.circle_filled(listener, 8.0, Color32::from_rgb(28, 58, 64));
    painter.text(
        listener + Vec2::new(0.0, 48.0),
        egui::Align2::CENTER_CENTER,
        "LISTENING POSITION",
        egui::FontId::proportional(9.0),
        Color32::from_rgba_unmultiplied(160, 185, 200, 110),
    );

    painter.text(
        egui::pos2(room.left() + 20.0, room.top() + 17.0),
        egui::Align2::LEFT_CENTER,
        "FRONT STAGE",
        egui::FontId::proportional(9.0),
        theme::FAINT,
    );
    painter.text(
        egui::pos2(room.left() + 20.0, room.bottom() - 14.0),
        egui::Align2::LEFT_CENTER,
        "SURROUND FIELD",
        egui::FontId::proportional(9.0),
        theme::FAINT,
    );

    let mut room_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(room)
            .layout(Layout::top_down(Align::Min)),
    );
    room_ui.set_clip_rect(room.shrink(1.0));

    for channel in layout.channels() {
        let index = channel.index();
        let is_hovered = hovered_channel == Some(*channel);
        let (changed, editor) = draw_card(
            &mut room_ui,
            *channel,
            &mut slots[index],
            peaks,
            card_rects[index],
            is_hovered,
        );
        response.changed |= changed;
        if editor {
            response.editor = Some(*channel);
        }
        slot_rects[index] = card_rects[index];
    }

    if ui.ctx().input(|input| input.pointer.any_released()) {
        if let Some(payload) = egui::DragAndDrop::take_payload::<PathBuf>(ui.ctx()) {
            if let Some(channel) = hovered_channel {
                response.dropped = Some((channel, (*payload).clone()));
            }
        }
    }

    response
}

fn draw_card(
    ui: &mut Ui,
    channel: Channel,
    slot: &mut Slot,
    peaks: Option<&PeakReport>,
    rect: egui::Rect,
    hovered: bool,
) -> (bool, bool) {
    let mut changed = false;
    let mut editor = false;
    let color = theme::channel_color(channel);
    let active = !slot.routes.is_empty();

    let painter = ui.painter();
    let fill = if hovered {
        theme::CARD_HOVER
    } else if active {
        theme::CARD
    } else {
        Color32::from_rgb(17, 23, 34)
    };
    painter.rect_filled(rect, 12, fill);
    painter.rect_stroke(
        rect,
        12,
        Stroke::new(
            if hovered { 2.0 } else { 1.0 },
            if hovered {
                theme::ACCENT
            } else {
                theme::BORDER
            },
        ),
        StrokeKind::Inside,
    );
    if hovered {
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + Vec2::new(12.0, 4.0),
                Vec2::new(rect.width() - 24.0, 3.0),
            ),
            2,
            theme::ACCENT,
        );
    }

    let content = rect.shrink2(Vec2::new(10.0, 8.0));
    let _ = ui.scope_builder(
        UiBuilder::new()
            .max_rect(content)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
            ui.set_width(content.width());

            let gate_reduction = peaks
                .map(|report| report.gate_reduction_db[channel.index()])
                .unwrap_or(0.0);
            let gate_active = gate_reduction > 1.0;
            let gate_color = if gate_active {
                theme::WARNING
            } else if slot.gate_enabled {
                theme::ACCENT
            } else {
                theme::MUTED
            };

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(channel.short())
                        .strong()
                        .size(15.0)
                        .color(color),
                )
                .on_hover_text(channel.name());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(RichText::new("S").size(9.0))
                                .selected(slot.solo)
                                .min_size(Vec2::new(17.0, 16.0)),
                        )
                        .on_hover_text("Solo this speaker")
                        .clicked()
                    {
                        slot.solo = !slot.solo;
                        changed = true;
                    }
                    if ui
                        .add(
                            Button::new(RichText::new("M").size(9.0))
                                .selected(slot.mute)
                                .min_size(Vec2::new(17.0, 16.0)),
                        )
                        .on_hover_text("Mute this speaker")
                        .clicked()
                    {
                        slot.mute = !slot.mute;
                        changed = true;
                    }
                    if !slot.routes.is_empty()
                        && ui
                            .add(
                                Button::new(RichText::new("Edit").size(8.0))
                                    .min_size(Vec2::new(26.0, 16.0)),
                            )
                            .on_hover_text("Edit layers")
                            .clicked()
                    {
                        editor = true;
                    }
                    let gate = ui
                        .menu_button(RichText::new("GATE").size(7.5).color(gate_color), |ui| {
                            ui.set_min_width(190.0);
                            if ui.checkbox(&mut slot.gate_enabled, "Noise gate").changed() {
                                changed = true;
                            }
                            if slot.gate_enabled {
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut slot.gate_threshold_db,
                                            GATE_THRESHOLD_MIN_DB..=GATE_THRESHOLD_MAX_DB,
                                        )
                                        .text("Threshold")
                                        .suffix(" dB"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut slot.gate_release_ms,
                                            GATE_RELEASE_MIN_MS..=GATE_RELEASE_MAX_MS,
                                        )
                                        .text("Release")
                                        .suffix(" ms"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.label(
                                    RichText::new(if gate_active {
                                        format!("Attenuating {gate_reduction:.0} dB")
                                    } else {
                                        "Gate open / idle".to_string()
                                    })
                                    .size(8.5)
                                    .color(if gate_active {
                                        theme::WARNING
                                    } else {
                                        theme::FAINT
                                    }),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Enable to silence low-level stem bleed.")
                                        .size(8.5)
                                        .color(theme::MUTED),
                                );
                            }
                        })
                        .response
                        .on_hover_text(if gate_active {
                            format!("Noise gate attenuating {gate_reduction:.0} dB")
                        } else if slot.gate_enabled {
                            "Noise gate enabled — click to adjust".to_string()
                        } else {
                            "Enable the noise gate for this speaker".to_string()
                        });
                    let _ = gate;
                });
            });

            if slot.routes.is_empty() {
                ui.add_space(5.0);
                ui.label(
                    RichText::new("Drop stem here")
                        .size(11.0)
                        .color(if hovered { theme::ACCENT } else { theme::MUTED }),
                );
                ui.label(
                    RichText::new(channel.default_mode().label())
                        .size(8.5)
                        .color(theme::FAINT),
                );
            } else {
                let primary_name = slot.routes[0].display_name();
                ui.label(
                    RichText::new(theme::short_path(&primary_name, 22))
                        .size(10.5)
                        .color(theme::TEXT),
                )
                .on_hover_text(slot.routes[0].stem_path.display().to_string());

                if slot.routes.len() > 1 {
                    ui.label(
                        RichText::new(format!("+{} layered source(s)", slot.routes.len() - 1))
                            .size(8.5)
                            .color(theme::ACCENT),
                    );
                } else {
                    ui.add_space(2.0);
                }

                let mut mode = slot.routes[0].mode;
                ComboBox::from_id_salt(("channel-mode", channel.index()))
                    .width(content.width().min(112.0))
                    .selected_text(RichText::new(mode.label()).size(10.0))
                    .show_ui(ui, |ui| {
                        for option in ChannelMode::ALL {
                            ui.selectable_value(&mut mode, option, option.label())
                                .on_hover_text(option.description());
                        }
                    });
                if mode != slot.routes[0].mode {
                    slot.routes[0].mode = mode;
                    changed = true;
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("GAIN").size(8.0).color(theme::FAINT));
                    let slider_width = (ui.available_width() - 52.0).max(48.0);
                    if ui
                        .add_sized(
                            [slider_width, 16.0],
                            egui::Slider::new(&mut slot.gain_db, -12.0..=12.0).show_value(false),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label(
                        RichText::new(format!("{:+.1}", slot.gain_db))
                            .size(9.0)
                            .color(theme::TEXT),
                    );
                });
            }

            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                let peak = peaks.map(|report| report.post_db[channel.index()]);
                draw_meter(ui, peak);
            });
        },
    );

    (changed, editor)
}

fn draw_meter(ui: &mut Ui, peak: Option<f32>) {
    let height = 5.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, 2, Color32::from_rgb(9, 12, 17));
    if let Some(db) = peak {
        let normalized = theme::db_norm(db);
        let filled = egui::Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width() * normalized, rect.height()),
        );
        ui.painter().rect_filled(filled, 2, theme::meter_color(db));
    }
}

pub fn show_audition_controls(
    ui: &mut Ui,
    layout: ChannelLayout,
    playing: bool,
    rendering: bool,
    on_play: &mut bool,
    on_stop: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !rendering && !playing,
                egui::Button::new(RichText::new("Play audition").size(10.0)),
            )
            .clicked()
        {
            *on_play = true;
        }
        if ui
            .add_enabled(
                playing || rendering,
                egui::Button::new(RichText::new("Stop").size(10.0)),
            )
            .clicked()
        {
            *on_stop = true;
        }
        if rendering {
            ui.spinner();
            ui.label(
                RichText::new("rendering folddown…")
                    .size(8.5)
                    .color(theme::FAINT),
            );
        } else if playing {
            ui.label(
                RichText::new("playing stereo folddown")
                    .size(8.5)
                    .color(theme::SUCCESS),
            );
        }
        let note = if layout.is_71() {
            "Headphone folddown · BL/BR at 0.707"
        } else {
            "Headphone folddown · SL/SR at 0.707"
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(note).size(8.0).color(theme::FAINT));
        });
    });
}

pub fn show_master_meters(ui: &mut Ui, layout: ChannelLayout, peaks: Option<&PeakReport>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("MASTER PEAK METERS")
                .strong()
                .size(11.0)
                .color(theme::TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(report) = peaks {
                if report.limiter_active {
                    ui.label(
                        RichText::new("LIMITER ACTIVE")
                            .strong()
                            .size(9.0)
                            .color(theme::WARNING),
                    );
                }
                let max_post = report
                    .post_db
                    .iter()
                    .copied()
                    .filter(|value| value.is_finite())
                    .fold(f32::NEG_INFINITY, f32::max);
                let text = if max_post.is_finite() {
                    format!("HEADROOM {:+.1} dB", -max_post)
                } else {
                    "HEADROOM —".to_string()
                };
                ui.label(RichText::new(text).size(9.0).color(theme::MUTED));
            } else {
                ui.label(RichText::new("No peak data").size(9.0).color(theme::FAINT));
            }
        });
    });

    for channel in layout.channels() {
        let index = channel.index();
        let pre = peaks.map(|report| report.pre_db[index]);
        let post = peaks.map(|report| report.post_db[index]);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{:>3}", channel.short()))
                    .monospace()
                    .size(9.0)
                    .color(theme::channel_color(*channel)),
            );
            let bar_width = (ui.available_width() - 118.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 7.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 2, Color32::from_rgb(9, 12, 17));
            if let Some(post_db) = post {
                let normalized = theme::db_norm(post_db);
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        rect.min,
                        Vec2::new(rect.width() * normalized, rect.height()),
                    ),
                    2,
                    theme::meter_color(post_db),
                );
                if let Some(pre_db) = pre {
                    if pre_db.is_finite() && pre_db > post_db + 0.05 {
                        let x = rect.left() + rect.width() * theme::db_norm(pre_db);
                        ui.painter().line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            Stroke::new(1.0, theme::WARNING),
                        );
                    }
                }
            }
            let value = match (pre, post) {
                (Some(pre), Some(post)) if pre.is_finite() || post.is_finite() => {
                    format!("{} / {}", theme::format_db(pre), theme::format_db(post))
                }
                _ => "— / —".to_string(),
            };
            ui.label(
                RichText::new(value)
                    .monospace()
                    .size(8.5)
                    .color(theme::MUTED),
            );
        });
    }
}
