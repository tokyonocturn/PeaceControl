//! Tray icon, global hotkeys and per-application automation wiring.
//!
//! The Win32 details live in [`crate::tray`]; this module turns their events
//! into interface actions and drives the automation poll that watches the
//! foreground application.

use crate::app::{PeaceApp, StatusKind};
use crate::tray::{self, HotkeyAction, MenuState, ShellEvent};
use peace_control_core::automation::{match_rule, Tracker};
use std::time::{Duration, Instant};

/// How often the foreground application is inspected while automation is on.
const AUTOMATION_INTERVAL: Duration = Duration::from_millis(400);

impl PeaceApp {
    /// Creates the tray icon and registers the configured hotkeys.
    pub(crate) fn init_shell(&mut self, ctx: &egui::Context) {
        let tooltip = self.tray_tooltip();
        match tray::Shell::new(&tooltip) {
            Some(mut shell) => {
                self.shell_problems = shell.register(&self.hotkey_bindings());
                shell.set_tooltip(&tooltip);
                self.tooltip_cache = tooltip;
                log::info!(
                    "tray icon created; {} hotkey(s) registered",
                    self.hotkey_bindings().len() - self.shell_problems.len()
                );
                self.shell = Some(shell);
            }
            None => {
                self.shell_problems
                    .push("the tray icon could not be created".to_string());
                log::warn!("tray integration unavailable");
            }
        }
        if self.start_hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_visible = false;
        }
    }

    /// Releases the icon and any registered hotkey. Safe to call twice.
    pub(crate) fn release_shell(&mut self) {
        self.shell = None;
    }

    pub(crate) fn tray_tooltip(&self) -> String {
        format!(
            "PeaceControl — {} ({})",
            self.preset.name,
            if self.settings.enabled {
                "enabled"
            } else {
                "bypassed"
            }
        )
    }

    fn hotkey_bindings(&self) -> Vec<(String, HotkeyAction)> {
        bindings_from(&self.settings)
    }

    /// Applies edited hotkeys without restarting the application.
    pub(crate) fn reload_hotkeys(&mut self) {
        let bindings = self.hotkey_bindings();
        let Some(shell) = self.shell.as_mut() else {
            return;
        };
        self.shell_problems = shell.register(&bindings);
        if self.shell_problems.is_empty() {
            log::info!("global hotkeys registered");
        }
    }

    fn update_tooltip(&mut self) {
        let tooltip = self.tray_tooltip();
        if tooltip == self.tooltip_cache {
            return;
        }
        if let Some(shell) = self.shell.as_ref() {
            shell.set_tooltip(&tooltip);
        }
        self.tooltip_cache = tooltip;
    }

    /// Drains tray and hotkey events.
    pub(crate) fn poll_shell(&mut self, ctx: &egui::Context) {
        self.update_tooltip();
        let Some(shell) = self.shell.as_mut() else {
            return;
        };
        let names: Vec<String> = self
            .preset_files
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        let state = MenuState {
            presets: &names,
            enabled: self.settings.enabled,
            visible: self.window_visible,
            preset_playing: &self.preset.name,
        };
        for event in shell.poll(&state) {
            self.handle_shell_event(ctx, event);
        }
    }

    fn handle_shell_event(&mut self, ctx: &egui::Context, event: ShellEvent) {
        match event {
            ShellEvent::ToggleWindow => self.set_window_visible(ctx, !self.window_visible),
            ShellEvent::ToggleEnabled | ShellEvent::HotkeyToggle => {
                self.set_enabled(!self.settings.enabled)
            }
            ShellEvent::OpenSettings => {
                self.set_window_visible(ctx, true);
                self.windows.settings = true;
                log::info!("settings window opened from the tray");
                self.status("Settings", StatusKind::Info);
            }
            ShellEvent::Exit => self.quit(ctx),
            ShellEvent::ActivatePreset(name) => self.activate_preset(&name, "tray menu"),
            ShellEvent::HotkeyNext => self.step_preset(1),
            ShellEvent::HotkeyPrevious => self.step_preset(-1),
        }
    }

    pub(crate) fn set_window_visible(&mut self, ctx: &egui::Context, visible: bool) {
        log::info!("window {}", if visible { "shown" } else { "hidden" });
        self.window_visible = visible;
        if visible {
            crate::tray::show_window();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        } else {
            // Restore from a minimised state first: a minimised window does not
            // paint either, and painting is what keeps the tray responsive.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            if !crate::tray::hide_window() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
    }

    /// Turns the equalizer on or off from the interface, the tray or a hotkey.
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        if self.settings.enabled == enabled {
            return;
        }
        self.settings.enabled = enabled;
        let _ = self.settings.save(&self.paths.settings_file());
        match self.engine.apply_state(&self.preset, enabled) {
            Ok(path) => {
                self.applied_at = Some(Instant::now());
                self.status(
                    if enabled {
                        format!(
                            "Equalizer enabled, applied to {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    } else {
                        "Equalizer bypassed: the configuration file is now empty".to_string()
                    },
                    if enabled {
                        StatusKind::Success
                    } else {
                        StatusKind::Info
                    },
                );
                log::info!("equalizer {}", if enabled { "enabled" } else { "bypassed" });
            }
            Err(error) => self.status(
                format!("Could not change the state: {error}"),
                StatusKind::Error,
            ),
        }
    }

    pub(crate) fn quit(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.save_preset();
        }
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Saves pending edits, loads `name` and applies it when appropriate.
    pub(crate) fn activate_preset(&mut self, name: &str, source: &str) {
        if self.dirty && self.preset.name != name {
            self.save_preset();
        }
        self.load_preset_named(name);
        if self.settings.enabled && self.engine.ready() {
            self.apply_now();
        }
        log::info!("activated configuration '{name}' from {source}");
    }

    /// Moves through the configuration list, wrapping at both ends.
    pub(crate) fn step_preset(&mut self, delta: i32) {
        let names: Vec<String> = self
            .preset_files
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        let Some(name) = cycle_preset(&names, &self.preset.name, delta) else {
            return;
        };
        self.activate_preset(&name, "hotkey");
        if !self.settings.enabled {
            self.status(
                format!("Switched to “{name}” (equalizer bypassed)"),
                StatusKind::Info,
            );
        }
    }

    /// Watches the foreground application and applies matching rules.
    pub(crate) fn poll_automation(&mut self, _ctx: &egui::Context) {
        if !self.settings.automate_apps || self.settings.app_rules.is_empty() {
            self.automation.reset();
            return;
        }
        if self.last_automation_poll.elapsed() < AUTOMATION_INTERVAL {
            return;
        }
        self.last_automation_poll = Instant::now();
        let Some(process) = tray::foreground_process() else {
            return;
        };
        if process.is_empty() || process == self.own_process() {
            return;
        }
        let matched = match_rule(&self.settings.app_rules, &process)
            .map(|rule| rule.preset.clone())
            .filter(|preset| self.preset_files.iter().any(|entry| entry.name == *preset));
        if let Some(name) = self.automation.update(matched.as_deref()) {
            if name != self.preset.name {
                self.status(format!("{process} matched “{name}”"), StatusKind::Info);
                self.activate_preset(&name, "application automation");
            }
        }
    }

    fn own_process(&self) -> String {
        std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
            })
            .unwrap_or_default()
    }

    pub(crate) fn tracker_active(&self) -> Option<&str> {
        self.automation.active()
    }

    /// Creates a rule for whatever is in front of the user right now.
    pub(crate) fn add_rule_for_foreground(&mut self) {
        let Some(process) = tray::foreground_process() else {
            self.status(
                "Could not read the foreground application",
                StatusKind::Warning,
            );
            return;
        };
        if process.is_empty() || process == self.own_process() {
            self.status(
                "Bring the application you want to match to the front first",
                StatusKind::Warning,
            );
            return;
        }
        let preset = self.preset.name.clone();
        self.settings
            .app_rules
            .push(peace_control_core::AppRule::new(
                format!("{process}.exe"),
                preset.clone(),
            ));
        self.settings.automate_apps = true;
        self.save_settings();
        self.status(
            format!("Rule added: {process} → {preset}"),
            StatusKind::Success,
        );
    }

    pub(crate) fn save_settings(&mut self) {
        let sanitized = self.settings.clone().sanitized();
        self.settings = sanitized;
        if let Err(error) = self.settings.save(&self.paths.settings_file()) {
            log::warn!("could not save settings: {error}");
        }
    }

    /// Binds the automation tracker for the first time.
    pub(crate) fn reset_automation(&mut self) {
        self.automation = Tracker::default();
    }
}

