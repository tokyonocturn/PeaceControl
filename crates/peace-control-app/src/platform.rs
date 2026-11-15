//! Thin platform layer.
//!
//! Everything Windows specific is behind `cfg(windows)` so the crate still
//! builds for developers on other systems.

use std::path::Path;

/// Opens a folder or file in Explorer.
pub fn reveal(path: &Path) {
    let target = if path.is_file() {
        format!("/select,{}", path.display())
    } else {
        path.display().to_string()
    };
    let _ = std::process::Command::new("explorer").arg(target).spawn();
}

/// Opens a file with its default application.
pub fn open(path: &Path) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .spawn();
}

pub const AUTOSTART_VALUE: &str = "PeaceControl";

/// Enables or disables starting PeaceControl with Windows.
#[cfg(windows)]
pub fn set_start_with_windows(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    let (key, _) = hkcu
        .create_subkey(path)
        .map_err(|error| error.to_string())?;

    if enabled {
        let command = format!("\"{}\" --minimized", current.display());
        key.set_value(AUTOSTART_VALUE, &command)
            .map_err(|error| error.to_string())
    } else {
        match key.delete_value(AUTOSTART_VALUE) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// Whether the autostart entry currently exists.
#[cfg(windows)]
pub fn start_with_windows_enabled() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .and_then(|key| key.get_value::<String, _>(AUTOSTART_VALUE))
        .is_ok()
}

#[cfg(not(windows))]
pub fn set_start_with_windows(_enabled: bool) -> Result<(), String> {
    Err("Autostart is only implemented on Windows".to_string())
}

/// Whether Windows is currently using its dark app theme.
#[cfg(windows)]
pub fn system_prefers_dark() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|value| value == 0)
        .unwrap_or(true)
}

#[cfg(not(windows))]
pub fn system_prefers_dark() -> bool {
    true
}

#[cfg(not(windows))]
pub fn start_with_windows_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_value_has_a_stable_name() {
        assert_eq!(AUTOSTART_VALUE, "PeaceControl");
    }

    #[test]
    fn theme_preference_is_always_answerable() {
        let _ = system_prefers_dark();
    }

    #[cfg(windows)]
    #[test]
    fn autostart_can_be_toggled_and_reverted() {
        let was_enabled = start_with_windows_enabled();
        set_start_with_windows(true).unwrap();
        assert!(start_with_windows_enabled());
        set_start_with_windows(false).unwrap();
        assert!(!start_with_windows_enabled());
        if was_enabled {
            let _ = set_start_with_windows(true);
        }
    }
}
