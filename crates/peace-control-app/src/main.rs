//! PeaceControl: a modern, open source configuration front end for
//! Equalizer APO, written entirely in Rust.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

mod app;
mod brand;
mod dialogs;
mod engine;
mod graph;
mod platform;
mod shell;
mod theme;
mod tray;
mod widgets;

use peace_control_core::logging;
use peace_control_core::paths::AppPaths;
use peace_control_core::settings::Settings;

/// Short product line used in the interface.
pub const SHORT_TAGLINE: &str = "Equalizer APO, under control";

fn main() -> eframe::Result<()> {
    let paths = AppPaths::discover();
    if let Err(error) = paths.ensure() {
        eprintln!("PeaceControl: could not create its data folder: {error}");
    }

    let settings = Settings::load(&paths.settings_file());
    if let Err(error) = logging::init(&paths.logs_dir(), &settings.log_level) {
        eprintln!("PeaceControl: logging is unavailable: {error}");
    }
    log::info!(
        "PeaceControl {} starting on {} ({})",
        peace_control_core::VERSION,
        std::env::consts::OS,
        if paths.is_portable() {
            "portable"
        } else {
            "installed"
        }
    );

    let (rgba, width, height) = brand::icon_rgba(64);
    let icon = egui::IconData {
        rgba,
        width,
        height,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("PeaceControl {}", peace_control_core::VERSION))
            .with_inner_size([settings.window_width, settings.window_height])
            .with_min_inner_size([980.0, 640.0])
            .with_app_id("peacecontrol")
            .with_icon(std::sync::Arc::new(icon)),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "PeaceControl",
        options,
        Box::new(move |cc| {
            let start_hidden = std::env::args().any(|arg| arg == "--minimized");
            Ok(Box::new(app::PeaceApp::new(
                cc,
                paths,
                settings,
                start_hidden,
            )))
        }),
    )
}
