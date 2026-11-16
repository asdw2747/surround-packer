#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="${ARCH:-x86_64}"
BIN="$ROOT/target/release/surround-packer"
APPDIR="$ROOT/target/appimage/SurroundPacker.AppDir"
OUT="$ROOT/dist/SurroundPacker-${VERSION}-${ARCH}.AppImage"

if [ ! -x "$BIN" ]; then
    echo "missing $BIN; run: cargo build --release" >&2
    exit 1
fi
if ! command -v appimagetool >/dev/null 2>&1; then
    echo "appimagetool was not found on PATH" >&2
    exit 1
fi

rm -rf "$APPDIR" "$OUT"
mkdir -p \
    "$APPDIR/usr/bin" \
    "$APPDIR/usr/share/doc/surround-packer" \
    "$APPDIR/usr/share/licenses/surround-packer"

install -m755 "$BIN" "$APPDIR/usr/bin/surround-packer"
install -m755 "$ROOT/packaging/appimage/AppRun" "$APPDIR/AppRun"
install -m644 "$ROOT/packaging/appimage/surround-packer.desktop" \
    "$APPDIR/surround-packer.desktop"
install -m644 "$ROOT/assets/icon.png" "$APPDIR/surround-packer.png"
install -m644 "$ROOT/LICENSE" \
    "$APPDIR/usr/share/licenses/surround-packer/LICENSE"
install -m644 "$ROOT/THIRD_PARTY_NOTICES.md" \
    "$APPDIR/usr/share/doc/surround-packer/THIRD_PARTY_NOTICES.md"
cp -R "$ROOT/LICENSES" \
    "$APPDIR/usr/share/licenses/surround-packer/LICENSES"

mkdir -p "$ROOT/dist"
ARCH="$ARCH" appimagetool --no-appstream "$APPDIR" "$OUT"
echo "Built $OUT"
