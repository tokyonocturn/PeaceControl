//! Secondary windows: effects, settings, commands, import, backups, help.

use crate::app::{PeaceApp, StatusKind};
use crate::platform;
use crate::theme;
use crate::widgets;
use eframe::egui::{self, RichText, Stroke};
use peace_control_core::model::{Effects, MAX_BANDS};
use peace_control_core::settings::Theme;
use peace_control_core::VERSION;

/// Shows every dialog that is currently open.
pub fn show_all(app: &mut PeaceApp, ctx: &egui::Context) {
    effects(app, ctx);
    settings(app, ctx);
    commands(app, ctx);
    import(app, ctx);
    backups(app, ctx);
    help(app, ctx);
    recovery(app, ctx);
    confirm_close(app, ctx);
    applications(app, ctx);
}

fn window_frame(palette: &theme::Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(palette.panel)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .inner_margin(egui::Margin::same(14.0))
}

fn header(ui: &mut egui::Ui, title: &str, subtitle: &str, palette: &theme::Palette) {
    ui.label(RichText::new(title).size(17.0).strong().color(palette.text));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).size(10.5).color(palette.muted));
    }
    ui.add_space(8.0);
}

fn param_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
    palette: &theme::Palette,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_min_width(150.0);
        ui.label(RichText::new(label).size(11.5).color(palette.muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::DragValue::new(value)
                        .speed(0.1)
                        .range(range.clone())
                        .suffix(suffix)
                        .fixed_decimals(1),
                )
                .changed()
            {
                changed = true;
            }
            if ui
                .add(egui::Slider::new(value, range).show_value(false))
                .changed()
            {
                changed = true;
            }
        });
    });
    changed
}

fn effects(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.effects {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("Effects")
        .id(egui::Id::new("effects-window"))
        .open(&mut open)
        .default_width(520.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Effects",
                "Everything here is written as standard Equalizer APO commands.",
                &palette,
            );

            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    let mut changed = false;
                    {
                        let effects = &mut app.preset.effects;
                        changed |= effects_block(ui, effects, &palette);
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    widgets::section_title(ui, "Surround stage", &palette);
                    {
                        let surround = &mut app.preset.surround;
                        changed |= widgets::toggle_row(
                            ui,
                            "Surround widening",
                            "Spreads stereo to the side and rear channels.",
                            &mut surround.enabled,
                            &palette,
                        );
                        if surround.enabled {
                            changed |= param_row(
                                ui,
                                "Front width",
                                &mut surround.front_width,
                                0.0..=1.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Side width",
                                &mut surround.side_width,
                                0.0..=1.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Rear width",
                                &mut surround.rear_width,
                                0.0..=1.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Rear level",
                                &mut surround.rear_level,
                                0.0..=2.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Subwoofer level",
                                &mut surround.subwoofer_level,
                                0.0..=2.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Front level",
                                &mut surround.front_level,
                                0.0..=2.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Centre level",
                                &mut surround.center_level,
                                0.0..=2.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Side level",
                                &mut surround.side_level,
                                0.0..=2.0,
                                "",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Front delay",
                                &mut surround.front_delay_ms,
                                0.0..=30.0,
                                " ms",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Subwoofer delay",
                                &mut surround.subwoofer_delay_ms,
                                0.0..=30.0,
                                " ms",
                                &palette,
                            );
                            changed |= param_row(
                                ui,
                                "Bass crossover",
                                &mut surround.bass_frequency_hz,
                                40.0..=400.0,
                                " Hz",
                                &palette,
                            );
                        }
                    }

                    if changed {
                        app.dirty = true;
                        app.last_edit = std::time::Instant::now();
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "Reset all effects", &palette).clicked() {
                    app.push_history();
                    app.preset.effects = Effects::default();
                    app.preset.surround.enabled = false;
                    app.dirty = true;
                    app.status("Effects reset", StatusKind::Info);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, "Apply now", app.engine.ready(), &palette)
                        .clicked()
                    {
                        app.apply_now();
                    }
                });
            });
        });
    app.windows.effects = open;
}

