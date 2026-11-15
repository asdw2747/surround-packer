# Third-Party Notices

**SurroundPacker 1.0.0** is licensed under the MIT License. See [`LICENSE`](LICENSE).

SurroundPacker bundles Rust crates and fonts in the application binary, and invokes optional external media tools as separate programs. The notices below identify those components and their licenses. Full license texts are in [`LICENSES/`](LICENSES/).

This table was generated from the resolved dependency graph in `Cargo.lock` (313 packages). Where a crate offers a choice of licenses, SurroundPacker relies on the permissive option unless a component is specifically noted below.

## Rust crates

| Crate | Version | License (SPDX) | Direct dependency |
| --- | --- | --- | --- |
| `accesskit` | 0.25.2 | MIT OR Apache-2.0 |  |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |  |
| `ahash` | 0.8.12 | MIT OR Apache-2.0 |  |
| `alsa` | 0.11.0 | Apache-2.0/MIT |  |
| `alsa-sys` | 0.4.0 | MIT |  |
| `android-activity` | 0.6.1 | MIT OR Apache-2.0 |  |
| `android-properties` | 0.2.2 | MIT |  |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | yes |
| `arrayvec` | 0.7.8 | MIT OR Apache-2.0 |  |
| `as-raw-xcb-connection` | 1.0.1 | MIT OR Apache-2.0 |  |
| `atomic-waker` | 1.1.2 | Apache-2.0 OR MIT |  |
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT |  |
| `bit-set` | 0.10.0 | Apache-2.0 OR MIT |  |
| `bit-vec` | 0.9.1 | Apache-2.0 OR MIT |  |
| `bitflags` | 1.3.2 | MIT/Apache-2.0 |  |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |  |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |  |
| `block2` | 0.5.1 | MIT |  |
| `block2` | 0.6.2 | MIT |  |
| `built` | 0.7.7 | MIT |  |
| `bumpalo` | 3.20.4 | MIT OR Apache-2.0 |  |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT |  |
| `bytemuck_derive` | 1.12.1 | Zlib OR Apache-2.0 OR MIT |  |
| `byteorder` | 1.5.0 | Unlicense OR MIT |  |
| `byteorder-lite` | 0.1.0 | Unlicense OR MIT |  |
| `bytes` | 1.12.1 | MIT |  |
| `calloop` | 0.13.0 | MIT |  |
| `cc` | 1.11.2 | MIT OR Apache-2.0 |  |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |  |
| `cfg_aliases` | 0.2.2 | MIT |  |
| `cgl` | 0.3.2 | MIT / Apache-2.0 |  |
| `codespan-reporting` | 0.13.1 | Apache-2.0 |  |
| `color` | 0.3.3 | Apache-2.0 OR MIT |  |
| `combine` | 4.6.9 | MIT |  |
| `concurrent-queue` | 2.5.0 | Apache-2.0 OR MIT |  |
| `core-foundation` | 0.9.4 | MIT OR Apache-2.0 |  |
| `core-foundation-sys` | 0.8.7 | MIT OR Apache-2.0 |  |
| `core-graphics` | 0.23.2 | MIT OR Apache-2.0 |  |
| `core-graphics-types` | 0.1.3 | MIT OR Apache-2.0 |  |
| `core_detect` | 1.0.0 | MIT/Apache-2.0 |  |
| `coreaudio-rs` | 0.14.2 | MIT/Apache-2.0 |  |
| `cpal` | 0.18.2 | Apache-2.0 | yes |
| `crc` | 3.4.0 | MIT OR Apache-2.0 |  |
| `crc-catalog` | 2.5.0 | MIT OR Apache-2.0 |  |
| `crc32fast` | 1.5.5 | MIT OR Apache-2.0 |  |
| `crossbeam-channel` | 0.5.17 | MIT OR Apache-2.0 |  |
| `crossbeam-utils` | 0.8.23 | MIT OR Apache-2.0 |  |
| `crunchy` | 0.2.4 | MIT |  |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |  |
| `cursor-icon` | 1.2.0 | MIT OR Apache-2.0 OR Zlib |  |
| `dasp_sample` | 0.11.0 | MIT OR Apache-2.0 |  |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |  |
| `dispatch` | 0.2.0 | MIT |  |
| `dispatch2` | 0.3.1 | Zlib OR Apache-2.0 OR MIT |  |
| `dlib` | 0.5.3 | MIT |  |
| `document-features` | 0.2.12 | MIT OR Apache-2.0 |  |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT |  |
| `ecolor` | 0.37.0 | MIT OR Apache-2.0 |  |
| `eframe` | 0.37.0 | MIT OR Apache-2.0 | yes |
| `egui` | 0.37.0 | MIT OR Apache-2.0 | yes |
| `egui-wgpu` | 0.37.0 | MIT OR Apache-2.0 |  |
| `egui-winit` | 0.37.0 | MIT OR Apache-2.0 |  |
| `egui_glow` | 0.37.0 | MIT OR Apache-2.0 |  |
| `either` | 1.19.0 | MIT OR Apache-2.0 |  |
| `emath` | 0.37.0 | MIT OR Apache-2.0 |  |
| `encoding_rs` | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause |  |
| `epaint` | 0.37.0 | MIT OR Apache-2.0 |  |
| `epaint_default_fonts` | 0.37.0 | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 |  |
| `equivalent` | 1.0.3 | Apache-2.0 OR MIT |  |
| `errno` | 0.3.15 | MIT OR Apache-2.0 |  |
| `euclid` | 0.22.14 | MIT OR Apache-2.0 |  |
| `extended` | 0.1.0 | MIT |  |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |  |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 |  |
| `fearless_simd` | 0.4.1 | Apache-2.0 OR MIT |  |
| `find-msvc-tools` | 0.1.15 | MIT OR Apache-2.0 |  |
| `flacenc` | 0.5.1 | Apache-2.0 | yes |
| `flate2` | 1.1.11 | MIT OR Apache-2.0 |  |
| `foldhash` | 0.2.0 | Zlib |  |
| `font-types` | 0.12.7 | MIT OR Apache-2.0 |  |
| `foreign-types` | 0.5.0 | MIT/Apache-2.0 |  |
| `foreign-types-macros` | 0.2.4 | MIT/Apache-2.0 |  |
| `foreign-types-shared` | 0.3.1 | MIT/Apache-2.0 |  |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 |  |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 |  |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 |  |
| `generic-array` | 0.14.7 | MIT |  |
| `gethostname` | 1.1.0 | Apache-2.0 |  |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |  |
| `gl_generator` | 0.14.0 | Apache-2.0 |  |
| `glifo` | 0.3.0 | Apache-2.0 OR MIT |  |
| `glow` | 0.17.0 | MIT OR Apache-2.0 OR Zlib |  |
| `glutin` | 0.32.3 | Apache-2.0 |  |
| `glutin-winit` | 0.5.0 | MIT |  |
| `glutin_egl_sys` | 0.7.1 | Apache-2.0 |  |
| `glutin_glx_sys` | 0.6.1 | Apache-2.0 |  |
| `glutin_wgl_sys` | 0.6.1 | Apache-2.0 |  |
| `guillotiere` | 0.7.0 | MIT/Apache-2.0 |  |
| `half` | 2.7.1 | MIT OR Apache-2.0 |  |
| `harfrust` | 0.12.0 | MIT |  |
| `hash32` | 0.3.1 | MIT OR Apache-2.0 |  |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 |  |
| `heapless` | 0.8.0 | MIT OR Apache-2.0 |  |
| `hermit-abi` | 0.5.3 | MIT OR Apache-2.0 |  |
| `image` | 0.25.10 | MIT OR Apache-2.0 | yes |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT |  |
| `itertools` | 0.15.0 | MIT OR Apache-2.0 |  |
| `itoa` | 1.0.19 | MIT OR Apache-2.0 |  |
| `jni` | 0.22.4 | MIT OR Apache-2.0 |  |
| `jni-macros` | 0.22.4 | MIT OR Apache-2.0 |  |
| `jni-sys` | 0.3.1 | MIT OR Apache-2.0 |  |
| `jni-sys` | 0.4.1 | MIT OR Apache-2.0 |  |
| `jni-sys-macros` | 0.4.1 | MIT OR Apache-2.0 |  |
| `jobserver` | 0.1.35 | MIT OR Apache-2.0 |  |
| `js-sys` | 0.3.107 | MIT OR Apache-2.0 |  |
| `khronos_api` | 3.1.0 | Apache-2.0 |  |
| `kurbo` | 0.13.1 | Apache-2.0 OR MIT |  |
| `lazy_static` | 1.5.1 | MIT OR Apache-2.0 |  |
| `libc` | 0.2.191 | MIT OR Apache-2.0 |  |
| `libloading` | 0.8.9 | ISC |  |
| `libm` | 0.2.16 | MIT |  |
| `libredox` | 0.1.25 | MIT |  |
| `linebender_resource_handle` | 0.1.1 | Apache-2.0 OR MIT |  |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |  |
| `linux-raw-sys` | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |  |
| `litrs` | 1.0.1 | MIT OR Apache-2.0 |  |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 |  |
| `log` | 0.4.35 | MIT OR Apache-2.0 |  |
| `mach2` | 0.6.0 | BSD-2-Clause OR MIT OR Apache-2.0 |  |
| `md-5` | 0.10.6 | MIT OR Apache-2.0 |  |
| `memchr` | 2.8.3 | Unlicense OR MIT |  |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 |  |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 |  |
| `moxcms` | 0.8.1 | BSD-3-Clause OR Apache-2.0 |  |
| `multiversion_no_op` | 1.0.0 | Apache-2.0 OR MIT |  |
| `naga` | 30.0.2 | MIT OR Apache-2.0 |  |
| `naga-types` | 30.0.2 | MIT OR Apache-2.0 |  |
| `ndk` | 0.9.0 | MIT OR Apache-2.0 |  |
| `ndk-context` | 0.1.1 | MIT OR Apache-2.0 |  |
| `ndk-sys` | 0.6.0+11769913 | MIT OR Apache-2.0 |  |
| `nohash-hasher` | 0.2.0 | Apache-2.0 OR MIT |  |
| `num-derive` | 0.4.2 | MIT OR Apache-2.0 |  |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 |  |
| `num_enum` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |  |
| `num_enum_derive` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |  |
| `objc-sys` | 0.3.5 | MIT |  |
| `objc2` | 0.5.3 | MIT |  |
| `objc2` | 0.6.5 | MIT |  |
| `objc2-app-kit` | 0.2.2 | MIT |  |
| `objc2-app-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-audio-toolbox` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-avf-audio` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-cloud-kit` | 0.2.2 | MIT |  |
| `objc2-contacts` | 0.2.2 | MIT |  |
| `objc2-core-audio` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-core-audio-types` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-core-data` | 0.2.2 | MIT |  |
| `objc2-core-foundation` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-core-image` | 0.2.2 | MIT |  |
| `objc2-core-location` | 0.2.2 | MIT |  |
| `objc2-encode` | 4.1.0 | MIT |  |
| `objc2-foundation` | 0.2.2 | MIT |  |
| `objc2-foundation` | 0.3.2 | MIT |  |
| `objc2-link-presentation` | 0.2.2 | MIT |  |
| `objc2-metal` | 0.2.2 | MIT |  |
| `objc2-quartz-core` | 0.2.2 | MIT |  |
| `objc2-symbols` | 0.2.2 | MIT |  |
| `objc2-ui-kit` | 0.2.2 | MIT |  |
| `objc2-ui-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-uniform-type-identifiers` | 0.2.2 | MIT |  |
| `objc2-uniform-type-identifiers` | 0.3.2 | Zlib OR Apache-2.0 OR MIT |  |
| `objc2-user-notifications` | 0.2.2 | MIT |  |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |  |
| `orbclient` | 0.3.55 | MIT |  |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 |  |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 |  |
| `peniko` | 0.6.1 | Apache-2.0 OR MIT |  |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 |  |
| `pin-project` | 1.1.13 | Apache-2.0 OR MIT |  |
| `pin-project-internal` | 1.1.13 | Apache-2.0 OR MIT |  |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT |  |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 |  |
| `plain` | 0.2.3 | MIT/Apache-2.0 |  |
| `png` | 0.18.1 | MIT OR Apache-2.0 |  |
| `polling` | 3.11.0 | Apache-2.0 OR MIT |  |
| `pollster` | 1.0.1 | Apache-2.0/MIT |  |
| `polycool` | 0.4.0 | MIT OR Apache-2.0 |  |
| `portable-atomic` | 1.15.0 | Apache-2.0 OR MIT |  |
| `portable-atomic-util` | 0.2.8 | Apache-2.0 OR MIT |  |
| `proc-macro-crate` | 3.5.0 | MIT OR Apache-2.0 |  |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |  |
| `profiling` | 1.0.18 | MIT OR Apache-2.0 |  |
| `pxfm` | 0.1.30 | BSD-3-Clause OR Apache-2.0 |  |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |  |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |  |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib |  |
| `read-fonts` | 0.41.0 | MIT OR Apache-2.0 |  |
| `redox_syscall` | 0.4.1 | MIT |  |
| `redox_syscall` | 0.5.18 | MIT |  |
| `redox_syscall` | 0.9.5 | MIT |  |
| `renderdoc-sys` | 1.1.0 | MIT OR Apache-2.0 |  |
| `rfd` | 0.18.0 | MIT | yes |
| `rustc-hash` | 1.1.0 | Apache-2.0/MIT |  |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |  |
| `rustc_version` | 0.4.1 | MIT OR Apache-2.0 |  |
| `rustix` | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |  |
| `rustix` | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |  |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 |  |
| `same-file` | 1.0.6 | Unlicense/MIT |  |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 |  |
| `self_cell` | 1.3.0 | Apache-2.0 OR GPL-2.0-only |  |
| `semver` | 1.0.28 | MIT OR Apache-2.0 |  |
| `send_wrapper` | 0.6.0 | MIT/Apache-2.0 |  |
| `seq-macro` | 0.3.6 | MIT OR Apache-2.0 |  |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | yes |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |  |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |  |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | yes |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 |  |
| `simd-adler32` | 0.3.10 | MIT |  |
| `simd_cesu8` | 1.2.0 | Apache-2.0 OR MIT |  |
| `simdutf8` | 0.1.5 | MIT OR Apache-2.0 |  |
| `skrifa` | 0.44.0 | MIT OR Apache-2.0 |  |
| `slab` | 0.4.12 | MIT |  |
| `slotmap` | 1.1.1 | Zlib |  |
| `smallvec` | 1.16.5 | MIT OR Apache-2.0 |  |
| `smol_str` | 0.2.2 | MIT OR Apache-2.0 |  |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 |  |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 |  |
| `symphonia` | 0.5.5 | MPL-2.0 | yes |
| `symphonia-bundle-flac` | 0.5.5 | MPL-2.0 |  |
| `symphonia-bundle-mp3` | 0.5.5 | MPL-2.0 |  |
| `symphonia-codec-pcm` | 0.5.5 | MPL-2.0 |  |
| `symphonia-codec-vorbis` | 0.5.5 | MPL-2.0 |  |
| `symphonia-core` | 0.5.5 | MPL-2.0 |  |
| `symphonia-format-ogg` | 0.5.5 | MPL-2.0 |  |
| `symphonia-format-riff` | 0.5.5 | MPL-2.0 |  |
| `symphonia-metadata` | 0.5.5 | MPL-2.0 |  |
| `symphonia-utils-xiph` | 0.5.5 | MPL-2.0 |  |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |  |
| `syn` | 3.0.10 | MIT OR Apache-2.0 |  |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | yes |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 |  |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 |  |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 |  |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 |  |
| `tokio` | 1.54.1 | MIT |  |
| `toml_datetime` | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |  |
| `toml_edit` | 0.25.19+spec-1.1.0 | MIT OR Apache-2.0 |  |
| `toml_parser` | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |  |
| `tracing` | 0.1.44 | MIT |  |
| `tracing-core` | 0.1.36 | MIT |  |
| `type-map` | 0.5.1 | MIT/Apache-2.0 |  |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |  |
| `unicode-general-category` | 1.1.0 | Apache-2.0 |  |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |  |
| `unicode-properties` | 0.1.5 | (MIT OR Apache-2.0) AND Unicode-3.0 |  |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 |  |
| `unicode-width` | 0.2.3 | MIT OR Apache-2.0 |  |
| `uuid` | 1.29.2 | Apache-2.0 OR MIT |  |
| `vello_common` | 0.2.0 | Apache-2.0 OR MIT |  |
| `vello_cpu` | 0.2.0 | Apache-2.0 OR MIT |  |
| `version_check` | 0.9.5 | MIT/Apache-2.0 |  |
| `walkdir` | 2.5.0 | Unlicense/MIT |  |
| `wasm-bindgen` | 0.2.130 | MIT OR Apache-2.0 |  |
| `wasm-bindgen-futures` | 0.4.80 | MIT OR Apache-2.0 |  |
| `wasm-bindgen-macro` | 0.2.130 | MIT OR Apache-2.0 |  |
| `wasm-bindgen-macro-support` | 0.2.130 | MIT OR Apache-2.0 |  |
| `wasm-bindgen-shared` | 0.2.130 | MIT OR Apache-2.0 |  |
| `web-sys` | 0.3.107 | MIT OR Apache-2.0 |  |
| `web-time` | 1.1.0 | MIT OR Apache-2.0 |  |
| `wgpu` | 30.0.2 | MIT OR Apache-2.0 |  |
| `wgpu-core` | 30.0.2 | MIT OR Apache-2.0 |  |
| `wgpu-core-deps-windows-linux-android` | 30.0.2 | MIT OR Apache-2.0 |  |
| `wgpu-hal` | 30.0.2 | MIT OR Apache-2.0 |  |
| `wgpu-naga-bridge` | 30.0.2 | MIT OR Apache-2.0 |  |
| `wgpu-types` | 30.0.2 | MIT OR Apache-2.0 |  |
| `winapi-util` | 0.1.11 | Unlicense OR MIT |  |
| `windows` | 0.62.2 | MIT OR Apache-2.0 |  |
| `windows-collections` | 0.3.2 | MIT OR Apache-2.0 |  |
| `windows-core` | 0.62.2 | MIT OR Apache-2.0 |  |
| `windows-future` | 0.3.2 | MIT OR Apache-2.0 |  |
| `windows-implement` | 0.60.2 | MIT OR Apache-2.0 |  |
| `windows-interface` | 0.59.3 | MIT OR Apache-2.0 |  |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |  |
| `windows-numerics` | 0.3.1 | MIT OR Apache-2.0 |  |
| `windows-result` | 0.4.1 | MIT OR Apache-2.0 |  |
| `windows-strings` | 0.5.1 | MIT OR Apache-2.0 |  |
| `windows-sys` | 0.52.0 | MIT OR Apache-2.0 |  |
| `windows-sys` | 0.59.0 | MIT OR Apache-2.0 |  |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |  |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows-threading` | 0.2.1 | MIT OR Apache-2.0 |  |
| `windows_aarch64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_aarch64_msvc` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_i686_gnu` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_i686_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_i686_msvc` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_x86_64_gnu` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_x86_64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 |  |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 |  |
| `winit` | 0.30.13 | Apache-2.0 |  |
| `winnow` | 1.0.5 | MIT |  |
| `x11-dl` | 2.21.0 | MIT |  |
| `x11rb` | 0.13.2 | MIT OR Apache-2.0 |  |
| `x11rb-protocol` | 0.13.2 | MIT OR Apache-2.0 |  |
| `xkbcommon-dl` | 0.4.2 | MIT |  |
| `xkeysym` | 0.2.1 | MIT OR Apache-2.0 OR Zlib |  |
| `xml-rs` | 0.8.29 | MIT |  |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |  |
| `zerocopy-derive` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |  |
| `zlib-rs` | 0.6.8 | Zlib |  |
| `zmij` | 1.0.23 | MIT |  |

### License choices and special notices

- `self_cell` is offered under `Apache-2.0 OR GPL-2.0-only`; SurroundPacker uses the **Apache-2.0** option.
- `r-efi` is offered under `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; SurroundPacker uses the **MIT/Apache-2.0** options.
- **Symphonia** (`symphonia` and its `symphonia-*` crates) is licensed under **MPL-2.0**. The source for those crates is available from crates.io and the Symphonia repository. SurroundPacker does not modify them.
- `encoding_rs` includes BSD-3-Clause (WHATWG) code; the BSD-3-Clause text is included in `LICENSES/BSD-3-Clause.txt`.
- `unicode-ident` and `unicode-properties` include Unicode-3.0 data; the Unicode license text is included in `LICENSES/Unicode-3.0.txt`.
- The Rust standard library is distributed under `MIT OR Apache-2.0`. The Windows build statically links the Microsoft C runtime under the Microsoft Visual Studio license terms.

