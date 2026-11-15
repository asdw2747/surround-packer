use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use surround_packer::audio::{load_stem, Stem, PROJECT_RATE};
use surround_packer::batch::BatchState;
use surround_packer::dsp::DspSettings;
use surround_packer::export::dvd::{
    build_dvdauthor_xml, build_title_card_mpg, probe_dvd_tools, run_dvdauthor, run_iso,
    DvdAudioCodec, DvdTrack,
};
use surround_packer::export::flac::{read_flac_comments, write_flac24, FlacTags};
use surround_packer::export::wav::{write_wav24, WAV_HEADER_LEN};
use surround_packer::export::{run_export, ExportEvent, ExportSettings, Format};
use surround_packer::mixdown::MixSnapshot;
use surround_packer::model::ChannelLayout;
use surround_packer::routing::{Channel, ChannelMode, PanLaw, Route, Slot};

fn tone(frequency: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
    let frames = (PROJECT_RATE as f32 * seconds) as usize;
    (0..frames)
        .map(|frame| {
            let t = frame as f32 / PROJECT_RATE as f32;
            amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin()
        })
        .collect()
}

fn test_channels(seconds: f32) -> [Vec<f32>; 6] {
    std::array::from_fn(|index| tone(220.0 * (index + 1) as f32, seconds, 0.12))
}

fn ffprobe_json(path: &Path) -> Option<serde_json::Value> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,codec_type,channels,sample_rate,bits_per_raw_sample,channel_layout",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

#[test]
fn native_wav_and_flac_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let channels = test_channels(1.0);

    let wav_path = directory.path().join("native.wav");
    write_wav24(
        &wav_path,
        ChannelLayout::Surround51,
        PROJECT_RATE,
        &channels,
        |_| {},
    )
    .unwrap();
    let wav_bytes = std::fs::read(&wav_path).unwrap();
    assert_eq!(&wav_bytes[..4], b"RIFF");
    assert_eq!(wav_bytes.len(), WAV_HEADER_LEN + channels[0].len() * 6 * 3);

    // Symphonia should be able to decode our WAVEFORMATEXTENSIBLE writer.
    let decoded = load_stem(&wav_path, 99).unwrap();
    assert_eq!(decoded.channel_count(), 6);
    assert_eq!(decoded.frames(), channels[0].len());
    assert_eq!(decoded.source_sample_rate, PROJECT_RATE);

    let flac_path = directory.path().join("native.flac");
    let tags = FlacTags {
        artist: "Integration Artist".into(),
        title: "Integration Title".into(),
        album: "Integration Album".into(),
        ..FlacTags::default()
    };
    write_flac24(
        &flac_path,
        ChannelLayout::Surround51,
        PROJECT_RATE,
        &channels,
        &tags,
        |_| {},
    )
    .unwrap();
    let flac_bytes = std::fs::read(&flac_path).unwrap();
    let comments = read_flac_comments(&flac_bytes).unwrap();
    assert!(comments
        .iter()
        .any(|comment| comment == "WAVEFORMATEXTENSIBLE_CHANNEL_MASK=0x003F"));
    assert!(comments
        .iter()
        .any(|comment| comment == "ARTIST=Integration Artist"));

    if let Some(value) = ffprobe_json(&flac_path) {
        let stream = &value["streams"][0];
        assert_eq!(stream["channels"], 6);
        assert_eq!(stream["bits_per_raw_sample"], "24");
        assert_eq!(stream["sample_rate"], "48000");
    }
}

