//! Built in configurations.
//!
//! These are original PeaceControl presets written from scratch. They cover
//! the common starting points: flat reference curves, tone shaping, speech
//! intelligibility, a subwoofer crossover and a stereo widening example.

use crate::model::*;

fn bass_lift() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Gentle Bass Lift".to_string();
    preset.description = "Adds 4 dB below 120 Hz without touching the midrange.".to_string();
    preset.preamp_db = -4.0;
    preset.speakers[0].bands = vec![
        Band {
            freq_hz: 60.0,
            gain_db: 4.0,
            q: 0.7,
            filter: FilterType::LowShelf,
            enabled: true,
        },
        Band::new(160.0, 1.5, 1.0),
        Band::new(1000.0, 0.0, 1.41),
        Band::new(4000.0, 0.0, 1.41),
        Band::new(10000.0, 0.0, 1.41),
    ];
    preset
}

fn treble_air() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Treble Air".to_string();
    preset.description = "A gentle high shelf lift for dull sounding headphones.".to_string();
    preset.preamp_db = -3.0;
    preset.speakers[0].bands = vec![
        Band {
            freq_hz: 8000.0,
            gain_db: 3.0,
            q: 0.7,
            filter: FilterType::HighShelf,
            enabled: true,
        },
        Band::new(2500.0, 1.0, 1.2),
        Band::new(300.0, -0.5, 1.4),
    ];
    preset
}

fn speech_clarity() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Speech Clarity".to_string();
    preset.description = "Cleans up podcast and film dialogue on small speakers.".to_string();
    preset.preamp_db = -4.0;
    preset.speakers[0].bands = vec![
        Band::new(180.0, -3.0, 1.2),
        Band::new(400.0, -1.5, 1.4),
        Band::new(3000.0, 3.5, 1.1),
        Band::new(5000.0, 2.0, 1.4),
        Band::new(9000.0, -1.0, 1.2),
    ];
    preset
}

fn late_night() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Late Night Listening".to_string();
    preset.description = "Tames deep bass and sharp treble at low volume.".to_string();
    preset.preamp_db = -2.0;
    preset.speakers[0].bands = vec![
        Band::new(45.0, -6.0, 0.9),
        Band::new(120.0, -2.0, 1.2),
        Band::new(800.0, 1.0, 1.4),
        Band::new(3000.0, 1.5, 1.2),
        Band::new(12000.0, -3.0, 0.9),
    ];
    preset
}

fn subwoofer_crossover() -> Preset {
    let mut preset = Preset::with_bands(1);
    preset.name = "Subwoofer Crossover 80 Hz".to_string();
    preset.description =
        "Splits the signal: low pass to the subwoofer, high pass to the main speakers.".to_string();
    preset.preamp_db = -2.0;
    preset.speakers = vec![
        SpeakerEq {
            name: "Subwoofer".to_string(),
            targets: "SUB".to_string(),
            preamp_db: 0.0,
            bands: vec![Band {
                freq_hz: 80.0,
                gain_db: 0.0,
                q: std::f32::consts::FRAC_1_SQRT_2,
                filter: FilterType::LowPassQ,
                enabled: true,
            }],
        },
        SpeakerEq {
            name: "Main speakers".to_string(),
            targets: "L R C".to_string(),
            preamp_db: 0.0,
            bands: vec![Band {
                freq_hz: 80.0,
                gain_db: 0.0,
                q: std::f32::consts::FRAC_1_SQRT_2,
                filter: FilterType::HighPassQ,
                enabled: true,
            }],
        },
    ];
    preset
}

fn wide_stereo() -> Preset {
    let mut preset = Preset::with_bands(5);
    preset.name = "Wide Stereo".to_string();
    preset.description =
        "Blends the channels slightly for a wider image on headphones.".to_string();
    preset.effects.fake_stereo = true;
    preset.effects.fake_stereo_delay_ms = 18.0;
    preset.effects.fake_stereo_freq_hz = 900.0;
    preset
}

fn loudness_contour() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Loudness Contour".to_string();
    preset.description = "Classic smile curve for quiet background listening.".to_string();
    preset.preamp_db = -6.0;
    preset.effects.bass = ToneControl {
        enabled: true,
        gain_db: 6.0,
        freq_hz: 120.0,
    };
    preset.effects.treble = ToneControl {
        enabled: true,
        gain_db: 3.0,
        freq_hz: 6000.0,
    };
    preset
}

fn reference_13() -> Preset {
    let mut preset = Preset::with_bands(13);
    preset.name = "Studio Flat 13".to_string();
    preset.description = "Neutral 13 band starting point, octave spaced.".to_string();
    preset
}

fn reference_31() -> Preset {
    let mut preset = Preset::third_octave();
    preset.name = "Third Octave 31".to_string();
    preset.description = "1/3 octave analyzer style grid, 20 Hz to 20 kHz.".to_string();
    preset
}

/// Every built in preset, in menu order.
pub fn builtin_presets() -> Vec<Preset> {
    vec![
        reference_13(),
        reference_31(),
        bass_lift(),
        treble_air(),
        speech_clarity(),
        late_night(),
        subwoofer_crossover(),
        wide_stereo(),
        loudness_contour(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_not_empty_and_named() {
        let presets = builtin_presets();
        assert!(presets.len() >= 8);
        assert!(presets.iter().all(|preset| !preset.name.trim().is_empty()));
        assert!(presets
            .iter()
            .all(|preset| !preset.description.trim().is_empty()));
    }

    #[test]
    fn library_names_are_unique() {
        let mut names: Vec<String> = builtin_presets()
            .into_iter()
            .map(|preset| preset.name)
            .collect();
        let before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn library_presets_are_inside_model_limits() {
        for preset in builtin_presets() {
            assert!(preset.band_count() <= MAX_BANDS);
            for speaker in &preset.speakers {
                for band in &speaker.bands {
                    assert!(band.freq_hz >= MIN_FREQ_HZ && band.freq_hz <= MAX_FREQ_HZ);
                    assert!(band.gain_db >= MIN_GAIN_DB && band.gain_db <= MAX_GAIN_DB);
                    assert!(band.q >= MIN_Q && band.q <= MAX_Q);
                }
            }
        }
    }

    #[test]
    fn flat_presets_render_without_filters() {
        let flat = builtin_presets()
            .into_iter()
            .find(|preset| preset.name == "Studio Flat 13")
            .unwrap();
        let text = crate::render::render_config(&flat, &Default::default());
        assert!(!text.contains("Filter:"));
    }

    #[test]
    fn crossover_uses_two_speaker_banks() {
        let preset = builtin_presets()
            .into_iter()
            .find(|preset| preset.name.starts_with("Subwoofer"))
            .unwrap();
        assert_eq!(preset.speakers.len(), 2);
        let text = crate::render::render_config(&preset, &Default::default());
        assert!(text.contains("Channel: SUB"));
        assert!(text.contains("LPQ"));
        assert!(text.contains("HPQ"));
    }
}
