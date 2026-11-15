use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use surround_packer::audio::{Stem, PROJECT_RATE};
use surround_packer::dsp::DspSettings;
use surround_packer::export::dvd::{build_title_card_mpg, probe_dvd_tools, DvdAudioCodec};
use surround_packer::export::wav::write_wav24;
use surround_packer::export::{
    spawn_export, ContainerChoice, ExportEvent, ExportReport, ExportSettings, Format,
};
use surround_packer::mixdown;
use surround_packer::mixdown::MixSnapshot;
use surround_packer::model::ChannelLayout;
use surround_packer::routing::{Channel, ChannelMode, PanLaw, Route, Slot};

const FREQUENCIES: [f32; 6] = [440.0, 550.0, 660.0, 80.0, 880.0, 1100.0];
const CHANNELS: [Channel; 6] = [
    Channel::Fl,
    Channel::Fr,
    Channel::Fc,
    Channel::Lfe,
    Channel::Sl,
    Channel::Sr,
];

const FREQUENCIES_71: [f32; 8] = [440.0, 550.0, 660.0, 80.0, 1000.0, 1120.0, 1240.0, 1360.0];
const CHANNELS_71: [Channel; 8] = [
    Channel::Fl,
    Channel::Fr,
    Channel::Fc,
    Channel::Lfe,
    Channel::Bl,
    Channel::Br,
    Channel::Sl,
    Channel::Sr,
];

fn tone(frequency: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
    let frames = (PROJECT_RATE as f32 * seconds) as usize;
    (0..frames)
        .map(|frame| {
            let t = frame as f32 / PROJECT_RATE as f32;
            amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin()
        })
        .collect()
}

fn snapshot(directory: &Path) -> MixSnapshot {
    let mut stems = HashMap::new();
    let mut slots = Slot::empty_slots();
    for (index, (channel, frequency)) in CHANNELS.iter().zip(FREQUENCIES).enumerate() {
        let id = index as u64 + 1;
        let path = directory.join(format!("tone_{frequency}.wav"));
        let stem = Arc::new(Stem {
            id,
            path: path.clone(),
            display_name: format!("tone_{frequency}"),
            source_sample_rate: PROJECT_RATE,
            channels: vec![tone(frequency, 2.0, 0.16)],
            waveform: Vec::new(),
        });
        stems.insert(id, stem);
        slots[channel.index()]
            .routes
            .push(Route::new(id, path, ChannelMode::LeftOnly, 0.0));
    }
    MixSnapshot {
        stems,
        slots,
        layout: ChannelLayout::Surround51,
        dsp: DspSettings {
            lfe_lowpass_enabled: false,
            surround_delay_enabled: false,
            limiter_enabled: false,
            ..DspSettings::default()
        },
        pan_law: PanLaw::Minus3,
    }
}

fn snapshot_71(directory: &Path) -> MixSnapshot {
    let mut stems = HashMap::new();
    let mut slots = Slot::empty_slots();
    for (index, (channel, frequency)) in CHANNELS_71.iter().zip(FREQUENCIES_71).enumerate() {
        let id = index as u64 + 1;
        let path = directory.join(format!("tone71_{frequency}.wav"));
        let stem = Arc::new(Stem {
            id,
            path: path.clone(),
            display_name: format!("tone71_{frequency}"),
            source_sample_rate: PROJECT_RATE,
            channels: vec![tone(frequency, 2.0, 0.16)],
            waveform: Vec::new(),
        });
        stems.insert(id, stem);
        slots[channel.index()]
            .routes
            .push(Route::new(id, path, ChannelMode::LeftOnly, 0.0));
    }
    MixSnapshot {
        stems,
        slots,
        layout: ChannelLayout::Surround71,
        dsp: DspSettings {
            lfe_lowpass_enabled: false,
            surround_delay_enabled: false,
            limiter_enabled: false,
            ..DspSettings::default()
        },
        pan_law: PanLaw::Minus3,
    }
}

fn wait_for_export(rx: std::sync::mpsc::Receiver<ExportEvent>) -> ExportReport {
    for event in rx {
        match event {
            ExportEvent::Done(report) => return report,
            ExportEvent::Failed(error) => panic!("export failed: {error}"),
            _ => {}
        }
    }
    panic!("export did not finish");
}