/// The next configuration name in `names`, wrapping at both ends.
///
/// A configuration that is no longer in the list restarts from the first entry,
/// which is what a user sees after deleting the active configuration.
pub(crate) fn cycle_preset(names: &[String], current: &str, delta: i32) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let index = names.iter().position(|name| name == current).unwrap_or(0) as i32;
    let next = (index + delta).rem_euclid(names.len() as i32) as usize;
    Some(names[next].clone())
}

/// Turns the three configured hotkeys into registrations, skipping blanks.
pub(crate) fn bindings_from(
    settings: &peace_control_core::Settings,
) -> Vec<(String, HotkeyAction)> {
    vec![
        (settings.hotkey_toggle.clone(), HotkeyAction::Toggle),
        (settings.hotkey_next.clone(), HotkeyAction::Next),
        (settings.hotkey_previous.clone(), HotkeyAction::Previous),
    ]
    .into_iter()
    .filter(|(spec, _)| !spec.trim().is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use peace_control_core::Settings;

    #[test]
    fn cycling_steps_and_wraps() {
        let names = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        assert_eq!(cycle_preset(&names, "A", 1).as_deref(), Some("B"));
        assert_eq!(cycle_preset(&names, "C", 1).as_deref(), Some("A"));
        assert_eq!(cycle_preset(&names, "A", -1).as_deref(), Some("C"));
        assert_eq!(cycle_preset(&names, "missing", 1).as_deref(), Some("B"));
        assert_eq!(cycle_preset(&[], "A", 1), None);
    }

    #[test]
    fn only_configured_hotkeys_are_registered() {
        let mut settings = Settings::default();
        settings.hotkey_previous.clear();
        let bindings = bindings_from(&settings);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].1, HotkeyAction::Toggle);
        assert_eq!(bindings[1].0, "Ctrl+Alt+Right");

        settings.hotkey_toggle = "  ".to_string();
        settings.hotkey_next.clear();
        assert!(bindings_from(&settings).is_empty());
    }
}
