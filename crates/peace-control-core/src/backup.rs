//! Configuration snapshots.
//!
//! Equalizer APO keeps its configurations inside its installation folder, so
//! uninstalling or repairing it can take a user's tuning with it. PeaceControl
//! therefore snapshots the folder into its own data directory, and can restore
//! a snapshot in one step.

use crate::error::Result;
use crate::fsutil;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// One snapshot folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub created_at: u64,
    pub files: usize,
}

impl Snapshot {
    pub fn label(&self) -> String {
        format!("{} ({} files)", self.path.file_name().unwrap_or_default().to_string_lossy(), self.files)
    }
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<usize> {
    let mut count = 0;
    fsutil::ensure_dir(destination)?;
    let entries = match std::fs::read_dir(source) {
        Ok(entries) => entries,
        Err(error) => return Err(crate::Error::io_read(source, error)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            count += copy_tree(&path, &target)?;
        } else if path.is_file() {
            fsutil::copy_file(&path, &target)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Copies `config_dir` into a new timestamped folder under `backups_root`.
pub fn snapshot(config_dir: &Path, backups_root: &Path) -> Result<Snapshot> {
    if !config_dir.is_dir() {
        return Err(crate::Error::ConfigDirMissing(config_dir.to_path_buf()));
    }
    fsutil::ensure_dir(backups_root)?;
    let created_at = timestamp();
    let mut path = backups_root.join(format!("snapshot-{created_at}"));
    let mut attempt = 1;
    while path.exists() {
        path = backups_root.join(format!("snapshot-{created_at}-{attempt}"));
        attempt += 1;
    }
    let files = copy_tree(config_dir, &path)?;
    Ok(Snapshot {
        path,
        created_at,
        files,
    })
}

/// Lists snapshots, newest first.
pub fn list(backups_root: &Path) -> Vec<Snapshot> {
    let mut snapshots: Vec<Snapshot> = std::fs::read_dir(backups_root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("snapshot-"))
                .unwrap_or(false)
        })
        .map(|path| {
            let files = std::fs::read_dir(&path)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| entry.path().is_file())
                .count();
            let created_at = path
                .file_name()
                .and_then(|name| name.to_string_lossy().split('-').nth(1).map(str::to_string))
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            Snapshot {
                path,
                created_at,
                files,
            }
        })
        .collect();
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.created_at));
    snapshots
}

/// Deletes the oldest snapshots so that at most `keep` remain.
pub fn prune(backups_root: &Path, keep: usize) -> Result<usize> {
    let snapshots = list(backups_root);
    let mut removed = 0;
    for snapshot in snapshots.into_iter().skip(keep.max(1)) {
        if std::fs::remove_dir_all(&snapshot.path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Copies a snapshot back over the configuration folder.
pub fn restore(snapshot: &Path, config_dir: &Path) -> Result<usize> {
    if !snapshot.is_dir() {
        return Err(crate::Error::ConfigDirMissing(snapshot.to_path_buf()));
    }
    fsutil::ensure_dir(config_dir)?;
    copy_tree(snapshot, config_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pc-backup-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn snapshot_copies_every_file() {
        let config = scratch("config");
        std::fs::write(config.join("config.txt"), "Preamp: 0 dB").unwrap();
        std::fs::create_dir_all(config.join("nested")).unwrap();
        std::fs::write(config.join("nested").join("device.txt"), "Filter: ON PK Fc 100 Hz Gain 1 dB Q 1").unwrap();
        let root = scratch("backups");
        let snapshot = snapshot(&config, &root).unwrap();
        assert_eq!(snapshot.files, 2);
        assert!(snapshot.path.join("config.txt").exists());
        assert!(snapshot.path.join("nested").join("device.txt").exists());
        let _ = std::fs::remove_dir_all(config);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restore_brings_files_back() {
        let config = scratch("restore-src");
        std::fs::write(config.join("config.txt"), "first").unwrap();
        let root = scratch("restore-backups");
        let snapshot = snapshot(&config, &root).unwrap();
        std::fs::write(config.join("config.txt"), "changed").unwrap();
        restore(&snapshot.path, &config).unwrap();
        assert_eq!(std::fs::read_to_string(config.join("config.txt")).unwrap(), "first");
        let _ = std::fs::remove_dir_all(config);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prune_keeps_the_requested_number() {
        let config = scratch("prune-config");
        std::fs::write(config.join("config.txt"), "x").unwrap();
        let root = scratch("prune-backups");
        for _ in 0..3 {
            snapshot(&config, &root).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }
        assert!(list(&root).len() >= 3);
        prune(&root, 1).unwrap();
        assert_eq!(list(&root).len(), 1);
        let _ = std::fs::remove_dir_all(config);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_of_missing_folder_is_an_error() {
        let root = scratch("missing");
        let result = snapshot(&root.join("nope"), &root);
        assert!(matches!(result, Err(crate::Error::ConfigDirMissing(_))));
        let _ = std::fs::remove_dir_all(root);
    }
}
