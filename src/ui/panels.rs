use egui::{
    Align, Button, Color32, ComboBox, Frame, Layout, Margin, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Ui, Vec2,
};

use crate::app::{AppTab, SurroundPackerApp};
use crate::batch::BatchStatus;
use crate::export::ffmpeg::{DTS_BITRATE_HIGH, DTS_BITRATE_LOW};
use crate::export::Format;
use crate::model::ChannelLayout;
use crate::routing::{Channel, PanLaw, Preset};
use crate::ui::theme;

pub fn show_header(app: &mut SurroundPackerApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SurroundPacker")
                        .strong()
                        .size(23.0)
                        .color(theme::TEXT),
                );
                badge(ui, env!("CARGO_PKG_VERSION"), theme::ACCENT_DIM, theme::ACCENT);
                badge(
                    ui,
                    &format!("SMPTE {}", app.layout.short_label()),
                    Color32::from_rgb(34, 45, 62),
                    theme::MUTED,
                );
                ui.add_space(10.0);
                for tab in [AppTab::StemPacker, AppTab::AudioDvd] {
                    if ui.selectable_label(app.tab == tab, tab.label()).clicked() {
                        app.tab = tab;
                    }
                }
            });
            ui.label(
                RichText::new("UVR5 stem router · native 24-bit WAV / FLAC · AC3 / E-AC3 / DTS")
                    .size(9.0)
                    .color(theme::FAINT),
            );
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if app.tab == AppTab::StemPacker {
            let export_text = if app.exporting {
                "Exporting…"
            } else {
                "Export 5.1 Mix"
            };
            let button = Button::new(RichText::new(export_text).strong().size(12.0))
                .fill(if app.can_export() {
                    theme::ACCENT_DIM
                } else {
                    Color32::from_rgb(28, 38, 48)
                })
                .min_size(Vec2::new(138.0, 32.0));
            let response = ui.add_enabled(app.can_export(), button);
            if response.clicked() {
                app.start_export();
            }
            if !app.can_export() && !app.exporting {
                response.on_hover_text(
                    "Route stems, select a format, and make sure ffmpeg is available for AC3/E-AC3/DTS.",
                );
            }

            if ui
                .add(
                    Button::new(RichText::new("Refresh").size(9.5))
                        .min_size(Vec2::new(54.0, 24.0)),
                )
                .on_hover_text("Re-check ffmpeg on PATH")
                .clicked()
            {
                app.refresh_ffmpeg();
                app.refresh_dvd_tools();
            }
            ffmpeg_badge(ui, app);

            if app.exporting {
                ui.add(
                    egui::ProgressBar::new(app.export_progress)
                        .desired_width(110.0)
                        .show_percentage(),
                );
            }
            }
        });
    });
}

fn ffmpeg_badge(ui: &mut Ui, app: &SurroundPackerApp) {
    let (text, color, tooltip) = if app.ffmpeg.available {
        (
            "ffmpeg ready",
            theme::SUCCESS,
            format!(
                "{}\nAC3, E-AC3, and DTS encoding are available.",
                app.ffmpeg.version
            ),
        )
    } else if app.ffmpeg.version.starts_with("checking") {
        (
            "ffmpeg checking…",
            theme::MUTED,
            "Looking for ffmpeg on PATH.".to_string(),
        )
    } else {
        (
            "ffmpeg missing",
            theme::WARNING,
            "AC3, E-AC3, and DTS export require ffmpeg on PATH.\n24-bit WAV and FLAC work natively without it."
                .to_string(),
        )
    };
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            24,
        ))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.painter()
                    .circle_filled(ui.cursor().min + Vec2::new(4.0, 10.0), 3.5, color);
                ui.add_space(10.0);
                ui.label(RichText::new(text).size(9.5).color(color));
            });
        })
        .response
        .on_hover_text(tooltip);
}