## Embedded fonts

The GUI uses the default fonts shipped by `epaint_default_fonts` 0.37.0 (part of egui):

| Font | License | License text |
| --- | --- | --- |
| Ubuntu Light | Ubuntu Font License 1.0 | `LICENSES/Ubuntu-font-1.0.txt` |
| Noto Emoji | SIL Open Font License 1.1 | `LICENSES/OFL-1.1.txt` |
| Hack | MIT / Bitstream Vera | `LICENSES/Hack-font.txt` |
| emoji-icon-font | MIT | `LICENSES/emoji-icon-font-MIT.txt` |
| egui-icons | egui MIT/Apache-2.0 | `LICENSES/MIT.txt`, `LICENSES/Apache-2.0.txt` |

## External tools (not bundled)

Audio-DVD authoring is optional and requires tools installed by the user. These programs are executed as separate processes; SurroundPacker does not link against or redistribute them.

| Tool | Typical license | Purpose |
| --- | --- | --- |
| FFmpeg | LGPL-2.1-or-later by default; GPL-2.0-or-later when built with `--enable-gpl` | AC-3/E-AC-3/DTS encoding, MPEG-2 title encoding, Audio-DVD audio |
| mplex (mjpegtools) | GPL-2.0-or-later | DVD NAV/VOBU multiplexing (`mplex -f 8`) |
| dvdauthor | GPL-2.0-or-later | VIDEO_TS/IFO authoring |
| xorriso | GPL-3.0-or-later | ISO 9660/UDF disc image creation |
| mkisofs (cdrtools) | CDDL-1.0 or GPL-2.0 | ISO 9660/UDF disc image creation |
| genisoimage | GPL-2.0-or-later | ISO 9660/UDF disc image creation |

FFmpeg, mplex, dvdauthor, xorriso, mkisofs, and genisoimage are not distributed with SurroundPacker. Their licenses and source distributions are available from their respective projects.

## Full license texts

The following texts are included in `LICENSES/`:

- `LICENSES/0BSD.txt`
- `LICENSES/Apache-2.0.txt`
- `LICENSES/BSD-2-Clause.txt`
- `LICENSES/BSD-3-Clause.txt`
- `LICENSES/Hack-font.txt`
- `LICENSES/ISC.txt`
- `LICENSES/MIT.txt`
- `LICENSES/MPL-2.0.txt`
- `LICENSES/OFL-1.1.txt`
- `LICENSES/Ubuntu-font-1.0.txt`
- `LICENSES/Unicode-3.0.txt`
- `LICENSES/Unlicense.txt`
- `LICENSES/Zlib.txt`
- `LICENSES/emoji-icon-font-MIT.txt`

For any license not reproduced here, the SPDX identifier above refers to the standard license text published by SPDX and the corresponding upstream package.
