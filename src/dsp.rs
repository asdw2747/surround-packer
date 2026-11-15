use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::model::MAX_CHANNELS;

pub const LIMITER_CEILING_DB: f32 = -0.3;

pub const GATE_ATTACK_MS: f32 = 4.0;
pub const GATE_HOLD_MS: f32 = 150.0;
pub const GATE_THRESHOLD_MIN_DB: f32 = -60.0;
pub const GATE_THRESHOLD_MAX_DB: f32 = -15.0;
pub const GATE_RELEASE_MIN_MS: f32 = 50.0;
pub const GATE_RELEASE_MAX_MS: f32 = 600.0;
pub const GATE_DEFAULT_THRESHOLD_DB: f32 = -32.0;
pub const GATE_DEFAULT_RELEASE_MS: f32 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GateSettings {
    pub enabled: bool,
    pub threshold_db: f32,
    pub release_ms: f32,
}

impl Default for GateSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: GATE_DEFAULT_THRESHOLD_DB,
            release_ms: GATE_DEFAULT_RELEASE_MS,
        }
    }
}

impl GateSettings {
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            threshold_db: self
                .threshold_db
                .clamp(GATE_THRESHOLD_MIN_DB, GATE_THRESHOLD_MAX_DB),
            release_ms: self
                .release_ms
                .clamp(GATE_RELEASE_MIN_MS, GATE_RELEASE_MAX_MS),
        }
    }
}

/// Apply a smooth RMS/peak-hybrid noise gate in place.
///
/// The detector uses a fast peak envelope blended with a short RMS window.
/// The gate opens with a 4 ms attack, stays open for a 150 ms hold after the
/// signal falls below the threshold, then releases exponentially. The release
/// parameter is the time to roughly -43 dB, so the tail reaches silence a
/// little after `release_ms`.
///
/// Returns the maximum attenuation in dB (0 means the signal passed at unity).
pub fn apply_noise_gate(
    samples: &mut [f32],
    sample_rate: f32,
    threshold_db: f32,
    release_ms: f32,
) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let settings = GateSettings {
        enabled: true,
        threshold_db,
        release_ms,
    }
    .clamped();
    let sr = sample_rate.max(1.0) as f64;
    let threshold = db_to_lin(settings.threshold_db) as f64;

    let attack_coeff = (-1.0 / (sr * (GATE_ATTACK_MS as f64 / 1000.0))).exp();
    let release_tau = (settings.release_ms as f64 / 5.0 / 1000.0 * sr).max(1.0);
    let release_coeff = (-1.0 / release_tau).exp();
    let peak_release = (-1.0 / (sr * 0.050)).exp();
    let rms_coeff = (-1.0 / (sr * 0.030)).exp();
    let hold_samples = (sr * GATE_HOLD_MS as f64 / 1000.0).round() as usize;

    let mut peak_env = 0.0f64;
    let mut rms_sq = 0.0f64;
    let mut gain = 0.0f64;
    let mut hold = 0usize;
    let mut has_opened = false;
    let mut min_gain = 1.0f64;

    for sample in samples.iter_mut() {
        let value = *sample as f64;
        let magnitude = value.abs();

        if magnitude > peak_env {
            peak_env = magnitude;
        } else {
            peak_env *= peak_release;
        }
        rms_sq = rms_coeff * rms_sq + (1.0 - rms_coeff) * value * value;
        let level = peak_env.max(rms_sq.sqrt());

        let target = if level >= threshold {
            hold = hold_samples;
            1.0
        } else if hold > 0 {
            hold -= 1;
            1.0
        } else {
            0.0
        };

        let coeff = if target > gain {
            attack_coeff
        } else {
            release_coeff
        };
        gain = target + (gain - target) * coeff;
        if gain >= 0.999 {
            has_opened = true;
        }
        // Ignore the brief start-up attack so a clean vocal does not latch the
        // activity indicator forever.
        if has_opened || level < threshold {
            min_gain = min_gain.min(gain);
        }
        if gain < 1e-5 {
            gain = 0.0;
        }
        *sample = (value * gain) as f32;
    }

    if !has_opened {
        min_gain = 0.0;
    }
    if min_gain <= 0.0 {
        60.0
    } else {
        ((-20.0 * min_gain.log10()) as f32).min(60.0)
    }
}