fn badge(ui: &mut Ui, text: &str, fill: Color32, color: Color32) {
    Frame::new()
        .fill(fill)
        .corner_radius(6)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(8.5).color(color));
        });
}

pub fn show_left(app: &mut SurroundPackerApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("STEM LIBRARY")
                .strong()
                .size(11.0)
                .color(theme::TEXT),
        );
        badge(
            ui,
            &app.stems.len().to_string(),
            Color32::from_rgb(26, 34, 47),
            theme::MUTED,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(
                    !app.stems.is_empty(),
                    Button::new(RichText::new("Clear").size(9.5)),
                )
                .clicked()
            {
                app.clear_session();
            }
        });
    });

    if ui
        .add_sized(
            [ui.available_width(), 30.0],
            Button::new(RichText::new("+  Load audio stems").size(11.0)),
        )
        .clicked()
    {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio stems", &["wav", "flac", "mp3", "ogg", "oga"])
            .set_directory(&app.export_settings.output_dir)
            .pick_files()
        {
            for path in paths {
                app.enqueue_load(path, None);
            }
        }
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new("CHANNEL LAYOUT")
            .strong()
            .size(10.0)
            .color(theme::MUTED),
    );
    ui.horizontal(|ui| {
        for layout in [ChannelLayout::Surround51, ChannelLayout::Surround71] {
            if ui
                .selectable_label(app.layout == layout, layout.label())
                .clicked()
            {
                app.set_layout(layout);
            }
        }
    });

    ui.add_space(4.0);
    ui.label(
        RichText::new("QUICK PRESETS")
            .strong()
            .size(10.0)
            .color(theme::MUTED),
    );
    for preset in [
        Preset::VocalSplit,
        Preset::VocalsDrumsFront,
        Preset::ClassicQuad,
    ] {
        let label = match preset {
            Preset::VocalSplit => "5-Stem Vocal Split\nLead / BGV / Drums / Bass / Other",
            Preset::VocalsDrumsFront => "Vocals + Drums Front\nInstruments Surround",
            Preset::ClassicQuad => "Classic Quad\n+ Center Vocal",
        };
        if ui
            .add_sized(
                [ui.available_width(), 40.0],
                Button::new(RichText::new(label).size(10.0)),
            )
            .on_hover_text(
                "Auto-assigns stems by UVR5 filename keywords. Existing speaker layers are replaced.",
            )
            .clicked()
        {
            app.apply_preset(preset);
        }
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new("ROUTING TEMPLATE")
            .strong()
            .size(10.0)
            .color(theme::MUTED),
    );
    ui.add(
        TextEdit::singleline(&mut app.template_name)
            .desired_width(ui.available_width())
            .hint_text("Template name"),
    );
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [ui.available_width() / 2.0 - 3.0, 26.0],
                Button::new("Save .json"),
            )
            .clicked()
        {
            app.save_template();
        }
        if ui
            .add_sized(
                [ui.available_width() - 3.0, 26.0],
                Button::new("Load .json"),
            )
            .clicked()
        {
            app.load_template();
        }
    });

    ui.add_space(6.0);
    ui.separator();

    if app.stems.is_empty() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No stems loaded")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.label(
                RichText::new("Drop WAV, FLAC, MP3, or OGG files\nonto the speaker room.")
                    .size(9.5)
                    .color(theme::FAINT),
            );
        });
        return;
    }

    let mut remove = None;
    let mut route = None;
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(false)
        .show(ui, |ui| {
            for stem in &app.stems {
                let kind_color = theme::kind_color(&stem.kind());
                let id = stem.id;
                let path = stem.path.clone();
                let drag_response = ui.dnd_drag_source(
                    egui::Id::unique(("stem-drag", stem.id)),
                    stem.path.clone(),
                    |ui| {
                        Frame::new()
                            .fill(theme::CARD)
                            .corner_radius(8)
                            .inner_margin(8)
                            .stroke(Stroke::new(1.0, theme::BORDER))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let (dot, _) = ui
                                        .allocate_exact_size(Vec2::new(7.0, 16.0), Sense::hover());
                                    ui.painter().circle_filled(dot.center(), 3.2, kind_color);
                                    ui.label(
                                        RichText::new(theme::short_path(&stem.display_name, 24))
                                            .size(10.5)
                                            .color(theme::TEXT),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .add(
                                                Button::new(RichText::new("×").size(11.0))
                                                    .min_size(Vec2::new(16.0, 16.0)),
                                            )
                                            .on_hover_text("Remove from library")
                                            .clicked()
                                        {
                                            remove = Some(id);
                                        }
                                    });
                                });
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {} ch · {}",
                                        stem.kind().label(),
                                        stem.channel_count(),
                                        duration_text(stem.duration_secs())
                                    ))
                                    .size(8.5)
                                    .color(theme::FAINT),
                                );
                                draw_waveform(ui, stem, kind_color);
                                ui.horizontal(|ui| {
                                    ui.menu_button(RichText::new("Route to…").size(9.0), |ui| {
                                        for channel in Channel::ALL {
                                            if ui
                                                .button(format!(
                                                    "{}   {}",
                                                    channel.short(),
                                                    channel.name()
                                                ))
                                                .clicked()
                                            {
                                                route = Some((id, path.clone(), channel));
                                                ui.close();
                                            }
                                        }
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.1} kHz / 48 kHz out",
                                            stem.source_sample_rate as f32 / 1000.0
                                        ))
                                        .size(8.0)
                                        .color(theme::FAINT),
                                    );
                                });
                            });
                    },
                );
                if !drag_response.response.hovered() {
                    // The response is only used to keep drag state alive.
                }
                ui.add_space(2.0);
            }
        });

    if let Some(id) = remove {
        app.remove_stem(id);
    }
    if let Some((id, path, channel)) = route {
        let _ = path;
        app.attach_stem_to_channel(id, channel);
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new("Drag a stem card onto any speaker.")
            .size(8.5)
            .color(theme::FAINT),
    );
}

