#!/usr/bin/env bash
# Builds the StateLab **browser-host** distributable: a self-contained
# StateLabServer.exe with the production UI embedded, plus docs, staged into
# dist-package/.
#
# This is the WebView-free alternative host. For the primary desktop app and its
# MSI/NSIS installers, use `npm run tauri build` instead (see docs/PACKAGING.md).
#
# Usage:  bash scripts/package.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUT="dist-package"
VERSION="$(node -p "require('./package.json').version" 2>/dev/null || echo 0.1.0)"

echo "==> Building the UI and embedding it into the app crate"
npm run sync-ui

echo "==> Building StateLabServer.exe (release, static CRT)"
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release -p statelab-app

echo "==> Staging $OUT/"
rm -rf "$OUT"
mkdir -p "$OUT/docs"
cp target/release/StateLabServer.exe "$OUT/"
cp README.md "$OUT/"
cp docs/ARCHITECTURE.md docs/PACKAGING.md docs/PERFORMANCE.md "$OUT/docs/"
mkdir -p "$OUT/docs/schema"
cp docs/schema/README.md "$OUT/docs/schema/"

cat > "$OUT/HOW-TO-RUN.txt" <<'EOF'
StateLab (browser host)
=======================

Double-click StateLabServer.exe.

A small console window opens showing a local address, and your default browser
launches with the app. Enter a number and click "Run trajectory".

Close the console window to quit.

The executable is self-contained: no installer, no runtime, no network access.
It listens only on 127.0.0.1 (loopback) on an ephemeral port.

Prefer the desktop app? Install StateLab from the MSI or NSIS installer instead —
same engine and UI, in a native window, no browser required.
EOF

echo "==> Package contents:"
find "$OUT" -type f | sort | sed 's/^/    /'

SIZE="$(du -sh "$OUT" | cut -f1)"
echo "==> Staged $OUT/ ($SIZE), version $VERSION"
echo "    Zip it with:  powershell -Command \"Compress-Archive -Path '$OUT/*' -DestinationPath 'statelab-$VERSION.zip' -Force\""
