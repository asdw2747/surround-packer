# SurroundPacker v1.0.2

This release adds first-class support for UVR5 Vocal Split Mode imports.

## New

- **5-Stem Vocal Split quick preset** under `QUICK PRESETS`:
  - Lead Vocals → FC with the noise gate enabled at −32 dB / 250 ms.
  - Background Vocals → SL and SR at 0 dB.
  - Drums → FL and FR at 0 dB.
  - Other/Instrumental → FL/FR at 0 dB and SL/SR at −4.5 dB.
  - Bass → LFE at 0 dB.
- **Automatic Vocal Split detection on import** when the loaded set contains lead vocals, background vocals, drums, bass, and other/instrumental.
- Filename classification for `(Lead Vocals)`, `(Background Vocals)`, `(Backing Vocals)`, `backing_vocals`, `harmony`, and `harmonies`.
- When both `(Lead Vocals)` and a combined `(Vocals)` file are loaded, the combined file is left unrouted so the lead is never doubled. The combined stem remains available in the stem library.
- Auto-routing only applies to a fresh, unrouted import; existing manual routing is never overwritten.

## Downloads

- `SurroundPacker-1.0.2-x86_64.AppImage` — Linux x86-64.
- `SurroundPacker-1.0.2-x86_64.exe` — Windows x86-64, static CRT.

## Requirements

- The AppImage is built on CachyOS with glibc 2.44 and requires **glibc 2.43 or newer**.
- Windows requires no Visual C++ Redistributable.
- `ffmpeg` is required for AC-3, E-AC-3, DTS, and Audio-DVD title encoding.
- Audio-DVD authoring requires `mplex` (mjpegtools), `dvdauthor`, and an ISO tool (`xorriso`, `mkisofs`, or `genisoimage`).

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (56 unit, 5 channel-mapping, 8 end-to-end)
- Linux and static-CRT Windows release builds
- AppImage smoke test

See `README.md` for full usage. Third-party notices are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`.