fn draw_waveform(ui: &mut Ui, stem: &crate::audio::Stem, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 4, Color32::from_rgb(10, 14, 21));
    if stem.waveform.is_empty() {
        return;
    }
    let center = rect.center().y;
    let count = stem.waveform.len().max(1);
    for (index, peak) in stem.waveform.iter().enumerate() {
        let x = rect.left() + rect.width() * (index as f32 + 0.5) / count as f32;
        let half = (peak.clamp(0.0, 1.0) * (rect.height() * 0.42)).max(0.5);
        ui.painter().line_segment(
            [egui::pos2(x, center - half), egui::pos2(x, center + half)],
            Stroke::new(1.0, color.gamma_multiply(0.75)),
        );
    }
}

fn duration_text(seconds: f64) -> String {
    let total = seconds.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

pub fn show_right(app: &mut SurroundPackerApp, ui: &mut Ui) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                RichText::new("DSP POLISH")
                    .strong()
                    .size(11.0)
                    .color(theme::TEXT),
            );
            let dsp_before = app.dsp.clone();
            ui.checkbox(
                &mut app.dsp.lfe_lowpass_enabled,
                RichText::new("LFE low-pass crossover").size(10.5),
            );
            ui.add_enabled(
                app.dsp.lfe_lowpass_enabled,
                egui::Slider::new(&mut app.dsp.lfe_cutoff_hz, 80.0..=120.0)
                    .suffix(" Hz")
                    .fixed_decimals(0)
                    .text("Butterworth"),
            );
            ui.checkbox(
                &mut app.dsp.surround_delay_enabled,
                RichText::new("Surround Haas / ambient delay").size(10.5),
            );
            ui.add_enabled(
                app.dsp.surround_delay_enabled,
                egui::Slider::new(&mut app.dsp.surround_delay_ms, 0.0..=35.0)
                    .suffix(" ms")
                    .fixed_decimals(1)
                    .text("SL / SR"),
            );
            if app.layout.is_71() {
                ui.add_enabled(
                    app.dsp.surround_delay_enabled,
                    egui::Slider::new(&mut app.dsp.back_delay_ms, 0.0..=35.0)
                        .suffix(" ms")
                        .fixed_decimals(1)
                        .text("BL / BR"),
                );
            }
            ui.checkbox(
                &mut app.dsp.limiter_enabled,
                RichText::new("True-peak safety limiter · −0.3 dBFS").size(10.5),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Mono sum pan law").size(9.5).color(theme::MUTED));
                ComboBox::from_id_salt("pan-law")
                    .width(72.0)
                    .selected_text(app.pan_law.label())
                    .show_ui(ui, |ui| {
                        for law in [PanLaw::Minus3, PanLaw::Minus6] {
                            ui.selectable_value(&mut app.pan_law, law, law.label());
                        }
                    });
            });
            if app.dsp != dsp_before {
                app.mark_dirty();
            }
            ui.horizontal(|ui| {
                if ui
                    .add_sized([116.0, 24.0], Button::new(RichText::new("Analyze peaks").size(9.5)))
                    .clicked()
                {
                    app.request_analysis_now();
                }
                if app.analysis_rx.is_some() {
                    ui.spinner();
                    ui.label(RichText::new("analyzing").size(8.5).color(theme::FAINT));
                } else if app.peaks.is_some() {
                    ui.label(RichText::new("meters fresh").size(8.5).color(theme::SUCCESS));
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.label(
                RichText::new("EXPORT")
                    .strong()
                    .size(11.0)
                    .color(theme::TEXT),
            );

            if !app.ffmpeg.available {
                Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(255, 184, 76, 18))
                    .corner_radius(7)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "ffmpeg not found — AC3, E-AC3, and DTS are unavailable. 24-bit WAV and FLAC still export natively.",
                            )
                            .size(9.0)
                            .color(theme::WARNING),
                        );
                    });
            }

            ui.horizontal(|ui| {
                ui.label(RichText::new("Folder").size(9.0).color(theme::MUTED));
                ui.label(
                    RichText::new(theme::short_path(
                        &app.export_settings.output_dir.to_string_lossy(),
                        34,
                    ))
                    .size(8.5)
                    .color(theme::TEXT),
                )
                .on_hover_text(app.export_settings.output_dir.display().to_string());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("Change").clicked() {
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_directory(&app.export_settings.output_dir)
                            .pick_folder()
                        {
                            app.export_settings.output_dir = folder;
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Name").size(9.0).color(theme::MUTED));
                ui.add(
                    TextEdit::singleline(&mut app.export_settings.base_name)
                        .desired_width(ui.available_width()),
                );
            });

            ui.add_space(2.0);
            let mut format_changes = Vec::new();
            for format in Format::ALL {
                let mut selected = app.export_settings.formats.contains(&format);
                let response = ui.checkbox(&mut selected, RichText::new(format.label()).size(9.8));
                let response = response.on_hover_text(format.tooltip());
                if response.changed() {
                    format_changes.push((format, selected));
                }
            }
            for (format, selected) in format_changes {
                app.set_format_selected(format, selected);
            }

            if app.export_settings.formats.contains(&Format::Dts) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("DTS bitrate").size(9.0).color(theme::MUTED));
                    ui.radio_value(
                        &mut app.export_settings.dts_bitrate,
                        DTS_BITRATE_HIGH,
                        "1509 kb/s",
                    );
                    ui.radio_value(
                        &mut app.export_settings.dts_bitrate,
                        DTS_BITRATE_LOW,
                        "768 kb/s",
                    );
                });
            }
            if app
                .export_settings
                .formats
                .iter()
                .any(|format| format.uses_container())
            {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Container").size(9.0).color(theme::MUTED));
                    ui.radio_value(
                        &mut app.export_settings.container,
                        crate::export::ContainerChoice::Mka,
                        "MKA",
                    );
                    ui.radio_value(
                        &mut app.export_settings.container,
                        crate::export::ContainerChoice::Raw,
                        "Raw",
                    );
                });
            }

            if app.layout.is_71()
                && app.export_settings.formats.contains(&Format::Eac3)
                && !app.ffmpeg.eac3_7_1
            {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(255, 184, 76, 18))
                    .corner_radius(7)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "This FFmpeg build cannot encode discrete 7.1 E-AC3. Enable the 5.1 compatibility fold or choose 7.1 WAV/FLAC.",
                            )
                            .size(9.0)
                            .color(theme::WARNING),
                        );
                        ui.checkbox(
                            &mut app.export_settings.allow_eac3_51_fallback,
                            RichText::new("Allow 5.1 E-AC3 compatibility fold").size(9.0),
                        );
                    });
            }

            egui::CollapsingHeader::new(RichText::new("Metadata tags").size(9.5))
                .default_open(false)
                .show(ui, |ui| {
                    metadata_row(ui, "Title", &mut app.export_settings.title);
                    metadata_row(ui, "Artist", &mut app.export_settings.artist);
                    metadata_row(ui, "Album", &mut app.export_settings.album);
                });

            ui.add_space(6.0);
            let export_label = if app.exporting {
                "Exporting…".to_string()
            } else {
                format!("Export {} format(s)", app.export_settings.formats.len())
            };
            let export_fill = if app.can_export() {
                theme::ACCENT_DIM
            } else {
                Color32::from_rgb(28, 38, 48)
            };
            let response = ui.add_enabled(
                app.can_export(),
                Button::new(RichText::new(export_label).strong().size(11.5))
                    .fill(export_fill)
                    .min_size(Vec2::new(ui.available_width(), 34.0)),
            );
            if response.clicked() {
                app.start_export();
            }

            if app.exporting || app.export_progress > 0.0 {
                ui.add(
                    egui::ProgressBar::new(app.export_progress)
                        .show_percentage()
                        .animate(app.exporting),
                );
                ui.label(RichText::new(&app.export_stage).size(8.5).color(theme::MUTED));
            }

            if let Some(report) = &app.last_report {
                ui.add_space(3.0);
                ui.label(
                    RichText::new(format!(
                        "Last export · {} file(s) · {:.1}s",
                        report.outputs.len(),
                        report.elapsed_secs
                    ))
                    .size(8.5)
                    .color(if report.errors.is_empty() {
                        theme::SUCCESS
                    } else {
                        theme::WARNING
                    }),
                );
                for path in report.outputs.iter().take(8) {
                    ui.label(
                        RichText::new(format!("• {}", path.file_name().unwrap_or_default().to_string_lossy()))
                            .size(8.0)
                            .color(theme::MUTED),
                    );
                }
                for error in report.errors.iter().take(3) {
                    ui.label(RichText::new(format!("! {error}")).size(8.0).color(theme::ERROR));
                }
            }

            ui.add_space(6.0);
            ui.separator();
            egui::CollapsingHeader::new(RichText::new("Activity log").size(9.5))
                .default_open(false)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .max_height(190.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &app.logs {
                                ui.label(
                                    RichText::new(line)
                                        .monospace()
                                        .size(8.0)
                                        .color(theme::MUTED),
                                );
                            }
                        });
                });
        });
}

