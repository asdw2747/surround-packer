use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

/// FFmpeg 9.0.x can deadlock in its filtergraph scheduler when `-t` closes one
/// input while another is still running. The process keeps all threads parked
/// and stops emitting progress. Detect that stall, kill the child, and retry.
pub(crate) const FFMPEG_STALL_MESSAGE: &str = "ffmpeg stopped responding";

const STALL_TIMEOUT: Duration = Duration::from_secs(20);

/// PID of the most recently spawned FFmpeg process, so the UI can terminate a
/// build immediately on cancel or window close instead of leaving an orphan.
static ACTIVE_FFMPEG: Mutex<Option<u32>> = Mutex::new(None);

struct ActiveFfmpeg;

impl ActiveFfmpeg {
    fn set(pid: u32) -> Self {
        if let Ok(mut slot) = ACTIVE_FFMPEG.lock() {
            *slot = Some(pid);
        }
        Self
    }
}

impl Drop for ActiveFfmpeg {
    fn drop(&mut self) {
        if let Ok(mut slot) = ACTIVE_FFMPEG.lock() {
            *slot = None;
        }
    }
}

pub fn terminate_active_ffmpeg() {
    let pid = ACTIVE_FFMPEG.lock().ok().and_then(|mut slot| slot.take());
    if let Some(pid) = pid {
        terminate_pid(pid);
    }
}

#[cfg(unix)]
fn terminate_pid(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(windows)]
fn terminate_pid(pid: u32) {
    let mut command = Command::new("taskkill");
    command.args(["/F", "/PID", &pid.to_string()]);
    hide_console(&mut command);
    let _ = command.stdout(Stdio::null()).stderr(Stdio::null()).status();
}

#[cfg(not(any(unix, windows)))]
fn terminate_pid(_pid: u32) {}

pub const EAC3_BITRATE_7_1: &str = "768k";
pub const AC3_BITRATE: &str = "640k";
pub const EAC3_BITRATE: &str = "640k";
pub const DTS_BITRATE_HIGH: u32 = 1509;
pub const DTS_BITRATE_LOW: u32 = 768;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Ac3,
    Eac3,
    Dts,
}

impl Codec {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ac3 => "ac3",
            Self::Eac3 => "eac3",
            Self::Dts => "dca",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Ac3 => "Dolby Digital",
            Self::Eac3 => "Dolby Digital Plus",
            Self::Dts => "DTS",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Mka,
    Raw,
}

#[derive(Clone, Debug)]
pub struct FfmpegInfo {
    pub available: bool,
    pub version: String,
    pub executable: String,
    /// Whether this FFmpeg build's native eac3 encoder advertises 7.1 support.
    pub eac3_7_1: bool,
}

impl Default for FfmpegInfo {
    fn default() -> Self {
        Self {
            available: false,
            version: "checking…".to_string(),
            executable: "ffmpeg".to_string(),
            eac3_7_1: false,
        }
    }
}

pub fn probe_ffmpeg() -> FfmpegInfo {
    let mut command = Command::new("ffmpeg");
    command.arg("-version");
    hide_console(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let version = text.lines().next().unwrap_or("ffmpeg").trim().to_string();
            let eac3_7_1 = probe_eac3_7_1();
            FfmpegInfo {
                available: true,
                version,
                executable: "ffmpeg".to_string(),
                eac3_7_1,
            }
        }
        Ok(output) => FfmpegInfo {
            available: false,
            version: format!("ffmpeg exited with {}", output.status),
            executable: "ffmpeg".to_string(),
            eac3_7_1: false,
        },
        Err(error) => FfmpegInfo {
            available: false,
            version: format!("ffmpeg not found ({error})"),
            executable: "ffmpeg".to_string(),
            eac3_7_1: false,
        },
    }
}

