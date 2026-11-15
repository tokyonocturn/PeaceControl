//! Import helpers for measurement and AutoEQ data.
//!
//! Three formats are supported:
//!
//! * Equalizer APO / AutoEQ parametric text (`Filter: ON PK ...`)
//! * AutoEQ `GraphicEQ` text and CSV (`frequency,gain` columns)
//! * Room EQ Wizard filter export (`* Filter 1: ON PK Fc ...`)
//!
//! All of them end up as editable [`Band`]s so the curve can be tuned further.

use crate::error::{Error, Result};
use crate::model::{Band, MAX_BANDS};
use crate::parse::{graphic_points_to_bands, parse_config, parse_graphic_eq, ImportReport};

/// One measurement or correction point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementPoint {
    pub freq_hz: f32,
    pub gain_db: f32,
}

/// Splits a CSV/text line into numeric tokens.
fn split_numbers(line: &str) -> Vec<f32> {
    line.split([',', ';', '\t', ' '])
        .filter_map(|token| {
            let token = token
                .trim()
                .trim_end_matches(|c: char| c.is_ascii_alphabetic());
            if token.is_empty() {
                return None;
            }
            token.replace(',', ".").parse::<f32>().ok()
        })
        .collect()
}

fn is_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("freq") && (lower.contains("gain") || lower.contains("db"))
}

/// Parses `frequency,gain` rows, ignoring headers and comments.
pub fn parse_csv(text: &str) -> Vec<MeasurementPoint> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| !is_header(line))
        .filter_map(|line| {
            let numbers = split_numbers(line);
            if numbers.len() < 2 {
                return None;
            }
            let (freq, gain) = (numbers[0], numbers[1]);
            if freq <= 0.0 || !freq.is_finite() || !gain.is_finite() {
                return None;
            }
            Some(MeasurementPoint {
                freq_hz: freq,
                gain_db: gain,
            })
        })
        .collect()
}

/// Parses a Room EQ Wizard "Filter" text export.
pub fn parse_rew_text(text: &str) -> Option<ImportReport> {
    let cleaned: String = text
        .lines()
        .map(|line| line.trim_start().trim_start_matches('*').trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    parse_config(&cleaned)
}

/// Is this text a Room EQ Wizard filter list?
pub fn looks_like_rew(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        let line = line.trim_start_matches('*').trim_start();
        line.to_ascii_lowercase().starts_with("filter") && line.to_ascii_lowercase().contains("fc")
    })
}

/// Converts measurement points into peak bands on a bounded grid.
pub fn points_to_bands(points: &[MeasurementPoint], limit: usize) -> Vec<Band> {
    let pairs: Vec<(f32, f32)> = points.iter().map(|p| (p.freq_hz, p.gain_db)).collect();
    graphic_points_to_bands(&pairs, limit.clamp(1, MAX_BANDS))
}

/// Result of reading a measurement file.
#[derive(Debug, Clone)]
pub struct MeasurementImport {
    pub bands: Vec<Band>,
    pub points: usize,
    pub format: MeasurementFormat,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementFormat {
    EqualizerApo,
    GraphicEq,
    Csv,
}

impl MeasurementFormat {
    pub fn label(self) -> &'static str {
        match self {
            MeasurementFormat::EqualizerApo => "Equalizer APO filters",
            MeasurementFormat::GraphicEq => "Graphic EQ curve",
            MeasurementFormat::Csv => "frequency/gain table",
        }
    }
}

