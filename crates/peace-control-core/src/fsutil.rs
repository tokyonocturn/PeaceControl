//! File system helpers: atomic writes, safe reads and backups.
//!
//! Every configuration PeaceControl touches is written atomically so that a
//! crash, a power loss or a full disk can never leave Equalizer APO with a
//! half written `config.txt`. The previous contents are kept next to the file
//! as `<name>.bak` for one step undo.

use crate::error::{Error, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Creates `path` and every missing parent directory.
pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| Error::io_write(path, e))
}

/// Reads a UTF-8 text file, reporting the path on failure.
pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read(path)
        .map_err(|e| Error::io_read(path, e))
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|source| Error::Encoding {
                path: path.to_path_buf(),
                source,
            })
        })
}

/// Writes `contents` to `path` through a temporary file plus rename.
///
/// A `.bak` copy of the previous contents is left behind on success.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_dir(parent)?;
        }
    }

    if path.exists() {
        let backup = backup_path(path);
        // A failed backup must never block the real write.
        let _ = fs::copy(path, &backup);
    }

    let temp = temp_path(path);
    {
        let mut file = fs::File::create(&temp).map_err(|e| Error::io_write(&temp, e))?;
        file.write_all(contents.as_bytes())
            .map_err(|e| Error::io_write(&temp, e))?;
        file.write_all(b"")
            .map_err(|e| Error::io_write(&temp, e))?;
        file.sync_all().map_err(|e| Error::io_write(&temp, e))?;
    }

    // Windows replaces an existing destination with rename().
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(first) => {
            // Some antivirus products briefly lock the destination.
            let _ = fs::remove_file(path);
            fs::rename(&temp, path).map_err(|_| {
                let _ = fs::remove_file(&temp);
                Error::io_write(path, first)
            })
        }
    }
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Copies `source` to `destination`, overwriting it.
pub fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        ensure_dir(parent)?;
    }
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|e| Error::io_write(destination, e))
}

/// Removes a file, treating "not found" as success.
pub fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io_write(path, e)),
    }
}

/// Lists files in `dir` with the given lowercase extensions, sorted by name.
pub fn list_with_extensions(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let ext = ext.to_ascii_lowercase();
                    extensions.iter().any(|wanted| *wanted == ext)
                })
                .unwrap_or(false)
        })
        .collect();
    found.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("peacecontrol-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn atomic_write_creates_file_and_backup() {
        let dir = scratch("atomic");
        let path = dir.join("config.txt");
        write_atomic(&path, "first").unwrap();
        assert_eq!(read_to_string(&path).unwrap(), "first");
        write_atomic(&path, "second").unwrap();
        assert_eq!(read_to_string(&path).unwrap(), "second");
        assert_eq!(read_to_string(&backup_path(&path)).unwrap(), "first");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_file_reports_read_error() {
        let missing = std::env::temp_dir().join("peacecontrol-does-not-exist.txt");
        let error = read_to_string(&missing).unwrap_err();
        assert!(matches!(error, Error::Read { .. }));
    }

    #[test]
    fn list_with_extensions_filters_and_sorts() {
        let dir = scratch("list");
        fs::write(dir.join("b.txt"), "").unwrap();
        fs::write(dir.join("A.TXT"), "").unwrap();
        fs::write(dir.join("c.peace"), "").unwrap();
        let found = list_with_extensions(&dir, &["txt"]);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["A.TXT", "b.txt"]);
        let _ = fs::remove_dir_all(dir);
    }
}