pub const fn output_extension(codec: Codec, container: Container) -> &'static str {
    match (codec, container) {
        (_, Container::Mka) => "mka",
        (Codec::Ac3, Container::Raw) => "ac3",
        (Codec::Eac3, Container::Raw) => "eac3",
        (Codec::Dts, Container::Raw) => "dts",
    }
}

fn bitrate(codec: Codec, dts_bitrate: u32) -> String {
    match codec {
        Codec::Ac3 => AC3_BITRATE.to_string(),
        Codec::Eac3 => EAC3_BITRATE.to_string(),
        Codec::Dts => format!("{}k", dts_bitrate),
    }
}

/// Build the exact FFmpeg argument list. This is deliberately pure so it can be tested.
pub fn build_args(
    input: &Path,
    output: &Path,
    codec: Codec,
    dts_bitrate: u32,
    container: Container,
    metadata: &[(String, String)],
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-nostats".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-i".into(),
        input.to_string_lossy().into_owned(),
        "-map".into(),
        "0:a:0".into(),
    ];

    if codec == Codec::Dts {
        args.push("-strict".into());
        args.push("-2".into());
    }
    args.push("-c:a".into());
    args.push(codec.name().into());
    args.push("-b:a".into());
    args.push(bitrate(codec, dts_bitrate));

    append_container(
        &mut args,
        output,
        container,
        metadata,
        match codec {
            Codec::Ac3 => "ac3",
            Codec::Eac3 => "eac3",
            Codec::Dts => "dts",
        },
    );
    args
}

fn append_container(
    args: &mut Vec<String>,
    output: &Path,
    container: Container,
    metadata: &[(String, String)],
    raw_muxer: &str,
) {
    if container == Container::Mka {
        for (key, value) in metadata {
            if !value.trim().is_empty() {
                args.push("-metadata".into());
                args.push(format!("{key}={}", value.trim()));
            }
        }
        args.push("-f".into());
        args.push("matroska".into());
    } else {
        args.push("-f".into());
        args.push(raw_muxer.into());
    }
    args.push(output.to_string_lossy().into_owned());
}

fn probe_eac3_7_1() -> bool {
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-h", "encoder=eac3"]);
    hide_console(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            text.lines()
                .find(|line| line.contains("Supported channel layouts"))
                .is_some_and(|line| {
                    line.split_whitespace()
                        .any(|layout| layout == "7.1" || layout.starts_with("7.1("))
                })
        }
        _ => false,
    }
}

/// Build a discrete 7.1 E-AC3 command. Only used when the local encoder advertises 7.1.
pub fn build_7_1_eac3_args(
    input: &Path,
    output: &Path,
    container: Container,
    metadata: &[(String, String)],
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-nostats".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-i".into(),
        input.to_string_lossy().into_owned(),
        "-map".into(),
        "0:a:0".into(),
        "-c:a".into(),
        "eac3".into(),
        "-b:a".into(),
        EAC3_BITRATE_7_1.into(),
    ];
    append_container(&mut args, output, container, metadata, "eac3");
    args
}

