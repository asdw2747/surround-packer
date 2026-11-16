# SurroundPacker v1.0.0

SurroundPacker is a native 5.1/7.1 surround stem router for UVR5 audio, with a visual speaker room and an Audio-DVD builder.

## Highlights

- SMPTE 5.1 and 7.1 routing with per-speaker gain, mute, solo, and layered sources.
- Per-speaker noise gate with adjustable threshold and release.
- LFE low-pass crossover, Haas surround/back delays, and a true-peak safety limiter.
- Native 24-bit WAV and FLAC export, plus AC-3, E-AC-3, and DTS 5.1 through FFmpeg.
- Audio-DVD builder with static title cards, AC-3 448 + optional DTS, valid NAV/VOBU packs via `mplex -f 8`, `VIDEO_TS` authoring, and a `VIDEO_TS`-only ISO.
- Live stereo headphone audition.
- Import/export routing templates as human-readable JSON.

## Downloads

- `SurroundPacker-1.0.0-x86_64.AppImage` — Linux x86-64.
- `SurroundPacker-1.0.0-x86_64.exe` — Windows x86-64, static CRT.

## Requirements

- The AppImage is built on CachyOS with glibc 2.44 and therefore requires **glibc 2.43 or newer**.
- Windows requires no Visual C++ Redistributable.
- `ffmpeg` is required for AC-3, E-AC-3, DTS, and Audio-DVD title encoding.
- Audio-DVD authoring requires `mplex` (mjpegtools), `dvdauthor`, and an ISO tool (`xorriso`, `mkisofs`, or `genisoimage`). These are external programs and are not bundled.

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (50 unit, 5 channel-mapping, 8 end-to-end)
- Linux and Windows release builds verified; Windows static CRT imports contain no `vcruntime`, `msvcp`, `ucrtbase`, or `api-ms-win-crt` DLLs.

See `README.md` for the full feature list and usage instructions. Third-party notices are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`.
