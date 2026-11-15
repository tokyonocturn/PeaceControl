//! Crash recovery.
//!
//! While the user edits, PeaceControl writes a draft of the current preset. If
//! the application exits without saving (a crash, a forced reboot or a power
//! loss) the next start offers the draft back.

use crate::error::Result;
use crate::fsutil;
use crate::model::Preset;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A recoverable editing session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub preset: Preset,
    /// Unix seconds when the draft was written.
    pub saved_at: u64,
    /// The preset file this draft belongs to, when it was loaded from disk.
    pub source: Option<String>,
}

impl Draft {
    pub fn age_seconds(&self) -> u64 {
        now().saturating_sub(self.saved_at)
    }

    pub fn age_label(&self) -> String {
        let seconds = self.age_seconds();
        if seconds < 90 {
            "just now".to_string()
        } else if seconds < 3600 {
            format!("{} minutes ago", seconds / 60)
        } else if seconds < 86_400 {
            format!("{} hours ago", seconds / 3600)
        } else {
            format!("{} days ago", seconds / 86_400)
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Writes the draft, replacing the previous one.
pub fn save(path: &Path, preset: &Preset, source: Option<&str>) -> Result<()> {
    let draft = Draft {
        preset: preset.clone(),
        saved_at: now(),
        source: source.map(str::to_string),
    };
    let text = serde_json::to_string(&draft).unwrap_or_default();
    fsutil::write_atomic(path, &text)
}

/// Reads a draft if one exists and is complete.
pub fn load(path: &Path) -> Option<Draft> {
    let text = fsutil::read_to_string(path).ok()?;
    serde_json::from_str::<Draft>(&text).ok()
}

/// Removes the draft after a clean save or an explicit discard.
pub fn clear(path: &Path) -> Result<()> {
    fsutil::remove_file_if_exists(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pc-session-{}-{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("draft.json")
    }

    #[test]
    fn round_trips_a_draft() {
        let path = temp("roundtrip");
        let mut preset = Preset::default();
        preset.name = "Draft".to_string();
        preset.speakers[0].bands[0].gain_db = 3.0;
        save(&path, &preset, Some("C:/presets/a.json")).unwrap();
        let draft = load(&path).unwrap();
        assert_eq!(draft.preset.name, "Draft");
        assert_eq!(draft.preset.speakers[0].bands[0].gain_db, 3.0);
        assert_eq!(draft.source.as_deref(), Some("C:/presets/a.json"));
        clear(&path).unwrap();
        assert!(load(&path).is_none());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_draft_is_none() {
        let path = std::env::temp_dir().join("pc-session-none.json");
        let _ = std::fs::remove_file(&path);
        assert!(load(&path).is_none());
    }

    #[test]
    fn age_label_is_human_readable() {
        let draft = Draft {
            preset: Preset::default(),
            saved_at: now() - 7200,
            source: None,
        };
        assert_eq!(draft.age_label(), "2 hours ago");
    }
}
