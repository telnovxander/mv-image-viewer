#!/usr/bin/env bash
# Rebuild all app icons from assets/brand: the full drawing for 32 px and up, the simplified one
# for 16 and 24 px (MV/BRAND.md). Needs rsvg-convert (librsvg2-bin) and npm dependencies.
set -euo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rsvg-convert -w 1024 -h 1024 assets/brand/icon.svg -o "$work/icon.png"
npx tauri icon "$work/icon.png" -o src-tauri/icons
rm -rf src-tauri/icons/android src-tauri/icons/ios
for size in 16 24; do rsvg-convert -w $size -h $size assets/brand/icon-small.svg -o "$work/$size.png"; done
for size in 32 48 256; do rsvg-convert -w $size -h $size assets/brand/icon.svg -o "$work/$size.png"; done
node scripts/build-ico.mjs src-tauri/icons/icon.ico "$work"/16.png "$work"/24.png "$work"/32.png "$work"/48.png "$work"/256.png