fn hide_console(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

fn command_line(args: &[String]) -> String {
    let mut line = String::from("ffmpeg");
    for arg in args {
        line.push(' ');
        if arg.contains(' ') || arg.contains('|') {
            line.push('"');
            line.push_str(arg);
            line.push('"');
        } else {
            line.push_str(arg);
        }
    }
    line
}

pub(crate) fn run_ffmpeg(
    args: &[String],
    duration_secs: f64,
    progress: &mut dyn FnMut(f32),
) -> Result<std::process::ExitStatus> {
    run_ffmpeg_cancellable(args, duration_secs, progress, None)
}

pub(crate) fn run_ffmpeg_cancellable(
    args: &[String],
    duration_secs: f64,
    progress: &mut dyn FnMut(f32),
    cancel: Option<&AtomicBool>,
) -> Result<std::process::ExitStatus> {
    let mut command = Command::new("ffmpeg");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);

    let mut child = command.spawn().context("spawn ffmpeg")?;
    let _active = ActiveFfmpeg::set(child.id());

    let stderr = child.stderr.take().context("capture ffmpeg stderr")?;
    let stderr_thread = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut text = String::new();
        let _ = reader.read_to_string(&mut text);
        text
    });

    let (line_tx, line_rx) = mpsc::channel::<String>();
    let stdout_thread = child.stdout.take().map(|stdout| {
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        })
    });

    let mut last_output = Instant::now();
    let mut stall = false;
    let mut cancelled = false;
    loop {
        match line_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                last_output = Instant::now();
                report_progress_line(&line, duration_secs, progress);
            }
            Err(RecvTimeoutError::Timeout) => {
                if is_cancelled(cancel) {
                    cancelled = true;
                    break;
                }
                if last_output.elapsed() >= STALL_TIMEOUT {
                    stall = true;
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    if stall || cancelled {
        let _ = child.kill();
        let _ = child.wait();
        if let Some(handle) = stdout_thread {
            let _ = handle.join();
        }
        let stderr = stderr_thread.join().unwrap_or_default();
        if cancelled {
            bail!("Audio-DVD build cancelled");
        }
        bail!(
            "{FFMPEG_STALL_MESSAGE} for {STALL_TIMEOUT:?} (stalled filtergraph)\ncommand: {}\n{}",
            command_line(args),
            stderr.trim()
        );
    }

    if let Some(handle) = stdout_thread {
        let _ = handle.join();
    }
    let status = child.wait().context("wait for ffmpeg")?;
    let stderr = stderr_thread.join().unwrap_or_default();
    if !status.success() {
        bail!(
            "ffmpeg failed with {status}\ncommand: {}\n{}",
            command_line(args),
            stderr.trim()
        );
    }
    Ok(status)
}

/// Run FFmpeg, retrying only when the process stalls. Real encode failures and
/// user cancellations return immediately.
pub(crate) fn run_ffmpeg_retry(
    args: &[String],
    duration_secs: f64,
    progress: &mut dyn FnMut(f32),
    cancel: Option<&AtomicBool>,
    attempts: usize,
) -> Result<std::process::ExitStatus> {
    let attempts = attempts.max(1);
    let mut last_error = None;
    for attempt in 0..attempts {
        if is_cancelled(cancel) {
            bail!("Audio-DVD build cancelled");
        }
        match run_ffmpeg_cancellable(args, duration_secs, progress, cancel) {
            Ok(status) => return Ok(status),
            Err(error) => {
                let stalled = format!("{error:#}").contains(FFMPEG_STALL_MESSAGE);
                last_error = Some(error);
                if !stalled || is_cancelled(cancel) || attempt + 1 >= attempts {
                    break;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("ffmpeg did not run")))
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

fn report_progress_line(line: &str, duration_secs: f64, progress: &mut dyn FnMut(f32)) {
    if let Some(value) = line.strip_prefix("out_time_us=") {
        if let Ok(micros) = value.trim().parse::<u64>() {
            if duration_secs > 0.0 {
                let seconds = micros as f64 / 1_000_000.0;
                progress((seconds / duration_secs).clamp(0.0, 0.995) as f32);
            }
        }
    } else if line.starts_with("progress=end") {
        progress(1.0);
    }
}

/// Encode discrete 7.1 E-AC3. The caller must confirm `FfmpegInfo::eac3_7_1`.
#[allow(clippy::too_many_arguments)]
pub fn encode_eac3_7_1_with_progress(
    input: &Path,
    output: &Path,
    container: Container,
    duration_secs: f64,
    metadata: &[(String, String)],
    progress: &mut dyn FnMut(f32),
) -> Result<()> {
    progress(0.0);
    let args = build_7_1_eac3_args(input, output, container, metadata);
    run_ffmpeg(&args, duration_secs, progress)?;
    progress(1.0);
    Ok(())
}

/// Encode directly from the WAVEFORMATEXTENSIBLE master. The WAV channel mask
/// is authoritative; FFmpeg must not guess or remap channels.
#[allow(clippy::too_many_arguments)]
pub fn encode_with_progress(
    input: &Path,
    output: &Path,
    codec: Codec,
    dts_bitrate: u32,
    container: Container,
    duration_secs: f64,
    metadata: &[(String, String)],
    progress: &mut dyn FnMut(f32),
) -> Result<()> {
    progress(0.0);
    let args = build_args(input, output, codec, dts_bitrate, container, metadata);
    run_ffmpeg(&args, duration_secs, progress)?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn args_for(codec: Codec, dts_bitrate: u32, container: Container) -> Vec<String> {
        build_args(
            Path::new("mix.wav"),
            Path::new("out.mka"),
            codec,
            dts_bitrate,
            container,
            &[],
        )
    }

    fn has_pair(args: &[String], first: &str, second: &str) -> bool {
        args.windows(2)
            .any(|window| window[0] == first && window[1] == second)
    }

    fn assert_no_layout_filters(args: &[String]) {
        assert!(!args.iter().any(|arg| arg == "-af"), "{args:?}");
        assert!(!args.iter().any(|arg| arg == "-channel_layout"), "{args:?}");
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("channelmap") || arg.contains("pan=")),
            "{args:?}"
        );
    }

    #[test]
    fn ac3_uses_direct_wav_without_filters() {
        let args = args_for(Codec::Ac3, DTS_BITRATE_HIGH, Container::Mka);
        assert_no_layout_filters(&args);
        assert!(has_pair(&args, "-c:a", "ac3"));
        assert!(has_pair(&args, "-b:a", "640k"));
        assert!(!args.contains(&"-strict".to_string()));
    }

    #[test]
    fn eac3_uses_direct_wav_without_filters() {
        let args = args_for(Codec::Eac3, DTS_BITRATE_HIGH, Container::Mka);
        assert_no_layout_filters(&args);
        assert!(has_pair(&args, "-c:a", "eac3"));
        assert!(has_pair(&args, "-b:a", "640k"));
    }

    #[test]
    fn dts_uses_strict_and_selected_bitrate() {
        let high = args_for(Codec::Dts, DTS_BITRATE_HIGH, Container::Mka);
        assert_no_layout_filters(&high);
        assert!(has_pair(&high, "-strict", "-2"));
        assert!(has_pair(&high, "-c:a", "dca"));
        assert!(has_pair(&high, "-b:a", "1509k"));

        let low = args_for(Codec::Dts, DTS_BITRATE_LOW, Container::Mka);
        assert!(has_pair(&low, "-b:a", "768k"));
    }

    #[test]
    fn eac3_71_uses_direct_71_wav_and_768k() {
        let args = build_7_1_eac3_args(
            Path::new("mix71.wav"),
            Path::new("out.mka"),
            Container::Mka,
            &[],
        );
        assert_no_layout_filters(&args);
        assert!(has_pair(&args, "-c:a", "eac3"));
        assert!(has_pair(&args, "-b:a", "768k"));
    }

    #[test]
    fn raw_extensions_and_muxers_are_correct() {
        assert_eq!(output_extension(Codec::Ac3, Container::Raw), "ac3");
        assert_eq!(output_extension(Codec::Eac3, Container::Raw), "eac3");
        assert_eq!(output_extension(Codec::Dts, Container::Raw), "dts");
        assert_eq!(output_extension(Codec::Dts, Container::Mka), "mka");

        let args = build_args(
            &PathBuf::from("mix.wav"),
            &PathBuf::from("out.dts"),
            Codec::Dts,
            DTS_BITRATE_HIGH,
            Container::Raw,
            &[("title".into(), "ignored for raw".into())],
        );
        assert!(has_pair(&args, "-f", "dts"));
        assert!(!args.contains(&"-metadata".to_string()));
    }
}