fn metadata_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(8.5).color(theme::MUTED));
        ui.add(TextEdit::singleline(value).desired_width(ui.available_width()));
    });
}

pub fn show_status(app: &SurroundPackerApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if app.exporting || app.loading > 0 || app.analysis_rx.is_some() || app.batch.converting {
            ui.spinner();
        } else {
            let (dot, _) = ui.allocate_exact_size(Vec2::new(9.0, 9.0), Sense::hover());
            ui.painter()
                .circle_filled(dot.center(), 3.5, theme::SUCCESS);
        }
        ui.label(
            RichText::new(theme::short_path(&app.status, 92))
                .size(9.0)
                .color(theme::MUTED),
        )
        .on_hover_text(&app.status);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("SMPTE {}", app.layout.short_label()))
                    .size(8.0)
                    .color(theme::FAINT),
            );
            ui.label(RichText::new("48 kHz").size(8.0).color(theme::FAINT));
            if app.tab == AppTab::AudioDvd {
                ui.label(
                    RichText::new(format!("{} DVD track(s)", app.batch.tracks.len()))
                        .size(8.0)
                        .color(theme::MUTED),
                );
            }
            if app.loading > 0 {
                ui.label(
                    RichText::new(format!("loading {}", app.loading))
                        .size(8.0)
                        .color(theme::ACCENT),
                );
            }
        });
    });
}

