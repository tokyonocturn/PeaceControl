//! The PeaceControl preset model.
//!
//! A preset describes everything PeaceControl needs in order to write an
//! Equalizer APO configuration: one EQ bank per speaker group, global
//! effects, raw command lines and an optional surround stage.
//!
//! The model is deliberately independent from the wire formats. [`crate::peace_ini`]
//! maps it to and from the interoperable `*.peace` INI dialect, and
//! [`crate::render`] maps it to Equalizer APO commands.

use serde::{Deserialize, Serialize};

pub const MAX_BANDS: usize = 31;
pub const MIN_FREQ_HZ: f32 = 10.0;
pub const MAX_FREQ_HZ: f32 = 20_000.0;
pub const MIN_GAIN_DB: f32 = -30.0;
pub const MAX_GAIN_DB: f32 = 30.0;
pub const MIN_Q: f32 = 0.1;
pub const MAX_Q: f32 = 40.0;

/// Filter shapes available in Equalizer APO, in the order the UI lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilterType {
    /// Bell shaped boost or cut. The default for every band.
    #[default]
    Peak,
    LowPass,
    HighPass,
    /// Low pass with an explicit Q (Equalizer APO `LPQ`).
    LowPassQ,
    /// High pass with an explicit Q (Equalizer APO `HPQ`).
    HighPassQ,
    BandPass,
    LowShelf,
    HighShelf,
    LowShelf6,
    HighShelf6,
    LowShelf12,
    HighShelf12,
    Notch,
    AllPass,
}

impl FilterType {
    pub const ALL: [FilterType; 14] = [
        FilterType::Peak,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::LowShelf6,
        FilterType::HighShelf6,
        FilterType::LowShelf12,
        FilterType::HighShelf12,
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::LowPassQ,
        FilterType::HighPassQ,
        FilterType::BandPass,
        FilterType::Notch,
        FilterType::AllPass,
    ];

