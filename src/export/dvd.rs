use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use crate::export::ffmpeg::{self, FfmpegInfo};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DvdAudioCodec {
    /// AC-3 first for universal NTSC DVD player support, DTS second for quality.
    #[default]
    Ac3Dts1509,
    Ac3Dts768,
    Ac3Only,
}

impl DvdAudioCodec {
    pub const ALL: [Self; 3] = [Self::Ac3Dts1509, Self::Ac3Dts768, Self::Ac3Only];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Ac3Dts1509 => "AC-3 448 + DTS 1509 kb/s (48 kHz 5.1)",
            Self::Ac3Dts768 => "AC-3 448 + DTS 768 kb/s (48 kHz 5.1)",
            Self::Ac3Only => "AC-3 448 kb/s (48 kHz 5.1)",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Ac3Dts1509 => "AC-3 448 + DTS 1509",
            Self::Ac3Dts768 => "AC-3 448 + DTS 768",
            Self::Ac3Only => "AC-3 448",
        }
    }

    pub const fn dts_kbps(self) -> Option<f64> {
        match self {
            Self::Ac3Dts1509 => Some(1509.0),
            Self::Ac3Dts768 => Some(768.0),
            Self::Ac3Only => None,
        }
    }

    pub const fn audio_kbps(self) -> f64 {
        448.0
            + match self.dts_kbps() {
                Some(kbps) => kbps,
                None => 0.0,
            }
    }

    /// AC-3 at the DVD-Video maximum of 448 kb/s. Always included as stream 0.
    pub fn ac3_args() -> Vec<String> {
        ["-c:a", "ac3", "-b:a", "448k"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    pub fn dts_args(self) -> Option<Vec<String>> {
        let bitrate = match self {
            Self::Ac3Dts1509 => "1509k",
            Self::Ac3Dts768 => "768k",
            Self::Ac3Only => return None,
        };
        Some(
            ["-strict", "-2", "-c:a", "dca", "-b:a", bitrate]
                .into_iter()
                .map(String::from)
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolInfo {
    pub program: Option<String>,
    pub version: Option<String>,
}

impl ToolInfo {
    pub fn available(&self) -> bool {
        self.program.is_some()
    }

    pub fn label(&self) -> String {
        match (&self.program, &self.version) {
            (Some(program), Some(version)) => format!("{program}: {version}"),
            (Some(program), None) => program.clone(),
            _ => "not found".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DvdTools {
    pub ffmpeg: FfmpegInfo,
    pub mplex: ToolInfo,
    pub dvdauthor: ToolInfo,
    pub iso: ToolInfo,
}

fn probe_tool(program: &str, version_arg: &str) -> ToolInfo {
    let mut command = Command::new(program);
    command.arg(version_arg);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    match command.output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            let version = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string);
            if output.status.success() || !text.trim().is_empty() {
                ToolInfo {
                    program: Some(program.to_string()),
                    version,
                }
            } else {
                ToolInfo::default()
            }
        }
        Err(_) => ToolInfo::default(),
    }
}

pub fn probe_dvd_tools() -> DvdTools {
    let iso = ["xorriso", "mkisofs", "genisoimage"]
        .into_iter()
        .find_map(|program| {
            let arg = if program == "xorriso" {
                "-version"
            } else {
                "--version"
            };
            let info = probe_tool(program, arg);
            info.available().then_some(info)
        })
        .unwrap_or_default();
    DvdTools {
        ffmpeg: ffmpeg::probe_ffmpeg(),
        mplex: probe_tool("mplex", "-V"),
        dvdauthor: probe_tool("dvdauthor", "--version"),
        iso,
    }
}

#[derive(Clone, Debug)]
pub struct DvdTrack {
    pub title: String,
    pub mpg_name: String,
    pub duration_secs: f64,
}

pub fn estimated_dvd_mb(tracks: &[DvdTrack], codec: DvdAudioCodec) -> f64 {
    let seconds: f64 = tracks
        .iter()
        .map(|track| track.duration_secs.max(0.0))
        .sum();
    // The title cards are static, so the encoded video averages roughly 0.4 Mbps.
    let video_kbps = 500.0;
    let mpeg_overhead = 1.04;
    seconds * (codec.audio_kbps() + video_kbps) * 1000.0 / 8.0 / 1_000_000.0 * mpeg_overhead
}

pub const DVD_R_CAPACITY_MB: f64 = 4_700.0;

fn temp_text_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "surround-packer-dvd-title-{}-{stamp}.txt",
        std::process::id()
    ))
}

fn font_file() -> Option<PathBuf> {
    [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
}

fn filter_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    value.replace(':', "\\:")
}

fn temp_video_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "surround-packer-dvd-video-{}-{stamp}.m2v",
        std::process::id()
    ))
}

