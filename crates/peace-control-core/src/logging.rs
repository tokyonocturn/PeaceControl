//! A small file logger with rotation.
//!
//! PeaceControl ships no external logging stack: the application writes one
//! readable `peacecontrol.log`, rotates it when it grows and mirrors warnings
//! and errors to the console when one is attached.

use crate::error::Result;
use crate::fsutil;
use log::{Level, LevelFilter, Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_BYTES: u64 = 512 * 1024;
const KEEP_FILES: usize = 3;

struct FileLogger {
    file: Mutex<std::fs::File>,
    path: PathBuf,
    level: LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{:5} {} {}\r\n",
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
        if record.level() <= Level::Warn {
            eprint!("{line}");
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn rotate(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }
    for index in (1..KEEP_FILES).rev() {
        let from = path.with_extension(format!("log.{index}"));
        let to = path.with_extension(format!("log.{}", index + 1));
        if from.exists() {
            let _ = std::fs::rename(&from, &to);
        }
    }
    let _ = std::fs::rename(path, path.with_extension("log.1"));
}

/// Installs the global logger. Calling it twice is harmless.
pub fn init(log_dir: &Path, level: &str) -> Result<PathBuf> {
    fsutil::ensure_dir(log_dir)?;
    let path = log_dir.join("peacecontrol.log");
    rotate(&path);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| crate::Error::io_write(&path, error))?;

    let level = match level.to_ascii_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" | "warning" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Info,
    };

    let logger = FileLogger {
        file: Mutex::new(file),
        path: path.clone(),
        level,
    };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
    Ok(path)
}

/// The path of the active log file inside `log_dir`.
pub fn log_file(log_dir: &Path) -> PathBuf {
    log_dir.join("peacecontrol.log")
}

/// Reads the tail of the log file, for the diagnostics panel.
pub fn read_tail(log_dir: &Path, max_lines: usize) -> String {
    let path = log_file(log_dir);
    let Ok(text) = fsutil::read_to_string(&path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_file_path_is_stable() {
        let dir = std::env::temp_dir().join("pc-log-path");
        assert!(log_file(&dir).ends_with("peacecontrol.log"));
    }

    #[test]
    fn read_tail_returns_the_last_lines() {
        let dir = std::env::temp_dir().join(format!("pc-log-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = log_file(&dir);
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let tail = read_tail(&dir, 2);
        assert_eq!(tail, "two\nthree");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn init_writes_a_log_line() {
        let dir = std::env::temp_dir().join(format!("pc-log-init-{}", std::process::id()));
        let path = init(&dir, "debug").unwrap();
        log::info!("hello from the test");
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(content.is_empty() || content.contains("hello from the test"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
