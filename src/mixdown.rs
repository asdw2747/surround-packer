use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};

use crate::audio::{Stem, PROJECT_RATE};
use crate::dsp::{self, db_to_lin, peak_dbfs, DspSettings, LIMITER_CEILING_DB};
use crate::model::{Channel, ChannelLayout, MAX_CHANNELS};
use crate::routing::{PanLaw, Slot};

#[derive(Clone)]
pub struct MixSnapshot {
    pub stems: HashMap<u64, Arc<Stem>>,
    pub slots: [Slot; MAX_CHANNELS],
    pub layout: ChannelLayout,
    pub dsp: DspSettings,
    pub pan_law: PanLaw,
}

impl MixSnapshot {
    pub fn has_audible_routes(&self) -> bool {
        let any_solo = self
            .layout
            .channels()
            .iter()
            .any(|channel| self.slots[channel.index()].solo);
        self.layout.channels().iter().any(|channel| {
            let slot = &self.slots[channel.index()];
            !slot.mute
                && (!any_solo || slot.solo)
                && slot
                    .routes
                    .iter()
                    .any(|route| !route.mute && self.stems.contains_key(&route.stem_id))
        })
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / PROJECT_RATE as f64
    }

    pub fn frames(&self) -> usize {
        let any_solo = self
            .layout
            .channels()
            .iter()
            .any(|channel| self.slots[channel.index()].solo);
        self.layout
            .channels()
            .iter()
            .map(|channel| &self.slots[channel.index()])
            .filter(|slot| !slot.mute && (!any_solo || slot.solo))
            .flat_map(|slot| slot.routes.iter())
            .filter(|route| !route.mute)
            .filter_map(|route| self.stems.get(&route.stem_id))
            .map(|stem| stem.frames())
            .max()
            .unwrap_or(0)
    }
}

pub struct Mix {
    pub layout: ChannelLayout,
    /// Active planes in layout/export order.
    pub planes: Vec<Vec<f32>>,
    pub pre_peaks_db: [f32; MAX_CHANNELS],
    pub post_peaks_db: [f32; MAX_CHANNELS],
    pub limiter_active: bool,
    pub gate_reduction_db: [f32; MAX_CHANNELS],
}

impl Mix {
    pub fn sample_rate(&self) -> u32 {
        PROJECT_RATE
    }

    pub fn frames(&self) -> usize {
        self.planes.first().map_or(0, Vec::len)
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / PROJECT_RATE as f64
    }

    pub fn channel(&self, channel: Channel) -> Option<&[f32]> {
        let index = self.layout.bus_index(channel)?;
        self.planes.get(index).map(Vec::as_slice)
    }

    pub fn peak_pre(&self, channel: Channel) -> f32 {
        self.pre_peaks_db[channel.index()]
    }

    pub fn peak_post(&self, channel: Channel) -> f32 {
        self.post_peaks_db[channel.index()]
    }

    /// Build the dedicated 5.1 bus used by legacy AC3 and DTS.
    /// In 7.1 mode BL/BR are folded into SL/SR at -3 dB (0.707).
    pub fn fold_to_5_1(&self) -> [Vec<f32>; 6] {
        let frames = self.frames();
        let channel_or_zero = |channel: Channel| -> &[f32] {
            self.channel(channel)
                .filter(|samples| samples.len() == frames)
                .unwrap_or(&[])
        };
        let copy = |channel: Channel| -> Vec<f32> {
            let source = channel_or_zero(channel);
            if source.len() == frames {
                source.to_vec()
            } else {
                vec![0.0; frames]
            }
        };

        let mut sl = copy(Channel::Sl);
        let mut sr = copy(Channel::Sr);
        if self.layout.is_71() {
            for (frame, sample) in sl.iter_mut().enumerate() {
                *sample += channel_or_zero(Channel::Bl)
                    .get(frame)
                    .copied()
                    .unwrap_or(0.0)
                    * std::f32::consts::FRAC_1_SQRT_2;
            }
            for (frame, sample) in sr.iter_mut().enumerate() {
                *sample += channel_or_zero(Channel::Br)
                    .get(frame)
                    .copied()
                    .unwrap_or(0.0)
                    * std::f32::consts::FRAC_1_SQRT_2;
            }
        }

        [
            copy(Channel::Fl),
            copy(Channel::Fr),
            copy(Channel::Fc),
            copy(Channel::Lfe),
            sl,
            sr,
        ]
    }