pub fn show_batch(app: &mut SurroundPackerApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("AUDIO-DVD BUILDER")
                .strong()
                .size(18.0)
                .color(theme::TEXT),
        );
        badge(ui, "DVD-Video", theme::ACCENT_DIM, theme::ACCENT);
        badge(
            ui,
            "48 kHz / 5.1",
            Color32::from_rgb(34, 45, 62),
            theme::MUTED,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let iso_label = app
                .dvd_tools
                .iso
                .program
                .clone()
                .unwrap_or_else(|| "ISO".to_string());
            tool_badge(
                ui,
                &format!("{iso_label} ISO"),
                app.dvd_tools.iso.available(),
            );
            tool_badge(ui, "mplex", app.dvd_tools.mplex.available());
            tool_badge(ui, "dvdauthor", app.dvd_tools.dvdauthor.available());
            tool_badge(ui, "ffmpeg", app.dvd_tools.ffmpeg.available);
        });
    });
    ui.label(
        RichText::new(
            "Build a DVD-Video disc with static title cards and 48 kHz 5.1 AC-3 448 plus optional DTS audio.",
        )
        .size(10.0)
        .color(theme::MUTED),
    );
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(88, 170, 255, 14))
        .corner_radius(7)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Tracks are folded to canonical 5.1, muxed as DVD-Video with mplex (valid VOBUs), authored into VIDEO_TS, and packaged as a VIDEO_TS-only Audio-DVD.iso.",
                )
                .size(9.0)
                .color(theme::MUTED),
            );
        });

    ui.add_space(4.0);
    Frame::new()
        .fill(theme::CARD)
        .corner_radius(10)
        .inner_margin(10)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Drop 5.1 / 7.1 WAV or FLAC files here")
                        .size(11.0)
                        .color(theme::TEXT),
                );
                ui.label(
                    RichText::new("or use Add files below")
                        .size(9.0)
                        .color(theme::FAINT),
                );
            });
        });

    ui.horizontal(|ui| {
        if ui
            .add_sized([160.0, 28.0], Button::new("+  Add surround files"))
            .clicked()
        {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Surround WAV/FLAC", &["wav", "flac"])
                .set_directory(&app.batch.output_dir)
                .pick_files()
            {
                let added = app.batch.add_paths(&paths);
                app.set_status(format!("Added {added} track(s) to the Audio-DVD queue."));
            }
        }
        if ui
            .add_enabled(
                !app.batch.tracks.is_empty(),
                Button::new(RichText::new("Clear queue").size(10.0)),
            )
            .clicked()
        {
            app.batch.tracks.clear();
        }
        ui.separator();
        ui.label(RichText::new("Output").size(9.0).color(theme::MUTED));
        ui.label(
            RichText::new(theme::short_path(
                &app.batch.output_dir.to_string_lossy(),
                44,
            ))
            .size(9.0)
            .color(theme::TEXT),
        )
        .on_hover_text(app.batch.output_dir.display().to_string());
        if ui.small_button("Change").clicked() {
            if let Some(folder) = rfd::FileDialog::new()
                .set_directory(&app.batch.output_dir)
                .pick_folder()
            {
                app.batch.output_dir = folder;
            }
        }
    });

    ui.add_space(4.0);
    ui.label(
        RichText::new("TRACKLIST")
            .strong()
            .size(10.5)
            .color(theme::MUTED),
    );
    if app.batch.tracks.is_empty() {
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Queue is empty. Add 5.1 or 7.1 WAV/FLAC mixes.")
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });
        return;
    }

    let mut move_up = None;
    let mut move_down = None;
    let mut remove = None;
    let scroll_height = (ui.available_height() - 8.0).max(120.0);
    ScrollArea::vertical()
        .max_height(scroll_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, track) in app.batch.tracks.iter().enumerate() {
                Frame::new()
                    .fill(theme::CARD)
                    .corner_radius(7)
                    .inner_margin(7)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:02}", index + 1))
                                    .monospace()
                                    .size(10.0)
                                    .color(theme::FAINT),
                            );
                            ui.label(
                                RichText::new(theme::short_path(&track.display_name, 38))
                                    .size(10.5)
                                    .color(theme::TEXT),
                            )
                            .on_hover_text(track.path.display().to_string());
                            badge(
                                ui,
                                track.layout_label(),
                                Color32::from_rgb(26, 40, 50),
                                if track.layout().is_some() {
                                    theme::ACCENT
                                } else {
                                    theme::ERROR
                                },
                            );
                            ui.label(
                                RichText::new(track.sample_rate_label())
                                    .size(8.5)
                                    .color(theme::MUTED),
                            );
                            ui.label(
                                RichText::new(track.duration_label())
                                    .size(8.5)
                                    .color(theme::FAINT),
                            );
                            match &track.status {
                                BatchStatus::Ready => {
                                    ui.label(
                                        RichText::new("ready").size(8.5).color(theme::SUCCESS),
                                    );
                                }
                                BatchStatus::Converting => {
                                    ui.spinner();
                                }
                                BatchStatus::Done(path) => {
                                    ui.label(
                                        RichText::new(format!(
                                            "✓ {}",
                                            path.file_name().unwrap_or_default().to_string_lossy()
                                        ))
                                        .size(8.5)
                                        .color(theme::SUCCESS),
                                    );
                                }
                                BatchStatus::Error(message) => {
                                    ui.label(
                                        RichText::new(theme::short_path(message, 38))
                                            .size(8.5)
                                            .color(theme::ERROR),
                                    )
                                    .on_hover_text(message);
                                }
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button("×").clicked() {
                                    remove = Some(index);
                                }
                                if ui.small_button("Dn").clicked() {
                                    move_down = Some(index);
                                }
                                if ui.small_button("Up").clicked() {
                                    move_up = Some(index);
                                }
                            });
                        });
                    });
            }
        });
    if let Some(index) = move_up {
        app.batch.move_up(index);
    }
    if let Some(index) = move_down {
        app.batch.move_down(index);
    }
    if let Some(index) = remove {
        if index < app.batch.tracks.len() {
            app.batch.tracks.remove(index);
        }
    }
}