#[test]
fn batch_export_writes_all_five_formats() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg is not available; skipping codec batch test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let left = tone(330.0, 1.4, 0.1);
    let right = tone(440.0, 1.4, 0.1);
    let stem = Arc::new(Stem {
        id: 1,
        path: directory.path().join("integration_(Vocals).flac"),
        display_name: "integration_(Vocals)".to_string(),
        source_sample_rate: PROJECT_RATE,
        channels: vec![left, right],
        waveform: Vec::new(),
    });
    let mut slots = Slot::empty_slots();
    slots[Channel::Fl.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::StereoL,
        0.0,
    ));
    slots[Channel::Fr.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::StereoR,
        0.0,
    ));
    slots[Channel::Fc.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::MonoSum,
        -6.0,
    ));
    slots[Channel::Lfe.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::MonoSum,
        -12.0,
    ));
    slots[Channel::Sl.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::StereoL,
        -3.0,
    ));
    slots[Channel::Sr.index()].routes.push(Route::new(
        1,
        stem.path.clone(),
        ChannelMode::StereoR,
        -3.0,
    ));

    let snapshot = MixSnapshot {
        stems: HashMap::from([(1, stem)]),
        slots,
        layout: ChannelLayout::Surround51,
        dsp: DspSettings {
            surround_delay_enabled: true,
            ..DspSettings::default()
        },
        pan_law: PanLaw::Minus3,
    };
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Integration 5.1".into(),
        formats: Format::ALL.to_vec(),
        dts_bitrate: 1509,
        ..ExportSettings::default()
    };

    let (tx, rx) = std::sync::mpsc::channel();
    run_export(&snapshot, &settings, &Format::ALL, tx);
    let mut report = None;
    for event in rx {
        if let ExportEvent::Done(done) = event {
            report = Some(done);
        }
    }
    let report = report.expect("export should finish");
    assert!(
        report.errors.is_empty(),
        "unexpected export errors: {:#?}",
        report.errors
    );
    assert_eq!(report.outputs.len(), 5, "outputs: {:#?}", report.outputs);

    for output in &report.outputs {
        assert!(output.is_file(), "missing output {}", output.display());
        let extension = output
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if matches!(extension, "mka" | "ac3" | "eac3" | "dts" | "flac") {
            if let Some(value) = ffprobe_json(output) {
                let stream = &value["streams"][0];
                assert_eq!(stream["channels"], 6, "{} is not 5.1", output.display());
                assert_eq!(stream["sample_rate"], "48000");
                match extension {
                    "flac" => {
                        assert_eq!(stream["codec_name"], "flac");
                        assert_eq!(stream["bits_per_raw_sample"], "24");
                    }
                    "ac3" | "mka" => {}
                    "eac3" => assert_eq!(stream["codec_name"], "eac3"),
                    "dts" => assert_eq!(stream["codec_name"], "dts"),
                    _ => {}
                }
            }
        }
    }
}

fn wait_for_export(
    rx: std::sync::mpsc::Receiver<ExportEvent>,
) -> surround_packer::export::ExportReport {
    for event in rx {
        if let ExportEvent::Done(report) = event {
            return report;
        }
    }
    panic!("export did not finish");
}

#[test]
fn seven_one_native_export_and_legacy_folddown() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg is not available; skipping 7.1 legacy test");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let left = tone(330.0, 1.2, 0.08);
    let right = tone(440.0, 1.2, 0.08);
    let stem = Arc::new(Stem {
        id: 1,
        path: directory.path().join("seven_one_(Vocals).flac"),
        display_name: "seven_one_(Vocals)".to_string(),
        source_sample_rate: PROJECT_RATE,
        channels: vec![left, right],
        waveform: Vec::new(),
    });
    let mut slots = Slot::empty_slots();
    for channel in [
        Channel::Fl,
        Channel::Fr,
        Channel::Fc,
        Channel::Lfe,
        Channel::Bl,
        Channel::Br,
        Channel::Sl,
        Channel::Sr,
    ] {
        slots[channel.index()].routes.push(Route::new(
            1,
            stem.path.clone(),
            channel.default_mode(),
            -6.0,
        ));
    }
    let snapshot = MixSnapshot {
        stems: HashMap::from([(1, stem)]),
        slots,
        layout: ChannelLayout::Surround71,
        dsp: DspSettings::default(),
        pan_law: PanLaw::Minus3,
    };
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "Seven One".into(),
        formats: vec![Format::Wav24, Format::Flac24, Format::Ac3],
        allow_eac3_51_fallback: true,
        ..ExportSettings::default()
    };
    let report = wait_for_export(surround_packer::export::spawn_export(
        snapshot,
        settings,
        vec![Format::Wav24, Format::Flac24, Format::Ac3],
    ));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 3);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("folded to 5.1")));

    let wav_path = report
        .outputs
        .iter()
        .find(|path| {
            path.extension().is_some_and(|extension| extension == "wav")
                && !path.to_string_lossy().ends_with(".dts.wav")
        })
        .unwrap();
    let wav = std::fs::read(wav_path).unwrap();
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 8);
    assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 0x063F);

    let flac_path = report
        .outputs
        .iter()
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "flac")
        })
        .unwrap();
    let flac = std::fs::read(flac_path).unwrap();
    let comments = read_flac_comments(&flac).unwrap();
    assert!(comments
        .iter()
        .any(|comment| comment == "WAVEFORMATEXTENSIBLE_CHANNEL_MASK=0x063F"));
}

