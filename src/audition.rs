use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::audio::resample_plane;
use crate::audio::PROJECT_RATE;
use crate::mixdown::{self, MixSnapshot};

struct Playback {
    samples: Vec<f32>,
    channels: usize,
    cursor_frames: usize,
    active: bool,
}

pub struct Audition {
    rx: Option<Receiver<Result<[Vec<f32>; 2], String>>>,
    stream: Option<cpal::Stream>,
    state: Arc<Mutex<Playback>>,
    error: Arc<Mutex<Option<String>>>,
    pub rendering: bool,
    pub playing: bool,
    pub last_error: Option<String>,
}

impl Default for Audition {
    fn default() -> Self {
        Self {
            rx: None,
            stream: None,
            state: Arc::new(Mutex::new(Playback {
                samples: Vec::new(),
                channels: 2,
                cursor_frames: 0,
                active: false,
            })),
            error: Arc::new(Mutex::new(None)),
            rendering: false,
            playing: false,
            last_error: None,
        }
    }
}

impl Audition {
    pub fn render(&mut self, snapshot: MixSnapshot) {
        self.stop();
        self.last_error = None;
        self.rendering = true;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = mixdown::render(&snapshot, |_| {})
                .map(|mix| mix.stereo_audition())
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(result);
        });
        self.rx = Some(rx);
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.rx = None;
                    self.rendering = false;
                    match result {
                        Ok(stereo) => {
                            if let Err(error) = self.play(stereo) {
                                self.last_error = Some(format!("{error:#}"));
                            }
                        }
                        Err(error) => self.last_error = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.rendering = false;
                    self.last_error = Some("Audition renderer stopped unexpectedly".to_string());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if let Ok(mut error) = self.error.lock() {
            if let Some(message) = error.take() {
                self.last_error = Some(message);
                self.playing = false;
            }
        }
        if self.playing {
            if let Ok(state) = self.state.lock() {
                if !state.active {
                    self.playing = false;
                }
            }
        }
    }

    pub fn stop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.active = false;
        }
        self.stream = None;
        self.playing = false;
        self.rendering = false;
        self.rx = None;
    }

    pub fn is_busy(&self) -> bool {
        self.rendering || self.playing
    }

    fn play(&mut self, stereo: [Vec<f32>; 2]) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default audio output device"))?;
        let default = device
            .default_output_config()
            .context("query default output format")?;

        let supported = if default.sample_format() == SampleFormat::F32 && default.channels() >= 2 {
            default
        } else {
            device
                .supported_output_configs()
                .ok()
                .and_then(|mut configs| {
                    configs
                        .find(|config| {
                            config.sample_format() == SampleFormat::F32 && config.channels() >= 2
                        })
                        .map(|config| config.with_max_sample_rate())
                })
                .unwrap_or(default)
        };

        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.config();

        let [mut left, mut right] = stereo;
        if sample_rate != PROJECT_RATE {
            left = resample_plane(&left, PROJECT_RATE, sample_rate);
            right = resample_plane(&right, PROJECT_RATE, sample_rate);
        }
        let frames = left.len().min(right.len());
        let mut samples = vec![0.0f32; frames * channels];
        for frame in 0..frames {
            if channels == 1 {
                samples[frame] = (left[frame] + right[frame]) * 0.5;
            } else {
                samples[frame * channels] = left[frame];
                samples[frame * channels + 1] = right[frame];
                for channel in 2..channels {
                    samples[frame * channels + channel] = 0.0;
                }
            }
        }

        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow!("audition state poisoned"))?;
            state.samples = samples;
            state.channels = channels;
            state.cursor_frames = 0;
            state.active = true;
        }
        self.error = Arc::new(Mutex::new(None));
        let error_slot = Arc::clone(&self.error);
        let stream = match sample_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            SampleFormat::U32 => {
                build_stream::<u32>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            SampleFormat::F64 => {
                build_stream::<f64>(&device, config, Arc::clone(&self.state), error_slot)?
            }
            other => {
                return Err(anyhow!("unsupported output sample format {other:?}"));
            }
        };
        stream.play().context("start audition stream")?;
        self.stream = Some(stream);
        self.playing = true;
        Ok(())
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    state: Arc<Mutex<Playback>>,
    error_slot: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32> + 'static,
{
    let channels = state
        .lock()
        .map_err(|_| anyhow!("audition state poisoned"))?
        .channels;
    let callback_state = Arc::clone(&state);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut state = match callback_state.lock() {
                    Ok(state) => state,
                    Err(_) => {
                        for sample in data {
                            *sample = T::from_sample(0.0f32);
                        }
                        return;
                    }
                };
                if !state.active {
                    for sample in data {
                        *sample = T::from_sample(0.0f32);
                    }
                    return;
                }
                let total_frames = state.samples.len() / channels.max(1);
                for frame in data.chunks_mut(channels.max(1)) {
                    for (channel, sample) in frame.iter_mut().enumerate() {
                        if state.cursor_frames < total_frames {
                            *sample = T::from_sample(
                                state.samples[state.cursor_frames * channels + channel],
                            );
                        } else {
                            *sample = T::from_sample(0.0f32);
                        }
                    }
                    state.cursor_frames += 1;
                }
                if state.cursor_frames >= total_frames {
                    state.active = false;
                }
            },
            move |error| {
                if let Ok(mut slot) = error_slot.lock() {
                    *slot = Some(format!("audio output error: {error}"));
                }
            },
            None,
        )
        .context("build audition output stream")
}
