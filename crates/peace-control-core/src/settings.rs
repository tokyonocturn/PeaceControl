//! Application settings, stored as readable JSON.

use crate::error::Result;
use crate::fsutil;
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Colour scheme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Light, Theme::System];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::System => "Follow Windows",
        }
    }
}

/// Everything the user can tune about the application itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Equalizer APO installation folder discovered or chosen by the user.
    pub equalizer_apo_dir: Option<String>,
    /// Target configuration file inside that folder.
    pub target_config: Option<String>,
    pub theme: Theme,
    /// Accent colour as RGB.
    pub accent: [u8; 3],
    /// Include the preamp in the response graph.
    pub preamp_in_graph: bool,
    /// Highest gain shown on the graph axis.
    pub graph_range_db: f32,
    /// Bands created for a new preset.
    pub default_band_count: usize,
    /// Write a recovery draft while editing.
    pub draft_autosave: bool,
    /// Write the Equalizer APO configuration whenever the preset changes.
    pub apply_automatically: bool,
    /// Snapshot the Equalizer APO folder when the application exits.
    pub backup_on_exit: bool,
    pub max_backups: usize,
    pub confirm_on_exit: bool,
    pub start_with_windows: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub last_preset: Option<String>,
    pub recent_presets: Vec<String>,
    pub log_level: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            equalizer_apo_dir: None,
            target_config: None,
            theme: Theme::Dark,
            accent: [94, 234, 212],
            preamp_in_graph: false,
            graph_range_db: 24.0,
            default_band_count: 13,
            draft_autosave: true,
            apply_automatically: true,
            backup_on_exit: true,
            max_backups: 12,
            confirm_on_exit: false,
            start_with_windows: false,
            window_width: 1180.0,
            window_height: 760.0,
            last_preset: None,
            recent_presets: Vec::new(),
            log_level: "info".to_string(),
        }
    }
}

impl Settings {
    /// Clamps everything a hand edited file could have broken.
    pub fn sanitized(&self) -> Settings {
        let mut settings = self.clone();
        settings.graph_range_db = settings.graph_range_db.clamp(6.0, 48.0);
        settings.default_band_count = settings.default_band_count.clamp(1, 31);
        settings.max_backups = settings.max_backups.clamp(1, 200);
        settings.window_width = settings.window_width.clamp(720.0, 10000.0);
        settings.window_height = settings.window_height.clamp(480.0, 10000.0);
        settings.recent_presets.truncate(12);
        settings.log_level = match settings.log_level.to_ascii_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {
                settings.log_level.to_ascii_lowercase()
            }
            _ => "info".to_string(),
        };
        settings
    }

    pub fn load(path: &Path) -> Settings {
        match fsutil::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(settings) => settings.sanitized(),
                Err(error) => {
                    log::warn!("settings file is unreadable, using defaults: {error}");
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text =
            serde_json::to_string_pretty(&self.sanitized()).unwrap_or_else(|_| "{}".to_string());
        fsutil::write_atomic(path, &text)
    }

    pub fn load_from(paths: &AppPaths) -> Settings {
        Settings::load(&paths.settings_file())
    }

    /// Remembers a preset at the front of the recent list.
    pub fn remember_preset(&mut self, name: &str) {
        self.recent_presets.retain(|entry| entry != name);
        self.recent_presets.insert(0, name.to_string());
        self.recent_presets.truncate(12);
        self.last_preset = Some(name.to_string());
    }

    pub fn equalizer_apo_path(&self) -> Option<std::path::PathBuf> {
        self.equalizer_apo_dir
            .as_ref()
            .map(std::path::PathBuf::from)
    }

    pub fn target_path(&self) -> Option<std::path::PathBuf> {
        self.target_config.as_ref().map(std::path::PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();
        assert_eq!(settings.default_band_count, 13);
        assert!(settings.backup_on_exit);
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = std::env::temp_dir().join("peacecontrol-missing-settings.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn round_trips_through_json() {
        let dir = std::env::temp_dir().join(format!("pc-settings-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        settings.theme = Theme::Light;
        settings.accent = [10, 20, 30];
        settings.default_band_count = 31;
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.accent, [10, 20, 30]);
        assert_eq!(loaded.default_band_count, 31);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_json_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("pc-settings-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sanitize_clamps_wild_values() {
        let mut settings = Settings::default();
        settings.graph_range_db = 500.0;
        settings.default_band_count = 0;
        settings.recent_presets = (0..30).map(|i| format!("p{i}")).collect();
        settings.log_level = "verbose".to_string();
        let sanitized = settings.sanitized();
        assert_eq!(sanitized.graph_range_db, 48.0);
        assert_eq!(sanitized.default_band_count, 1);
        assert_eq!(sanitized.recent_presets.len(), 12);
        assert_eq!(sanitized.log_level, "info");
    }

    #[test]
    fn recent_presets_are_unique_and_capped() {
        let mut settings = Settings::default();
        settings.remember_preset("A");
        settings.remember_preset("B");
        settings.remember_preset("A");
        assert_eq!(settings.recent_presets, vec!["A", "B"]);
        assert_eq!(settings.last_preset.as_deref(), Some("A"));
    }
}