fn decode_multichannel(path: &Path, audio_stream: usize, count: usize) -> Vec<Vec<f32>> {
    let map = format!("0:a:{audio_stream}");
    let count_arg = count.to_string();
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map", &map, "-ac", &count_arg, "-ar", "48000", "-f", "s16le", "-",
        ])
        .output()
        .expect("run ffmpeg decode");
    assert!(
        output.status.success(),
        "decode failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let samples: Vec<i16> = output
        .stdout
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect();
    assert!(samples.len() > 48_000 * count);
    let frames = samples.len() / count;
    let mut channels: Vec<Vec<f32>> = (0..count).map(|_| Vec::with_capacity(frames)).collect();
    for frame in 0..frames {
        for channel in 0..count {
            channels[channel].push(samples[frame * count + channel] as f32 / 32768.0);
        }
    }
    channels
}

fn goertzel(samples: &[f32], sample_rate: f32, frequency: f32) -> f64 {
    let omega = 2.0 * std::f64::consts::PI * frequency as f64 / sample_rate as f64;
    let coefficient = 2.0 * omega.cos();
    let mut s1 = 0.0f64;
    let mut s2 = 0.0f64;
    for &sample in samples {
        let s0 = sample as f64 + coefficient * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - coefficient * s1 * s2
}

fn assert_channel_order_generic(
    path: &Path,
    audio_stream: usize,
    frequencies: &[f32],
    channels: &[Channel],
) {
    let decoded = decode_multichannel(path, audio_stream, channels.len());
    let start = 12_000;
    let energies: Vec<Vec<f64>> = decoded
        .iter()
        .map(|channel| {
            frequencies
                .iter()
                .map(|frequency| goertzel(&channel[start..], 48_000.0, *frequency))
                .collect()
        })
        .collect();

    for (target_channel, frequency) in frequencies.iter().enumerate() {
        let mut best_channel = 0usize;
        let mut best_energy = 0.0f64;
        for (channel, channel_energies) in energies.iter().enumerate() {
            if channel_energies[target_channel] > best_energy {
                best_energy = channel_energies[target_channel];
                best_channel = channel;
            }
        }
        assert_eq!(
            best_channel,
            target_channel,
            "{frequency} Hz should be dominant in {} but was in {} ({:?})",
            channels[target_channel].short(),
            channels[best_channel].short(),
            path.display()
        );
        let second_best = energies
            .iter()
            .enumerate()
            .filter(|(channel, _)| *channel != target_channel)
            .map(|(_, channel_energies)| channel_energies[target_channel])
            .fold(0.0f64, f64::max);
        assert!(
            best_energy > second_best * 3.0,
            "{frequency} Hz cross-talk is too high in {}: {best_energy} vs {second_best}",
            path.display()
        );
        let strongest_target = energies[target_channel]
            .iter()
            .copied()
            .fold(0.0f64, f64::max);
        assert!(
            energies[target_channel][target_channel] > strongest_target * 0.1,
            "{} lacks its {frequency} Hz tone",
            channels[target_channel].short()
        );
    }
}

fn assert_channel_order(path: &Path, audio_stream: usize) {
    assert_channel_order_generic(path, audio_stream, &FREQUENCIES, &CHANNELS);
}

#[test]
fn six_channel_isolation_all_formats() {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("ffmpeg not available; skipping channel mapping test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let formats = Format::ALL.to_vec();
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Isolation".to_string(),
        formats: formats.clone(),
        container: ContainerChoice::Raw,
        ..ExportSettings::default()
    };
    let report = wait_for_export(spawn_export(snapshot(directory.path()), settings, formats));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 5);

    for output in &report.outputs {
        assert_channel_order(output, 0);
    }
}