fn effects_block(ui: &mut egui::Ui, effects: &mut Effects, palette: &theme::Palette) -> bool {
    let mut changed = false;

    widgets::section_title(ui, "Tone", palette);
    changed |= widgets::toggle_row(
        ui,
        "Bass shelf",
        "Adds or removes weight below the chosen frequency.",
        &mut effects.bass.enabled,
        palette,
    );
    if effects.bass.enabled {
        changed |= param_row(
            ui,
            "Bass gain",
            &mut effects.bass.gain_db,
            -15.0..=15.0,
            " dB",
            palette,
        );
        changed |= param_row(
            ui,
            "Bass frequency",
            &mut effects.bass.freq_hz,
            40.0..=500.0,
            " Hz",
            palette,
        );
    }
    changed |= widgets::toggle_row(
        ui,
        "Treble shelf",
        "Brightens or softens everything above the frequency.",
        &mut effects.treble.enabled,
        palette,
    );
    if effects.treble.enabled {
        changed |= param_row(
            ui,
            "Treble gain",
            &mut effects.treble.gain_db,
            -15.0..=15.0,
            " dB",
            palette,
        );
        changed |= param_row(
            ui,
            "Treble frequency",
            &mut effects.treble.freq_hz,
            2000.0..=16000.0,
            " Hz",
            palette,
        );
    }

    ui.add_space(6.0);
    widgets::section_title(ui, "Stereo", palette);
    changed |= widgets::toggle_row(
        ui,
        "Crossfeed",
        "Mixes a little of each channel into the other for headphone listening.",
        &mut effects.crossfeed,
        palette,
    );
    if effects.crossfeed {
        changed |= param_row(
            ui,
            "Feed level",
            &mut effects.crossfeed_db,
            -24.0..=-1.0,
            " dB",
            palette,
        );
    }
    changed |= widgets::toggle_row(
        ui,
        "Duophonic widening",
        "Delays a mono copy of the signal to widen the image.",
        &mut effects.fake_stereo,
        palette,
    );
    if effects.fake_stereo {
        changed |= param_row(
            ui,
            "Delay",
            &mut effects.fake_stereo_delay_ms,
            1.0..=60.0,
            " ms",
            palette,
        );
        changed |= param_row(
            ui,
            "Split frequency",
            &mut effects.fake_stereo_freq_hz,
            200.0..=4000.0,
            " Hz",
            palette,
        );
    }
    changed |= widgets::toggle_row(
        ui,
        "Swap left and right",
        "Exchanges the two stereo channels.",
        &mut effects.swap_speakers,
        palette,
    );
    changed |= widgets::toggle_row(
        ui,
        "Reverse channel order",
        "Writes the channels back to front, so the left channel plays on the right.",
        &mut effects.reverse_channels,
        palette,
    );

    ui.add_space(6.0);
    widgets::section_title(ui, "Time and space", palette);
    changed |= param_row(
        ui,
        "Delay",
        &mut effects.delay_ms,
        0.0..=200.0,
        " ms",
        palette,
    );
    changed |= widgets::toggle_row(
        ui,
        "Echo",
        "Repeats the signal a few times with decreasing level.",
        &mut effects.echo,
        palette,
    );
    if effects.echo {
        changed |= param_row(
            ui,
            "Echo delay",
            &mut effects.echo_delay_ms,
            40.0..=1200.0,
            " ms",
            palette,
        );
        ui.horizontal(|ui| {
            ui.set_min_width(150.0);
            ui.label(
                RichText::new("Echo repeats")
                    .size(11.5)
                    .color(palette.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut effects.echo_count).range(1..=4))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    }

    ui.add_space(6.0);
    widgets::section_title(ui, "Channel layout", palette);
    changed |= widgets::toggle_row(
        ui,
        "Upmix to surround",
        "Copies the front channels to side and rear outputs.",
        &mut effects.upmix,
        palette,
    );
    changed |= widgets::toggle_row(
        ui,
        "Downmix 5.1 to stereo",
        "Folds centre, subwoofer and rear channels into the front pair.",
        &mut effects.downmix_51,
        palette,
    );
    changed |= widgets::toggle_row(
        ui,
        "Downmix 7.1 to stereo",
        "Folds a full 7.1 layout into the front pair.",
        &mut effects.downmix_71,
        palette,
    );
    changed |= widgets::toggle_row(
        ui,
        "Graphic EQ (convolution)",
        "Writes GraphicEQ lines instead of biquad filters.",
        &mut effects.graphic_eq,
        palette,
    );

    ui.add_space(6.0);
    widgets::section_title(ui, "Routing", palette);
    ui.add(
        egui::TextEdit::singleline(&mut effects.routing)
            .hint_text("R=1.0*L C=1.0*L")
            .desired_width(f32::INFINITY),
    );
    ui.label(
        RichText::new("Raw Equalizer APO copy expressions, applied before the effects.")
            .size(9.5)
            .color(palette.muted),
    );

    changed
}

fn settings(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.settings {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    let mut theme_changed = false;
    let mut settings_changed = false;

    egui::Window::new("Settings")
        .id(egui::Id::new("settings-window"))
        .open(&mut open)
        .default_width(560.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Settings",
                "Preferences are stored in the PeaceControl data folder.",
                &palette,
            );

            egui::ScrollArea::vertical()
                .max_height(430.0)
                .show(ui, |ui| {
                    widgets::section_title(ui, "Status", &palette);
                    ui.label(
                        RichText::new(format!("Equalizer: {}", app.ui.settings_eq_label))
                            .size(11.0)
                            .color(palette.text),
                    );
                    ui.label(
                        RichText::new(format!("Active configuration: {}", app.ui.preset))
                            .size(11.0)
                            .color(palette.text),
                    );
                    ui.label(
                        RichText::new(format!("Applied: {}", app.ui.applied_label))
                            .size(10.5)
                            .color(palette.muted),
                    );
                    ui.add_space(10.0);
                    ui.separator();
                    widgets::section_title(ui, "Equalizer APO", &palette);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let label = app
                            .engine
                            .apo_dir
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "Not detected".to_string());
                        ui.label(RichText::new(label).size(10.5).monospace().color(
                            if app.engine.connected() {
                                palette.text
                            } else {
                                palette.warn
                            },
                        ));
                    });
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, "Choose folder…", &palette).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Select the Equalizer APO installation folder")
                                .pick_folder()
                            {
                                app.settings.equalizer_apo_dir = Some(path.display().to_string());
                                app.engine.refresh(&app.settings);
                                settings_changed = true;
                            }
                        }
                        if widgets::ghost_button(ui, "Detect again", &palette).clicked() {
                            app.settings.equalizer_apo_dir = None;
                            app.engine.refresh(&app.settings);
                            settings_changed = true;
                        }
                        if widgets::ghost_button(ui, "Open config folder", &palette).clicked() {
                            if let Some(dir) = &app.engine.config_dir {
                                platform::reveal(dir);
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Target configuration file")
                            .size(10.5)
                            .color(palette.muted),
                    );
                    let current = app
                        .engine
                        .target
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "none".to_string());
                    egui::ComboBox::from_id_salt("target-config")
                        .selected_text(current)
                        .width(240.0)
                        .show_ui(ui, |ui| {
                            for target in &app.engine.targets {
                                let selected = app.engine.target.as_ref() == Some(&target.path);
                                if ui.selectable_label(selected, &target.label).clicked() {
                                    app.settings.target_config =
                                        Some(target.path.display().to_string());
                                    app.engine.target = Some(target.path.clone());
                                    settings_changed = true;
                                }
                            }
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    widgets::section_title(ui, "Appearance", &palette);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Theme").size(11.5).color(palette.muted));
                        for theme in Theme::ALL {
                            if ui
                                .selectable_label(app.settings.theme == theme, theme.label())
                                .clicked()
                            {
                                app.settings.theme = theme;
                                theme_changed = true;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Accent").size(11.5).color(palette.muted));
                        if ui
                            .color_edit_button_srgb(&mut app.settings.accent)
                            .changed()
                        {
                            theme_changed = true;
                        }
                        ui.label(
                            RichText::new("Used across the interface and the graphs.")
                                .size(9.5)
                                .color(palette.muted),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Graph range").size(11.5).color(palette.muted));
                        ui.add(
                            egui::Slider::new(&mut app.settings.graph_range_db, 6.0..=48.0)
                                .suffix(" dB"),
                        );
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    widgets::section_title(ui, "Behaviour", &palette);
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Apply automatically",
                        "Rewrite the Equalizer APO file shortly after a change.",
                        &mut app.settings.apply_automatically,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Save recovery drafts",
                        "Keep an unsaved copy so a crash never loses your tuning.",
                        &mut app.settings.draft_autosave,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Back up on exit",
                        "Snapshot the Equalizer APO configuration folder when closing.",
                        &mut app.settings.backup_on_exit,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Start with Windows",
                        "Adds a per user autostart entry.",
                        &mut app.settings.start_with_windows,
                        &palette,
                    );
                    if let Err(error) =
                        platform::set_start_with_windows(app.settings.start_with_windows)
                    {
                        ui.label(RichText::new(error).size(9.5).color(palette.warn));
                    }
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Include preamp in the graph",
                        "Show the effect of the preamp on the predicted curve.",
                        &mut app.settings.preamp_in_graph,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Confirm before closing",
                        "Ask before discarding an unsaved configuration.",
                        &mut app.settings.confirm_on_exit,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Close to tray",
                        "Keep running in the tray when the window is closed.",
                        &mut app.settings.close_to_tray,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Minimize to tray",
                        "Hide the window instead of minimising it to the taskbar.",
                        &mut app.settings.minimize_to_tray,
                        &palette,
                    );
                    settings_changed |= widgets::toggle_row(
                        ui,
                        "Switch per application",
                        "Activate the configuration mapped to the foreground program.",
                        &mut app.settings.automate_apps,
                        &palette,
                    );
                    if widgets::ghost_button(ui, "Edit application rules...", &palette).clicked() {
                        app.windows.apps = true;
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    widgets::section_title(ui, "Global hotkeys", &palette);
                    let mut hotkeys_changed = false;
                    hotkeys_changed |= hotkey_row(
                        ui,
                        "Toggle equalizer",
                        &mut app.settings.hotkey_toggle,
                        &palette,
                    );
                    hotkeys_changed |= hotkey_row(
                        ui,
                        "Next configuration",
                        &mut app.settings.hotkey_next,
                        &palette,
                    );
                    hotkeys_changed |= hotkey_row(
                        ui,
                        "Previous configuration",
                        &mut app.settings.hotkey_previous,
                        &palette,
                    );
                    if hotkeys_changed {
                        app.reload_hotkeys();
                        settings_changed = true;
                    }
                    if app.shell.is_none() {
                        ui.label(
                            RichText::new(
                                "The tray icon and hotkeys are unavailable in this session.",
                            )
                            .size(9.5)
                            .color(palette.warn),
                        );
                    }
                    for problem in &app.shell_problems {
                        ui.label(RichText::new(problem).size(9.5).color(palette.warn));
                    }
                    ui.label(
                        RichText::new(
                            "Hotkeys work while any program is focused. Leave a field empty to disable it.",
                        )
                        .size(9.5)
                        .color(palette.muted),
                    );
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Log level").size(11.5).color(palette.muted));
                        egui::ComboBox::from_id_salt("log-level")
                            .selected_text(app.settings.log_level.clone())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for level in ["error", "warn", "info", "debug", "trace"] {
                                    if ui
                                        .selectable_label(app.settings.log_level == level, level)
                                        .clicked()
                                    {
                                        app.settings.log_level = level.to_string();
                                        peace_control_core::logging::set_level(level);
                                        settings_changed = true;
                                    }
                                }
                            });
                        ui.label(
                            RichText::new("Applied to the log file immediately.")
                                .size(9.5)
                                .color(palette.muted),
                        );
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    widgets::section_title(ui, "Defaults", &palette);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Bands for new configurations")
                                .size(11.5)
                                .color(palette.muted),
                        );
                        if ui
                            .add(
                                egui::DragValue::new(&mut app.settings.default_band_count)
                                    .range(1..=MAX_BANDS),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Snapshots to keep")
                                .size(11.5)
                                .color(palette.muted),
                        );
                        if ui
                            .add(egui::DragValue::new(&mut app.settings.max_backups).range(1..=100))
                            .changed()
                        {
                            settings_changed = true;
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, "Open data folder", &palette).clicked() {
                            platform::reveal(app.paths.root());
                        }
                        if widgets::ghost_button(ui, "Open log file", &palette).clicked() {
                            platform::open(&peace_control_core::logging::log_file(
                                &app.paths.logs_dir(),
                            ));
                        }
                        if widgets::ghost_button(ui, "Reset settings", &palette).clicked() {
                            app.reset_settings();
                            theme_changed = true;
                            settings_changed = true;
                        }
                    });
                });

            let _ = settings_changed;
        });

    if theme_changed {
        app.apply_theme(ctx);
    }
    if settings_changed || theme_changed {
        let _ = app.settings.save(&app.paths.settings_file());
        app.show_preamp = app.settings.preamp_in_graph;
    }
    app.windows.settings = open;
}