    /// The token Equalizer APO expects after `Filter: ON `.
    pub fn token(self) -> &'static str {
        match self {
            FilterType::Peak => "PK",
            FilterType::LowPass => "LP",
            FilterType::HighPass => "HP",
            FilterType::LowPassQ => "LPQ",
            FilterType::HighPassQ => "HPQ",
            FilterType::BandPass => "BP",
            FilterType::LowShelf => "LS",
            FilterType::HighShelf => "HS",
            FilterType::LowShelf6 => "LS 6dB",
            FilterType::HighShelf6 => "HS 6dB",
            FilterType::LowShelf12 => "LS 12dB",
            FilterType::HighShelf12 => "HS 12dB",
            FilterType::Notch => "NOTCH",
            FilterType::AllPass => "AP",
        }
    }

    /// Human readable name used in the interface.
    pub fn label(self) -> &'static str {
        match self {
            FilterType::Peak => "Peak",
            FilterType::LowPass => "Low pass",
            FilterType::HighPass => "High pass",
            FilterType::LowPassQ => "Low pass (Q)",
            FilterType::HighPassQ => "High pass (Q)",
            FilterType::BandPass => "Band pass",
            FilterType::LowShelf => "Low shelf",
            FilterType::HighShelf => "High shelf",
            FilterType::LowShelf6 => "Low shelf 6 dB",
            FilterType::HighShelf6 => "High shelf 6 dB",
            FilterType::LowShelf12 => "Low shelf 12 dB",
            FilterType::HighShelf12 => "High shelf 12 dB",
            FilterType::Notch => "Notch",
            FilterType::AllPass => "All pass",
        }
    }

    /// Short explanation shown next to the filter picker.
    pub fn hint(self) -> &'static str {
        match self {
            FilterType::Peak => "Bell shaped boost or cut around the band frequency.",
            FilterType::LowPass => "Keeps everything below the frequency, rolls off above it.",
            FilterType::HighPass => "Keeps everything above the frequency, rolls off below it.",
            FilterType::LowPassQ => "Low pass with an explicit resonance (Q).",
            FilterType::HighPassQ => "High pass with an explicit resonance (Q).",
            FilterType::BandPass => "Keeps a band around the frequency and rejects the rest.",
            FilterType::LowShelf => "Boosts or cuts everything below the frequency.",
            FilterType::HighShelf => "Boosts or cuts everything above the frequency.",
            FilterType::LowShelf6 => "Gentle low shelf with a 6 dB/octave slope.",
            FilterType::HighShelf6 => "Gentle high shelf with a 6 dB/octave slope.",
            FilterType::LowShelf12 => "Steeper low shelf with a 12 dB/octave slope.",
            FilterType::HighShelf12 => "Steeper high shelf with a 12 dB/octave slope.",
            FilterType::Notch => "Narrow rejection of a single frequency.",
            FilterType::AllPass => "Shifts phase only; the level stays untouched.",
        }
    }

    /// Whether the band gain has any effect for this shape.
    pub fn uses_gain(self) -> bool {
        !matches!(
            self,
            FilterType::LowPass
                | FilterType::HighPass
                | FilterType::LowPassQ
                | FilterType::HighPassQ
                | FilterType::BandPass
                | FilterType::Notch
                | FilterType::AllPass
        )
    }

    /// Whether the quality factor matters for this shape.
    pub fn uses_q(self) -> bool {
        !matches!(self, FilterType::LowShelf6 | FilterType::HighShelf6)
    }

    /// The integer Peace writes into `Filter{n}` keys of a `.peace` file.
    pub fn peace_code(self) -> i32 {
        match self {
            FilterType::Peak => 0,
            FilterType::LowPass => 1,
            FilterType::HighPass => 2,
            FilterType::BandPass => 3,
            FilterType::LowShelf => 4,
            FilterType::HighShelf => 5,
            FilterType::Notch => 6,
            FilterType::AllPass => 7,
            FilterType::LowShelf12 => 8,
            FilterType::HighShelf12 => 9,
            FilterType::LowPassQ => 10,
            FilterType::HighPassQ => 11,
            FilterType::LowShelf6 => 14,
            FilterType::HighShelf6 => 15,
        }
    }

    /// Inverse of [`FilterType::peace_code`].
    ///
    /// Codes 12 and 13 have been observed in shipped presets for low pass
    /// variants; both are folded onto the closest supported shape so that
    /// foreign presets keep loading.
    pub fn from_peace_code(code: i32) -> FilterType {
        match code {
            1 | 12 | 13 => FilterType::LowPass,
            2 => FilterType::HighPass,
            3 => FilterType::BandPass,
            4 => FilterType::LowShelf,
            5 => FilterType::HighShelf,
            6 => FilterType::Notch,
            7 => FilterType::AllPass,
            8 => FilterType::LowShelf12,
            9 => FilterType::HighShelf12,
            10 => FilterType::LowPassQ,
            11 => FilterType::HighPassQ,
            14 => FilterType::LowShelf6,
            15 => FilterType::HighShelf6,
            _ => FilterType::Peak,
        }
    }
}

/// One adjustable equalizer band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub freq_hz: f32,
    pub gain_db: f32,
    #[serde(default = "default_q")]
    pub q: f32,
    #[serde(default)]
    pub filter: FilterType,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_q() -> f32 {
    1.41
}

fn default_true() -> bool {
    true
}

impl Default for Band {
    fn default() -> Self {
        Band {
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 1.41,
            filter: FilterType::Peak,
            enabled: true,
        }
    }
}

impl Band {
    pub fn new(freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Band {
            freq_hz,
            gain_db,
            q,
            filter: FilterType::Peak,
            enabled: true,
        }
    }

    /// Clamps every field into the range the interface and Equalizer APO accept.
    pub fn sanitized(&self) -> Band {
        Band {
            freq_hz: self.freq_hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ),
            gain_db: self.gain_db.clamp(MIN_GAIN_DB, MAX_GAIN_DB),
            q: self.q.clamp(MIN_Q, MAX_Q),
            filter: self.filter,
            enabled: self.enabled,
        }
    }

    /// True when this band leaves the signal unchanged.
    pub fn is_neutral(&self) -> bool {
        !self.enabled
            || (!self.filter.uses_gain() && self.gain_db.abs() < 0.05)
            || self.gain_db.abs() < 0.05
    }
}