    /// Stereo headphone audition folddown. 7.1 adds BL/BR at 0.707.
    pub fn stereo_audition(&self) -> [Vec<f32>; 2] {
        let frames = self.frames();
        let sample = |channel: Channel, frame: usize| -> f32 {
            self.channel(channel)
                .and_then(|samples| samples.get(frame))
                .copied()
                .unwrap_or(0.0)
        };
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for frame in 0..frames {
            let center = sample(Channel::Fc, frame) * std::f32::consts::FRAC_1_SQRT_2;
            let lfe = sample(Channel::Lfe, frame) * 0.5;
            let mut l = sample(Channel::Fl, frame)
                + center
                + lfe
                + sample(Channel::Sl, frame) * std::f32::consts::FRAC_1_SQRT_2;
            let mut r = sample(Channel::Fr, frame)
                + center
                + lfe
                + sample(Channel::Sr, frame) * std::f32::consts::FRAC_1_SQRT_2;
            if self.layout.is_71() {
                l += sample(Channel::Bl, frame) * std::f32::consts::FRAC_1_SQRT_2;
                r += sample(Channel::Br, frame) * std::f32::consts::FRAC_1_SQRT_2;
            }
            left.push(l);
            right.push(r);
        }
        let mut stereo = vec![left, right];
        dsp::limit_channels(&mut stereo, LIMITER_CEILING_DB);
        let [left, right]: [Vec<f32>; 2] = stereo
            .try_into()
            .expect("stereo folddown always has two planes");
        [left, right]
    }
}

