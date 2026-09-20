#!/bin/bash
# tauri-build — wrapper around `tauri build`, used by tauri-action's
# `tauriScript` input (see .github/workflows/release.yml).
#
# It runs the Tauri CLI with the arguments the action passes, then applies the
# Linux AppImage workaround *inside* the same command, so the patched AppImage
# is what tauri-action uploads (no need to replace the release asset).
#
# This wrapper is intentionally generic and harmless to keep forever: if
# scripts/patch-appimage.sh is deleted (its header explains when), the second
# step simply does nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ -x "$ROOT/node_modules/.bin/tauri" ]; then
  "$ROOT/node_modules/.bin/tauri" "$@"
elif command -v tauri >/dev/null 2>&1; then
  tauri "$@"
else
  echo "tauri-build: Tauri CLI not found; run 'npm install' first" >&2
  exit 1
fi

if [ -x "$ROOT/scripts/patch-appimage.sh" ]; then
  "$ROOT/scripts/patch-appimage.sh"
fi