/// One EQ bank: every band that applies to a speaker or speaker group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeakerEq {
    pub name: String,
    /// Equalizer APO channel list, for example `L R` or `all`.
    pub targets: String,
    pub preamp_db: f32,
    pub bands: Vec<Band>,
}

impl SpeakerEq {
    pub fn new(name: impl Into<String>, targets: impl Into<String>) -> Self {
        SpeakerEq {
            name: name.into(),
            targets: targets.into(),
            preamp_db: 0.0,
            bands: Vec::new(),
        }
    }

    /// Every band is at 0 dB (or the bank has no bands at all).
    pub fn is_flat(&self) -> bool {
        self.bands.iter().all(|b| b.gain_db.abs() < 0.05) && self.preamp_db.abs() < 0.05
    }

    pub fn set_band_count(&mut self, count: usize) {
        let count = count.clamp(1, MAX_BANDS);
        while self.bands.len() > count {
            self.bands.pop();
        }
        while self.bands.len() < count {
            let freq = default_frequency(self.bands.len(), count);
            self.bands.push(Band::new(freq, 0.0, 1.41));
        }
        self.bands.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
    }

    /// Spreads `total` bands logarithmically between 10 Hz and 20 kHz.
    pub fn with_log_bands(mut self, total: usize) -> Self {
        self.bands = log_bands(total);
        self
    }
}

/// Default centre frequency for band `index` out of `count`, mirroring the
/// octave spacing used by the stock Peace configurations.
pub fn default_frequency(index: usize, count: usize) -> f32 {
    if count <= 1 {
        return 1000.0;
    }
    let lo = MIN_FREQ_HZ.log10();
    let hi = MAX_FREQ_HZ.log10();
    let t = index as f32 / (count - 1) as f32;
    round_frequency(10f32.powf(lo + (hi - lo) * t))
}

/// Rounds to a friendly value so the interface shows readable frequencies.
pub fn round_frequency(freq: f32) -> f32 {
    let freq = freq.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ);
    if freq >= 1000.0 {
        (freq / 10.0).round() * 10.0
    } else if freq >= 100.0 {
        (freq / 5.0).round() * 5.0
    } else {
        freq.round()
    }
}

/// Builds `count` logarithmically spaced bands with 0 dB gain.
pub fn log_bands(count: usize) -> Vec<Band> {
    let count = count.clamp(1, MAX_BANDS);
    (0..count)
        .map(|i| Band::new(default_frequency(i, count), 0.0, 1.41))
        .collect()
}

/// Standard 1/3 octave centre frequencies (31 bands, 20 Hz to 20 kHz).
pub fn third_octave_frequencies() -> Vec<f32> {
    vec![
        20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
        500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0,
        6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
    ]
}

/// A simple bass or treble control rendered as a shelving filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneControl {
    pub enabled: bool,
    pub gain_db: f32,
    pub freq_hz: f32,
}

impl Default for ToneControl {
    fn default() -> Self {
        ToneControl {
            enabled: false,
            gain_db: 0.0,
            freq_hz: 200.0,
        }
    }
}

/// Global effects that are rendered after the equalizer banks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effects {
    pub crossfeed: bool,
    pub crossfeed_db: f32,
    pub fake_stereo: bool,
    pub fake_stereo_delay_ms: f32,
    pub fake_stereo_freq_hz: f32,
    pub echo: bool,
    pub echo_count: u32,
    pub echo_delay_ms: f32,
    pub swap_speakers: bool,
    pub reverse_channels: bool,
    pub upmix: bool,
    pub downmix_51: bool,
    pub downmix_71: bool,
    pub delay_ms: f32,
    pub bass: ToneControl,
    pub treble: ToneControl,
    pub graphic_eq: bool,
    /// Raw Equalizer APO routing expression, for example `R=1.0*L C=1.0*L`.
    pub routing: String,
}

