#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use surround_packer::app::{show_error_dialog, SurroundPackerApp};
use surround_packer::icon;

fn main() {
    install_panic_hook();

    let mut startup_paths = Vec::new();
    let mut startup_preset = None;
    let mut startup_layout = None;
    let mut startup_tab = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        let value = argument.to_string_lossy();
        if value == "--tab" {
            if let Some(name) = args.next() {
                startup_tab = match name.to_string_lossy().as_ref() {
                    "batch" | "dvd" | "audio-dvd" => Some(surround_packer::app::AppTab::AudioDvd),
                    "stem" | "packer" => Some(surround_packer::app::AppTab::StemPacker),
                    _ => None,
                };
            }
        } else if value == "--layout" {
            if let Some(name) = args.next() {
                startup_layout = match name.to_string_lossy().as_ref() {
                    "7.1" | "71" => Some(surround_packer::model::ChannelLayout::Surround71),
                    "5.1" | "51" => Some(surround_packer::model::ChannelLayout::Surround51),
                    _ => None,
                };
            }
        } else if value == "--preset" {
            if let Some(name) = args.next() {
                startup_preset = match name.to_string_lossy().as_ref() {
                    "quad" | "classic-quad" => Some(surround_packer::routing::Preset::ClassicQuad),
                    "vocals-drums" | "front" => {
                        Some(surround_packer::routing::Preset::VocalsDrumsFront)
                    }
                    "vocal-split" | "5-stem" | "lead-vocals" => {
                        Some(surround_packer::routing::Preset::VocalSplit)
                    }
                    _ => None,
                };
            }
        } else if !value.starts_with('-') {
            let path = std::path::PathBuf::from(argument);
            if surround_packer::audio::is_supported_path(&path) {
                startup_paths.push(path);
            }
        }
    }

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("SurroundPacker")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1280.0, 820.0])
            .with_icon(icon::load_icon()),
        ..Default::default()
    };

    let result = eframe::run_native(
        "SurroundPacker",
        native_options,
        Box::new(move |cc| {
            let mut app = if startup_tab == Some(surround_packer::app::AppTab::AudioDvd) {
                SurroundPackerApp::new(cc)
            } else {
                SurroundPackerApp::new_with_paths(cc, &startup_paths)
            };
            if let Some(layout) = startup_layout {
                app.set_layout(layout);
            }
            if let Some(tab) = startup_tab {
                app.tab = tab;
                if tab == surround_packer::app::AppTab::AudioDvd {
                    app.batch.add_paths(&startup_paths);
                }
            }
            if let Some(preset) = startup_preset {
                app.queue_preset(preset);
            }
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    );

    if let Err(error) = result {
        let message = format!(
            "SurroundPacker could not start the graphical interface.\n\n{error}\n\n\
             On Linux, check that an X11/OpenGL session is available. On Windows, verify the executable was built with the included static-CRT configuration."
        );
        eprintln!("{message}");
        show_error_dialog("SurroundPacker — Startup Error", &message);
        std::process::exit(1);
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        default_hook(panic_info);
        let message = format!("SurroundPacker encountered an unexpected error:\n\n{panic_info}");
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("SurroundPacker — Unexpected Error")
            .set_description(&message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }));
}
