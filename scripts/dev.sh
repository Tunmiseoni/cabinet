#!/bin/bash
# dev — run The Cabinet in development (Vite on :1420 + Rust backend).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
command -v cargo >/dev/null 2>&1 || { echo "cargo not found — run the rustup installer (see AGENTS.md)" >&2; exit 1; }

[ -d node_modules ] || npm install
[ -d frontend/node_modules ] || npm install --prefix frontend

exec npm run tauri dev