fn tool_badge(ui: &mut Ui, label: &str, available: bool) {
    let color = if available {
        theme::SUCCESS
    } else {
        theme::WARNING
    };
    let text = format!("{label}: {}", if available { "OK" } else { "missing" });
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            22,
        ))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(8.0).color(color));
        });
}

pub fn show_batch_footer(app: &mut SurroundPackerApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Audio codec").size(9.0).color(theme::MUTED));
        egui::ComboBox::from_id_salt("dvd-audio-codec")
            .width(245.0)
            .selected_text(app.batch.codec.label())
            .show_ui(ui, |ui| {
                for codec in crate::export::dvd::DvdAudioCodec::ALL {
                    ui.selectable_value(&mut app.batch.codec, codec, codec.label());
                }
            });
        ui.separator();
        ui.label(
            RichText::new("Source channel order")
                .size(9.0)
                .color(theme::MUTED),
        );
        egui::ComboBox::from_id_salt("batch-source-order")
            .width(280.0)
            .selected_text(app.batch.source_order.label())
            .show_ui(ui, |ui| {
                for order in [
                    crate::batch::SourceChannelOrder::Smpte51,
                    crate::batch::SourceChannelOrder::FilmDts51,
                ] {
                    ui.selectable_value(&mut app.batch.source_order, order, order.label());
                }
            });
        ui.label(
            RichText::new("(6-channel sources)")
                .size(8.0)
                .color(theme::FAINT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mb = app.batch.estimated_size_mb();
            let over = app.batch.over_capacity();
            ui.label(
                RichText::new(format!(
                    "{mb:.0} MB / {:.0} MB DVD-R",
                    crate::export::dvd::DVD_R_CAPACITY_MB
                ))
                .monospace()
                .size(10.0)
                .color(if over { theme::ERROR } else { theme::MUTED }),
            );
        });
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Disc title").size(9.0).color(theme::MUTED));
        ui.add(TextEdit::singleline(&mut app.batch.album_title).desired_width(230.0));
        ui.label(
            RichText::new(".mpg titles are still written if authoring tools are missing.")
                .size(8.0)
                .color(theme::FAINT),
        );
        if !app.dvd_tools.mplex.available() {
            ui.label(
                RichText::new("mplex missing: .mpg only, no VIDEO_TS/ISO")
                    .size(8.5)
                    .color(theme::WARNING),
            );
        }
        if !app.dvd_tools.dvdauthor.available() {
            ui.label(
                RichText::new("dvdauthor missing: no VIDEO_TS/ISO")
                    .size(8.5)
                    .color(theme::WARNING),
            );
        }
        if !app.dvd_tools.iso.available() {
            ui.label(
                RichText::new("ISO tool missing: no .iso")
                    .size(8.5)
                    .color(theme::WARNING),
            );
        }
    });
    ui.horizontal(|ui| {
        let enabled = app.batch.can_build() && app.ffmpeg.available;
        let response = ui.add_enabled(
            enabled,
            Button::new(
                RichText::new(if app.batch.converting {
                    "Building Audio-DVD…"
                } else {
                    "Build Audio-DVD"
                })
                .strong()
                .size(11.5),
            )
            .fill(if enabled {
                theme::ACCENT_DIM
            } else {
                Color32::from_rgb(28, 38, 48)
            })
            .min_size(Vec2::new(220.0, 30.0)),
        );
        if response.clicked() {
            app.start_dvd_build();
        }
        if app.batch.converting {
            let cancel = ui.add(
                Button::new(RichText::new("Cancel").strong().size(10.5))
                    .fill(Color32::from_rgb(64, 32, 36))
                    .min_size(Vec2::new(72.0, 30.0)),
            );
            if cancel.clicked() {
                app.batch.cancel_build();
            }
        }
        let bar_width = (ui.available_width() - 170.0).max(160.0);
        ui.add(
            egui::ProgressBar::new(app.batch.progress)
                .desired_width(bar_width)
                .show_percentage()
                .animate(app.batch.converting),
        );
        ui.label(
            RichText::new(theme::short_path(&app.batch.stage, 34))
                .size(8.5)
                .color(theme::MUTED),
        )
        .on_hover_text(&app.batch.stage);
    });
    if !app.batch.last_errors.is_empty() {
        ui.label(
            RichText::new(theme::short_path(&app.batch.last_errors.join(" | "), 160))
                .size(8.0)
                .color(theme::ERROR),
        );
    } else if !app.batch.last_outputs.is_empty() {
        ui.label(
            RichText::new(format!(
                "{} MPG title(s){}",
                app.batch.last_outputs.len(),
                app.batch
                    .last_iso
                    .as_ref()
                    .map(|path| format!(
                        " · ISO: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ))
                    .unwrap_or_default()
            ))
            .size(8.0)
            .color(theme::SUCCESS),
        );
    }
}
