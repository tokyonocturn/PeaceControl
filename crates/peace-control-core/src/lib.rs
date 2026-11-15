//! PeaceControl core: domain model, Equalizer APO engine and persistence.
//!
//! The crate has no UI dependencies, which keeps it testable and lets the GUI,
//! the command line tool and future integrations share one implementation.
//!
//! * [`model`] – presets, speaker banks, bands and effects
//! * [`peace_ini`] – interoperable `*.peace` reading and writing
//! * [`preset_store`] – the JSON preset store used by PeaceControl itself
//! * [`render`] – preset to Equalizer APO command text
//! * [`parse`] – Equalizer APO configuration text to preset
//! * [`autoeq`] – AutoEQ / REW / CSV measurement import
//! * [`audacity`] – Audacity equalizer export
//! * [`library`] – built in configuration library
//! * [`paths`] – Equalizer APO discovery and app data locations
//! * [`settings`] – application settings
//! * [`backup`] – configuration snapshots
//! * [`logging`] – file logger
//! * [`session`] – crash recovery drafts
//! * [`dsp`] – predicted frequency response
//! * [`automation`] – per application profile rules
//! * [`hotkeys`] – global hotkey descriptions

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

pub mod audacity;
pub mod autoeq;
pub mod automation;
pub mod backup;
pub mod dsp;
pub mod error;
pub mod fsutil;
pub mod hotkeys;
pub mod library;
pub mod logging;
pub mod model;
pub mod parse;
pub mod paths;
pub mod peace_ini;
pub mod preset_store;
pub mod render;
pub mod session;
pub mod settings;

pub use automation::AppRule;
pub use error::{Error, Result};
pub use model::{Band, Effects, FilterType, Preset, SpeakerEq};
pub use settings::Settings;

/// Product identity used across every binary.
pub const PRODUCT_NAME: &str = "PeaceControl";
pub const PRODUCT_TAGLINE: &str = "Precision equalizer control for Equalizer APO";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