impl Default for Effects {
    fn default() -> Self {
        Effects {
            crossfeed: false,
            crossfeed_db: -6.0,
            fake_stereo: false,
            fake_stereo_delay_ms: 20.0,
            fake_stereo_freq_hz: 1000.0,
            echo: false,
            echo_count: 2,
            echo_delay_ms: 250.0,
            swap_speakers: false,
            reverse_channels: false,
            upmix: false,
            downmix_51: false,
            downmix_71: false,
            delay_ms: 0.0,
            bass: ToneControl {
                freq_hz: 200.0,
                ..Default::default()
            },
            treble: ToneControl {
                freq_hz: 4000.0,
                ..Default::default()
            },
            graphic_eq: false,
            routing: String::new(),
        }
    }
}

impl Effects {
    /// True when at least one effect will add commands to the output.
    pub fn any_active(&self) -> bool {
        self.crossfeed
            || self.fake_stereo
            || self.echo
            || self.swap_speakers
            || self.reverse_channels
            || self.upmix
            || self.downmix_51
            || self.downmix_71
            || self.delay_ms.abs() > 0.001
            || self.bass.enabled
            || self.treble.enabled
            || self.graphic_eq
            || !self.routing.trim().is_empty()
    }

    /// Number of active effects, shown as a badge in the interface.
    pub fn active_count(&self) -> usize {
        [
            self.crossfeed,
            self.fake_stereo,
            self.echo,
            self.swap_speakers,
            self.reverse_channels,
            self.upmix,
            self.downmix_51,
            self.downmix_71,
            self.delay_ms.abs() > 0.001,
            self.bass.enabled,
            self.treble.enabled,
            self.graphic_eq,
            !self.routing.trim().is_empty(),
        ]
        .iter()
        .filter(|active| **active)
        .count()
    }
}

/// The optional surround stage (stereo enhancement for 5.1/7.1 playback).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surround {
    pub enabled: bool,
    pub bass_frequency_hz: f32,
    pub front_level: f32,
    pub center_level: f32,
    pub subwoofer_level: f32,
    pub side_level: f32,
    pub rear_level: f32,
    pub front_delay_ms: f32,
    pub subwoofer_delay_ms: f32,
    pub front_width: f32,
    pub side_width: f32,
    pub rear_width: f32,
}

impl Default for Surround {
    fn default() -> Self {
        Surround {
            enabled: false,
            bass_frequency_hz: 350.0,
            front_level: 1.0,
            center_level: 1.0,
            subwoofer_level: 1.0,
            side_level: 1.0,
            rear_level: 1.0,
            front_delay_ms: 0.0,
            subwoofer_delay_ms: 0.0,
            front_width: 0.0,
            side_width: 0.3,
            rear_width: 0.5,
        }
    }
}

/// A complete configuration: equalizer banks plus everything else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub description: String,
    /// Preamp applied once, before every speaker bank.
    pub preamp_db: f32,
    pub speakers: Vec<SpeakerEq>,
    pub effects: Effects,
    pub surround: Surround,
    /// Verbatim Equalizer APO command lines, one string per line.
    pub commands: Vec<String>,
    /// Optional labels for the five "effect sliders" of a configuration.
    pub slider_labels: Vec<String>,
    /// The `HotKey` value stored in the configuration, if any.
    pub hotkey: String,
}

impl Default for Preset {
    fn default() -> Self {
        Preset {
            name: "Untitled".to_string(),
            description: String::new(),
            preamp_db: 0.0,
            speakers: vec![SpeakerEq::new("All", "all").with_log_bands(13)],
            effects: Effects::default(),
            surround: Surround::default(),
            commands: Vec::new(),
            slider_labels: Vec::new(),
            hotkey: String::new(),
        }
    }
}

impl Preset {
    /// A new preset with `count` logarithmically spaced bands on the "All" bank.
    pub fn with_bands(count: usize) -> Self {
        let mut preset = Preset::default();
        preset.speakers[0] = SpeakerEq::new("All", "all").with_log_bands(count);
        preset
    }

    /// A new preset using the classic 31 band third octave layout.
    pub fn third_octave() -> Self {
        let mut preset = Preset::with_bands(1);
        preset.speakers[0].bands = third_octave_frequencies()
            .into_iter()
            .map(|f| Band::new(f, 0.0, 4.36))
            .collect();
        preset
    }

    pub fn active_speaker(&self, index: usize) -> &SpeakerEq {
        &self.speakers[index.min(self.speakers.len() - 1)]
    }