#[test]
fn seven_one_eac3_uses_discrete_or_compat_fallback() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg is not available; skipping 7.1 E-AC3 test");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let stem = Arc::new(Stem {
        id: 1,
        path: directory.path().join("eac3_71.flac"),
        display_name: "eac3_71".to_string(),
        source_sample_rate: PROJECT_RATE,
        channels: vec![tone(330.0, 1.0, 0.06), tone(440.0, 1.0, 0.06)],
        waveform: Vec::new(),
    });
    let mut slots = Slot::empty_slots();
    for channel in [
        Channel::Fl,
        Channel::Fr,
        Channel::Fc,
        Channel::Lfe,
        Channel::Bl,
        Channel::Br,
        Channel::Sl,
        Channel::Sr,
    ] {
        slots[channel.index()].routes.push(Route::new(
            1,
            stem.path.clone(),
            channel.default_mode(),
            -6.0,
        ));
    }
    let snapshot = MixSnapshot {
        stems: HashMap::from([(1, stem)]),
        slots,
        layout: ChannelLayout::Surround71,
        dsp: DspSettings::default(),
        pan_law: PanLaw::Minus3,
    };
    let settings = ExportSettings {
        output_dir: directory.path().to_path_buf(),
        base_name: "EAC3 71".into(),
        formats: vec![Format::Eac3],
        allow_eac3_51_fallback: true,
        ..ExportSettings::default()
    };
    let report = wait_for_export(surround_packer::export::spawn_export(
        snapshot,
        settings,
        vec![Format::Eac3],
    ));
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert_eq!(report.outputs.len(), 1);
    let info = surround_packer::export::ffmpeg::probe_ffmpeg();
    let dts = &report.outputs[0];
    if let Some(value) = ffprobe_json(dts) {
        let stream = &value["streams"][0];
        assert_eq!(stream["codec_name"], "eac3");
        if info.eac3_7_1 {
            assert_eq!(stream["channels"], 8);
        } else {
            assert_eq!(stream["channels"], 6);
            assert!(report
                .warnings
                .iter()
                .any(|warning| warning.contains("lacks discrete 7.1")));
        }
    }
}

