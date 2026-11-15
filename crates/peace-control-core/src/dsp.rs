//! Small DSP helpers used by the response graph and by tests.
//!
//! PeaceControl does not process audio itself: Equalizer APO does. These
//! helpers only predict what the written filters will do, so the curve on
//! screen matches the sound the user hears.

use crate::model::{Band, FilterType};

/// Sample rate assumed when predicting the response. Equalizer APO runs at the
/// device rate; 48 kHz is the common case and keeps the curve accurate in the
/// audible range.
pub const ANALYSIS_SAMPLE_RATE: f32 = 48_000.0;

/// A normalised biquad section.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    /// Evaluates |H(e^jw)| at `freq_hz` and returns it in decibels.
    pub fn magnitude_db(&self, freq_hz: f32, sample_rate: f32) -> f64 {
        let w = 2.0 * std::f64::consts::PI * (freq_hz as f64) / (sample_rate as f64);
        let (sin1, cos1) = w.sin_cos();
        let (sin2, cos2) = (2.0 * w).sin_cos();

        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -(self.b1 * sin1 + self.b2 * sin2);
        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -(self.a1 * sin1 + self.a2 * sin2);

        let num = (num_re * num_re + num_im * num_im).sqrt();
        let den = (den_re * den_re + den_im * den_im).sqrt();
        if den <= f64::EPSILON {
            return 0.0;
        }
        let magnitude = (num / den).max(1e-9);
        20.0 * magnitude.log10()
    }

    /// The transfer function at DC and Nyquist, used by tests.
    fn dc_gain_db(&self) -> f64 {
        let num = self.b0 + self.b1 + self.b2;
        let den = 1.0 + self.a1 + self.a2;
        if den.abs() < f64::EPSILON {
            return 0.0;
        }
        20.0 * (num / den).abs().max(1e-9).log10()
    }
}

