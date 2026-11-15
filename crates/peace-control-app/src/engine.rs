//! Equalizer APO integration: discovery, applying configurations, snapshots.

use peace_control_core::backup::{self, Snapshot};
use peace_control_core::fsutil;
use peace_control_core::paths::{self, AppPaths};
use peace_control_core::render::{self, RenderOptions};
use peace_control_core::settings::Settings;
use peace_control_core::{Error, Preset, Result};
use std::path::{Path, PathBuf};

/// One selectable configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigTarget {
    pub path: PathBuf,
    pub label: String,
    pub is_main: bool,
}

/// Owns the connection to an Equalizer APO installation.
#[derive(Debug, Clone)]
pub struct Engine {
    paths: AppPaths,
    pub apo_dir: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub targets: Vec<ConfigTarget>,
    pub target: Option<PathBuf>,
}

impl Engine {
    pub fn new(paths: AppPaths) -> Self {
        Engine {
            paths,
            apo_dir: None,
            config_dir: None,
            targets: Vec::new(),
            target: None,
        }
    }

    /// Re-detects Equalizer APO and refreshes the target list.
    pub fn refresh(&mut self, settings: &Settings) {
        let configured = settings.equalizer_apo_dir.as_ref().map(PathBuf::from);
        self.apo_dir = paths::detect_equalizer_apo(configured.as_deref());
        self.targets.clear();
        self.config_dir = None;

        if let Some(dir) = &self.apo_dir {
            let config_dir = paths::config_dir(dir);
            if config_dir.is_dir() {
                self.targets = paths::config_targets(&config_dir)
                    .into_iter()
                    .map(|path| ConfigTarget {
                        label: path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.display().to_string()),
                        is_main: path
                            .file_name()
                            .map(|name| name.eq_ignore_ascii_case("config.txt"))
                            .unwrap_or(false),
                        path,
                    })
                    .collect();
                self.config_dir = Some(config_dir);
            }
        }

