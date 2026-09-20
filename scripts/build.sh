#!/bin/bash
# build — produce a production The Cabinet bundle for the current OS.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
command -v cargo >/dev/null 2>&1 || { echo "cargo not found — run the rustup installer (see AGENTS.md)" >&2; exit 1; }

[ -d node_modules ] || npm install
[ -d frontend/node_modules ] || npm install --prefix frontend

npm run tauri build

# Linux AppImage workaround: strip the over-bundled display-stack libraries so
# WebKit can create an EGL display on Mesa 25+ (see scripts/patch-appimage.sh,
# which documents when this can be removed). No-op off Linux / once upstream
# excludes those libraries.
if [ -x "$ROOT/scripts/patch-appimage.sh" ]; then
  "$ROOT/scripts/patch-appimage.sh"
fi

echo
echo "Bundle:"
find src-tauri/target/release/bundle -maxdepth 3 -type d -name '*.app' -o -maxdepth 3 -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' -o -name '*.msi' -o -name '*-setup.exe' \) 2>/dev/null | sed 's/^/  /'
