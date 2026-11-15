//! Audacity equalizer export.
//!
//! Exporting the current curve lets a mix sound the same when the audio is
//! rendered to a file, where Equalizer APO is not in the signal path.

use crate::model::Preset;
use crate::render::num;

/// Formats one speaker bank as an Audacity equalizer XML document.
pub fn to_audacity_xml(preset: &Preset, speaker_index: usize) -> String {
    let speaker = preset.active_speaker(speaker_index);
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<equalizationeffect>\n");
    xml.push_str(&format!(
        "  <curve name=\"{}\">\n",
        escape(&format!("{} - {}", preset.name, speaker.name))
    ));
    let curve = crate::dsp::response_curve(&speaker.bands, 10.0, 20_000.0, 240);
    for (freq, gain) in curve {
        xml.push_str(&format!(
            "    <point dbo=\"{:.2}\" f=\"{}\"/>\n",
            gain,
            num(freq)
        ));
    }
    xml.push_str("  </curve>\n");
    xml.push_str("</equalizationeffect>\n");
    xml
}

/// Formats one speaker bank as an Audacity equalizer text file.
pub fn to_audacity_txt(preset: &Preset, speaker_index: usize) -> String {
    let speaker = preset.active_speaker(speaker_index);
    let mut text = String::new();
    for (freq, gain) in crate::dsp::response_curve(&speaker.bands, 10.0, 20_000.0, 240) {
        text.push_str(&format!("{}\t{:.2}\n", num(freq), gain));
    }
    text
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Band;

    fn preset() -> Preset {
        let mut preset = Preset::default();
        preset.name = "Demo & Test".to_string();
        preset.speakers[0].bands[0].gain_db = 6.0;
        preset
    }

    #[test]
    fn xml_contains_curve_and_points() {
        let xml = to_audacity_xml(&preset(), 0);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<equalizationeffect>"));
        assert!(xml.contains("Demo &amp; Test"));
        assert!(xml.contains("<point dbo="));
        assert!(xml.contains("</curve>"));
    }

    #[test]
    fn txt_has_two_columns() {
        let text = to_audacity_txt(&preset(), 0);
        let first = text.lines().next().unwrap();
        let columns: Vec<&str> = first.split('\t').collect();
        assert_eq!(columns.len(), 2);
        assert_eq!(text.lines().count(), 240);
    }

    #[test]
    fn speaker_index_is_clamped() {
        let xml = to_audacity_xml(&preset(), 99);
        assert!(xml.contains("curve"));
    }

    #[test]
    fn neutral_preset_exports_flat_curve() {
        let text = to_audacity_txt(&Preset::default(), 0);
        for line in text.lines() {
            let gain: f32 = line.split('\t').nth(1).unwrap().parse().unwrap();
            assert!(gain.abs() < 0.01);
        }
    }

    #[test]
    fn boost_shows_up_in_the_curve() {
        let mut preset = Preset::default();
        preset.speakers[0].bands = vec![Band::new(1000.0, 12.0, 4.0)];
        let text = to_audacity_txt(&preset, 0);
        let max: f32 = text
            .lines()
            .map(|line| line.split('\t').nth(1).unwrap().parse::<f32>().unwrap())
            .fold(f32::MIN, f32::max);
        assert!(max > 10.0);
    }
}
