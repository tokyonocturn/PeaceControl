//! Error types shared by every PeaceControl layer.

use std::path::PathBuf;

/// Everything that can go wrong while loading, editing or writing a configuration.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the file {0} is not a valid configuration: {1}")]
    InvalidPreset(PathBuf, String),

    #[error("the text is not a valid configuration: {0}")]
    InvalidPresetText(String),

    #[error("could not decode {path} as UTF-8: {source}")]
    Encoding {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("Equalizer APO was not detected. Install it first, or choose its folder in Settings.")]
    EqualizerApoNotFound,

    #[error("the configuration folder {0} does not exist")]
    ConfigDirMissing(PathBuf),

    #[error("no writable target configuration was selected")]
    NoTargetSelected,

    #[error("value out of range: {0}")]
    OutOfRange(String),

    #[error("{0}")]
    Message(String),
}

impl Error {
    pub fn io_read(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Read {
            path: path.into(),
            source,
        }
    }

    pub fn io_write(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Write {
            path: path.into(),
            source,
        }
    }

    pub fn msg(text: impl Into<String>) -> Self {
        Error::Message(text.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
