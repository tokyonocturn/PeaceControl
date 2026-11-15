//! Imports existing Equalizer APO configuration text.
//!
//! Paste or open a `config.txt` (or an AutoEQ `ParametricEQ.txt`) and
//! PeaceControl reconstructs an editable preset. Channel blocks become speaker
//! banks, biquad lines become bands, `GraphicEQ` lines become bands sampled on
//! a third octave grid, and anything that cannot be modelled is preserved
//! verbatim in the command list.

use crate::model::*;
use crate::render::num;

/// Result of an import, including what could not be modelled.
#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub preset: Preset,
    /// Lines kept as raw commands.
    pub passthrough: Vec<String>,
    /// Human readable notes about approximated or skipped content.
    pub notes: Vec<String>,
    pub filters: usize,
    pub preamps: usize,
    pub graphic_eq_points: usize,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("{} filters", self.filters)];
        if self.preamps > 0 {
            parts.push(format!("{} preamps", self.preamps));
        }
        if self.graphic_eq_points > 0 {
            parts.push(format!("{} GraphicEQ points", self.graphic_eq_points));
        }
        if !self.passthrough.is_empty() {
            parts.push(format!("{} raw commands", self.passthrough.len()));
        }
        parts.join(" · ")
    }
}

fn parse_float(token: &str) -> Option<f32> {
    token
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_alphabetic())
        .replace(',', ".")
        .parse::<f32>()
        .ok()
}

fn token_after<'a>(tokens: &'a [&'a str], key: &str) -> Option<&'a str> {
    tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case(key))
        .and_then(|index| tokens.get(index + 1))
        .copied()
}

fn filter_from_token(token: &str) -> Option<FilterType> {
    let normalised: String = token
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    FilterType::ALL
        .into_iter()
        .find(|filter| {
            filter
                .token()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .eq_ignore_ascii_case(&normalised)
        })
        .or(match normalised.as_str() {
            "LS6" | "LS6DB" => Some(FilterType::LowShelf6),
            "HS6" | "HS6DB" => Some(FilterType::HighShelf6),
            "LS12" | "LS12DB" => Some(FilterType::LowShelf12),
            "HS12" | "HS12DB" => Some(FilterType::HighShelf12),
            "LSQ" => Some(FilterType::LowShelf),
            "HSQ" => Some(FilterType::HighShelf),
            _ => None,
        })
}

/// Parses a `Filter:` line.
fn parse_filter(line: &str) -> Option<Band> {
    let body = line.split_once(':')?.1.trim();
    let tokens: Vec<&str> = body.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let enabled = !tokens[0].eq_ignore_ascii_case("OFF");
    let mut index = 1;
    // A filter index such as `Filter 3: ON PK ...` is not part of the shape.
    while index < tokens.len() && tokens[index].chars().all(|c| c.is_ascii_digit()) {
        index += 1;
    }
    // Shelf shapes are written as two tokens: `HS 12dB`.
    let mut shape = tokens.get(index)?.to_string();
    if matches!(shape.to_ascii_uppercase().as_str(), "LS" | "HS") {
        if let Some(next) = tokens.get(index + 1) {
            if next.to_ascii_lowercase().ends_with("db") {
                shape = format!("{shape} {next}");
            }
        }
    }
    let filter = filter_from_token(&shape)?;
    let freq = token_after(&tokens, "Fc").and_then(parse_float)?;
    let gain = token_after(&tokens, "Gain")
        .and_then(parse_float)
        .unwrap_or(0.0);
    let q = token_after(&tokens, "Q")
        .and_then(parse_float)
        .unwrap_or(1.41);
    Some(Band {
        freq_hz: freq,
        gain_db: gain,
        q,
        filter,
        enabled,
    })
}

/// Parses a `GraphicEQ:` line into frequency/gain points.
pub fn parse_graphic_eq(line: &str) -> Vec<(f32, f32)> {
    let Some((_, body)) = line.split_once(':') else {
        return Vec::new();
    };
    body.split(';')
        .filter_map(|pair| {
            let mut parts = pair.split_whitespace();
            let freq = parts.next().and_then(parse_float)?;
            let gain = parts.next().and_then(parse_float)?;
            Some((freq, gain))
        })
        .collect()
}