/// Builds the biquad for one band, following the RBJ audio EQ cookbook formulas
/// that Equalizer APO also uses.
pub fn band_biquad(band: &Band, sample_rate: f32) -> Biquad {
    let fs = sample_rate as f64;
    let freq = (band.freq_hz as f64).clamp(1.0, fs / 2.0 - 1.0);
    let w0 = 2.0 * std::f64::consts::PI * freq / fs;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let gain = band.gain_db as f64;
    let a = 10f64.powf(gain / 40.0);
    let q = (band.q as f64).clamp(0.05, 40.0);
    let alpha = sin_w0 / (2.0 * q);

    let (b0, b1, b2, a0, a1, a2) = match band.filter {
        FilterType::Peak => (
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        ),
        FilterType::LowPass | FilterType::LowPassQ => (
            (1.0 - cos_w0) / 2.0,
            1.0 - cos_w0,
            (1.0 - cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::HighPass | FilterType::HighPassQ => (
            (1.0 + cos_w0) / 2.0,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::BandPass => (
            alpha,
            0.0,
            -alpha,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::AllPass => (
            1.0 - alpha,
            -2.0 * cos_w0,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::LowShelf | FilterType::LowShelf6 | FilterType::LowShelf12 => {
            let s = q.clamp(0.1, 4.0);
            let beta = 2.0 * a.sqrt() * alpha;
            let _ = s;
            let _ = beta;
            shelf_low(a, cos_w0, sin_w0, q, sample_rate as f64, freq)
        }
        FilterType::HighShelf | FilterType::HighShelf6 | FilterType::HighShelf12 => {
            shelf_high(a, cos_w0, sin_w0, q, sample_rate as f64, freq)
        }
    };

    Biquad {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

fn shelf_low(a: f64, cos_w0: f64, sin_w0: f64, q: f64, fs: f64, freq: f64) -> (f64, f64, f64, f64, f64, f64) {
    let _ = fs;
    let _ = freq;
    let s = q.clamp(0.1, 4.0);
    let beta = 2.0 * a.sqrt() * (sin_w0 / (2.0 * (1.0 / s).sqrt().max(0.05)));
    let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + beta);
    let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
    let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - beta);
    let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + beta;
    let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
    let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - beta;
    (b0, b1, b2, a0, a1, a2)
}

fn shelf_high(a: f64, cos_w0: f64, sin_w0: f64, q: f64, fs: f64, freq: f64) -> (f64, f64, f64, f64, f64, f64) {
    let _ = fs;
    let _ = freq;
    let s = q.clamp(0.1, 4.0);
    let beta = 2.0 * a.sqrt() * (sin_w0 / (2.0 * (1.0 / s).sqrt().max(0.05)));
    let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + beta);
    let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
    let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - beta);
    let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + beta;
    let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
    let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - beta;
    (b0, b1, b2, a0, a1, a2)
}

/// Predicted gain of one band at `freq_hz`, in dB.
pub fn band_gain_db(band: &Band, freq_hz: f32) -> f64 {
    band_biquad(band, ANALYSIS_SAMPLE_RATE).magnitude_db(freq_hz, ANALYSIS_SAMPLE_RATE)
}

/// Sum of every enabled band at `freq_hz`, in dB.
pub fn response_db(bands: &[Band], freq_hz: f32) -> f64 {
    bands
        .iter()
        .filter(|band| band.enabled)
        .map(|band| band_gain_db(band, freq_hz))
        .sum()
}

/// A response curve sampled on `points` logarithmically spaced frequencies.
pub fn response_curve(bands: &[Band], first_hz: f32, last_hz: f32, points: usize) -> Vec<(f32, f64)> {
    let points = points.max(2);
    let lo = first_hz.max(1.0).log10() as f64;
    let hi = last_hz.max(first_hz + 1.0).log10() as f64;
    (0..points)
        .map(|i| {
            let t = i as f64 / (points - 1) as f64;
            let freq = 10f64.powf(lo + (hi - lo) * t) as f32;
            (freq, response_db(bands, freq))
        })
        .collect()
}

/// A `GraphicEQ:` point list for the current gains, in the 1/12 octave
/// resolution Equalizer APO interpolates linearly.
pub fn graphic_eq_points(bands: &[Band]) -> Vec<(f32, f64)> {
    let mut points = Vec::new();
    for band in bands.iter().filter(|band| band.enabled) {
        let value = band_gain_db(band, band.freq_hz);
        if value.abs() > 0.01 {
            points.push((band.freq_hz, value));
        }
    }
    if points.is_empty() {
        points.push((20.0, 0.0));
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points
}

/// Peak absolute gain of a bank, used to warn about clipping.
pub fn peak_gain_db(bands: &[Band]) -> f64 {
    response_curve(bands, 10.0, 20_000.0, 240)
        .into_iter()
        .map(|(_, db)| db)
        .fold(f64::MIN, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} ±{tolerance}, got {actual}"
        );
    }

    #[test]
    fn peak_band_hits_its_gain_at_centre() {
        let band = Band::new(1000.0, 6.0, 1.41);
        assert_close(band_gain_db(&band, 1000.0), 6.0, 0.1);
    }

    #[test]
    fn peak_cut_is_negative() {
        let band = Band::new(1000.0, -9.0, 1.0);
        assert_close(band_gain_db(&band, 1000.0), -9.0, 0.1);
    }

    #[test]
    fn peak_band_falls_away_from_centre() {
        let band = Band::new(1000.0, 12.0, 4.0);
        let centre = band_gain_db(&band, 1000.0);
        let far = band_gain_db(&band, 8000.0);
        assert!(far < centre - 6.0);
    }

    #[test]
    fn low_pass_is_minus_three_at_cutoff() {
        let band = Band {
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 0.7071,
            filter: FilterType::LowPass,
            enabled: true,
        };
        assert_close(band_gain_db(&band, 1000.0), -3.01, 0.4);
        assert!(band_gain_db(&band, 8000.0) < -30.0);
    }

    #[test]
    fn disabled_bands_are_ignored() {
        let mut band = Band::new(1000.0, 12.0, 1.41);
        band.enabled = false;
        assert_close(response_db(&[band], 1000.0), 0.0, 0.001);
    }

    #[test]
    fn low_shelf_lifts_only_the_bottom() {
        let band = Band {
            freq_hz: 200.0,
            gain_db: 6.0,
            q: 0.7,
            filter: FilterType::LowShelf,
            enabled: true,
        };
        let low = band_gain_db(&band, 20.0);
        let high = band_gain_db(&band, 10_000.0);
        assert!(low > 4.0, "low end should be lifted, got {low}");
        assert!(high.abs() < 0.6, "high end should be flat, got {high}");
    }

    #[test]
    fn high_shelf_lifts_only_the_top() {
        let band = Band {
            freq_hz: 4000.0,
            gain_db: -6.0,
            q: 0.7,
            filter: FilterType::HighShelf,
            enabled: true,
        };
        let low = band_gain_db(&band, 50.0);
        let high = band_gain_db(&band, 18_000.0);
        assert!(low.abs() < 0.6, "low end should be flat, got {low}");
        assert!(high < -4.0, "top end should be cut, got {high}");
    }

    #[test]
    fn bands_sum_in_db_space() {
        let a = Band::new(100.0, 3.0, 1.41);
        let b = Band::new(100.0, 4.0, 1.41);
        assert_close(response_db(&[a, b], 100.0), 7.0, 0.2);
    }

    #[test]
    fn response_curve_is_monotonic_in_frequency() {
        let bands = crate::model::log_bands(13);
        let curve = response_curve(&bands, 10.0, 20_000.0, 64);
        assert_eq!(curve.len(), 64);
        assert!(curve.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn graphic_eq_points_skip_flat_bands() {
        let bands = vec![Band::new(100.0, 0.0, 1.0), Band::new(1000.0, 5.0, 1.0)];
        let points = graphic_eq_points(&bands);
        assert_eq!(points.len(), 1);
        assert!((points[0].0 - 1000.0).abs() < 0.01);
    }

    #[test]
    fn peak_gain_reports_the_highest_boost() {
        let bands = vec![Band::new(100.0, 8.0, 1.41)];
        assert!(peak_gain_db(&bands) > 7.0);
    }

    #[test]
    fn shelf_dc_gain_approaches_the_requested_lift() {
        let band = Band {
            freq_hz: 200.0,
            gain_db: 6.0,
            q: 0.7,
            filter: FilterType::LowShelf,
            enabled: true,
        };
        let biquad = band_biquad(&band, ANALYSIS_SAMPLE_RATE);
        assert!(biquad.dc_gain_db() > 4.0);
    }
}
