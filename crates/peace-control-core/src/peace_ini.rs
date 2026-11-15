//! Reader and writer for the interoperable `.peace` configuration dialect.
//!
//! PeaceControl stores its own presets as JSON, but it reads and writes this
//! INI dialect so configurations created with the original application keep
//! working. The dialect is plain `key=value` data grouped in bracketed
//! sections; band values are stored once per speaker bank with a numeric
//! suffix on the section name (`[Frequencies]`, `[Frequencies1]`, ...).

use crate::error::{Error, Result};
use crate::fsutil;
use crate::model::*;
use std::path::Path;

/// One parsed INI document, preserving entry order.
#[derive(Debug, Default, Clone)]
pub struct Ini {
    entries: Vec<(String, String, String)>,
}

impl Ini {
    pub fn parse(text: &str) -> Ini {
        let mut entries = Vec::new();
        let mut section = String::new();
        for raw in text.trim_start_matches('\u{feff}').lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                if let Some(end) = line.find(']') {
                    section = line[1..end].trim().to_string();
                }
                continue;
            }
            if let Some(split) = line.find('=') {
                let key = line[..split].trim().to_string();
                let mut value = line[split + 1..].trim().to_string();
                if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                    value = value[1..value.len() - 1].to_string();
                }
                if !key.is_empty() {
                    entries.push((section.clone(), key, value));
                }
            }
        }
        Ini { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn find(&self, section: &str, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(s, k, _)| s.eq_ignore_ascii_case(section) && k.eq_ignore_ascii_case(key))
            .map(|(_, _, v)| v.as_str())
    }

    /// All key/value pairs of a section, in file order.
    fn section(&self, section: &str) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter(|(s, _, _)| s.eq_ignore_ascii_case(section))
            .map(|(_, k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    fn has_section(&self, section: &str) -> bool {
        self.entries
            .iter()
            .any(|(s, _, _)| s.eq_ignore_ascii_case(section))
    }

    fn number(&self, section: &str, key: &str) -> Option<f32> {
        self.find(section, key).and_then(parse_number)
    }

    fn integer(&self, section: &str, key: &str) -> Option<i32> {
        self.find(section, key).and_then(|value| {
            value
                .trim()
                .parse::<i32>()
                .ok()
                .or_else(|| parse_number(value).map(|n| n.round() as i32))
        })
    }

    fn boolean(&self, section: &str, key: &str) -> bool {
        match self.find(section, key) {
            Some(value) => {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("on")
                    || value.eq_ignore_ascii_case("yes")
            }
            None => false,
        }
    }
}

/// Parses a number, tolerating a decimal comma and stray units.
fn parse_number(value: &str) -> Option<f32> {
    let cleaned: String = value
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == ' ')
        .replace(',', ".");
    cleaned.parse::<f32>().ok()
}

/// Index of a suffixed section or key, for example `Frequency12` -> 12.
fn trailing_number(key: &str, prefix: &str) -> Option<i32> {
    let rest = key
        .get(prefix.len()..)
        .filter(|_| key.len() > prefix.len())?;
    rest.trim().parse::<i32>().ok()
}

fn band_section(base: &str, speaker_index: usize) -> String {
    if speaker_index == 0 {
        base.to_string()
    } else {
        format!("{base}{speaker_index}")
    }
}

