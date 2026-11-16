# SurroundPacker v1.0.1

This release fixes the 24-bit FLAC compression bug from v1.0.0.

## FLAC fix

- Replaced the `flacenc` 0.5.1 encoder with **`libflac-rs` 0.143.1**, a pure-Rust, bit-exact port of libFLAC 1.4.3.
- FLAC exports now use genuine prediction and Rice entropy coding instead of pathological uncompressed frames.
- A 10-second 48 kHz 24-bit 5.1 file is now under 8 MB and decodes bit-identically with the audio MD5 intact.
- `WAVEFORMATEXTENSIBLE_CHANNEL_MASK` (`0x003F` for 5.1, `0x063F` for 7.1) and Vorbis tags are preserved.
- Fixed the upstream 24-bit Rice overflow that produced multi-GB frames when a block mixed high-entropy audio with silence.

## Downloads

- `SurroundPacker-1.0.1-x86_64.AppImage` — Linux x86-64.
- `SurroundPacker-1.0.1-x86_64.exe` — Windows x86-64, static CRT.

## Requirements

- The AppImage is built on CachyOS with glibc 2.44 and requires **glibc 2.43 or newer**.
- Windows requires no Visual C++ Redistributable.
- `ffmpeg` is required for AC-3, E-AC-3, DTS, and Audio-DVD title encoding.
- Audio-DVD authoring requires `mplex` (mjpegtools), `dvdauthor`, and an ISO tool (`xorriso`, `mkisofs`, or `genisoimage`).

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (including 10-second FLAC compression and bit-exact decode tests)
- Linux and static-CRT Windows release builds
- AppImage smoke test

See `README.md` for full usage. Third-party notices are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`.