fn commands(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.commands {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("Commands")
        .id(egui::Id::new("commands-window"))
        .open(&mut open)
        .default_width(640.0)
        .default_height(480.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Commands",
                "Raw Equalizer APO lines appended after the equalizer and effects.",
                &palette,
            );
            ui.label(
                RichText::new("One command per line. Blank lines and # comments are ignored.")
                    .size(10.0)
                    .color(palette.muted),
            );
            ui.add_space(6.0);

            let mut buffer = app.commands_buffer.clone();
            let response = ui.add(
                egui::TextEdit::multiline(&mut buffer)
                    .desired_rows(14)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Preamp: -3 dB\nStage: post-mix"),
            );
            if response.changed() {
                app.commands_buffer = buffer;
                app.preset.commands = app
                    .commands_buffer
                    .lines()
                    .map(|line| line.trim_end().to_string())
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                app.dirty = true;
                app.last_edit = std::time::Instant::now();
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "Preview configuration", &palette).clicked() {
                    app.export_text = app.engine.preview(&app.preset);
                    app.export_title = "Rendered configuration".to_string();
                    ui.ctx().copy_text(app.export_text.clone());
                    app.status(
                        "Rendered configuration copied to the clipboard",
                        StatusKind::Success,
                    );
                }
                ui.label(
                    RichText::new(format!(
                        "{} command line(s)",
                        app.preset.commands_effective().len()
                    ))
                    .size(10.0)
                    .color(palette.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, "Apply now", app.engine.ready(), &palette)
                        .clicked()
                    {
                        app.apply_now();
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            widgets::section_title(ui, "Rendered preview", &palette);
            egui::ScrollArea::vertical()
                .id_salt("command-preview")
                .max_height(160.0)
                .show(ui, |ui| {
                    let preview = app.engine.preview(&app.preset);
                    ui.label(
                        RichText::new(preview)
                            .size(10.5)
                            .monospace()
                            .color(palette.muted),
                    );
                });
        });
    app.windows.commands = open;
}

fn import(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.import {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("Import")
        .id(egui::Id::new("import-window"))
        .open(&mut open)
        .default_width(600.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Import a configuration or measurement",
                "Equalizer APO config files, REW filter exports, AutoEQ curves and CSV tables.",
                &palette,
            );

            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "Choose file…", &palette).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("All supported", &["txt", "peace", "csv", "json"])
                        .pick_file()
                    {
                        app.import_from_path(&path);
                    }
                }
                if widgets::ghost_button(ui, "Analyse", &palette).clicked() {
                    app.analyse_import();
                }
            });
            ui.add_space(6.0);

            ui.add(
                egui::TextEdit::multiline(&mut app.import_text)
                    .desired_rows(10)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Paste Equalizer APO commands or frequency,gain rows here"),
            );

            if let Some(summary) = &app.import_summary {
                ui.add_space(6.0);
                ui.label(RichText::new(summary).size(11.5).color(palette.ok));
            }
            for note in &app.import_notes {
                ui.label(RichText::new(note).size(10.0).color(palette.muted));
            }
            if let Some(bands) = &app.import_bands {
                ui.label(
                    RichText::new(format!(
                        "{} band(s) ready; {} filters in the curve.",
                        bands.len(),
                        bands
                            .iter()
                            .filter(|band| band.gain_db.abs() > 0.05)
                            .count()
                    ))
                    .size(10.0)
                    .color(palette.muted),
                );
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(
                    ui,
                    "Replace configuration",
                    !app.import_text.is_empty(),
                    &palette,
                )
                .clicked()
                {
                    app.apply_import(true);
                }
                if widgets::ghost_button(ui, "Replace this speaker bank", &palette).clicked() {
                    app.apply_import(false);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::ghost_button(ui, "Close", &palette).clicked() {
                        app.windows.import = false;
                    }
                });
            });
        });
    if !open {
        app.windows.import = false;
    }
}