/// Builds the band list of one speaker bank.
fn read_bands(ini: &Ini, speaker_index: usize) -> Vec<Band> {
    let main_freq = band_section("Frequencies", speaker_index);
    let mut section = main_freq.clone();
    if !ini.has_section(&section) && speaker_index > 0 {
        section = "Frequencies".to_string();
    }
    if !ini.has_section(&section) {
        return log_bands(13);
    }

    let mut indices: Vec<i32> = ini
        .section(&section)
        .iter()
        .filter_map(|(key, _)| trailing_number(key, "Frequency"))
        .collect();
    indices.sort_unstable();
    indices.dedup();
    if indices.is_empty() {
        return log_bands(13);
    }

    let gain_section = if ini.has_section(&band_section("Gains", speaker_index)) {
        band_section("Gains", speaker_index)
    } else {
        "Gains".to_string()
    };
    let quality_section = if ini.has_section(&band_section("Qualities", speaker_index)) {
        band_section("Qualities", speaker_index)
    } else {
        "Qualities".to_string()
    };
    let filter_section = if ini.has_section(&band_section("Filters", speaker_index)) {
        band_section("Filters", speaker_index)
    } else {
        "Filters".to_string()
    };
    let disabled_section = if ini.has_section(&band_section("Disabled", speaker_index)) {
        band_section("Disabled", speaker_index)
    } else {
        "Disabled".to_string()
    };

    let mut bands = Vec::with_capacity(indices.len());
    for index in indices {
        let frequency = ini
            .number(&section, &format!("Frequency{index}"))
            .unwrap_or(1000.0);
        let gain = ini
            .number(&gain_section, &format!("Gain{index}"))
            .unwrap_or(0.0);
        let q = ini
            .number(&quality_section, &format!("Quality{index}"))
            .unwrap_or(1.41);
        let filter = ini
            .integer(&filter_section, &format!("Filter{index}"))
            .map(FilterType::from_peace_code)
            .unwrap_or(FilterType::Peak);
        let disabled = ini.boolean(&disabled_section, &format!("Disabled{index}"));
        bands.push(Band {
            freq_hz: frequency,
            gain_db: gain,
            q,
            filter,
            enabled: !disabled,
        });
    }
    bands.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
    bands
}

fn read_general(ini: &Ini, preset: &mut Preset) {
    preset.preamp_db = ini.number("General", "PreAmp").unwrap_or(0.0);
    if let Some(description) = ini.find("General", "Description") {
        preset.description = description.replace('\u{0e}', "\n");
    }
    if let Some(routing) = ini.find("General", "Routing") {
        preset.effects.routing = routing.to_string();
    }
    preset.effects.graphic_eq = ini.boolean("General", "GraphicEQ");
    preset.effects.swap_speakers = ini.boolean("General", "Swap Speakers");
    preset.effects.reverse_channels = ini.boolean("General", "Reverse");
    preset.effects.upmix = ini.boolean("General", "Upmix");
    preset.effects.downmix_51 = ini.boolean("General", "Downmix 5.1");
    preset.effects.downmix_71 = ini.boolean("General", "Downmix 7.1");
    preset.effects.crossfeed = ini.boolean("General", "Crossfeed");
    preset.effects.fake_stereo = ini.boolean("General", "Fake Stereo");
    preset.effects.echo =
        ini.boolean("General", "Echo Count") || ini.find("General", "Echo Count").is_some();
    preset.effects.delay_ms = ini.number("General", "Delay").unwrap_or(0.0);
    preset.effects.echo_count = ini
        .integer("General", "Echo Count")
        .unwrap_or(0)
        .clamp(0, 8) as u32;
    preset.effects.echo_delay_ms = ini.number("General", "Echo Delay").unwrap_or(250.0);
    preset.effects.fake_stereo_delay_ms =
        ini.number("General", "Fake Stereo Delay").unwrap_or(20.0);
    preset.effects.fake_stereo_freq_hz = ini
        .number("General", "Fake Stereo Frequency")
        .unwrap_or(1000.0);

    if let Some(gain) = ini.number("General", "Bass Gain") {
        preset.effects.bass.enabled = gain.abs() > 0.01;
        preset.effects.bass.gain_db = gain;
    }
    if let Some(freq) = ini.number("General", "Bass Frequency") {
        preset.effects.bass.freq_hz = freq;
    }
    if let Some(gain) = ini.number("General", "Treble Gain") {
        preset.effects.treble.enabled = gain.abs() > 0.01;
        preset.effects.treble.gain_db = gain;
    }
    if let Some(freq) = ini.number("General", "Treble Frequency") {
        preset.effects.treble.freq_hz = freq;
    }
}