/// Converts graphic equalizer points into editable peak bands.
pub fn graphic_points_to_bands(points: &[(f32, f32)], limit: usize) -> Vec<Band> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    sorted.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01);
    // Closed loop gain control: with narrow bands the requested gain is what
    // the user sees, so a Q of ~1.4 matches a one octave spacing best.
    let step = if sorted.len() > 1 {
        let ratio = sorted[sorted.len() - 1].0 / sorted[0].0.max(1.0);
        ratio.powf(1.0 / (sorted.len() - 1) as f32)
    } else {
        1.0
    };
    let q = (1.0 / (2f32 * (step.max(1.01).ln() / 2.0).sinh())).clamp(0.3, 6.0);
    sorted
        .into_iter()
        .take(limit)
        .map(|(freq, gain)| Band::new(freq, gain, q))
        .collect()
}

/// Extracts the channel targets from a `Channel:` line.
fn parse_channel(line: &str) -> Option<String> {
    let body = line.split_once(':')?.1.trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

fn speaker_name(targets: &str) -> String {
    speaker_name_for(targets, 0)
}

/// Parses Equalizer APO configuration text into an editable preset.
pub fn parse_config(text: &str) -> Option<ImportReport> {
    let mut report = ImportReport::default();
    let mut preset = Preset {
        name: "Imported configuration".to_string(),
        speakers: Vec::new(),
        ..Default::default()
    };

    let mut current: Option<SpeakerEq> = None;
    let mut pending_graphic: Vec<(f32, f32)> = Vec::new();

    let flush = |current: &mut Option<SpeakerEq>,
                 pending: &mut Vec<(f32, f32)>,
                 speakers: &mut Vec<SpeakerEq>,
                 report: &mut ImportReport| {
        if let Some(mut speaker) = current.take() {
            if !pending.is_empty() {
                speaker.bands = graphic_points_to_bands(pending, MAX_BANDS);
                report.graphic_eq_points += pending.len();
                pending.clear();
            }
            if !speaker.bands.is_empty() || speaker.preamp_db.abs() > 0.001 {
                speakers.push(speaker);
            }
        }
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let lower = line.to_ascii_lowercase();

        if lower.starts_with("channel:") {
            flush(
                &mut current,
                &mut pending_graphic,
                &mut preset.speakers,
                &mut report,
            );
            if let Some(targets) = parse_channel(line) {
                current = Some(SpeakerEq::new(speaker_name(&targets), targets));
            }
            continue;
        }

        if lower.starts_with("preamp:") {
            let value = line
                .split_once(':')
                .and_then(|(_, body)| body.split_whitespace().next())
                .and_then(parse_float)
                .unwrap_or(0.0);
            report.preamps += 1;
            match current.as_mut() {
                Some(speaker) => speaker.preamp_db = value,
                None => preset.preamp_db = value,
            }
            continue;
        }

        if lower.starts_with("filter:") || lower.starts_with("filter ") {
            if let Some(band) = parse_filter(line) {
                report.filters += 1;
                current
                    .get_or_insert_with(|| SpeakerEq::new("All", "all"))
                    .bands
                    .push(band);
            }
            continue;
        }

        if lower.starts_with("graphiceq:") {
            let points = parse_graphic_eq(line);
            if !points.is_empty() {
                pending_graphic.extend(points);
                current.get_or_insert_with(|| SpeakerEq::new("All", "all"));
            }
            continue;
        }

        if lower.starts_with("include:") {
            report
                .notes
                .push(format!("Include line kept as a raw command: {}", line));
            report.passthrough.push(line.to_string());
            continue;
        }

        if lower.starts_with("device:") {
            report
                .notes
                .push(format!("Device line kept as a raw command: {line}"));
            report.passthrough.push(line.to_string());
            continue;
        }

        report.passthrough.push(line.to_string());
    }

    flush(
        &mut current,
        &mut pending_graphic,
        &mut preset.speakers,
        &mut report,
    );

    if preset.speakers.is_empty() {
        preset
            .speakers
            .push(SpeakerEq::new("All", "all").with_log_bands(13));
    }
    for speaker in &mut preset.speakers {
        speaker
            .bands
            .sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        speaker.bands.truncate(MAX_BANDS);
        speaker.preamp_db = speaker.preamp_db.clamp(-60.0, 30.0);
    }
    preset.commands = report.passthrough.clone();
    report.preset = preset;
    Some(report)
}

/// Convenience wrapper used by tests and the CLI: is this text an Equalizer
/// APO configuration at all?
pub fn looks_like_config(text: &str) -> bool {
    text.lines().take(400).any(|line| {
        let line = line.trim().to_ascii_lowercase();
        line.starts_with("filter:")
            || line.starts_with("preamp:")
            || line.starts_with("graphiceq:")
            || line.starts_with("channel:")
    })
}

/// Renders a human readable diff summary between two presets.
pub fn describe_changes(from: &Preset, to: &Preset) -> Vec<String> {
    let mut changes = Vec::new();
    if (from.preamp_db - to.preamp_db).abs() > 0.05 {
        changes.push(format!(
            "Preamp {} dB → {} dB",
            num(from.preamp_db),
            num(to.preamp_db)
        ));
    }
    if from.speakers.len() != to.speakers.len() {
        changes.push(format!(
            "Speaker banks {} → {}",
            from.speakers.len(),
            to.speakers.len()
        ));
    }
    for (index, (a, b)) in from.speakers.iter().zip(to.speakers.iter()).enumerate() {
        if a.bands.len() != b.bands.len() {
            changes.push(format!(
                "Bank {index}: {} → {} bands",
                a.bands.len(),
                b.bands.len()
            ));
        }
        let changed = a
            .bands
            .iter()
            .zip(b.bands.iter())
            .filter(|(x, y)| (x.gain_db - y.gain_db).abs() > 0.05)
            .count();
        if changed > 0 {
            changes.push(format!("Bank {index}: {changed} band(s) changed"));
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filter_lines() {
        let band = parse_filter("Filter: ON PK Fc 1000 Hz Gain 5.5 dB Q 1.41").unwrap();
        assert_eq!(band.filter, FilterType::Peak);
        assert_eq!(band.freq_hz, 1000.0);
        assert_eq!(band.gain_db, 5.5);
        assert_eq!(band.q, 1.41);
    }

    #[test]
    fn parses_filter_without_gain_or_q() {
        let band = parse_filter("Filter: ON LP Fc 80 Hz").unwrap();
        assert_eq!(band.filter, FilterType::LowPass);
        assert_eq!(band.gain_db, 0.0);
        assert_eq!(band.q, 1.41);
    }

    #[test]
    fn parses_shelf_tokens_with_spaces() {
        let band = parse_filter("Filter: ON HS 12dB Fc 4000 Hz Gain -3 dB Q 0.7").unwrap();
        assert_eq!(band.filter, FilterType::HighShelf12);
        assert_eq!(band.gain_db, -3.0);
    }

    #[test]
    fn parses_disabled_filters() {
        let band = parse_filter("Filter: OFF PK Fc 200 Hz Gain 2 dB Q 1").unwrap();
        assert!(!band.enabled);
    }

    #[test]
    fn parses_a_complete_config() {
        let text = "\
Preamp: -6 dB
Channel: L R
Filter: ON PK Fc 100 Hz Gain 4 dB Q 1.2
Filter: ON PK Fc 1000 Hz Gain -2 dB Q 1.4
Channel: SUB
Filter: ON LP Fc 80 Hz
";
        let report = parse_config(text).unwrap();
        assert_eq!(report.preset.speakers.len(), 2);
        assert_eq!(report.preset.speakers[0].targets, "L R");
        assert_eq!(report.preset.speakers[0].bands.len(), 2);
        assert_eq!(report.preset.speakers[1].name, "Subwoofer");
        assert_eq!(report.filters, 3);
    }

    #[test]
    fn graphic_eq_is_converted_to_bands() {
        let text = "GraphicEQ: 20 4; 40 3; 80 0; 160 -3\n";
        let report = parse_config(text).unwrap();
        let bands = &report.preset.speakers[0].bands;
        assert_eq!(bands.len(), 4);
        assert_eq!(report.graphic_eq_points, 4);
        assert!((bands[0].freq_hz - 20.0).abs() < 0.01);
    }

    #[test]
    fn unknown_commands_are_preserved() {
        let text = "Plugin: vst/Cool.dll\nStage: post-mix\n";
        let report = parse_config(text).unwrap();
        assert_eq!(report.preset.commands.len(), 2);
        assert_eq!(report.passthrough.len(), 2);
    }

    #[test]
    fn detects_equalizer_apo_text() {
        assert!(looks_like_config("Preamp: -2 dB\n"));
        assert!(!looks_like_config("hello world"));
    }

    #[test]
    fn import_of_rendered_output_keeps_values() {
        let mut preset = Preset::default();
        preset.preamp_db = -2.5;
        preset.speakers[0].bands[2].gain_db = 7.0;
        let text = crate::render::render_config(&preset, &Default::default());
        let report = parse_config(&text).unwrap();
        assert!((report.preset.preamp_db + 2.5).abs() < 0.01);
        assert_eq!(report.preset.speakers[0].bands.len(), 1);
    }

    #[test]
    fn describe_changes_reports_preamp() {
        let a = Preset::default();
        let mut b = Preset::default();
        b.preamp_db = -3.0;
        let changes = describe_changes(&a, &b);
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("Preamp"));
    }
}