/// Video-only pass. Keeping this graph single-input avoids the FFmpeg 9
/// filtergraph scheduler deadlock that occurs when `-t` closes one of several
/// inputs.
pub fn build_title_video_args(
    output_video: &Path,
    text_file: &Path,
    font: &Path,
    duration_secs: f64,
) -> Vec<String> {
    let duration = duration_secs.max(0.1);
    let filter = format!(
        "color=c=0x101820:s=720x480:r=1:d={duration:.3},drawtext=fontfile='{}':textfile='{}':fontcolor=0xE6EEF5:fontsize=34:line_spacing=18:x=(w-text_w)/2:y=(h-text_h)/2",
        filter_path(font),
        filter_path(text_file)
    );
    vec![
        "-y".into(),
        "-hide_banner".into(),
        "-nostats".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        filter,
        "-t".into(),
        format!("{duration:.3}"),
        "-target".into(),
        "ntsc-dvd".into(),
        "-c:v".into(),
        "mpeg2video".into(),
        "-b:v".into(),
        "2000k".into(),
        "-maxrate".into(),
        "8000k".into(),
        "-bufsize".into(),
        "1835k".into(),
        "-f".into(),
        "mpeg2video".into(),
        output_video.to_string_lossy().into_owned(),
    ]
}

fn temp_audio_path(extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "surround-packer-dvd-audio-{}-{stamp}.{extension}",
        std::process::id()
    ))
}

fn ffmpeg_audio_pass(
    input_wav: &Path,
    output: &Path,
    format: &str,
    codec_args: &[String],
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-nostats".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-i".into(),
        input_wav.to_string_lossy().into_owned(),
        "-ar".into(),
        "48000".into(),
    ];
    args.extend(codec_args.iter().cloned());
    args.extend(["-f".into(), format.into()]);
    args.push(output.to_string_lossy().into_owned());
    args
}

pub fn build_ac3_args(input_wav: &Path, output_ac3: &Path) -> Vec<String> {
    ffmpeg_audio_pass(input_wav, output_ac3, "ac3", &DvdAudioCodec::ac3_args())
}

pub fn build_dts_args(
    input_wav: &Path,
    output_dts: &Path,
    codec: DvdAudioCodec,
) -> Option<Vec<String>> {
    codec
        .dts_args()
        .map(|args| ffmpeg_audio_pass(input_wav, output_dts, "dts", &args))
}

/// Fallback mux used only when mplex is unavailable. The resulting MPEG has no
/// NAV packs, so it is playable as a file but not used for DVD authoring.
pub fn build_title_mux_args(video: &Path, input_wav: &Path, output_mpg: &Path) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-nostats".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-i".into(),
        input_wav.to_string_lossy().into_owned(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
        "-c:v".into(),
        "copy".into(),
        "-ar".into(),
        "48000".into(),
    ];
    args.extend(DvdAudioCodec::ac3_args());
    args.extend(["-f".into(), "dvd".into()]);
    args.push(output_mpg.to_string_lossy().into_owned());
    args
}