fn read_surround(ini: &Ini, preset: &mut Preset) {
    if !ini.has_section("Surround") {
        return;
    }
    let surround = &mut preset.surround;
    surround.enabled = ini.boolean("Surround", "Surround On");
    surround.bass_frequency_hz = ini
        .number("Surround", "Bass Frequency")
        .unwrap_or(surround.bass_frequency_hz);
    surround.front_level = ini.number("Surround", "Front Level").unwrap_or(1.0);
    surround.center_level = ini.number("Surround", "Center Level").unwrap_or(1.0);
    surround.subwoofer_level = ini.number("Surround", "Subwoofer Level").unwrap_or(1.0);
    surround.side_level = ini.number("Surround", "Side Level").unwrap_or(1.0);
    surround.rear_level = ini.number("Surround", "Rear Level").unwrap_or(1.0);
    surround.front_delay_ms = ini.number("Surround", "Front Delay").unwrap_or(0.0);
    surround.subwoofer_delay_ms = ini.number("Surround", "Subwoofer Delay").unwrap_or(0.0);
    surround.front_width = ini.number("Surround", "Front Width").unwrap_or(0.0);
    surround.side_width = ini.number("Surround", "Side Width").unwrap_or(0.3);
    surround.rear_width = ini.number("Surround", "Rear Width").unwrap_or(0.5);
}

/// Parses the interoperable INI dialect into a [`Preset`].
pub fn parse_preset(text: &str) -> Result<Preset> {
    let ini = Ini::parse(text);
    if ini.is_empty() {
        return Err(Error::InvalidPresetText(
            "the file contains no sections or keys".to_string(),
        ));
    }

    let mut preset = Preset {
        speakers: Vec::new(),
        ..Default::default()
    };
    read_general(&ini, &mut preset);
    read_surround(&ini, &mut preset);

    let mut ids: Vec<i32> = ini
        .section("Speakers")
        .iter()
        .filter_map(|(key, _)| trailing_number(key, "SpeakerId"))
        .collect();
    ids.sort_unstable();
    ids.dedup();

    if ids.is_empty() {
        let mut speaker = SpeakerEq::new("All", "all");
        speaker.bands = read_bands(&ini, 0);
        speaker.preamp_db = ini.number("General", "PreAmp1").unwrap_or(0.0);
        preset.speakers.push(speaker);
    } else {
        for id in ids {
            let name = ini
                .find("Speakers", &format!("SpeakerName{id}"))
                .map(str::to_string)
                .unwrap_or_else(|| format!("Speaker {}", id + 1));
            let targets = ini
                .find("Speakers", &format!("SpeakerTargets{id}"))
                .map(str::to_string)
                .unwrap_or_else(|| "all".to_string());
            let mut speaker = SpeakerEq::new(name, targets);
            speaker.bands = read_bands(&ini, id.max(0) as usize);
            if id > 0 {
                speaker.preamp_db = ini.number("General", &format!("PreAmp{id}")).unwrap_or(0.0);
            }
            preset.speakers.push(speaker);
        }
    }
    if preset.speakers.is_empty() {
        preset
            .speakers
            .push(SpeakerEq::new("All", "all").with_log_bands(13));
    }

    for (_, value) in ini.section("Commands") {
        for line in value.split('\t') {
            if !line.trim().is_empty() {
                preset.commands.push(line.trim().to_string());
            }
        }
    }

    preset.hotkey = ini
        .find("Configuration", "HotKey")
        .unwrap_or("")
        .to_string();

    for (key, value) in ini.section("Sliders") {
        if key.to_ascii_lowercase().ends_with("-0") && !value.trim().is_empty() {
            preset.slider_labels.push(value.trim().to_string());
        }
    }

    Ok(preset)
}

fn push_key(out: &mut String, key: &str, value: impl std::fmt::Display) {
    out.push_str(&format!("{key}={value}\r\n"));
}

fn push_section(out: &mut String, name: &str) {
    out.push('[');
    out.push_str(name);
    out.push_str("]\r\n");
}