#[derive(Clone, Debug)]
pub struct PeakReport {
    pub pre_db: [f32; MAX_CHANNELS],
    pub post_db: [f32; MAX_CHANNELS],
    pub limiter_active: bool,
    /// Maximum gate attenuation per speaker, in dB. 0 means the gate passed the signal.
    pub gate_reduction_db: [f32; MAX_CHANNELS],
}

impl Default for PeakReport {
    fn default() -> Self {
        Self {
            pre_db: [f32::NEG_INFINITY; MAX_CHANNELS],
            post_db: [f32::NEG_INFINITY; MAX_CHANNELS],
            limiter_active: false,
            gate_reduction_db: [0.0; MAX_CHANNELS],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DspSettings {
    pub lfe_lowpass_enabled: bool,
    pub lfe_cutoff_hz: f32,
    pub surround_delay_enabled: bool,
    pub surround_delay_ms: f32,
    pub back_delay_ms: f32,
    pub limiter_enabled: bool,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            lfe_lowpass_enabled: true,
            lfe_cutoff_hz: 100.0,
            surround_delay_enabled: true,
            surround_delay_ms: 15.0,
            back_delay_ms: 22.0,
            limiter_enabled: true,
        }
    }
}

impl DspSettings {
    pub fn delay_samples(&self, sample_rate: u32) -> usize {
        self.surround_delay_samples(sample_rate)
    }

    pub fn surround_delay_samples(&self, sample_rate: u32) -> usize {
        if !self.surround_delay_enabled {
            return 0;
        }
        let ms = self.surround_delay_ms.clamp(0.0, 35.0);
        (ms * sample_rate as f32 / 1000.0).round() as usize
    }

    pub fn back_delay_samples(&self, sample_rate: u32) -> usize {
        if !self.surround_delay_enabled {
            return 0;
        }
        let ms = self.back_delay_ms.clamp(0.0, 35.0);
        (ms * sample_rate as f32 / 1000.0).round() as usize
    }
}

pub fn db_to_lin(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

pub fn lin_to_db(value: f32) -> f32 {
    if value <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * value.log10()
    }
}

#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    /// Second-order Butterworth low-pass (RBJ cookbook).
    fn lowpass(sample_rate: f32, cutoff_hz: f32) -> Self {
        let sr = sample_rate.max(1.0) as f64;
        let cutoff = (cutoff_hz as f64).clamp(1.0, sr * 0.45);
        let w0 = 2.0 * PI * cutoff / sr;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let q = std::f64::consts::FRAC_1_SQRT_2;
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let x = input as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }
}

/// Apply the LFE 2nd-order Butterworth low-pass in place.
pub fn apply_lfe_lowpass(samples: &mut [f32], sample_rate: f32, cutoff_hz: f32) {
    let mut filter = Biquad::lowpass(sample_rate, cutoff_hz);
    for sample in samples {
        *sample = filter.process(*sample);
    }
}

/// Delay a channel in place, filling the beginning with silence.
pub fn apply_delay(samples: &mut [f32], delay_samples: usize) {
    if delay_samples == 0 {
        return;
    }
    let len = samples.len();
    if delay_samples >= len {
        samples.fill(0.0);
        return;
    }
    samples.copy_within(0..len - delay_samples, delay_samples);
    samples[..delay_samples].fill(0.0);
}

fn soft_limit_sample(value: f32, ceiling: f32) -> f32 {
    let magnitude = value.abs();
    let knee = ceiling * 0.9;
    if magnitude <= knee {
        return value;
    }
    let range = (ceiling - knee).max(1e-9);
    let over = (magnitude - knee) / range;
    let limited = knee + range * over.tanh();
    value.signum() * limited
}

fn catmull_rom(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let t = t as f64;
    let y0 = y0 as f64;
    let y1 = y1 as f64;
    let y2 = y2 as f64;
    let y3 = y3 as f64;
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    (a0 * t * t * t + a1 * t * t + a2 * t + y1) as f32
}

pub fn sample_peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
}

/// 4x oversampled inter-sample peak estimate.
pub fn true_peak(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut peak = sample_peak(samples);
    if samples.len() < 2 {
        return peak;
    }
    for index in 0..samples.len() - 1 {
        let y0 = samples[index.saturating_sub(1)];
        let y1 = samples[index];
        let y2 = samples[index + 1];
        let y3 = samples.get(index + 2).copied().unwrap_or(y2);
        for t in [0.25, 0.5, 0.75] {
            peak = peak.max(catmull_rom(y0, y1, y2, y3, t).abs());
        }
    }
    peak
}