/// Render the active layout's channels from the routed slots and apply DSP.
pub fn render<F>(snapshot: &MixSnapshot, mut progress: F) -> Result<Mix>
where
    F: FnMut(f32),
{
    if !snapshot.has_audible_routes() {
        bail!("No stems are routed to audible speakers.");
    }

    let frames = snapshot.frames();
    if frames == 0 {
        bail!("The routed stems contain no audio samples.");
    }

    let active = snapshot.layout.channels();
    let any_solo = active
        .iter()
        .any(|channel| snapshot.slots[channel.index()].solo);
    let mut planes: Vec<Vec<f32>> = (0..active.len()).map(|_| vec![0.0f32; frames]).collect();
    let mut gate_reduction_db = [0.0f32; MAX_CHANNELS];

    for (bus, channel) in active.iter().enumerate() {
        let slot = &snapshot.slots[channel.index()];
        if !slot.mute && (!any_solo || slot.solo) {
            let channel_gain = db_to_lin(slot.gain_db.clamp(-24.0, 24.0));
            for route in &slot.routes {
                if route.mute {
                    continue;
                }
                let Some(stem) = snapshot.stems.get(&route.stem_id) else {
                    continue;
                };
                let Some(tap) = route.mode.tap(stem.channels.len(), snapshot.pan_law) else {
                    continue;
                };
                let gain = db_to_lin(route.gain_db.clamp(-60.0, 24.0)) * channel_gain;
                let source_frames = stem.frames().min(frames);
                let source = &stem.channels;
                let destination = &mut planes[bus];
                for (frame, sample) in destination.iter_mut().enumerate().take(source_frames) {
                    *sample += tap.read(source, frame) * gain;
                }
            }
            if slot.gate_enabled && !slot.routes.is_empty() {
                gate_reduction_db[channel.index()] = dsp::apply_noise_gate(
                    &mut planes[bus],
                    PROJECT_RATE as f32,
                    slot.gate_threshold_db,
                    slot.gate_release_ms,
                );
            }
        }
        progress((bus + 1) as f32 / (active.len() as f32 + 4.0));
    }

    if snapshot.dsp.lfe_lowpass_enabled {
        if let Some(index) = snapshot.layout.bus_index(Channel::Lfe) {
            dsp::apply_lfe_lowpass(
                &mut planes[index],
                PROJECT_RATE as f32,
                snapshot.dsp.lfe_cutoff_hz.clamp(80.0, 120.0),
            );
        }
    }
    progress(7.0 / 10.0);

    if snapshot.dsp.surround_delay_enabled {
        let surround_delay = snapshot.dsp.surround_delay_samples(PROJECT_RATE);
        if let Some(index) = snapshot.layout.bus_index(Channel::Sl) {
            dsp::apply_delay(&mut planes[index], surround_delay);
        }
        if let Some(index) = snapshot.layout.bus_index(Channel::Sr) {
            dsp::apply_delay(&mut planes[index], surround_delay);
        }
        if snapshot.layout.is_71() {
            let back_delay = snapshot.dsp.back_delay_samples(PROJECT_RATE);
            if let Some(index) = snapshot.layout.bus_index(Channel::Bl) {
                dsp::apply_delay(&mut planes[index], back_delay);
            }
            if let Some(index) = snapshot.layout.bus_index(Channel::Br) {
                dsp::apply_delay(&mut planes[index], back_delay);
            }
        }
    }
    progress(8.0 / 10.0);

    let mut pre_peaks_db = [f32::NEG_INFINITY; MAX_CHANNELS];
    for (bus, channel) in active.iter().enumerate() {
        pre_peaks_db[channel.index()] = peak_dbfs(dsp::true_peak(&planes[bus]));
    }

    let limiter_active = if snapshot.dsp.limiter_enabled {
        dsp::limit_channels(&mut planes, LIMITER_CEILING_DB)
    } else {
        false
    };
    progress(9.5 / 10.0);

    let mut post_peaks_db = [f32::NEG_INFINITY; MAX_CHANNELS];
    for (bus, channel) in active.iter().enumerate() {
        post_peaks_db[channel.index()] = peak_dbfs(dsp::true_peak(&planes[bus]));
    }
    progress(1.0);

    Ok(Mix {
        layout: snapshot.layout,
        planes,
        pre_peaks_db,
        post_peaks_db,
        limiter_active,
        gate_reduction_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChannelLayout;
    use crate::routing::{ChannelMode, Route};

    fn stem(id: u64, left: f32, right: f32) -> Arc<Stem> {
        Arc::new(Stem {
            id,
            path: format!("stem{id}.flac").into(),
            display_name: format!("stem{id}"),
            source_sample_rate: PROJECT_RATE,
            channels: vec![vec![left; 100], vec![right; 100]],
            waveform: Vec::new(),
        })
    }

    fn snapshot(layout: ChannelLayout) -> MixSnapshot {
        let stems = HashMap::from([(1, stem(1, 1.0, 0.5)), (2, stem(2, 0.25, 0.25))]);
        let mut slots = Slot::empty_slots();
        slots[Channel::Fl.index()].add_route(Route::new(
            1,
            "stem1.flac".into(),
            ChannelMode::StereoL,
            0.0,
        ));
        slots[Channel::Fr.index()].add_route(Route::new(
            1,
            "stem1.flac".into(),
            ChannelMode::StereoR,
            0.0,
        ));
        slots[Channel::Lfe.index()].add_route(Route::new(
            2,
            "stem2.flac".into(),
            ChannelMode::MonoSum,
            0.0,
        ));
        MixSnapshot {
            stems,
            slots,
            layout,
            dsp: DspSettings {
                lfe_lowpass_enabled: false,
                surround_delay_enabled: false,
                limiter_enabled: false,
                ..DspSettings::default()
            },
            pan_law: PanLaw::Minus3,
        }
    }

    #[test]
    fn routes_stereo_sources_to_smpte_planes() {
        let mix = render(&snapshot(ChannelLayout::Surround51), |_| {}).unwrap();
        assert!((mix.channel(Channel::Fl).unwrap()[0] - 1.0).abs() < 1e-6);
        assert!((mix.channel(Channel::Fr).unwrap()[0] - 0.5).abs() < 1e-6);
        assert!((mix.channel(Channel::Lfe).unwrap()[0] - 0.353_553).abs() < 1e-4);
        assert_eq!(mix.channel(Channel::Bl), None);
    }

    #[test]
    fn slot_gate_silences_low_level_bus_and_reports_reduction() {
        let mut snapshot = snapshot(ChannelLayout::Surround51);
        let fl = &mut snapshot.slots[Channel::Fl.index()];
        fl.routes[0].gain_db = -34.0;
        fl.gate_enabled = true;
        fl.gate_threshold_db = -32.0;
        fl.gate_release_ms = 250.0;

        let mix = render(&snapshot, |_| {}).unwrap();
        let front_left = mix.channel(Channel::Fl).unwrap();
        assert!(
            front_left.iter().all(|sample| sample.abs() < 1e-6),
            "gated low-level bus should be silent"
        );
        assert!(mix.gate_reduction_db[Channel::Fl.index()] > 20.0);
        assert!((mix.channel(Channel::Fr).unwrap()[0] - 0.5).abs() < 1e-6);
        assert_eq!(mix.gate_reduction_db[Channel::Fr.index()], 0.0);
    }

    fn impulse_stem(frames: usize) -> Arc<Stem> {
        let mut plane = vec![0.0f32; frames];
        plane[0] = 1.0;
        Arc::new(Stem {
            id: 9,
            path: "impulse.flac".into(),
            display_name: "impulse".to_string(),
            source_sample_rate: PROJECT_RATE,
            channels: vec![plane],
            waveform: Vec::new(),
        })
    }

    #[test]
    fn surround_and_back_delays_are_applied_in_render() {
        let mut input = snapshot(ChannelLayout::Surround51);
        for slot in &mut input.slots {
            slot.routes.clear();
        }
        input.stems.insert(9, impulse_stem(2_000));
        input.slots[Channel::Sl.index()].add_route(Route::new(
            9,
            "impulse.flac".into(),
            ChannelMode::LeftOnly,
            0.0,
        ));
        input.dsp = DspSettings {
            lfe_lowpass_enabled: false,
            surround_delay_enabled: true,
            surround_delay_ms: 15.0,
            back_delay_ms: 22.0,
            limiter_enabled: false,
            ..DspSettings::default()
        };
        let mix = render(&input, |_| {}).unwrap();
        let surround = mix.channel(Channel::Sl).unwrap();
        assert_eq!(surround[0], 0.0);
        assert!((surround[720] - 1.0).abs() < 1e-6);
        assert_eq!(surround[719], 0.0);

        let mut input = snapshot(ChannelLayout::Surround71);
        for slot in &mut input.slots {
            slot.routes.clear();
        }
        input.stems.insert(9, impulse_stem(2_000));
        input.slots[Channel::Bl.index()].add_route(Route::new(
            9,
            "impulse.flac".into(),
            ChannelMode::LeftOnly,
            0.0,
        ));
        input.dsp = DspSettings {
            lfe_lowpass_enabled: false,
            surround_delay_enabled: true,
            surround_delay_ms: 15.0,
            back_delay_ms: 22.0,
            limiter_enabled: false,
            ..DspSettings::default()
        };
        let mix = render(&input, |_| {}).unwrap();
        let back = mix.channel(Channel::Bl).unwrap();
        assert_eq!(back[0], 0.0);
        assert!((back[1_056] - 1.0).abs() < 1e-6);
        assert_eq!(back[1_055], 0.0);
    }

    #[test]
    fn render_limiter_reports_and_holds_the_ceiling() {
        let mut snapshot = snapshot(ChannelLayout::Surround51);
        snapshot.slots[Channel::Fl.index()].routes[0].gain_db = 6.0;
        snapshot.dsp.limiter_enabled = true;
        let mix = render(&snapshot, |_| {}).unwrap();
        assert!(mix.limiter_active);
        for channel in [Channel::Fl, Channel::Fr, Channel::Lfe] {
            let peak = peak_dbfs(dsp::true_peak(mix.channel(channel).unwrap()));
            assert!(
                peak <= LIMITER_CEILING_DB + 1e-3,
                "{} peak {peak} exceeded ceiling {LIMITER_CEILING_DB}",
                channel.short()
            );
        }
    }

    #[test]
    fn solo_silences_other_channels() {
        let mut snapshot = snapshot(ChannelLayout::Surround51);
        snapshot.slots[Channel::Fl.index()].solo = true;
        let mix = render(&snapshot, |_| {}).unwrap();
        assert!((mix.channel(Channel::Fl).unwrap()[0] - 1.0).abs() < 1e-6);
        assert_eq!(mix.channel(Channel::Fr).unwrap()[0], 0.0);
        assert_eq!(mix.channel(Channel::Lfe).unwrap()[0], 0.0);
    }

    #[test]
    fn empty_snapshot_is_rejected() {
        let mut snapshot = snapshot(ChannelLayout::Surround51);
        for slot in &mut snapshot.slots {
            slot.routes.clear();
        }
        assert!(render(&snapshot, |_| {}).is_err());
    }

    #[test]
    fn fold_71_to_51_adds_back_channels_at_minus_3db() {
        let mut snapshot = snapshot(ChannelLayout::Surround71);
        snapshot.stems.insert(3, stem(3, 1.0, 1.0));
        snapshot.slots[Channel::Bl.index()].add_route(Route::new(
            3,
            "stem3.flac".into(),
            ChannelMode::StereoL,
            0.0,
        ));
        snapshot.slots[Channel::Br.index()].add_route(Route::new(
            3,
            "stem3.flac".into(),
            ChannelMode::StereoR,
            0.0,
        ));
        snapshot.slots[Channel::Sl.index()].add_route(Route::new(
            2,
            "stem2.flac".into(),
            ChannelMode::StereoL,
            0.0,
        ));
        snapshot.slots[Channel::Sr.index()].add_route(Route::new(
            2,
            "stem2.flac".into(),
            ChannelMode::StereoR,
            0.0,
        ));
        let mix = render(&snapshot, |_| {}).unwrap();
        let folded = mix.fold_to_5_1();
        assert!((folded[4][0] - (0.25 + std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-4);
        assert!((folded[5][0] - (0.25 + std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-4);
        assert!((folded[0][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn stereo_audition_folds_back_channels_at_0707() {
        let mut snapshot = snapshot(ChannelLayout::Surround71);
        snapshot.stems.insert(3, stem(3, 1.0, 1.0));
        snapshot.slots[Channel::Bl.index()].add_route(Route::new(
            3,
            "stem3.flac".into(),
            ChannelMode::StereoL,
            0.0,
        ));
        snapshot.slots[Channel::Br.index()].add_route(Route::new(
            3,
            "stem3.flac".into(),
            ChannelMode::StereoR,
            0.0,
        ));
        let mix = render(&snapshot, |_| {}).unwrap();
        let [left, right] = mix.stereo_audition();
        assert!(left[0] > 0.7);
        assert!(right[0] > 0.7);
    }
}
