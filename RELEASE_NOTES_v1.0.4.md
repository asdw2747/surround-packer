# SurroundPacker v1.0.4

This release removes automatic speaker routing when audio files are loaded.

## Changes

- Loading stems from **+ Load audio stems**, the file library, or general drag-and-drop now:
  - decodes and classifies the role,
  - shows the exact role in the stem card subtitle, and
  - leaves every speaker slot empty.
- Routing now happens only when the user:
  - clicks a **QUICK PRESETS** button (`5-Stem Vocal Split`, `Vocals + Drums Front`, or `Classic Quad`),
  - loads a saved **.json** template, or
  - explicitly drops a stem onto a specific speaker.
- The `--preset` startup flag remains an explicit command-line routing choice.
- Role classification, exact labels, and the distinct Lead/Background Vocals colors are unchanged.
- The five/six-stem Vocal Split mapping and redundant combined-`(Vocals)` suppression remain available through the Quick Preset.

## Downloads

- `SurroundPacker-1.0.4-x86_64.AppImage` — Linux x86-64.
- `SurroundPacker-1.0.4-x86_64.exe` — Windows x86-64, static CRT.

## Requirements

- The AppImage is built on CachyOS with glibc 2.44 and requires **glibc 2.43 or newer**.
- Windows requires no Visual C++ Redistributable.
- `ffmpeg` is required for AC-3, E-AC-3, DTS, and Audio-DVD title encoding.
- Audio-DVD authoring requires `mplex` (mjpegtools), `dvdauthor`, and an ISO tool (`xorriso`, `mkisofs`, or `genisoimage`).

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- Linux and static-CRT Windows release builds
- AppImage smoke test

See `README.md` for full usage. Third-party notices are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`.
