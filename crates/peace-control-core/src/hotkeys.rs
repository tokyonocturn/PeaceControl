//! Global hotkey descriptions.
//!
//! The settings file stores hotkeys as text such as `Ctrl+Alt+P`. This module
//! parses that text into a virtual key code plus modifiers and formats it back,
//! without depending on Windows, so the behaviour is unit tested everywhere.
//! Registration itself lives in the application's platform layer.

/// A parsed global hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Windows virtual key code.
    pub vk: u32,
}

impl Hotkey {
    /// Parses `Ctrl+Alt+P`, `shift+control+F5`, `Win+Space` and similar.
    ///
    /// At least one modifier is required so a configuration cannot capture
    /// ordinary typing, exactly one key is allowed and the key must be known.
    pub fn parse(spec: &str) -> Option<Hotkey> {
        let mut hotkey = Hotkey {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            vk: 0,
        };
        let mut key: Option<u32> = None;
        for token in spec.split('+').map(str::trim) {
            if token.is_empty() {
                return None;
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => hotkey.ctrl = true,
                "alt" => hotkey.alt = true,
                "shift" => hotkey.shift = true,
                "win" | "super" | "meta" => hotkey.win = true,
                other => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(virtual_key(other)?);
                }
            }
        }
        let vk = key?;
        if !(hotkey.ctrl || hotkey.alt || hotkey.shift || hotkey.win) {
            return None;
        }
        hotkey.vk = vk;
        Some(hotkey)
    }

    /// Canonical text form, used for the settings file and the interface.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }
        parts.push(key_name(self.vk).unwrap_or("?"));
        parts.join("+")
    }

    /// Parses and reports the canonical form, for normalising user input.
    pub fn normalized(spec: &str) -> Option<String> {
        Hotkey::parse(spec).map(|hotkey| hotkey.describe())
    }
}

/// Virtual key code for a case-insensitive key name.
pub fn virtual_key(name: &str) -> Option<u32> {
    let name = name.trim();
    let upper = name.to_ascii_uppercase();
    let mut chars = upper.chars();
    if let (Some(first), None) = (chars.next(), chars.next()) {
        if first.is_ascii_uppercase() {
            return Some(first as u32);
        }
        if first.is_ascii_digit() {
            return Some(first as u32);
        }
    }
    if let Some(number) = upper.strip_prefix('F') {
        if let Ok(index) = number.parse::<u32>() {
            if (1..=24).contains(&index) {
                return Some(0x70 + index - 1);
            }
        }
    }
    let vk = match upper.as_str() {
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "BACKSPACE" => 0x08,
        "INSERT" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDN" => 0x22,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        "PAUSE" => 0x13,
        "PRINTSCREEN" => 0x2C,
        "MEDIANEXT" => 0xB0,
        "MEDIAPREV" => 0xB1,
        "MEDIASTOP" => 0xB2,
        "MEDIAPLAY" => 0xB3,
        "VOLUMEMUTE" => 0xAD,
        "VOLUMEDOWN" => 0xAE,
        "VOLUMEUP" => 0xAF,
        _ => return None,
    };
    Some(vk)
}

/// Canonical name for a virtual key code.
pub fn key_name(vk: u32) -> Option<&'static str> {
    const NAMES: [(u32, &str); 18] = [
        (0x20, "Space"),
        (0x09, "Tab"),
        (0x0D, "Enter"),
        (0x1B, "Esc"),
        (0x08, "Backspace"),
        (0x2D, "Insert"),
        (0x2E, "Delete"),
        (0x24, "Home"),
        (0x23, "End"),
        (0x21, "PageUp"),
        (0x22, "PageDown"),
        (0x25, "Left"),
        (0x26, "Up"),
        (0x27, "Right"),
        (0x28, "Down"),
        (0x13, "Pause"),
        (0x2C, "PrintScreen"),
        (0xB0, "MediaNext"),
    ];
    if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) {
        // Letters and digits are formatted from a static table so the returned
        // string has a stable lifetime without allocating.
        const ALPHABET: [&str; 26] = [
            "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
            "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
        ];
        const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        return if (0x41..=0x5A).contains(&vk) {
            Some(ALPHABET[(vk - 0x41) as usize])
        } else {
            Some(DIGITS[(vk - 0x30) as usize])
        };
    }
    if (0x70..=0x87).contains(&vk) {
        const FUNCTION_KEYS: [&str; 24] = [
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13",
            "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
        ];
        return Some(FUNCTION_KEYS[(vk - 0x70) as usize]);
    }
    NAMES
        .iter()
        .find(|(code, _)| *code == vk)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_in_any_order_and_case() {
        let hotkey = Hotkey::parse("shift+control+f5").unwrap();
        assert!(hotkey.shift && hotkey.ctrl);
        assert!(!hotkey.alt && !hotkey.win);
        assert_eq!(hotkey.vk, 0x74);
        assert_eq!(hotkey.describe(), "Ctrl+Shift+F5");
    }

    #[test]
    fn parses_letters_digits_and_named_keys() {
        assert_eq!(Hotkey::parse("Ctrl+Alt+P").unwrap().vk, 0x50);
        assert_eq!(Hotkey::parse("Win+7").unwrap().vk, 0x37);
        assert_eq!(Hotkey::parse("Ctrl+PageUp").unwrap().vk, 0x21);
        assert_eq!(Hotkey::parse("Alt+Space").unwrap().describe(), "Alt+Space");
    }

    #[test]
    fn rejects_unsafe_or_malformed_specs() {
        assert!(Hotkey::parse("").is_none());
        assert!(Hotkey::parse("Ctrl").is_none());
        assert!(Hotkey::parse("P").is_none());
        assert!(Hotkey::parse("Ctrl+Alt").is_none());
        assert!(Hotkey::parse("Ctrl+Nope").is_none());
        assert!(Hotkey::parse("Ctrl+A+B").is_none());
        assert!(Hotkey::parse("Ctrl++A").is_none());
        assert!(Hotkey::parse("Ctrl+F25").is_none());
    }

    #[test]
    fn every_named_key_round_trips_through_its_name() {
        for vk in [
            0x20u32, 0x09, 0x0D, 0x1B, 0x24, 0x23, 0x21, 0x22, 0x25, 0x26, 0x27, 0x28,
        ] {
            let name = key_name(vk).unwrap();
            assert_eq!(virtual_key(name), Some(vk), "{name} did not round trip");
        }
        for vk in 0x70u32..=0x87 {
            let name = key_name(vk).unwrap();
            assert_eq!(virtual_key(name), Some(vk));
        }
    }

    #[test]
    fn defaults_are_valid_hotkeys() {
        for spec in ["Ctrl+Alt+P", "Ctrl+Alt+Right", "Ctrl+Alt+Left"] {
            assert!(Hotkey::parse(spec).is_some(), "{spec} must parse");
            assert_eq!(Hotkey::normalized(spec).as_deref(), Some(spec));
        }
    }
}