#[test]
fn audio_dvd_mpeg_titles_keep_six_speaker_mapping() {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("ffmpeg not available; skipping Audio-DVD mapping test");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let mix = mixdown::render(&snapshot(directory.path()), |_| {}).unwrap();
    let wav = directory.path().join("isolation_51.wav");
    write_wav24(&wav, ChannelLayout::Surround51, 48_000, &mix.planes, |_| {}).unwrap();

    let tools = probe_dvd_tools();
    for codec in DvdAudioCodec::ALL {
        let mpg = directory.path().join(format!(
            "{}.mpg",
            codec.short_label().replace([' ', '+'], "_")
        ));
        build_title_card_mpg(
            &wav,
            &mpg,
            1,
            "Isolation Test",
            codec,
            2.0,
            &tools.mplex,
            None,
            |_| {},
        )
        .unwrap();
        assert!(mpg.is_file(), "missing {}", mpg.display());
        assert_channel_order(&mpg, 0);
        if codec.dts_kbps().is_some() {
            assert_channel_order(&mpg, 1);
        }
    }
}

#[test]
fn seven_channel_isolation_wav_and_flac() {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("ffmpeg not available; skipping 7.1 mapping test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let formats = vec![Format::Wav24, Format::Flac24];
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Isolation 7.1".to_string(),
        formats: formats.clone(),
        container: ContainerChoice::Raw,
        ..ExportSettings::default()
    };
    let report = wait_for_export(spawn_export(
        snapshot_71(directory.path()),
        settings,
        formats,
    ));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 2);
    for output in &report.outputs {
        assert_channel_order_generic(output, 0, &FREQUENCIES_71, &CHANNELS_71);
    }
}

#[test]
fn six_channel_isolation_dts_mka() {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("ffmpeg not available; skipping DTS MKA test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let formats = vec![Format::Dts];
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Isolation MKA".to_string(),
        formats: formats.clone(),
        container: ContainerChoice::Mka,
        ..ExportSettings::default()
    };
    let report = wait_for_export(spawn_export(snapshot(directory.path()), settings, formats));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 1);
    let output = &report.outputs[0];
    assert!(output.to_string_lossy().ends_with(".mka"));
    assert_channel_order(output, 0);
}

#[test]
fn seven_one_folddown_keeps_51_order() {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("ffmpeg not available; skipping 7.1 folddown test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let formats = vec![Format::Ac3, Format::Dts];
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Folddown 7.1".to_string(),
        formats: formats.clone(),
        container: ContainerChoice::Raw,
        ..ExportSettings::default()
    };
    let report = wait_for_export(spawn_export(
        snapshot_71(directory.path()),
        settings,
        formats,
    ));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 2);

    for output in &report.outputs {
        let decoded = decode_multichannel(output, 0, 6);
        let start = 12_000;
        let energy = |channel: usize, frequency: f32| {
            goertzel(&decoded[channel][start..], 48_000.0, frequency)
        };

        for (channel, frequency) in [(0, 440.0), (1, 550.0), (2, 660.0), (3, 80.0)] {
            assert!(
                energy(channel, frequency) > energy(channel, 1000.0) * 5.0,
                "{frequency} Hz should dominate channel {channel} in {}",
                output.display()
            );
        }

        // SL = 1240 Hz + 0.707 * BL(1000 Hz); SR = 1360 Hz + 0.707 * BR(1120 Hz).
        let sl_main = energy(4, 1240.0);
        let sl_fold = energy(4, 1000.0);
        let sr_main = energy(5, 1360.0);
        let sr_fold = energy(5, 1120.0);
        assert!(
            sl_main > sl_fold * 1.5,
            "SL should be dominated by 1240 Hz after folddown in {}",
            output.display()
        );
        assert!(
            sl_fold > sl_main * 0.2,
            "SL should still contain the folded BL tone in {}",
            output.display()
        );
        assert!(
            sr_main > sr_fold * 1.5,
            "SR should be dominated by 1360 Hz after folddown in {}",
            output.display()
        );
        assert!(
            sr_fold > sr_main * 0.2,
            "SR should still contain the folded BR tone in {}",
            output.display()
        );
        assert!(
            energy(0, 1000.0) < sl_main * 0.05,
            "BL tone leaked into FL in {}",
            output.display()
        );
        assert!(
            energy(1, 1120.0) < sr_main * 0.05,
            "BR tone leaked into FR in {}",
            output.display()
        );
    }
}