/// Sniffs the format of `text` and imports it.
pub fn import_text(text: &str, max_bands: usize) -> Result<MeasurementImport> {
    if text.trim().is_empty() {
        return Err(Error::msg("the file is empty"));
    }

    if text
        .lines()
        .any(|line| line.trim_start().starts_with("GraphicEQ"))
    {
        let mut points = Vec::new();
        for line in text.lines() {
            if line.trim_start().starts_with("GraphicEQ") {
                points.extend(parse_graphic_eq(line));
            }
        }
        if points.is_empty() {
            return Err(Error::msg("the GraphicEQ line contains no points"));
        }
        let bands = graphic_points_to_bands(&points, max_bands.clamp(1, MAX_BANDS));
        return Ok(MeasurementImport {
            bands,
            points: points.len(),
            format: MeasurementFormat::GraphicEq,
            notes: vec!["Graphic EQ curve converted to editable bands.".to_string()],
        });
    }

    if crate::parse::looks_like_config(text) || looks_like_rew(text) {
        let report = parse_rew_text(text).or_else(|| parse_config(text));
        if let Some(report) = report {
            let mut bands: Vec<Band> = report
                .preset
                .speakers
                .iter()
                .flat_map(|speaker| speaker.bands.iter().cloned())
                .collect();
            bands.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
            bands.truncate(max_bands.clamp(1, MAX_BANDS));
            if bands.is_empty() {
                return Err(Error::msg("no filters were found in the file"));
            }
            let mut notes = report.notes.clone();
            if report.preset.speakers.len() > 1 {
                notes.push(format!(
                    "{} channels were merged into one equalizer.",
                    report.preset.speakers.len()
                ));
            }
            return Ok(MeasurementImport {
                bands,
                points: report.filters,
                format: MeasurementFormat::EqualizerApo,
                notes,
            });
        }
    }

    let points = parse_csv(text);
    if points.len() < 2 {
        return Err(Error::msg(
            "the file could not be read as Equalizer APO filters or as frequency/gain rows",
        ));
    }
    Ok(MeasurementImport {
        bands: points_to_bands(&points, max_bands),
        points: points.len(),
        format: MeasurementFormat::Csv,
        notes: vec!["Curve resampled onto editable bands.".to_string()],
    })
}

/// Reads a measurement file from disk.
pub fn import_file(path: &std::path::Path, max_bands: usize) -> Result<MeasurementImport> {
    let text = crate::fsutil::read_to_string(path)?;
    import_text(&text, max_bands)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "frequency,gain\n20,6\n100,4\n1000,0\n10000,-3\n";

    #[test]
    fn csv_header_is_skipped() {
        let points = parse_csv(CSV);
        assert_eq!(points.len(), 4);
        assert_eq!(points[0].freq_hz, 20.0);
        assert_eq!(points[3].gain_db, -3.0);
    }

    #[test]
    fn csv_accepts_spaces_and_semicolons() {
        let points = parse_csv("20 6\n100;4\n1000\t0\n");
        assert_eq!(points.len(), 3);
    }

    #[test]
    fn csv_ignores_broken_rows() {
        let points = parse_csv("oops\n20,1\nbad,row\n30\n40,2\n");
        assert_eq!(points.len(), 2);
    }

    #[test]
    fn imports_a_csv_curve() {
        let result = import_text(CSV, 31).unwrap();
        assert_eq!(result.format, MeasurementFormat::Csv);
        assert_eq!(result.bands.len(), 4);
        assert!((result.bands[0].freq_hz - 20.0).abs() < 0.01);
    }

    #[test]
    fn imports_graphic_eq() {
        let text = "GraphicEQ: 20 5; 200 2; 2000 -2; 20000 -6";
        let result = import_text(text, 31).unwrap();
        assert_eq!(result.format, MeasurementFormat::GraphicEq);
        assert_eq!(result.bands.len(), 4);
    }

    #[test]
    fn imports_rew_filters() {
        let text = "* Filter 1: ON PK Fc 100 Hz Gain 6 dB Q 1.0\n* Filter 2: ON PK Fc 4000 Hz Gain -4 dB Q 2.0\n";
        assert!(looks_like_rew(text));
        let result = import_text(text, 31).unwrap();
        assert_eq!(result.format, MeasurementFormat::EqualizerApo);
        assert_eq!(result.bands.len(), 2);
    }

    #[test]
    fn rejects_empty_text() {
        assert!(import_text("   ", 31).is_err());
    }

    #[test]
    fn rejects_nonsense() {
        assert!(import_text("just some words", 31).is_err());
    }

    #[test]
    fn respects_the_band_limit() {
        let mut text = String::from("GraphicEQ: ");
        for index in 0..40 {
            if index > 0 {
                text.push_str("; ");
            }
            let freq = 20.0 * 1.1f32.powi(index);
            text.push_str(&format!("{freq} 1"));
        }
        let result = import_text(&text, 10).unwrap();
        assert_eq!(result.bands.len(), 10);
    }
}
