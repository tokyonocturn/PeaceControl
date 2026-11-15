//! Converts a [`Preset`] into an Equalizer APO configuration file.
//!
//! The output is plain Equalizer APO command text. Nothing here talks to the
//! audio engine: writing the file is enough, Equalizer APO reloads it.

use crate::dsp;
use crate::model::*;

/// Knobs that control how much of the preset ends up in the file.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Write the global preamp line.
    pub include_preamp: bool,
    /// Write tone controls, routing and the effect stages.
    pub include_effects: bool,
    /// Append the raw command lines of the preset.
    pub include_commands: bool,
    /// Emit `GraphicEQ` convolution lines instead of biquad filters.
    pub graphic_eq: bool,
    /// Add a comment header with the preset name and generator version.
    pub header: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            include_preamp: true,
            include_effects: true,
            include_commands: true,
            graphic_eq: false,
            header: true,
        }
    }
}

impl RenderOptions {
    /// Options that follow the preset's own Graphic EQ switch.
    pub fn from_preset(preset: &Preset) -> Self {
        RenderOptions {
            graphic_eq: preset.effects.graphic_eq,
            ..Default::default()
        }
    }
}

/// Formats a number the way Equalizer APO expects it: no trailing zeros.
pub fn num(value: f32) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let rounded = (value * 1000.0).round() / 1000.0;
    if (rounded - rounded.round()).abs() < 1e-6 {
        format!("{}", rounded.round() as i64)
    } else {
        let text = format!("{rounded:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn filter_line(band: &Band) -> String {
    let band = band.sanitized();
    let mut line = format!(
        "Filter: ON {} Fc {} Hz",
        band.filter.token(),
        num(band.freq_hz)
    );
    if band.filter.uses_gain() {
        line.push_str(&format!(" Gain {} dB", num(band.gain_db)));
    }
    if band.filter.uses_q() {
        line.push_str(&format!(" Q {}", num(band.q)));
    }
    line
}

/// Formats one `GraphicEQ:` line from a set of points.
pub fn graphic_eq_line(points: &[(f32, f64)]) -> String {
    let mut text = String::from("GraphicEQ: ");
    for (index, (freq, gain)) in points.iter().enumerate() {
        if index > 0 {
            text.push_str("; ");
        }
        text.push_str(&format!("{} {}", num(*freq), num(*gain as f32)));
    }
    text
}

fn channel_line(targets: &str) -> String {
    let targets = targets.trim();
    if targets.is_empty() {
        "Channel: all".to_string()
    } else {
        format!("Channel: {targets}")
    }
}

fn render_tone(out: &mut Vec<String>, control: &ToneControl, shelf: FilterType) {
    if !control.enabled || control.gain_db.abs() < 0.01 {
        return;
    }
    let band = Band {
        freq_hz: control.freq_hz,
        gain_db: control.gain_db,
        q: 0.7,
        filter: shelf,
        enabled: true,
    };
    out.push(channel_line("all"));
    out.push(filter_line(&band));
}

fn render_effects(preset: &Preset, out: &mut Vec<String>) {
    let effects = &preset.effects;

    render_tone(out, &effects.bass, FilterType::LowShelf);
    render_tone(out, &effects.treble, FilterType::HighShelf);

    if !effects.routing.trim().is_empty() {
        out.push(format!("Copy: {}", effects.routing.trim()));
    }

    if effects.crossfeed {
        // Linear coefficient, because Equalizer APO accepts plain multipliers
        // and this keeps the expression readable.
        let amount = 10f32.powf(effects.crossfeed_db / 20.0).clamp(0.0, 1.0);
        out.push("Copy: L2=L R2=R".to_string());
        out.push(format!("Copy: L=L+{0}*R2 R=R+{0}*L2", num(amount)));
        out.push("Copy: L2=0 R2=0".to_string());
    }

    if effects.fake_stereo {
        out.push("Copy: M=0.5*L+0.5*R".to_string());
        out.push("Copy: L2=M R2=M".to_string());
        out.push(channel_line("L2"));
        if effects.fake_stereo_delay_ms.abs() > 0.001 {
            out.push(format!("Delay: {} ms", num(effects.fake_stereo_delay_ms)));
        }
        out.push(format!(
            "Filter: ON HP Fc {} Hz",
            num(effects.fake_stereo_freq_hz)
        ));
        out.push(channel_line("R2"));
        if effects.fake_stereo_delay_ms.abs() > 0.001 {
            out.push(format!("Delay: {} ms", num(effects.fake_stereo_delay_ms)));
        }
        out.push(format!(
            "Filter: ON LP Fc {} Hz",
            num(effects.fake_stereo_freq_hz)
        ));
        out.push(channel_line("all"));
        out.push("Copy: L=0.7*L+0.3*L2 R=0.7*R+0.3*R2".to_string());
    }

    if effects.echo && effects.echo_count > 0 {
        let count = effects.echo_count.min(4) as usize;
        let copies: Vec<String> = (1..=count)
            .map(|index| format!("L{0}=L R{0}=R", index + 1))
            .collect();
        out.push(format!("Copy: {}", copies.join(" ")));
        let mut mix = String::from("Copy: L=0.7*L");
        for index in 0..count {
            out.push(channel_line(&format!("L{} R{}", index + 2, index + 2)));
            out.push(format!(
                "Delay: {} ms",
                num(effects.echo_delay_ms * (index + 1) as f32)
            ));
            mix.push_str(&format!("+{}*L{}", 0.4 / (index + 1) as f32, index + 2));
        }
        out.push(channel_line("all"));
        out.push(format!("{mix} R=0.7*R"));
    }

    if effects.swap_speakers || effects.reverse_channels {
        out.push("Copy: L2=L R2=R".to_string());
        out.push("Copy: L=R2 R=L2".to_string());
    }

    if effects.upmix {
        out.push("Copy: SL=L SR=R RL=L RR=R".to_string());
    }

    if effects.downmix_51 {
        out.push("Copy: L=0.314*L+0.222*C+0.031*SUB+0.268*RL+0.164*RR".to_string());
        out.push("Copy: R=0.314*R+0.222*C+0.031*SUB+0.268*RR+0.164*RL".to_string());
    }

    if effects.downmix_71 {
        out.push(
            "Copy: L=0.222*L+0.157*C+0.022*SUB+0.189*RL+0.116*RR+0.203*SL+0.090*SR".to_string(),
        );
        out.push(
            "Copy: R=0.222*R+0.157*C+0.022*SUB+0.189*RR+0.116*RL+0.203*SR+0.090*SL".to_string(),
        );
    }

    if effects.delay_ms.abs() > 0.001 {
        out.push(format!("Delay: {} ms", num(effects.delay_ms)));
    }
}

fn render_surround(preset: &Preset, out: &mut Vec<String>) {
    let surround = &preset.surround;
    if !surround.enabled {
        return;
    }
    // A width controlled stereo to surround spread. Equalizer APO has no
    // built in surround matrix, so this is built from Copy/Delay/Preamp.
    let side = surround.side_width.clamp(0.0, 1.0);
    let rear = surround.rear_width.clamp(0.0, 1.0);
    out.push(format!(
        "Copy: SL={0}*L+{1}*R SR={0}*R+{1}*L",
        num(1.0 - side * 0.5),
        num(side * 0.5)
    ));
    out.push(format!(
        "Copy: RL={0}*L+{1}*R RR={0}*R+{1}*L",
        num(1.0 - rear * 0.5),
        num(rear * 0.5)
    ));
    if surround.side_level.abs() > 0.001 {
        out.push(channel_line("SL SR"));
        out.push(format!(
            "Preamp: {} dB",
            num(20.0 * surround.side_level.max(1e-3).log10())
        ));
    }
    if surround.rear_level.abs() > 0.001 {
        out.push(channel_line("RL RR"));
        out.push(format!(
            "Preamp: {} dB",
            num(20.0 * surround.rear_level.max(1e-3).log10())
        ));
    }
    out.push(channel_line("all"));
}

/// Renders the whole preset as Equalizer APO configuration text.
pub fn render_config(preset: &Preset, options: &RenderOptions) -> String {
    let mut lines: Vec<String> = Vec::new();

    if options.header {
        lines.push(format!(
            "# {} {} - {}",
            crate::PRODUCT_NAME,
            crate::VERSION,
            preset.name
        ));
        lines.push("# Generated configuration. Edit with PeaceControl.".to_string());
    }

    if options.include_preamp && preset.preamp_db.abs() > 0.0005 {
        lines.push(format!("Preamp: {} dB", num(preset.preamp_db)));
    }

    for speaker in &preset.speakers {
        let has_preamp = speaker.preamp_db.abs() > 0.0005;
        let active_bands: Vec<&Band> = speaker
            .bands
            .iter()
            .filter(|band| {
                band.enabled && (!band.filter.uses_gain() || band.gain_db.abs() > 0.0005)
            })
            .collect();
        if active_bands.is_empty() && !has_preamp {
            continue;
        }
        lines.push(channel_line(&speaker.targets));
        if has_preamp {
            lines.push(format!("Preamp: {} dB", num(speaker.preamp_db)));
        }
        if options.graphic_eq {
            let points = dsp::graphic_eq_points(&speaker.bands);
            lines.push(graphic_eq_line(&points));
        } else {
            for band in active_bands {
                lines.push(filter_line(band));
            }
        }
    }

    if options.include_effects {
        render_effects(preset, &mut lines);
        render_surround(preset, &mut lines);
    }

    if options.include_commands {
        for command in preset.commands_effective() {
            lines.push(command.to_string());
        }
    }

    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    text
}

/// Renders the response of one speaker bank as rows of `frequency, gain`.
pub fn render_response_csv(preset: &Preset, speaker_index: usize, points: usize) -> String {
    let speaker = preset.active_speaker(speaker_index);
    let mut text = String::from("frequency_hz,gain_db\n");
    for (freq, gain) in dsp::response_curve(&speaker.bands, 10.0, 20_000.0, points) {
        text.push_str(&format!("{}, {:.2}\n", num(freq), gain));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peace_ini;

    fn preset_with_bands() -> Preset {
        let mut preset = Preset::default();
        preset.name = "Test".to_string();
        preset.preamp_db = -3.0;
        preset.speakers[0].bands[0].freq_hz = 100.0;
        preset.speakers[0].bands[0].gain_db = 5.0;
        preset.speakers[0].bands[0].q = 1.0;
        preset
    }

    #[test]
    fn renders_preamp_and_filter_lines() {
        let text = render_config(&preset_with_bands(), &RenderOptions::default());
        assert!(text.contains("Preamp: -3 dB"));
        assert!(text.contains("Channel: all"));
        assert!(text.contains("Filter: ON PK Fc 100 Hz Gain 5 dB Q 1"));
        assert!(text.ends_with("\r\n"));
    }

    #[test]
    fn flat_preset_renders_no_filters() {
        let text = render_config(&Preset::default(), &RenderOptions::default());
        assert!(!text.contains("Filter:"));
    }

    #[test]
    fn disabled_bands_are_not_rendered() {
        let mut preset = preset_with_bands();
        preset.speakers[0].bands[0].enabled = false;
        let text = render_config(&preset, &RenderOptions::default());
        assert!(!text.contains("Filter:"));
    }

    #[test]
    fn speaker_targets_become_channel_lines() {
        let mut preset = preset_with_bands();
        preset
            .speakers
            .push(SpeakerEq::new("Front", "L C R").with_log_bands(3));
        preset.speakers[1].bands[0].gain_db = 4.0;
        let text = render_config(&preset, &RenderOptions::default());
        assert!(text.contains("Channel: L C R"));
    }

    #[test]
    fn effects_are_written_when_enabled() {
        let mut preset = preset_with_bands();
        preset.effects.crossfeed = true;
        preset.effects.swap_speakers = true;
        preset.effects.delay_ms = 12.0;
        let text = render_config(&preset, &RenderOptions::default());
        assert!(text.contains("Copy: L2=L R2=R"));
        assert!(text.contains("Delay: 12 ms"));
    }

    #[test]
    fn effects_can_be_skipped() {
        let mut preset = preset_with_bands();
        preset.effects.delay_ms = 12.0;
        let options = RenderOptions {
            include_effects: false,
            ..Default::default()
        };
        let text = render_config(&preset, &options);
        assert!(!text.contains("Delay:"));
    }

    #[test]
    fn graphic_eq_renders_a_graphic_line() {
        let mut preset = preset_with_bands();
        preset.effects.graphic_eq = true;
        let text = render_config(&preset, &RenderOptions::from_preset(&preset));
        assert!(text.contains("GraphicEQ: 100 5"));
        assert!(!text.contains("Filter: ON"));
    }

    #[test]
    fn commands_are_appended_verbatim() {
        let mut preset = preset_with_bands();
        preset.commands.push("Stage: post-mix".to_string());
        preset.commands.push("   ".to_string());
        let text = render_config(&preset, &RenderOptions::default());
        assert!(text.contains("Stage: post-mix"));
        assert!(!text.contains("\r\n\r\n"));
    }

    #[test]
    fn header_can_be_disabled() {
        let options = RenderOptions {
            header: false,
            ..Default::default()
        };
        let text = render_config(&Preset::default(), &options);
        assert!(!text.contains("PeaceControl"));
    }

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(5.0), "5");
        assert_eq!(num(-2.50), "-2.5");
        assert_eq!(num(1.414), "1.414");
        assert_eq!(num(0.0), "0");
    }

    #[test]
    fn render_output_can_be_imported_again() {
        let preset = preset_with_bands();
        let text = render_config(&preset, &RenderOptions::default());
        let parsed = crate::parse::parse_config(&text).unwrap();
        assert_eq!(parsed.preset.speakers.len(), 1);
        assert_eq!(parsed.preset.speakers[0].bands.len(), 1);
        assert!((parsed.preset.speakers[0].bands[0].freq_hz - 100.0).abs() < 0.5);
        assert!((parsed.preset.speakers[0].bands[0].gain_db - 5.0).abs() < 0.05);
    }

    #[test]
    fn rendered_config_round_trips_through_peace_ini() {
        let preset = preset_with_bands();
        let ini = peace_ini::to_peace_string(&preset);
        let back = peace_ini::parse_preset(&ini).unwrap();
        assert!((back.preamp_db - preset.preamp_db).abs() < 0.01);
    }
}