    pub fn active_speaker_mut(&mut self, index: usize) -> &mut SpeakerEq {
        let index = index.min(self.speakers.len() - 1);
        &mut self.speakers[index]
    }

    /// Adds a speaker bank with a generated name and the given channel targets.
    pub fn add_speaker(&mut self, targets: &str) -> usize {
        let name = speaker_name_for(targets, self.speakers.len());
        self.speakers.push(SpeakerEq::new(name, targets));
        self.speakers.len() - 1
    }

    /// True when nothing would alter the sound.
    pub fn is_neutral(&self) -> bool {
        self.preamp_db.abs() < 0.05
            && self.speakers.iter().all(|s| s.is_flat())
            && !self.effects.any_active()
            && !self.surround.enabled
            && self.commands_effective().is_empty()
    }

    /// Command lines without blank lines and comments.
    pub fn commands_effective(&self) -> Vec<&str> {
        self.commands
            .iter()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    /// Highest band count across all speaker banks.
    pub fn band_count(&self) -> usize {
        self.speakers
            .iter()
            .map(|s| s.bands.len())
            .max()
            .unwrap_or(0)
    }

    /// Applies `f` to every band of every speaker bank.
    pub fn for_each_band_mut(&mut self, mut f: impl FnMut(&mut Band)) {
        for speaker in &mut self.speakers {
            for band in &mut speaker.bands {
                f(band);
            }
        }
    }

    /// Sets every band to 0 dB and clears preamps, keeping the frequencies.
    pub fn flatten(&mut self) {
        self.preamp_db = 0.0;
        for speaker in &mut self.speakers {
            speaker.preamp_db = 0.0;
            for band in &mut speaker.bands {
                band.gain_db = 0.0;
                band.enabled = true;
            }
        }
    }

    /// Copies one speaker bank onto another, keeping the destination targets.
    pub fn copy_bands(&mut self, from: usize, to: usize) {
        if from == to || from >= self.speakers.len() || to >= self.speakers.len() {
            return;
        }
        let source = self.speakers[from].bands.clone();
        self.speakers[to].bands = source;
    }

    /// Shifts every gain by `delta` dB, clamping at the model limits.
    pub fn shift_gains(&mut self, delta: f32) {
        self.for_each_band_mut(|band| {
            band.gain_db = (band.gain_db + delta).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        });
    }

    /// Scales every gain by `factor`, which is how expand/compress is implemented.
    pub fn scale_gains(&mut self, factor: f32) {
        self.for_each_band_mut(|band| {
            band.gain_db = (band.gain_db * factor).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        });
    }

    /// Regenerates every frequency logarithmically over `count` bands.
    pub fn redistribute(&mut self, count: usize) {
        for speaker in &mut self.speakers {
            speaker.set_band_count(count);
        }
    }

    /// A short, single line summary used by the preset list.
    pub fn summary(&self) -> String {
        let bands = self.band_count();
        let speakers = self.speakers.len();
        if self.description.trim().is_empty() {
            format!("{bands} bands · {speakers} speaker banks")
        } else {
            self.description.clone()
        }
    }
}

/// Friendly default name for a channel target list.
pub fn speaker_name_for(targets: &str, index: usize) -> String {
    match targets.trim() {
        "all" => "All".to_string(),
        "L" => "Left".to_string(),
        "R" => "Right".to_string(),
        "C" => "Center".to_string(),
        "SUB" => "Subwoofer".to_string(),
        "L C R" => "Front".to_string(),
        "SL SR" => "Side".to_string(),
        "RL RR" => "Rear".to_string(),
        "L R" => "Stereo".to_string(),
        "" => format!("Speaker {}", index + 1),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_has_thirteen_bands() {
        let preset = Preset::default();
        assert_eq!(preset.band_count(), 13);
        assert!(preset.is_neutral());
    }

    #[test]
    fn log_bands_are_sorted_and_bounded() {
        let preset = Preset::with_bands(31);
        let bands = &preset.speakers[0].bands;
        assert_eq!(bands.len(), 31);
        assert!(bands.windows(2).all(|w| w[0].freq_hz <= w[1].freq_hz));
        assert!(bands
            .iter()
            .all(|b| b.freq_hz >= MIN_FREQ_HZ && b.freq_hz <= MAX_FREQ_HZ));
    }

    #[test]
    fn band_count_change_keeps_existing_gains_and_clears_new_ones() {
        let mut speaker = SpeakerEq::new("All", "all").with_log_bands(13);
        speaker.bands[0].gain_db = 6.0;
        speaker.set_band_count(5);
        assert_eq!(speaker.bands.len(), 5);
        assert!(speaker.bands.iter().any(|b| (b.gain_db - 6.0).abs() < 0.01));
        speaker.set_band_count(13);
        assert_eq!(speaker.bands.len(), 13);
        assert_eq!(
            speaker
                .bands
                .iter()
                .filter(|b| b.gain_db.abs() > 0.01)
                .count(),
            1
        );
    }

    #[test]
    fn filter_codes_round_trip() {
        for filter in FilterType::ALL {
            assert_eq!(FilterType::from_peace_code(filter.peace_code()), filter);
        }
    }

    #[test]
    fn flatten_clears_gains() {
        let mut preset = Preset::default();
        preset.speakers[0].bands[0].gain_db = -12.0;
        preset.speakers[0].preamp_db = -4.0;
        preset.flatten();
        assert!(preset.is_neutral());
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let band = Band {
            freq_hz: 1.0,
            gain_db: 99.0,
            q: 0.0,
            filter: FilterType::Peak,
            enabled: true,
        }
        .sanitized();
        assert_eq!(band.freq_hz, MIN_FREQ_HZ);
        assert_eq!(band.gain_db, MAX_GAIN_DB);
        assert_eq!(band.q, MIN_Q);
    }

    #[test]
    fn speaker_names_are_friendly() {
        assert_eq!(speaker_name_for("L C R", 0), "Front");
        assert_eq!(speaker_name_for("SUB", 0), "Subwoofer");
        assert_eq!(speaker_name_for("L2 R2", 3), "L2 R2");
    }

    #[test]
    fn effects_badge_counts_active_effects() {
        let mut effects = Effects::default();
        assert_eq!(effects.active_count(), 0);
        effects.crossfeed = true;
        effects.bass.enabled = true;
        assert_eq!(effects.active_count(), 2);
        assert!(effects.any_active());
    }
}

impl Preset {
    /// Solos one band of one speaker bank: every other band in that bank is
    /// disabled so the rendered configuration contains only this filter.
    /// Calling it again when the bank is already soloed re-enables every band,
    /// which keeps the action reversible from the same control.
    ///
    /// Returns `true` when the bank ended up soloed.
    pub fn toggle_solo(&mut self, bank: usize, index: usize) -> bool {
        let Some(speaker) = self.speakers.get_mut(bank) else {
            return false;
        };
        if index >= speaker.bands.len() {
            return false;
        }
        let already_soloed = speaker.bands.iter().enumerate().all(|(position, band)| {
            if position == index {
                band.enabled
            } else {
                !band.enabled
            }
        });
        if already_soloed {
            for band in &mut speaker.bands {
                band.enabled = true;
            }
            false
        } else {
            for (position, band) in speaker.bands.iter_mut().enumerate() {
                band.enabled = position == index;
            }
            true
        }
    }
}

#[cfg(test)]
mod solo_tests {
    use super::*;

    #[test]
    fn solo_disables_the_other_bands_and_toggles_back() {
        let mut preset = Preset::with_bands(5);
        assert!(preset.toggle_solo(0, 2));
        let enabled: Vec<usize> = preset.speakers[0]
            .bands
            .iter()
            .enumerate()
            .filter(|(_, band)| band.enabled)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(enabled, vec![2]);
        assert!(!preset.toggle_solo(0, 2));
        assert!(preset.speakers[0].bands.iter().all(|band| band.enabled));
    }

    #[test]
    fn solo_ignores_missing_banks_and_bands() {
        let mut preset = Preset::with_bands(3);
        assert!(!preset.toggle_solo(9, 0));
        assert!(!preset.toggle_solo(0, 9));
    }
}