fn backups(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.backups {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("Backups")
        .id(egui::Id::new("backups-window"))
        .open(&mut open)
        .default_width(520.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Configuration snapshots",
                "Snapshots copy the whole Equalizer APO config folder into the PeaceControl data folder.",
                &palette,
            );
            if widgets::primary_button(ui, "Create snapshot now", app.engine.config_dir.is_some(), &palette).clicked() {
                app.backup_now();
            }
            ui.add_space(8.0);

            let snapshots = app.engine.snapshots();
            if snapshots.is_empty() {
                ui.label(
                    RichText::new("No snapshots yet.")
                        .size(11.0)
                        .color(palette.muted),
                );
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for snapshot in snapshots {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(snapshot.label())
                                .size(11.0)
                                .monospace()
                                .color(palette.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::ghost_button(ui, "Restore", &palette).clicked() {
                                match app.engine.restore(&snapshot.path) {
                                    Ok(count) => app.status(
                                        format!("Restored {count} files"),
                                        StatusKind::Success,
                                    ),
                                    Err(error) => app.status(
                                        format!("Restore failed: {error}"),
                                        StatusKind::Error,
                                    ),
                                }
                            }
                            if ui.small_button("open").clicked() {
                                platform::reveal(&snapshot.path);
                            }
                        });
                    });
                }
            });
        });
    app.windows.backups = open;
}

