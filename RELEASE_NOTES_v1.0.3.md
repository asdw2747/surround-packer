# SurroundPacker v1.0.3

This release fixes UVR5 Vocal Split role detection and preset labels.

## Fixes

- Stem roles are now read from the **trailing** parenthesized or suffixed UVR5 tag, ignoring earlier title metadata such as `(feat. Artist)`.
- Recognized roles include Lead Vocals, Background Vocals, Vocals, Drums, Bass, Other, and Instrumental (mapped to Other).
- Stem library cards show the exact role label, e.g. `Lead Vocals · 2 ch · 2:57` and `Background Vocals · 2 ch · 2:57`.
- Lead Vocals and Background Vocals now use distinct dot/waveform colors.
- Auto-routing clears existing routes and applies the 5-Stem Vocal Split layout when a complete set is loaded.
- A redundant combined `(Vocals)` file is never routed when Lead Vocals and Background Vocals are present.

## Routing

- Lead Vocals → FC, gate enabled at −32 dB / 250 ms
- Background Vocals → SL/SR at 0 dB
- Drums → FL/FR at 0 dB
- Other → FL/FR at 0 dB and SL/SR at −4.5 dB
- Bass → LFE at 0 dB

## Downloads

- `SurroundPacker-1.0.3-x86_64.AppImage` — Linux x86-64.
- `SurroundPacker-1.0.3-x86_64.exe` — Windows x86-64, static CRT.

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