#[test]
fn audio_dvd_authors_video_ts_and_iso() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg not available; skipping Audio-DVD authoring test");
        return;
    }
    let tools = probe_dvd_tools();
    if !tools.dvdauthor.available() || !tools.mplex.available() {
        eprintln!("dvdauthor or mplex not available; skipping VIDEO_TS test");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let workdir = directory.path().join("Audio-DVD");
    std::fs::create_dir_all(&workdir).unwrap();
    let channels = test_channels(3.0);
    let mut dvd_tracks = Vec::new();
    for index in 0..2 {
        let wav = directory.path().join(format!("track{index}.wav"));
        write_wav24(
            &wav,
            ChannelLayout::Surround51,
            PROJECT_RATE,
            &channels,
            |_| {},
        )
        .unwrap();
        let name = format!("{:02} - Track {}.mpg", index + 1, index + 1);
        let mpg = workdir.join(&name);
        build_title_card_mpg(
            &wav,
            &mpg,
            index + 1,
            &format!("Track {}", index + 1),
            DvdAudioCodec::Ac3Dts1509,
            3.0,
            &tools.mplex,
            None,
            |_| {},
        )
        .unwrap();
        dvd_tracks.push(DvdTrack {
            title: format!("Track {}", index + 1),
            mpg_name: name,
            duration_secs: 3.0,
        });
    }

    for track in &dvd_tracks {
        let mpg = workdir.join(&track.mpg_name);
        let navs = count_nav_packs(&mpg);
        assert!(
            navs >= 3,
            "{} has {navs} NAV packs; mplex should create one about every 0.6 s",
            mpg.display()
        );
        let json = ffprobe_json(&mpg).expect("ffprobe title");
        let codecs: Vec<&str> = json["streams"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|stream| stream["codec_type"] == "audio")
            .filter_map(|stream| stream["codec_name"].as_str())
            .collect();
        assert_eq!(codecs, ["ac3", "dts"]);
    }

    let xml_path = workdir.join("dvdauthor.xml");
    let iso_root = workdir.join("iso_root");
    std::fs::create_dir_all(&iso_root).unwrap();
    std::fs::write(&xml_path, build_dvdauthor_xml(&dvd_tracks)).unwrap();
    run_dvdauthor(&tools.dvdauthor, &workdir, &xml_path, &iso_root).unwrap();
    let video_ts = iso_root.join("VIDEO_TS");
    assert!(
        video_ts.join("VIDEO_TS.IFO").is_file(),
        "dvdauthor did not create iso_root/VIDEO_TS/VIDEO_TS.IFO"
    );
    for entry in std::fs::read_dir(&iso_root).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(
            name == "VIDEO_TS" || name == "AUDIO_TS",
            "ISO root contains unexpected {}",
            name.to_string_lossy()
        );
    }
    let authored_vob = std::fs::read_dir(&video_ts)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "VOB"))
        .expect("authored VOB");
    assert!(count_nav_packs(&authored_vob) >= 6);

    if tools.iso.available() {
        let iso = directory.path().join("Audio-DVD.iso");
        run_iso(&tools.iso, &iso_root, &iso).unwrap();
        assert!(iso.is_file());
        assert!(std::fs::metadata(&iso).unwrap().len() > 1_000_000);

        if Command::new("isoinfo").arg("-version").output().is_ok() {
            let listing = Command::new("isoinfo")
                .arg("-i")
                .arg(&iso)
                .arg("-f")
                .output()
                .expect("list ISO");
            assert!(
                listing.status.success(),
                "isoinfo failed: {}",
                String::from_utf8_lossy(&listing.stderr)
            );
            let text = String::from_utf8_lossy(&listing.stdout);
            for line in text
                .lines()
                .map(str::trim)
                .filter(|line| line.starts_with('/'))
            {
                assert!(
                    line.starts_with("/VIDEO_TS") || line.starts_with("/AUDIO_TS"),
                    "ISO contains unexpected path {line}"
                );
                assert!(
                    !line.to_ascii_lowercase().ends_with(".mpg"),
                    "ISO leaked intermediate MPG {line}"
                );
            }
            assert!(text.contains("/VIDEO_TS/VIDEO_TS.IFO"));

            let extracted = directory.path().join("iso_vts_01_1.vob");
            let vob = Command::new("isoinfo")
                .arg("-i")
                .arg(&iso)
                .args(["-x", "/VIDEO_TS/VTS_01_1.VOB;1"])
                .output()
                .expect("extract ISO VOB");
            assert!(vob.status.success(), "isoinfo -x failed");
            std::fs::write(&extracted, &vob.stdout).unwrap();
            let json = ffprobe_json(&extracted).expect("ffprobe ISO VOB");
            let codecs: Vec<&str> = json["streams"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|stream| stream["codec_type"] == "audio")
                .filter_map(|stream| stream["codec_name"].as_str())
                .collect();
            assert_eq!(codecs, ["ac3", "dts"], "ISO VOB audio streams");
        }
    }
}

#[test]
fn batch_audio_dvd_build_produces_titles_and_iso() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg not available; skipping batch Audio-DVD test");
        return;
    }
    let tools = probe_dvd_tools();
    let directory = tempfile::tempdir().unwrap();
    let channels = test_channels(3.0);
    let mut paths = Vec::new();
    for index in 0..2 {
        let path = directory
            .path()
            .join(format!("Batch DVD {}.wav", index + 1));
        write_wav24(
            &path,
            ChannelLayout::Surround51,
            PROJECT_RATE,
            &channels,
            |_| {},
        )
        .unwrap();
        paths.push(path);
    }
    let mut state = BatchState::new(directory.path().to_path_buf(), "Batch DVD Test");
    assert_eq!(state.add_paths(&paths), 2);
    state.start_build(tools.clone());
    for _ in 0..600 {
        state.poll();
        if !state.converting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(!state.converting, "batch DVD build did not finish");
    assert!(state.last_errors.is_empty(), "{:#?}", state.last_errors);
    assert_eq!(state.last_outputs.len(), 2);
    for output in &state.last_outputs {
        assert!(output.is_file(), "missing {}", output.display());
        if tools.mplex.available() {
            assert!(
                count_nav_packs(output) >= 3,
                "{} has no DVD NAV packs",
                output.display()
            );
            let json = ffprobe_json(output).expect("ffprobe title");
            let codecs: Vec<&str> = json["streams"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|stream| stream["codec_type"] == "audio")
                .filter_map(|stream| stream["codec_name"].as_str())
                .collect();
            assert_eq!(codecs, ["ac3", "dts"]);
        }
    }
    if tools.mplex.available() && tools.dvdauthor.available() {
        let video_ts = state
            .last_video_ts
            .as_ref()
            .expect("batch did not author VIDEO_TS");
        assert!(video_ts.join("VIDEO_TS.IFO").is_file());
        let vob = std::fs::read_dir(video_ts)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "VOB"))
            .expect("authored VOB");
        assert!(count_nav_packs(&vob) >= 6);
        let iso_root = video_ts.parent().unwrap();
        for entry in std::fs::read_dir(iso_root).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                name == "VIDEO_TS" || name == "AUDIO_TS",
                "ISO root contains unexpected {}",
                name.to_string_lossy()
            );
        }
    }
    if tools.iso.available() && state.last_video_ts.is_some() {
        assert!(state.last_iso.as_ref().is_some_and(|path| path.is_file()));
    }
}