fn help(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.help {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("About PeaceControl")
        .id(egui::Id::new("help-window"))
        .open(&mut open)
        .default_width(560.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                crate::brand::logo(ui, 46.0, &palette);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("PeaceControl")
                            .size(20.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.label(
                        RichText::new(crate::SHORT_TAGLINE)
                            .size(11.0)
                            .color(palette.muted),
                    );
                });
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "Version {VERSION} · MIT licensed · an independent, open source configuration front end for Equalizer APO."
                ))
                .size(11.0)
                .color(palette.text),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "PeaceControl is not affiliated with Equalizer APO or with any other equalizer product. \
                     It only writes configuration files that Equalizer APO already understands; no audio is \
                     processed by this application.",
                )
                .size(10.5)
                .color(palette.muted),
            );

            ui.add_space(10.0);
            widgets::section_title(ui, "Diagnostics", &palette);
            ui.add_space(4.0);
            let diagnostics = format!(
                "{}\nLog: {}",
                app.engine.diagnostics(),
                peace_control_core::logging::log_file(&app.paths.logs_dir()).display()
            );
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                ui.label(
                    RichText::new(diagnostics.clone())
                        .size(10.0)
                        .monospace()
                        .color(palette.muted),
                );
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "Copy diagnostics", &palette).clicked() {
                    ui.ctx().copy_text(diagnostics);
                    app.status("Diagnostics copied", StatusKind::Success);
                }
                if widgets::ghost_button(ui, "Open log", &palette).clicked() {
                    platform::open(&peace_control_core::logging::log_file(&app.paths.logs_dir()));
                }
                if widgets::ghost_button(ui, "Shortcuts", &palette).clicked() {
                    app.windows.about = !app.windows.about;
                }
            });

            if app.windows.about {
                ui.add_space(8.0);
                ui.separator();
                widgets::section_title(ui, "Keyboard", &palette);
                for (keys, action) in [
                    ("Ctrl+S", "Save the configuration"),
                    ("Ctrl+E", "Apply to Equalizer APO"),
                    ("Ctrl+Z / Ctrl+Y", "Undo and redo"),
                    ("Ctrl+O", "Open a configuration file"),
                    ("Shift + drag", "Change a band frequency on the graph"),
                    ("Right click a band", "Reset its gain to 0 dB"),
                    ("F1", "This window"),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(keys)
                                .size(10.0)
                                .monospace()
                                .color(palette.accent),
                        );
                        ui.label(RichText::new(action).size(10.0).color(palette.muted));
                    });
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Close", true, &palette).clicked() {
                    app.windows.help = false;
                }
                if widgets::ghost_button(ui, "Snapshots…", &palette).clicked() {
                    app.windows.backups = true;
                }
            });
        });
    app.windows.help = open;
}