        let preferred = settings.target_config.as_ref().map(PathBuf::from);
        self.target = preferred.filter(|path| path.exists()).or_else(|| {
            self.config_dir
                .as_ref()
                .and_then(|dir| paths::default_target(dir))
        });
    }

    pub fn connected(&self) -> bool {
        self.apo_dir.is_some()
    }

    pub fn ready(&self) -> bool {
        self.target.is_some()
    }

    /// Writes the preset, or an empty configuration when the equalizer is
    /// bypassed. Equalizer APO reads an empty file as "change nothing", which is
    /// exactly what the global enable switch has to do.
    pub fn apply_state(&self, preset: &Preset, enabled: bool) -> Result<PathBuf> {
        let target = self.target.clone().ok_or(Error::NoTargetSelected)?;
        let text = if enabled {
            render::render_config(preset, &RenderOptions::from_preset(preset))
        } else {
            String::new()
        };
        fsutil::write_atomic(&target, &text)?;
        log::info!(
            "applied preset '{}' to {} ({} bytes)",
            preset.name,
            target.display(),
            text.len()
        );
        Ok(target)
    }

    /// Renders what would be written, without touching the disk.
    pub fn preview(&self, preset: &Preset) -> String {
        render::render_config(preset, &RenderOptions::from_preset(preset))
    }

    /// Takes a snapshot of the whole Equalizer APO config folder.
    pub fn snapshot(&self, keep: usize) -> Result<Option<Snapshot>> {
        let Some(config_dir) = &self.config_dir else {
            return Ok(None);
        };
        if !config_dir.is_dir() {
            return Ok(None);
        }
        let snapshot = backup::snapshot(config_dir, &self.paths.backups_dir())?;
        backup::prune(&self.paths.backups_dir(), keep)?;
        log::info!(
            "configuration snapshot written to {} ({} files)",
            snapshot.path.display(),
            snapshot.files
        );
        Ok(Some(snapshot))
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        backup::list(&self.paths.backups_dir())
    }

    pub fn restore(&self, snapshot: &Path) -> Result<usize> {
        let Some(config_dir) = &self.config_dir else {
            return Err(Error::NoTargetSelected);
        };
        let restored = backup::restore(snapshot, config_dir)?;
        log::warn!(
            "restored {restored} configuration files from {}",
            snapshot.display()
        );
        Ok(restored)
    }

    /// A short human readable status for the header.
    pub fn status_text(&self) -> String {
        match (&self.target, &self.apo_dir) {
            (Some(target), _) => target
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| target.display().to_string()),
            (None, Some(_)) => "no configuration file".to_string(),
            (None, None) => "Equalizer APO not detected".to_string(),
        }
    }

    /// A multi line diagnostics report for the help window.
    pub fn diagnostics(&self) -> String {
        let mut text = String::new();
        text.push_str(&format!(
            "Equalizer APO: {}\n",
            self.apo_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not detected".to_string())
        ));
        text.push_str(&format!(
            "Config folder: {}\n",
            self.config_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not found".to_string())
        ));
        text.push_str(&format!(
            "Target file: {}\n",
            self.target
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        text.push_str(&format!("Data folder: {}\n", self.paths.root().display()));
        text.push_str("Targets:\n");
        for target in &self.targets {
            text.push_str(&format!(
                "  - {}{}\n",
                target.label,
                if target.is_main { " (main)" } else { "" }
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_without_apo_is_honest() {
        let paths = AppPaths::with_root(std::env::temp_dir().join("pc-engine-none"));
        let mut engine = Engine::new(paths);
        engine.refresh(&Settings::default());
        assert_eq!(engine.connected(), engine.apo_dir.is_some());
        assert!(!engine.status_text().is_empty());
    }

    #[test]
    fn apply_without_target_fails_cleanly() {
        let paths = AppPaths::with_root(std::env::temp_dir().join("pc-engine-apply"));
        let engine = Engine::new(paths);
        let error = engine.apply_state(&Preset::default(), true).unwrap_err();
        assert!(matches!(error, Error::NoTargetSelected));
    }

    #[test]
    fn apply_writes_the_rendered_configuration() {
        let dir = std::env::temp_dir().join(format!("pc-engine-write-{}", std::process::id()));
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let target = config_dir.join("config.txt");
        std::fs::write(&target, "").unwrap();

        let mut engine = Engine::new(AppPaths::with_root(dir.join("data")));
        engine.config_dir = Some(config_dir.clone());
        engine.target = Some(target.clone());
        engine.targets = vec![ConfigTarget {
            path: target.clone(),
            label: "config.txt".to_string(),
            is_main: true,
        }];

        let mut preset = Preset::default();
        preset.preamp_db = -3.0;
        preset.speakers[0].bands[0].gain_db = 6.0;
        let written = engine.apply_state(&preset, true).unwrap();
        assert_eq!(written, target);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.contains("Preamp: -3 dB"));
        assert!(text.contains("Filter: ON PK"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bypassing_writes_an_empty_configuration() {
        let dir = std::env::temp_dir().join(format!("pc-engine-bypass-{}", std::process::id()));
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let target = config_dir.join("config.txt");
        std::fs::write(
            &target,
            "Preamp: -6 dB
",
        )
        .unwrap();

        let mut engine = Engine::new(AppPaths::with_root(dir.join("data")));
        engine.target = Some(target.clone());
        engine.apply_state(&Preset::default(), false).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "");
        engine.apply_state(&Preset::default(), true).unwrap();
        assert!(!std::fs::read_to_string(&target).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn snapshot_requires_a_config_folder() {
        let paths = AppPaths::with_root(std::env::temp_dir().join("pc-engine-snap"));
        let engine = Engine::new(paths);
        assert!(engine.snapshot(3).unwrap().is_none());
    }

    #[test]
    fn preview_matches_render_output() {
        let paths = AppPaths::with_root(std::env::temp_dir().join("pc-engine-preview"));
        let engine = Engine::new(paths);
        let preset = Preset::default();
        assert_eq!(
            engine.preview(&preset),
            render::render_config(&preset, &RenderOptions::from_preset(&preset))
        );
    }
}