#[test]
fn audio_dvd_title_build_survives_repeated_long_runs() {
    let iterations = std::env::var("SP_DVD_STRESS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if iterations == 0 {
        return;
    }
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg not available; skipping Audio-DVD stress test");
        return;
    }

    let seconds = std::env::var("SP_DVD_STRESS_SECS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(30.0);
    let directory = tempfile::tempdir().unwrap();
    let channels = test_channels(seconds as f32);
    let wav = directory.path().join("stress_51.wav");
    write_wav24(
        &wav,
        ChannelLayout::Surround51,
        PROJECT_RATE,
        &channels,
        |_| {},
    )
    .unwrap();

    let mplex = probe_dvd_tools().mplex;
    for run in 0..iterations {
        let mpg = directory.path().join(format!("stress {run}.mpg"));
        build_title_card_mpg(
            &wav,
            &mpg,
            run + 1,
            "Stress Test",
            DvdAudioCodec::Ac3Dts1509,
            seconds,
            &mplex,
            None,
            |_| {},
        )
        .unwrap_or_else(|error| panic!("run {run} failed: {error:#}"));
        assert!(mpg.is_file());
        assert!(std::fs::metadata(&mpg).unwrap().len() > 1_000_000);
    }
}

#[test]
fn audio_dvd_builds_real_inputs_from_env() {
    let Some(list) = std::env::var_os("SP_DVD_REAL_INPUTS") else {
        return;
    };
    let paths: Vec<std::path::PathBuf> = std::env::split_paths(&list).collect();
    if paths.is_empty() {
        return;
    }
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg not available; skipping real-input build");
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let mut state = BatchState::new(directory.path().to_path_buf(), "Real Input Test");
    assert_eq!(state.add_paths(&paths), paths.len());
    let tools = probe_dvd_tools();
    state.start_build(tools.clone());
    for _ in 0..12_000 {
        state.poll();
        if !state.converting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(!state.converting, "build did not finish");
    assert!(state.last_errors.is_empty(), "{:#?}", state.last_errors);
    assert_eq!(state.last_outputs.len(), paths.len());
    for output in &state.last_outputs {
        assert!(output.is_file(), "missing {}", output.display());
        assert!(std::fs::metadata(output).unwrap().len() > 1_000_000);
        if tools.mplex.available() {
            assert!(
                count_nav_packs(output) >= 10,
                "{} has no DVD NAV packs",
                output.display()
            );
            let json = ffprobe_json(output).expect("ffprobe title");
            let codecs: Vec<&str> = json["streams"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|stream| stream["codec_type"] == "audio")
                .filter_map(|stream| stream["codec_name"].as_str())
                .collect();
            assert_eq!(codecs, ["ac3", "dts"]);
        }
    }
    if tools.mplex.available() && tools.dvdauthor.available() {
        let video_ts = state
            .last_video_ts
            .as_ref()
            .expect("real-input build did not author VIDEO_TS");
        let vob = std::fs::read_dir(video_ts)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "VOB"))
            .expect("authored VOB");
        assert!(count_nav_packs(&vob) >= 100);
    }
}

fn count_nav_packs(path: &Path) -> usize {
    let data = std::fs::read(path).unwrap();
    data.chunks(2048)
        .filter(|pack| {
            pack.starts_with(&[0x00, 0x00, 0x01, 0xBA])
                && pack.get(38..42) == Some(&[0x00, 0x00, 0x01, 0xBF])
        })
        .count()
}