fn hotkey_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    palette: &theme::Palette,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.5).color(palette.muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let valid = value.trim().is_empty()
                || peace_control_core::hotkeys::Hotkey::parse(value).is_some();
            ui.label(
                RichText::new(if valid { "ready" } else { "invalid" })
                    .size(9.5)
                    .color(if valid { palette.ok } else { palette.warn }),
            );
            if ui
                .add(
                    egui::TextEdit::singleline(value)
                        .desired_width(150.0)
                        .hint_text("Ctrl+Alt+P"),
                )
                .changed()
            {
                changed = true;
            }
        });
    });
    changed
}

fn applications(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.apps {
        return;
    }
    let mut open = true;
    let palette = app.palette;
    let names: Vec<String> = app
        .preset_files
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    let foreground = crate::tray::foreground_process();
    let mut changed = false;
    let mut remove: Option<usize> = None;

    egui::Window::new("Applications")
        .id(egui::Id::new("applications-window"))
        .open(&mut open)
        .default_width(600.0)
        .resizable(true)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Per-application configurations",
                "When a matching program is in front, its configuration is activated and applied.",
                &palette,
            );

            changed |= widgets::toggle_row(
                ui,
                "Automation enabled",
                "Watch the foreground program while PeaceControl runs.",
                &mut app.settings.automate_apps,
                &palette,
            );

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Foreground now: {}",
                        foreground.clone().unwrap_or_else(|| "unknown".to_string())
                    ))
                    .size(10.0)
                    .monospace()
                    .color(palette.muted),
                );
                if widgets::ghost_button(ui, "Add current application", &palette).clicked() {
                    app.add_rule_for_foreground();
                    changed = true;
                }
            });
            if let Some(active) = app.tracker_active() {
                ui.label(
                    RichText::new(format!("Automation is holding \u{201c}{active}\u{201d}"))
                        .size(9.5)
                        .color(palette.muted),
                );
            }

            ui.add_space(8.0);
            ui.separator();
            widgets::section_title(ui, "Rules", &palette);
            ui.label(
                RichText::new("First match wins. Process names ignore case and the .exe suffix.")
                    .size(9.5)
                    .color(palette.muted),
            );
            ui.add_space(4.0);

            if app.settings.app_rules.is_empty() {
                ui.label(
                    RichText::new("No rules yet. Add the program you are using, or add one below.")
                        .size(11.0)
                        .color(palette.muted),
                );
            }

            egui::ScrollArea::vertical()
                .id_salt("app-rules")
                .max_height(260.0)
                .show(ui, |ui| {
                    for (index, rule) in app.settings.app_rules.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut rule.process)
                                        .desired_width(200.0)
                                        .hint_text("game.exe"),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label(RichText::new("\u{2192}").size(11.0).color(palette.muted));
                            egui::ComboBox::from_id_salt(("rule-preset", index))
                                .selected_text(if rule.preset.is_empty() {
                                    "choose".to_string()
                                } else {
                                    rule.preset.clone()
                                })
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for name in &names {
                                        if ui.selectable_label(rule.preset == *name, name).clicked()
                                        {
                                            rule.preset = name.clone();
                                            changed = true;
                                        }
                                    }
                                });
                            if ui.small_button("remove").clicked() {
                                remove = Some(index);
                            }
                        });
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Add rule", true, &palette).clicked() {
                    let preset = names.first().cloned().unwrap_or_default();
                    app.settings
                        .app_rules
                        .push(peace_control_core::AppRule::new("", preset));
                    changed = true;
                }
                if widgets::ghost_button(ui, "Clear rules", &palette).clicked() {
                    app.settings.app_rules.clear();
                    changed = true;
                }
                if widgets::ghost_button(ui, "Close", &palette).clicked() {
                    app.windows.apps = false;
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Switching saves the configuration you were editing first, so no work is lost.",
                )
                .size(9.5)
                .color(palette.muted),
            );
        });

    if let Some(index) = remove {
        app.settings.app_rules.remove(index);
        changed = true;
    }
    if changed {
        app.save_settings();
        app.reset_automation();
    }
    app.windows.apps = open;
}

