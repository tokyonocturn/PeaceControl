//! Discovery of Equalizer APO, configuration targets and PeaceControl folders.

use crate::error::{Error, Result};
use crate::fsutil;
use std::path::{Path, PathBuf};

/// Where PeaceControl keeps its own data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    root: PathBuf,
    portable: bool,
}

impl AppPaths {
    /// Resolves the data root.
    ///
    /// In portable mode (a `peacecontrol.portable` file next to the executable
    /// or `PEACECONTROL_PORTABLE=1`) everything lives in a `data` folder beside
    /// the executable, so a USB stick keeps its own settings.
    pub fn discover() -> AppPaths {
        if let Ok(custom) = std::env::var("PEACECONTROL_DATA_DIR") {
            if !custom.trim().is_empty() {
                return AppPaths {
                    root: PathBuf::from(custom),
                    portable: true,
                };
            }
        }

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let portable_flag = std::env::var("PEACECONTROL_PORTABLE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
            || exe_dir
                .as_ref()
                .map(|dir| dir.join("peacecontrol.portable").exists())
                .unwrap_or(false);

        if portable_flag {
            let root = exe_dir
                .unwrap_or_else(|| PathBuf::from("."))
                .join("data");
            return AppPaths {
                root,
                portable: true,
            };
        }

        let root = appdata_root()
            .unwrap_or_else(|| PathBuf::from(".").join(".peacecontrol"))
            .join(crate::PRODUCT_NAME);
        AppPaths {
            root,
            portable: false,
        }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> AppPaths {
        AppPaths {
            root: root.into(),
            portable: true,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_portable(&self) -> bool {
        self.portable
    }

    pub fn presets_dir(&self) -> PathBuf {
        self.root.join("presets")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn drafts_dir(&self) -> PathBuf {
        self.root.join("recovery")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn draft_file(&self) -> PathBuf {
        self.drafts_dir().join("draft.json")
    }

    /// Creates every folder PeaceControl needs.
    pub fn ensure(&self) -> Result<()> {
        fsutil::ensure_dir(&self.root)?;
        fsutil::ensure_dir(&self.presets_dir())?;
        fsutil::ensure_dir(&self.logs_dir())?;
        fsutil::ensure_dir(&self.backups_dir())?;
        fsutil::ensure_dir(&self.drafts_dir())?;
        Ok(())
    }
}

fn appdata_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

/// Folders where Equalizer APO is normally installed.
pub fn equalizer_apo_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.push(PathBuf::from(base).join("EqualizerAPO"));
        }
    }
    candidates.push(PathBuf::from(r"C:\Program Files\EqualizerAPO"));
    candidates.push(PathBuf::from(r"C:\Program Files (x86)\EqualizerAPO"));
    candidates.sort();
    candidates.dedup();
    candidates
}

/// True when `dir` looks like an Equalizer APO installation.
pub fn is_equalizer_apo_dir(dir: &Path) -> bool {
    dir.join("config").is_dir()
        || dir.join("EqualizerAPO.dll").exists()
        || dir.join("Editor.exe").exists()
        || dir.join("Configurator.exe").exists()
}

/// Finds an Equalizer APO installation, honouring a user configured path first.
pub fn detect_equalizer_apo(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = configured {
        if is_equalizer_apo_dir(dir) {
            return Some(dir.to_path_buf());
        }
    }
    equalizer_apo_candidates()
        .into_iter()
        .find(|candidate| is_equalizer_apo_dir(candidate))
}

/// The folder Equalizer APO reads configurations from.
pub fn config_dir(install_dir: &Path) -> PathBuf {
    install_dir.join("config")
}

/// Every `.txt` target inside the Equalizer APO config folder, `config.txt` first.
pub fn config_targets(config_dir: &Path) -> Vec<PathBuf> {
    let mut targets = fsutil::list_with_extensions(config_dir, &["txt"]);
    targets.sort_by_key(|path| {
        let is_main = path
            .file_name()
            .map(|name| name.eq_ignore_ascii_case("config.txt"))
            .unwrap_or(false);
        (!is_main, path.to_string_lossy().to_ascii_lowercase())
    });
    targets
}

/// The default target: `config.txt` when present, otherwise the first `.txt`.
pub fn default_target(config_dir: &Path) -> Option<PathBuf> {
    config_targets(config_dir).into_iter().next()
}

/// Resolves a target file, falling back to the default one.
pub fn resolve_target(config_dir: &Path, preferred: Option<&Path>) -> Result<PathBuf> {
    if let Some(preferred) = preferred {
        if preferred.exists() {
            return Ok(preferred.to_path_buf());
        }
        if let Some(name) = preferred.file_name() {
            let candidate = config_dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    default_target(config_dir).ok_or_else(|| Error::NoTargetSelected)
}

/// Lists `Include:` targets of a configuration file.
pub fn includes_of(config_file: &Path) -> Vec<String> {
    let mut includes = Vec::new();
    if let Ok(text) = fsutil::read_to_string(config_file) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Include:") {
                includes.push(rest.trim().to_string());
            }
        }
    }
    includes
}

/// The per user Documents folder, used for exported presets.
pub fn documents_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join("Documents"))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_root_has_expected_children() {
        let paths = AppPaths::with_root(std::env::temp_dir().join("peacecontrol-paths"));
        assert!(paths.presets_dir().ends_with("presets"));
        assert!(paths.settings_file().ends_with("settings.json"));
        assert!(paths.is_portable());
    }

    #[test]
    fn candidates_are_unique() {
        let candidates = equalizer_apo_candidates();
        let mut sorted = candidates.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(candidates.len(), sorted.len());
    }

    #[test]
    fn config_dir_is_named_config() {
        assert_eq!(config_dir(Path::new(r"C:\EQ")), PathBuf::from(r"C:\EQ").join("config"));
    }

    #[test]
    fn detect_returns_none_for_missing_configured_dir() {
        assert!(detect_equalizer_apo(Some(Path::new(r"C:\definitely-not-here"))).is_none_or(|dir| is_equalizer_apo_dir(&dir)));
    }

    #[test]
    fn includes_are_parsed() {
        let dir = std::env::temp_dir().join(format!("pc-includes-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("config.txt");
        std::fs::write(&file, "Preamp: 0 dB\nInclude: device1.txt\n").unwrap();
        assert_eq!(includes_of(&file), vec!["device1.txt"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_target_reports_when_empty() {
        let dir = std::env::temp_dir().join(format!("pc-empty-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert!(resolve_target(&dir, None).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