pub fn peak_dbfs(peak: f32) -> f32 {
    lin_to_db(peak)
}

/// Soft brickwall plus true-peak safety trim. Returns true if the limiter acted.
pub fn limit_channels(channels: &mut [Vec<f32>], ceiling_db: f32) -> bool {
    let ceiling = db_to_lin(ceiling_db).clamp(0.1, 0.999_999);
    let mut acted = false;

    for plane in channels.iter_mut() {
        for sample in plane.iter_mut() {
            if sample.abs() > ceiling * 0.9 {
                acted = true;
            }
            *sample = soft_limit_sample(*sample, ceiling);
        }
    }

    let max_true_peak = channels
        .iter()
        .map(|plane| true_peak(plane))
        .fold(0.0f32, f32::max);
    if max_true_peak > ceiling {
        acted = true;
        let trim = ceiling / max_true_peak;
        for plane in channels.iter_mut() {
            for sample in plane.iter_mut() {
                *sample *= trim;
            }
        }
    }

    acted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f32, sample_rate: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
        let frames = (sample_rate * seconds) as usize;
        (0..frames)
            .map(|frame| {
                let t = frame as f32 / sample_rate;
                amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn lfe_lowpass_attenuates_above_cutoff() {
        let sr = 48_000.0;
        let mut high = sine(1_000.0, sr, 1.0, 0.5);
        let before = rms(&high[24_000..]);
        apply_lfe_lowpass(&mut high, sr, 100.0);
        let after = rms(&high[24_000..]);
        assert!(
            after < before * 0.05,
            "1 kHz should be heavily attenuated: {before} -> {after}"
        );

        let mut above = sine(150.0, sr, 1.0, 0.5);
        let above_before = rms(&above[24_000..]);
        apply_lfe_lowpass(&mut above, sr, 100.0);
        let above_after = rms(&above[24_000..]);
        assert!(
            above_after < above_before * 0.6,
            "150 Hz should be attenuated above the 120 Hz band: {above_before} -> {above_after}"
        );

        let mut low = sine(50.0, sr, 1.0, 0.5);
        let low_before = rms(&low[24_000..]);
        apply_lfe_lowpass(&mut low, sr, 100.0);
        let low_after = rms(&low[24_000..]);
        assert!(
            low_after > low_before * 0.8,
            "50 Hz should pass: {low_before} -> {low_after}"
        );
    }

    #[test]
    fn fifteen_millisecond_delay_is_720_samples_at_48k() {
        let mut samples = vec![0.0f32; 4_000];
        samples[0] = 1.0;
        apply_delay(&mut samples, 720);
        assert!(samples[..720].iter().all(|sample| *sample == 0.0));
        assert!((samples[720] - 1.0).abs() < 1e-9);
        assert!(samples[721..].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn limiter_holds_true_peak_below_ceiling() {
        let mut channels: [Vec<f32>; MAX_CHANNELS] = std::array::from_fn(|_| Vec::new());
        channels[0] = sine(997.0, 48_000.0, 0.2, 0.99);
        channels[1] = sine(440.0, 48_000.0, 0.2, 0.8);
        channels[3] = vec![0.0; 9_600];
        channels[3][100] = 2.0;
        let ceiling = db_to_lin(LIMITER_CEILING_DB);
        assert!(limit_channels(&mut channels, LIMITER_CEILING_DB));
        for plane in &channels {
            let peak = true_peak(plane);
            assert!(
                peak <= ceiling + 1e-4,
                "peak {peak} exceeded ceiling {ceiling}"
            );
        }
    }

    #[test]
    fn peak_helpers_handle_silence() {
        assert_eq!(sample_peak(&[]), 0.0);
        assert_eq!(true_peak(&[]), 0.0);
        assert!(peak_dbfs(0.0).is_infinite());
    }

    #[test]
    fn noise_gate_silences_bleed_and_passes_vocals() {
        let sr = 48_000.0;
        let bleed = db_to_lin(-34.0);
        let vocal = db_to_lin(-12.0);
        let mut samples = sine(440.0, sr, 0.4, bleed);
        samples.extend(sine(440.0, sr, 0.4, vocal));
        samples.extend(std::iter::repeat_n(0.0, (sr * 0.4) as usize));

        let reduction = apply_noise_gate(
            &mut samples,
            sr,
            GATE_DEFAULT_THRESHOLD_DB,
            GATE_DEFAULT_RELEASE_MS,
        );

        let bleed_tail = &samples[(sr * 0.2) as usize..(sr * 0.4) as usize];
        assert!(
            sample_peak(bleed_tail) < 1e-4,
            "-34 dB bleed should be gated to silence, peak {}",
            sample_peak(bleed_tail)
        );

        let vocal_window = &samples[(sr * 0.6) as usize..(sr * 0.78) as usize];
        let expected = vocal / std::f32::consts::SQRT_2;
        let measured = rms(vocal_window);
        assert!(
            (measured - expected).abs() / expected < 0.02,
            "-12 dB vocals should pass at unity: expected {expected}, got {measured}"
        );

        let tail = &samples[(sr * 1.1) as usize..];
        assert!(
            sample_peak(tail) < 1e-4,
            "gate release should end in silence, peak {}",
            sample_peak(tail)
        );
        assert!(
            reduction > 20.0,
            "gating should report meaningful attenuation, got {reduction} dB"
        );
    }

    #[test]
    fn gate_threshold_and_release_are_adjustable() {
        let sr = 48_000.0;
        let quiet = db_to_lin(-35.0);

        let mut passing = sine(440.0, sr, 0.5, quiet);
        apply_noise_gate(&mut passing, sr, -45.0, GATE_DEFAULT_RELEASE_MS);
        let passed = rms(&passing[(sr * 0.2) as usize..]);
        let expected = quiet / std::f32::consts::SQRT_2;
        assert!(
            (passed - expected).abs() / expected < 0.05,
            "-45 dB threshold should pass a -35 dB signal: expected {expected}, got {passed}"
        );

        let mut gated = sine(440.0, sr, 0.5, quiet);
        apply_noise_gate(&mut gated, sr, -25.0, GATE_DEFAULT_RELEASE_MS);
        assert!(
            sample_peak(&gated[(sr * 0.2) as usize..]) < 1e-4,
            "-25 dB threshold should gate a -35 dB signal"
        );

        let voice = db_to_lin(-12.0);
        // A quiet probe tone below the threshold makes the release envelope
        // audible: the gate's gain scales the probe after the vocal ends.
        let probe = db_to_lin(-45.0);
        let mut fast = sine(440.0, sr, 0.5, voice);
        fast.extend(sine(440.0, sr, 0.5, probe));
        let mut slow = fast.clone();
        apply_noise_gate(&mut fast, sr, GATE_DEFAULT_THRESHOLD_DB, 100.0);
        apply_noise_gate(&mut slow, sr, GATE_DEFAULT_THRESHOLD_DB, 500.0);

        // The detector and 150 ms hold delay the release start; by 0.85 s both
        // have been releasing for a while.
        let start = (sr * 0.85) as usize;
        let window = (sr * 0.02) as usize;
        let fast_tail = rms(&fast[start..start + window]);
        let slow_tail = rms(&slow[start..start + window]);
        assert!(
            slow_tail > fast_tail * 5.0,
            "500 ms release should retain more tail than 100 ms: {} vs {}",
            slow_tail,
            fast_tail
        );
        assert!(
            fast_tail < probe * 0.05,
            "100 ms release should be nearly closed by 0.85 s: {fast_tail}"
        );
        assert!(
            slow_tail > probe * 0.1,
            "500 ms release should still be open at 0.85 s: {slow_tail}"
        );
        let fast_end = rms(&fast[(sr * 0.95) as usize..]);
        let slow_end = rms(&slow[(sr * 0.95) as usize..]);
        assert!(
            fast_end < probe * 0.01,
            "100 ms release should reach silence before the buffer ends: {fast_end}"
        );
        assert!(
            slow_end > probe * 0.05,
            "500 ms release should still be fading at 0.95 s: {slow_end}"
        );
    }

    #[test]
    fn gate_settings_clamp_to_supported_ranges() {
        let settings = GateSettings {
            enabled: true,
            threshold_db: -90.0,
            release_ms: 10.0,
        }
        .clamped();
        assert_eq!(settings.threshold_db, GATE_THRESHOLD_MIN_DB);
        assert_eq!(settings.release_ms, GATE_RELEASE_MIN_MS);
        assert_eq!(GateSettings::default().threshold_db, -32.0);
        assert_eq!(GateSettings::default().release_ms, 250.0);
    }
}