pub fn build_mplex_args(
    video: &Path,
    ac3: &Path,
    dts: Option<&Path>,
    output_mpg: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-f".into(),
        "8".into(),
        "-o".into(),
        output_mpg.to_string_lossy().into_owned(),
        video.to_string_lossy().into_owned(),
        ac3.to_string_lossy().into_owned(),
    ];
    if let Some(dts) = dts {
        args.push(dts.to_string_lossy().into_owned());
    }
    args
}

fn hide_console(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

pub fn run_mplex(mplex: &ToolInfo, args: &[String]) -> Result<()> {
    let program = mplex
        .program
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("mplex is not installed"))?;
    let mut command = Command::new(program);
    command.args(args);
    hide_console(&mut command);
    let output = command.output().context("run mplex")?;
    if !output.status.success() {
        bail!(
            "mplex failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn build_title_card_mpg<F>(
    input_wav: &Path,
    output_mpg: &Path,
    track_number: usize,
    title: &str,
    codec: DvdAudioCodec,
    duration_secs: f64,
    mplex: &ToolInfo,
    cancel: Option<&AtomicBool>,
    mut progress: F,
) -> Result<()>
where
    F: FnMut(f32),
{
    let Some(font) = font_file() else {
        bail!("no usable TrueType font found for DVD title cards");
    };
    let text_path = temp_text_path();
    let video_path = temp_video_path();
    let ac3_path = temp_audio_path("ac3");
    let dts_path = codec.dts_args().map(|_| temp_audio_path("dts"));
    let text = format!(
        "Track {track_number:02} - {title}\n48 kHz 5.1 {}",
        codec.short_label()
    );
    std::fs::write(&text_path, text).with_context(|| format!("write {}", text_path.display()))?;

    let video_args = build_title_video_args(&video_path, &text_path, &font, duration_secs);
    let result = (|| -> Result<()> {
        ffmpeg::run_ffmpeg_retry(
            &video_args,
            duration_secs,
            &mut |fraction| progress(fraction * 0.10),
            cancel,
            2,
        )
        .context("render DVD title video")?;

        if mplex.available() {
            let ac3_args = build_ac3_args(input_wav, &ac3_path);
            ffmpeg::run_ffmpeg_retry(
                &ac3_args,
                duration_secs,
                &mut |fraction| progress(0.10 + fraction * 0.08),
                cancel,
                2,
            )
            .context("encode AC-3 audio")?;

            if let Some(dts_path) = &dts_path {
                let dts_args = build_dts_args(input_wav, dts_path, codec)
                    .expect("DTS path exists only for DTS codecs");
                ffmpeg::run_ffmpeg_retry(
                    &dts_args,
                    duration_secs,
                    &mut |fraction| progress(0.18 + fraction * 0.08),
                    cancel,
                    2,
                )
                .context("encode DTS audio")?;
            }

            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                bail!("Audio-DVD build cancelled");
            }
            progress(0.28);
            let mplex_args =
                build_mplex_args(&video_path, &ac3_path, dts_path.as_deref(), output_mpg);
            run_mplex(mplex, &mplex_args).context("mux DVD title with mplex")?;
            progress(1.0);
        } else {
            let mux_args = build_title_mux_args(&video_path, input_wav, output_mpg);
            ffmpeg::run_ffmpeg_retry(
                &mux_args,
                duration_secs,
                &mut |fraction| progress(0.10 + fraction * 0.90),
                cancel,
                2,
            )
            .context("mux DVD title")?;
        }
        Ok(())
    })();

    let _ = std::fs::remove_file(&text_path);
    let _ = std::fs::remove_file(&video_path);
    let _ = std::fs::remove_file(&ac3_path);
    if let Some(dts_path) = dts_path {
        let _ = std::fs::remove_file(dts_path);
    }
    result
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn build_dvdauthor_xml(tracks: &[DvdTrack]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<dvdauthor dest=\"VIDEO_TS\" format=\"ntsc\">\n  <vmgm />\n  <titleset>\n    <titles>\n      <video format=\"ntsc\" aspect=\"4:3\" />\n",
    );
    for track in tracks {
        xml.push_str(&format!(
            "      <pgc><vob file=\"{}\" /></pgc>\n",
            xml_escape(&track.mpg_name)
        ));
    }
    xml.push_str("    </titles>\n  </titleset>\n</dvdauthor>\n");
    xml
}

pub fn run_dvdauthor(
    dvdauthor: &ToolInfo,
    workdir: &Path,
    xml_path: &Path,
    dest: &Path,
) -> Result<()> {
    let program = dvdauthor
        .program
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("dvdauthor is not installed"))?;
    let output = Command::new(program)
        .current_dir(workdir)
        .arg("-o")
        .arg(dest)
        .arg("-x")
        .arg(xml_path)
        .output()
        .context("run dvdauthor")?;
    if !output.status.success() {
        bail!(
            "dvdauthor failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

pub fn run_iso(iso: &ToolInfo, workdir: &Path, output: &Path) -> Result<()> {
    let program = iso
        .program
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no ISO authoring tool installed"))?;
    let mut command = Command::new(program);
    if program == "xorriso" {
        command.args(["-as", "mkisofs"]);
    }
    command
        .arg("-dvd-video")
        .arg("-V")
        .arg("AUDIO_DVD")
        .arg("-o")
        .arg(output)
        .arg(workdir);
    let result = command.output().context("run ISO authoring tool")?;
    if !result.status.success() {
        bail!(
            "ISO authoring failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(index: usize) -> DvdTrack {
        DvdTrack {
            title: format!("Track {index}"),
            mpg_name: format!("{index:02} - Track.mpg"),
            duration_secs: 180.0,
        }
    }

    #[test]
    fn dvd_title_video_args_are_single_input_and_dvd_targeted() {
        let args = build_title_video_args(
            Path::new("title.m2v"),
            Path::new("title.txt"),
            Path::new("font.ttf"),
            5.0,
        );
        assert_eq!(args.iter().filter(|arg| *arg == "-i").count(), 1);
        assert!(args.windows(2).any(|pair| pair == ["-target", "ntsc-dvd"]));
        assert!(args.windows(2).any(|pair| pair == ["-c:v", "mpeg2video"]));
        assert!(args.windows(2).any(|pair| pair == ["-f", "mpeg2video"]));
        assert!(args.windows(2).any(|pair| pair == ["-t", "5.000"]));
        assert!(args.windows(2).any(|pair| pair == ["-b:v", "2000k"]));
        assert!(args.iter().any(|arg| arg.contains("720x480")));
        assert!(args.iter().any(|arg| arg == "-nostdin"));
        assert!(!args.iter().any(|arg| arg == "-map"));
    }

    #[test]
    fn dvd_audio_passes_are_dvd_compliant() {
        let ac3 = build_ac3_args(Path::new("audio.wav"), Path::new("audio.ac3"));
        assert!(ac3.windows(2).any(|pair| pair == ["-c:a", "ac3"]));
        assert!(ac3.windows(2).any(|pair| pair == ["-b:a", "448k"]));
        assert!(ac3.windows(2).any(|pair| pair == ["-ar", "48000"]));
        assert!(ac3.windows(2).any(|pair| pair == ["-f", "ac3"]));

        let dts = build_dts_args(
            Path::new("audio.wav"),
            Path::new("audio.dts"),
            DvdAudioCodec::Ac3Dts1509,
        )
        .unwrap();
        assert!(dts.windows(2).any(|pair| pair == ["-strict", "-2"]));
        assert!(dts.windows(2).any(|pair| pair == ["-c:a", "dca"]));
        assert!(dts.windows(2).any(|pair| pair == ["-b:a", "1509k"]));
        assert!(dts.windows(2).any(|pair| pair == ["-f", "dts"]));

        let dts768 = build_dts_args(
            Path::new("audio.wav"),
            Path::new("audio.dts"),
            DvdAudioCodec::Ac3Dts768,
        )
        .unwrap();
        assert!(dts768.windows(2).any(|pair| pair == ["-b:a", "768k"]));

        assert!(build_dts_args(
            Path::new("audio.wav"),
            Path::new("audio.dts"),
            DvdAudioCodec::Ac3Only
        )
        .is_none());
        assert_eq!(DvdAudioCodec::Ac3Dts1509.audio_kbps(), 1957.0);
        assert_eq!(DvdAudioCodec::Ac3Only.audio_kbps(), 448.0);
    }

    #[test]
    fn mplex_uses_dvd_mode_and_ac3_first() {
        let args = build_mplex_args(
            Path::new("video.m2v"),
            Path::new("audio.ac3"),
            Some(Path::new("audio.dts")),
            Path::new("out.mpg"),
        );
        assert!(args.windows(2).any(|pair| pair == ["-f", "8"]));
        assert_eq!(args.iter().filter(|arg| arg.ends_with(".m2v")).count(), 1);
        let video = args
            .iter()
            .position(|arg| arg.ends_with("video.m2v"))
            .unwrap();
        let ac3 = args
            .iter()
            .position(|arg| arg.ends_with("audio.ac3"))
            .unwrap();
        let dts = args
            .iter()
            .position(|arg| arg.ends_with("audio.dts"))
            .unwrap();
        let output = args
            .iter()
            .position(|arg| arg.ends_with("out.mpg"))
            .unwrap();
        assert!(output < video && video < ac3 && ac3 < dts);

        let ac3_only = build_mplex_args(
            Path::new("video.m2v"),
            Path::new("audio.ac3"),
            None,
            Path::new("out.mpg"),
        );
        assert_eq!(
            ac3_only
                .iter()
                .filter(|arg| arg.ends_with(".ac3") || arg.ends_with(".dts"))
                .count(),
            1
        );
    }

    #[test]
    fn ffmpeg_fallback_mux_copies_video_and_uses_ac3() {
        let args = build_title_mux_args(
            Path::new("title.m2v"),
            Path::new("audio.wav"),
            Path::new("out.mpg"),
        );
        assert!(args.windows(2).any(|pair| pair == ["-c:v", "copy"]));
        assert!(args.windows(2).any(|pair| pair == ["-ar", "48000"]));
        assert!(args.windows(2).any(|pair| pair == ["-b:a", "448k"]));
        assert!(args.windows(2).any(|pair| pair == ["-f", "dvd"]));
        assert!(!args.iter().any(|arg| arg.contains("lavfi")));
        assert!(!args
            .iter()
            .any(|arg| arg.contains("channelmap") || arg.contains("pan=")));
    }

    #[test]
    fn dvdauthor_xml_has_ordered_titles() {
        let xml = build_dvdauthor_xml(&[track(1), track(2), track(3)]);
        assert!(xml.contains("<dvdauthor dest=\"VIDEO_TS\" format=\"ntsc\">"));
        assert!(xml.contains("<pgc><vob file=\"01 - Track.mpg\" /></pgc>"));
        let first = xml.find("01 - Track.mpg").unwrap();
        let second = xml.find("02 - Track.mpg").unwrap();
        let third = xml.find("03 - Track.mpg").unwrap();
        assert!(first < second && second < third);
    }

    #[test]
    fn dvd_size_estimate_tracks_audio_and_video() {
        let mb = estimated_dvd_mb(&[track(1), track(2)], DvdAudioCodec::Ac3Dts1509);
        assert!(mb > 100.0 && mb < 130.0, "unexpected estimate {mb}");
    }
}