fn confirm_close(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.confirm_close {
        return;
    }
    let palette = app.palette;
    let mut decision: Option<bool> = None;
    egui::Window::new("Unsaved changes")
        .id(egui::Id::new("confirm-close-window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Save this configuration?",
                "The window is closing and the current configuration has unsaved changes.",
                &palette,
            );
            ui.label(
                RichText::new(format!("“{}”", app.preset.name))
                    .size(11.5)
                    .color(palette.text),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Save and close", true, &palette).clicked() {
                    decision = Some(true);
                }
                if widgets::ghost_button(ui, "Close without saving", &palette).clicked() {
                    decision = Some(false);
                }
                if widgets::ghost_button(ui, "Keep editing", &palette).clicked() {
                    app.windows.confirm_close = false;
                    decision = None;
                }
            });
        });
    if let Some(save) = decision {
        if save {
            app.save_preset();
        }
        app.windows.confirm_close = false;
        app.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn recovery(app: &mut PeaceApp, ctx: &egui::Context) {
    if !app.windows.recovery {
        return;
    }
    let Some(draft) = app.draft.clone() else {
        app.windows.recovery = false;
        return;
    };
    let mut open = true;
    let palette = app.palette;
    egui::Window::new("Recover unsaved work")
        .id(egui::Id::new("recovery-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .frame(window_frame(&palette))
        .show(ctx, |ui| {
            header(
                ui,
                "Unsaved configuration found",
                "PeaceControl closed before this configuration was saved.",
                &palette,
            );
            ui.label(
                RichText::new(format!(
                    "“{}” · {} · {} bands",
                    draft.preset.name,
                    draft.age_label(),
                    draft.preset.band_count()
                ))
                .size(11.5)
                .color(palette.text),
            );
            if let Some(source) = &draft.source {
                ui.label(
                    RichText::new(source)
                        .size(9.5)
                        .monospace()
                        .color(palette.muted),
                );
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Recover it", true, &palette).clicked() {
                    app.push_history();
                    app.preset = draft.preset.clone();
                    app.speaker = 0;
                    app.clamp_selection();
                    app.dirty = true;
                    app.windows.recovery = false;
                    let _ = peace_control_core::session::clear(&app.paths.draft_file());
                    app.status("Recovered the unsaved configuration", StatusKind::Success);
                }
                if widgets::ghost_button(ui, "Discard", &palette).clicked() {
                    app.windows.recovery = false;
                    let _ = peace_control_core::session::clear(&app.paths.draft_file());
                    app.draft = None;
                    app.status("Draft discarded", StatusKind::Info);
                }
                if widgets::ghost_button(ui, "Keep for later", &palette).clicked() {
                    app.windows.recovery = false;
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Recovering replaces the configuration currently in the editor.")
                    .size(9.5)
                    .color(palette.muted),
            );
        });
    app.windows.recovery = open;
}