fn format_number(value: f32) -> String {
    if (value - value.round()).abs() < 0.0005 {
        format!("{}", value.round() as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Serialises a preset into the interoperable INI dialect.
pub fn to_peace_string(preset: &Preset) -> String {
    let mut out = String::new();

    let mut general = String::new();
    if !preset.description.trim().is_empty() {
        let description = preset.description.replace('\n', "\u{0e}");
        push_key(&mut general, "Description", description);
    }
    if preset.preamp_db.abs() > 0.0005 {
        push_key(&mut general, "PreAmp", format_number(preset.preamp_db));
    }
    let effects = &preset.effects;
    if effects.bass.enabled {
        push_key(
            &mut general,
            "Bass Gain",
            format_number(effects.bass.gain_db),
        );
        push_key(
            &mut general,
            "Bass Frequency",
            format_number(effects.bass.freq_hz),
        );
    }
    if effects.treble.enabled {
        push_key(
            &mut general,
            "Treble Gain",
            format_number(effects.treble.gain_db),
        );
        push_key(
            &mut general,
            "Treble Frequency",
            format_number(effects.treble.freq_hz),
        );
    }
    if effects.delay_ms.abs() > 0.0005 {
        push_key(&mut general, "Delay", format_number(effects.delay_ms));
    }
    if effects.crossfeed {
        push_key(&mut general, "Crossfeed", 1);
    }
    if effects.fake_stereo {
        push_key(&mut general, "Fake Stereo", 1);
        push_key(
            &mut general,
            "Fake Stereo Delay",
            format_number(effects.fake_stereo_delay_ms),
        );
        push_key(
            &mut general,
            "Fake Stereo Frequency",
            format_number(effects.fake_stereo_freq_hz),
        );
    }
    if effects.echo {
        push_key(&mut general, "Echo Count", effects.echo_count);
        push_key(
            &mut general,
            "Echo Delay",
            format_number(effects.echo_delay_ms),
        );
    }
    if effects.swap_speakers {
        push_key(&mut general, "Swap Speakers", 1);
    }
    if effects.reverse_channels {
        push_key(&mut general, "Reverse", 1);
    }
    if effects.upmix {
        push_key(&mut general, "Upmix", 1);
    }
    if effects.downmix_51 {
        push_key(&mut general, "Downmix 5.1", 1);
    }
    if effects.downmix_71 {
        push_key(&mut general, "Downmix 7.1", 1);
    }
    if effects.graphic_eq {
        push_key(&mut general, "GraphicEQ", 1);
    }
    if !effects.routing.trim().is_empty() {
        push_key(&mut general, "Routing", effects.routing.trim());
    }
    if !general.is_empty() {
        out.push_str("[General]\r\n");
        out.push_str(&general);
    }

    out.push_str("[Speakers]\r\n");
    for (index, speaker) in preset.speakers.iter().enumerate() {
        push_key(&mut out, &format!("SpeakerId{index}"), index);
        push_key(
            &mut out,
            &format!("SpeakerTargets{index}"),
            speaker.targets.trim(),
        );
        push_key(&mut out, &format!("SpeakerName{index}"), &speaker.name);
    }

    for (index, speaker) in preset.speakers.iter().enumerate() {
        let suffix = if index == 0 {
            String::new()
        } else {
            index.to_string()
        };
        push_section(&mut out, &format!("Frequencies{suffix}"));
        for (band_index, band) in speaker.bands.iter().enumerate() {
            push_key(
                &mut out,
                &format!("Frequency{}", band_index + 1),
                format_number(band.freq_hz),
            );
        }

        push_section(&mut out, &format!("Qualities{suffix}"));
        for (band_index, band) in speaker.bands.iter().enumerate() {
            push_key(
                &mut out,
                &format!("Quality{}", band_index + 1),
                format_number(band.q),
            );
        }

        if speaker.bands.iter().any(|band| band.gain_db.abs() > 0.0005) {
            push_section(&mut out, &format!("Gains{suffix}"));
            for (band_index, band) in speaker.bands.iter().enumerate() {
                if band.gain_db.abs() > 0.0005 {
                    push_key(
                        &mut out,
                        &format!("Gain{}", band_index + 1),
                        format_number(band.gain_db),
                    );
                }
            }
        }

        if speaker
            .bands
            .iter()
            .any(|band| band.filter != FilterType::Peak)
        {
            push_section(&mut out, &format!("Filters{suffix}"));
            for (band_index, band) in speaker.bands.iter().enumerate() {
                if band.filter != FilterType::Peak {
                    push_key(
                        &mut out,
                        &format!("Filter{}", band_index + 1),
                        band.filter.peace_code(),
                    );
                }
            }
        }

        if speaker.bands.iter().any(|band| !band.enabled) {
            push_section(&mut out, &format!("Disabled{suffix}"));
            for (band_index, band) in speaker.bands.iter().enumerate() {
                if !band.enabled {
                    push_key(&mut out, &format!("Disabled{}", band_index + 1), 1);
                }
            }
        }
    }

    if !preset.commands.is_empty() {
        out.push_str("[Commands]\r\n");
        for (index, command) in preset.commands.iter().enumerate() {
            push_key(&mut out, &index.to_string(), command.trim());
        }
    }

    if !preset.hotkey.trim().is_empty() {
        out.push_str("[Configuration]\r\n");
        push_key(&mut out, "HotKey", preset.hotkey.trim());
    }

    if preset.surround.enabled {
        let surround = &preset.surround;
        out.push_str("[Surround]\r\n");
        push_key(&mut out, "Surround On", 1);
        push_key(
            &mut out,
            "Bass Frequency",
            format_number(surround.bass_frequency_hz),
        );
        push_key(&mut out, "Front Level", format_number(surround.front_level));
        push_key(
            &mut out,
            "Center Level",
            format_number(surround.center_level),
        );
        push_key(
            &mut out,
            "Subwoofer Level",
            format_number(surround.subwoofer_level),
        );
        push_key(&mut out, "Side Level", format_number(surround.side_level));
        push_key(&mut out, "Rear Level", format_number(surround.rear_level));
        push_key(
            &mut out,
            "Front Delay",
            format_number(surround.front_delay_ms),
        );
        push_key(
            &mut out,
            "Subwoofer Delay",
            format_number(surround.subwoofer_delay_ms),
        );
        push_key(&mut out, "Front Width", format_number(surround.front_width));
        push_key(&mut out, "Side Width", format_number(surround.side_width));
        push_key(&mut out, "Rear Width", format_number(surround.rear_width));
    }

    if !preset.slider_labels.is_empty() {
        out.push_str("[Sliders]\r\n");
        for (index, label) in preset.slider_labels.iter().take(5).enumerate() {
            push_key(&mut out, &format!("Slider{}-0", index + 1), label);
            for slot in 1..3 {
                push_key(&mut out, &format!("Slider{}-{slot}", index + 1), "\"\"");
            }
        }
    }

    out
}

/// Loads a `.peace` file from disk.
pub fn load_preset_file(path: &Path) -> Result<Preset> {
    let text = fsutil::read_to_string(path)?;
    let mut preset = parse_preset(&text).map_err(|error| match error {
        Error::InvalidPresetText(message) => Error::InvalidPreset(path.to_path_buf(), message),
        other => other,
    })?;
    if let Some(stem) = path.file_stem() {
        preset.name = stem.to_string_lossy().to_string();
    }
    Ok(preset)
}

/// Writes a `.peace` file to disk atomically.
pub fn save_preset_file(path: &Path, preset: &Preset) -> Result<()> {
    fsutil::write_atomic(path, &to_peace_string(preset))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "[General]\r\nDescription=Bass boost using a low shelf filter\r\nPreAmp=-4\r\n\
[Speakers]\r\nSpeakerId0=0\r\nSpeakerTargets0=all\r\nSpeakerName0=All\r\n\
SpeakerId1=1\r\nSpeakerTargets1=L\r\nSpeakerName1=Left\r\n\
[Frequencies]\r\nFrequency1=10\r\nFrequency2=100\r\nFrequency3=1000\r\n\
[Qualities]\r\nQuality1=0.4\r\nQuality2=0.7\r\nQuality3=1.41\r\n\
[Gains]\r\nGain2=10\r\n\
[Filters]\r\nFilter2=14\r\n\
[Disabled]\r\nDisabled3=1\r\n\
[Commands]\r\n0=Preamp: -3 dB\tChannel: L R\r\n";

    #[test]
    fn parses_general_section() {
        let preset = parse_preset(SAMPLE).unwrap();
        assert_eq!(preset.preamp_db, -4.0);
        assert!(preset.description.contains("low shelf"));
        assert_eq!(preset.speakers.len(), 2);
        assert_eq!(preset.speakers[1].name, "Left");
        assert_eq!(preset.speakers[1].targets, "L");
    }

    #[test]
    fn parses_bands_with_filter_and_disabled_flags() {
        let preset = parse_preset(SAMPLE).unwrap();
        let bands = &preset.speakers[0].bands;
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[1].gain_db, 10.0);
        assert_eq!(bands[1].filter, FilterType::LowShelf6);
        assert!(!bands[2].enabled);
        assert_eq!(bands[0].q, 0.4);
    }

    #[test]
    fn commands_are_split_on_tabs() {
        let preset = parse_preset(SAMPLE).unwrap();
        assert_eq!(preset.commands.len(), 2);
        assert_eq!(preset.commands[1], "Channel: L R");
    }

    #[test]
    fn second_speaker_inherits_shared_band_sections() {
        let preset = parse_preset(SAMPLE).unwrap();
        assert_eq!(preset.speakers[1].bands.len(), 3);
    }

    #[test]
    fn round_trip_preserves_the_model() {
        let original = parse_preset(SAMPLE).unwrap();
        let text = to_peace_string(&original);
        let reparsed = parse_preset(&text).unwrap();
        assert_eq!(original.preamp_db, reparsed.preamp_db);
        assert_eq!(original.description, reparsed.description);
        assert_eq!(original.speakers.len(), reparsed.speakers.len());
        for (a, b) in original.speakers.iter().zip(reparsed.speakers.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.targets, b.targets);
            assert_eq!(a.bands.len(), b.bands.len());
            for (x, y) in a.bands.iter().zip(b.bands.iter()) {
                assert!((x.freq_hz - y.freq_hz).abs() < 0.01);
                assert!((x.gain_db - y.gain_db).abs() < 0.01);
                assert_eq!(x.filter, y.filter);
                assert_eq!(x.enabled, y.enabled);
            }
        }
    }

    #[test]
    fn written_file_uses_crlf_and_sections() {
        let preset = Preset::default();
        let text = to_peace_string(&preset);
        assert!(text.contains("\r\n"));
        assert!(text.contains("[Speakers]"));
        assert!(text.contains("SpeakerTargets0=all"));
        assert!(!text.contains("[Frequencies]\r\nFrequency0"));
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(parse_preset("   \n\n").is_err());
    }

    #[test]
    fn quoted_values_are_unquoted() {
        let ini = Ini::parse("[A]\nKey=\"hello world\"\n");
        assert_eq!(ini.find("A", "Key"), Some("hello world"));
    }

    #[test]
    fn numbers_accept_decimal_commas() {
        assert_eq!(parse_number("2,5"), Some(2.5));
        assert_eq!(parse_number(" 6 dB "), Some(6.0));
    }

    #[test]
    fn disabled_bands_render_into_disabled_section() {
        let mut preset = Preset::default();
        preset.speakers[0].bands[0].enabled = false;
        let text = to_peace_string(&preset);
        assert!(text.contains("[Disabled]"));
        assert!(text.contains("Disabled1=1"));
    }
}
