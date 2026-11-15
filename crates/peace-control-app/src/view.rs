//! The text the interface shows, derived from application state in one place.
//!
//! The header and the settings panel read these values, so the labels a user
//! sees and the snapshot the verification script reads (`PEACECONTROL_UI_DUMP`)
//! cannot drift apart: both come from this struct.

use peace_control_core::model::Preset;
use peace_control_core::settings::Settings;
use std::sync::Mutex;
use std::time::Instant;

/// Everything the header, the status bar and the settings status section show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    /// The active configuration, shown in the header and in Settings.
    pub preset: String,
    /// Header button text: "EQ on" or "EQ bypassed".
    pub eq_label: String,
    /// Settings status text: "Enabled" or "Bypassed".
    pub settings_eq_label: String,
    /// Status bar text: "applied 4s ago" or "not applied yet".
    pub applied_label: String,
    /// The configuration file the engine writes, as the status bar names it.
    pub target_label: String,
    /// Whether the Settings panel is on screen.
    pub settings_open: bool,
    /// Whether the window is on screen rather than parked in the tray.
    pub window_visible: bool,
}

impl UiState {
    pub fn new(
        settings: &Settings,
        preset: &Preset,
        target_label: String,
        applied_at: Option<Instant>,
        settings_open: bool,
        window_visible: bool,
    ) -> UiState {
        UiState {
            preset: preset.name.clone(),
            eq_label: if settings.enabled {
                "EQ on".to_string()
            } else {
                "EQ bypassed".to_string()
            },
            settings_eq_label: if settings.enabled {
                "Enabled".to_string()
            } else {
                "Bypassed".to_string()
            },
            applied_label: match applied_at {
                Some(at) => format!("applied {}s ago", at.elapsed().as_secs()),
                None => "not applied yet".to_string(),
            },
            target_label,
            settings_open,
            window_visible,
        }
    }

    /// One `key=value` line per rendered value.
    pub fn report(&self) -> String {
        format!(
            "preset={}\nsettings_preset={}\neq={}\nsettings_eq={}\napplied={}\ntarget={}\nsettings_open={}\nwindow={}\n",
            self.preset,
            self.preset,
            self.eq_label,
            self.settings_eq_label,
            self.applied_label,
            self.target_label,
            self.settings_open,
            if self.window_visible {
                "onscreen"
            } else {
                "parked"
            }
        )
    }

    /// Identifies the user visible state without the seconds in `applied`, so a
    /// dump is only rewritten when something a user would notice changed.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.preset,
            self.eq_label,
            self.settings_eq_label,
            !self.applied_label.starts_with("not applied"),
            self.settings_open,
            self.window_visible
        )
    }
}

/// Writes the rendered state for the verification script when it asked for it.
///
/// `PEACECONTROL_UI_DUMP` names the file; the application writes it atomically
/// and only when the state above changed.
pub fn dump_if_requested(state: &UiState) {
    let Some(path) = std::env::var_os("PEACECONTROL_UI_DUMP") else {
        return;
    };
    static LAST: Mutex<String> = Mutex::new(String::new());
    let key = state.key();
    if let Ok(mut last) = LAST.lock() {
        if *last == key {
            return;
        }
        *last = key;
    }
    let path = std::path::PathBuf::from(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temporary = path.with_extension("tmp");
    if std::fs::write(&temporary, state.report()).is_ok() {
        let _ = std::fs::rename(&temporary, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(settings: &Settings, preset: &Preset, open: bool, visible: bool) -> UiState {
        UiState::new(
            settings,
            preset,
            "config.txt".to_string(),
            None,
            open,
            visible,
        )
    }

    #[test]
    fn enabling_changes_what_the_header_and_settings_show() {
        let mut settings = Settings::default();
        let mut preset = Preset::default();
        preset.name = "Studio".to_string();

        let on = state(&settings, &preset, false, true);
        assert_eq!(on.eq_label, "EQ on");
        assert_eq!(on.settings_eq_label, "Enabled");
        assert!(on.report().contains("eq=EQ on"));

        // This is exactly the field ShellEvent::ToggleEnabled flips.
        settings.enabled = false;
        let off = state(&settings, &preset, false, true);
        assert_eq!(off.eq_label, "EQ bypassed");
        assert_eq!(off.settings_eq_label, "Bypassed");
        assert!(off.report().contains("settings_eq=Bypassed"));
        assert_ne!(on.key(), off.key());
    }

    #[test]
    fn activating_a_configuration_changes_the_header_and_settings() {
        let settings = Settings::default();
        let mut preset = Preset::default();
        preset.name = "Studio".to_string();
        let before = state(&settings, &preset, true, true);

        // This is exactly what ShellEvent::ActivatePreset loads.
        preset.name = "Late Night Listening".to_string();
        let after = state(&settings, &preset, true, true);

        assert_eq!(after.preset, "Late Night Listening");
        assert!(after.report().contains("preset=Late Night Listening"));
        assert!(after
            .report()
            .contains("settings_preset=Late Night Listening"));
        assert_ne!(before.key(), after.key());
    }

    #[test]
    fn the_settings_panel_and_window_state_are_reported() {
        let settings = Settings::default();
        let mut preset = Preset::default();
        preset.name = "Studio".to_string();
        let open = state(&settings, &preset, true, false);
        assert!(open.report().contains("settings_open=true"));
        assert!(open.report().contains("window=parked"));
        let closed = state(&settings, &preset, false, true);
        assert!(closed.report().contains("settings_open=false"));
        assert!(closed.report().contains("window=onscreen"));
    }

    #[test]
    fn applied_state_is_reported_without_making_every_second_a_new_key() {
        let settings = Settings::default();
        let mut preset = Preset::default();
        preset.name = "Studio".to_string();
        let fresh = UiState::new(
            &settings,
            &preset,
            "config.txt".to_string(),
            Some(Instant::now()),
            false,
            true,
        );
        assert!(fresh.report().contains("applied=applied 0s ago"));
        let mut later = fresh.clone();
        later.applied_label = "applied 9s ago".to_string();
        assert_eq!(fresh.key(), later.key());
    }
}
