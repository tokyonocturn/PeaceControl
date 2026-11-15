//! Per-application profile automation.
//!
//! PeaceControl watches the name of the process that owns the foreground
//! window and can switch to a preset mapped to it. The matching rules live in
//! the settings file and are edited in the Applications window. Everything in
//! this module is pure logic so the behaviour can be tested without a desktop.

use serde::{Deserialize, Serialize};

/// One "when this program is in front, use that configuration" rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRule {
    /// Process name, with or without `.exe` and with or without a full path.
    pub process: String,
    /// Name of the configuration to activate.
    pub preset: String,
}

impl AppRule {
    pub fn new(process: impl Into<String>, preset: impl Into<String>) -> Self {
        AppRule {
            process: process.into(),
            preset: preset.into(),
        }
    }
}

/// Reduces a process name, a full path or a `C:\...\game.exe` string to the
/// comparable form used by the rules: lower case, no directories, no `.exe`.
pub fn normalize_process(name: &str) -> String {
    let trimmed = name.trim().trim_matches('"');
    let base = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed).trim();
    let lower = base.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .unwrap_or(&lower)
        .trim()
        .to_ascii_lowercase()
}

/// Finds the rule that applies to `process`.
///
/// Rules are consulted in order, so the first matching entry wins and the user
/// can put a specific rule above a broader one. Rules with an empty process or
/// preset are ignored instead of matching everything.
pub fn match_rule<'a>(rules: &'a [AppRule], process: &str) -> Option<&'a AppRule> {
    let target = normalize_process(process);
    if target.is_empty() {
        return None;
    }
    rules.iter().find(|rule| {
        let rule_process = normalize_process(&rule.process);
        !rule_process.is_empty() && !rule.preset.trim().is_empty() && rule_process == target
    })
}

/// Keeps track of the automation decision so a preset is only applied when the
/// foreground application starts matching a different preset.
#[derive(Debug, Default, Clone)]
pub struct Tracker {
    active: Option<String>,
}

impl Tracker {
    /// Forgets the last decision, so the next match is applied again.
    pub fn reset(&mut self) {
        self.active = None;
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// Feeds the preset that currently matches (if any) and returns the preset
    /// that should be activated now, or `None` when nothing changed.
    ///
    /// Losing the match resets the tracker: bringing the same application back
    /// to the front activates its preset again, which is what a user expects
    /// after editing that preset in the meantime.
    pub fn update(&mut self, matched: Option<&str>) -> Option<String> {
        match matched {
            Some(name) if !name.trim().is_empty() => {
                if self.active.as_deref() == Some(name) {
                    None
                } else {
                    self.active = Some(name.to_string());
                    Some(name.to_string())
                }
            }
            _ => {
                self.active = None;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_names_normalise_to_a_comparable_form() {
        assert_eq!(normalize_process("Chrome.exe"), "chrome");
        assert_eq!(normalize_process("C:\\Games\\Hades\\Hades2.EXE"), "hades2");
        assert_eq!(
            normalize_process("  \"C:/Tools/foobar2000.exe\"  "),
            "foobar2000"
        );
        assert_eq!(normalize_process(""), "");
    }

    #[test]
    fn matching_ignores_case_paths_and_the_exe_suffix() {
        let rules = vec![
            AppRule::new("chrome", "Headphones"),
            AppRule::new("C:\\Games\\Hades\\Hades2.exe", "Cinema"),
        ];
        assert_eq!(
            match_rule(&rules, "C:\\Program Files\\Google\\Chrome\\chrome.exe")
                .map(|rule| rule.preset.as_str()),
            Some("Headphones")
        );
        assert_eq!(
            match_rule(&rules, "hades2").map(|rule| rule.preset.as_str()),
            Some("Cinema")
        );
        assert!(match_rule(&rules, "explorer.exe").is_none());
    }

    #[test]
    fn empty_rules_never_match_everything() {
        let rules = vec![AppRule::new("", "Headphones"), AppRule::new("chrome", "")];
        assert!(match_rule(&rules, "chrome.exe").is_none());
        assert!(match_rule(&rules, "").is_none());
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let rules = vec![AppRule::new("game", "A"), AppRule::new("game.exe", "B")];
        assert_eq!(
            match_rule(&rules, "game.exe").map(|rule| rule.preset.as_str()),
            Some("A")
        );
    }

    #[test]
    fn tracker_reports_changes_only() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.update(Some("A")).as_deref(), Some("A"));
        assert_eq!(tracker.update(Some("A")), None);
        assert_eq!(tracker.update(Some("B")).as_deref(), Some("B"));
        assert_eq!(tracker.update(None), None);
        assert!(tracker.active().is_none());
        assert_eq!(tracker.update(Some("B")).as_deref(), Some("B"));
    }
}
