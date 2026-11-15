//! The preset library on disk.
//!
//! PeaceControl keeps its own presets as pretty printed JSON, one file per
//! preset, so they are easy to read, diff and back up. Imported `.peace`
//! files can be converted into this format and exported back again.

use crate::error::{Error, Result};
use crate::fsutil;
use crate::model::Preset;
use std::path::{Path, PathBuf};

/// An entry in the preset list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPreset {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
}

/// Reads and writes presets inside one folder.
#[derive(Debug, Clone)]
pub struct PresetStore {
    dir: PathBuf,
}

impl PresetStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        PresetStore { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn ensure(&self) -> Result<()> {
        fsutil::ensure_dir(&self.dir)
    }

    /// Every preset file, sorted by name.
    pub fn list(&self) -> Vec<StoredPreset> {
        fsutil::list_with_extensions(&self.dir, &["json"])
            .into_iter()
            .filter_map(|path| {
                let name = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                let description = fsutil::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Preset>(&text).ok())
                    .map(|preset| preset.description)
                    .unwrap_or_default();
                Some(StoredPreset {
                    name,
                    path,
                    description,
                })
            })
            .collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.path_for(name).exists()
    }

    pub fn path_for(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitize_name(name)))
    }

    pub fn save(&self, preset: &Preset) -> Result<PathBuf> {
        self.ensure()?;
        let path = self.path_for(&preset.name);
        let text = serde_json::to_string_pretty(preset)
            .map_err(|error| Error::msg(format!("could not encode the preset: {error}")))?;
        fsutil::write_atomic(&path, &text)?;
        Ok(path)
    }

    /// Saves under an explicit file name while keeping the display name.
    pub fn save_as(&self, preset: &Preset, file_name: &str) -> Result<PathBuf> {
        self.ensure()?;
        let path = self.path_for(file_name);
        let mut copy = preset.clone();
        copy.name = file_name.to_string();
        let text = serde_json::to_string_pretty(&copy)
            .map_err(|error| Error::msg(format!("could not encode the preset: {error}")))?;
        fsutil::write_atomic(&path, &text)?;
        Ok(path)
    }

    pub fn load(&self, name: &str) -> Result<Preset> {
        let path = self.path_for(name);
        let text = fsutil::read_to_string(&path)?;
        serde_json::from_str::<Preset>(&text)
            .map_err(|error| Error::InvalidPreset(path, error.to_string()))
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        fsutil::remove_file_if_exists(&self.path_for(name))
    }

    /// Adds every built in preset that is not present yet.
    pub fn seed_library(&self) -> Result<usize> {
        self.ensure()?;
        let mut installed = 0;
        for preset in crate::library::builtin_presets() {
            if !self.contains(&preset.name) {
                self.save(&preset)?;
                installed += 1;
            }
        }
        Ok(installed)
    }

    /// Newest first, by modification time.
    pub fn list_recent(&self, limit: usize) -> Vec<StoredPreset> {
        let mut entries: Vec<(std::time::SystemTime, StoredPreset)> = self
            .list()
            .into_iter()
            .map(|entry| {
                let modified = std::fs::metadata(&entry.path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                (modified, entry)
            })
            .collect();
        entries.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        entries
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect()
    }
}

/// Replaces characters Windows does not allow in a file name.
pub fn sanitize_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 => '-',
            c => c,
        })
        .collect();
    cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        cleaned = "Untitled".to_string();
    }
    if cleaned.len() > 100 {
        cleaned.truncate(100);
        cleaned = cleaned.trim().to_string();
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (PresetStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pc-store-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        (PresetStore::new(&dir), dir)
    }

    #[test]
    fn saves_and_loads_presets() {
        let (store, dir) = store("save");
        let mut preset = Preset::default();
        preset.name = "My Curve".to_string();
        preset.description = "test".to_string();
        preset.speakers[0].bands[0].gain_db = 4.0;
        let path = store.save(&preset).unwrap();
        assert!(path.exists());
        let loaded = store.load("My Curve").unwrap();
        assert_eq!(loaded.speakers[0].bands[0].gain_db, 4.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn list_returns_saved_names() {
        let (store, dir) = store("list");
        for name in ["B preset", "A preset"] {
            let mut preset = Preset::default();
            preset.name = name.to_string();
            store.save(&preset).unwrap();
        }
        let names: Vec<String> = store.list().into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["A preset", "B preset"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn delete_removes_the_file() {
        let (store, dir) = store("delete");
        let mut preset = Preset::default();
        preset.name = "Temp".to_string();
        store.save(&preset).unwrap();
        store.delete("Temp").unwrap();
        assert!(!store.contains("Temp"));
        store.delete("Temp").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn seeding_installs_the_library_once() {
        let (store, dir) = store("seed");
        let first = store.seed_library().unwrap();
        assert!(first > 0);
        let second = store.seed_library().unwrap();
        assert_eq!(second, 0);
        assert!(store.list().len() >= first);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize_name("a/b:c*d"), "a-b-c-d");
        assert_eq!(sanitize_name("   "), "Untitled");
        assert_eq!(sanitize_name("trailing. "), "trailing");
    }

    #[test]
    fn loading_unknown_preset_is_an_error() {
        let (store, dir) = store("missing");
        assert!(store.load("nope").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recent_list_returns_newest_first() {
        let (store, dir) = store("recent");
        for name in ["one", "two"] {
            let mut preset = Preset::default();
            preset.name = name.to_string();
            store.save(&preset).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let recent = store.list_recent(1);
        assert_eq!(recent[0].name, "two");
        let _ = std::fs::remove_dir_all(dir);
    }
}
