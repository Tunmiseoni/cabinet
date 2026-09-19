#!/bin/bash
# setup-linux — one-shot setup + build for a Linux (Arch/CachyOS) machine.
# Installs Tauri system deps, the Rust toolchain, and JS deps, then builds.
#
#   scripts/setup-linux.sh          # install deps + build a bundle
#   scripts/setup-linux.sh --dev    # install deps, then run in dev mode
#   scripts/setup-linux.sh --deps   # only install the system/pacman deps
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="build"
case "${1:-}" in
  --dev)  MODE="dev" ;;
  --deps) MODE="deps" ;;
  "")     ;;
  *) echo "usage: $0 [--dev|--deps]" >&2; exit 2 ;;
esac

echo "==> cabinet Linux setup ($MODE)"

if command -v pacman >/dev/null 2>&1; then
  echo "==> installing system dependencies (sudo pacman)"
  if ! sudo pacman -S --needed --noconfirm \
      webkit2gtk-4.1 base-devel curl wget file openssl \
      appmenu-gtk-module libappindicator-gtk3 librsvg xdotool; then
    echo "warning: pacman failed — install the Tauri Linux deps manually:" >&2
    echo "  sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \\" >&2
    echo "    appmenu-gtk-module libappindicator-gtk3 librsvg xdotool" >&2
  fi
  echo "==> installing runtime tools (node, npm, flatpak)"
  sudo pacman -S --needed --noconfirm nodejs npm flatpak \
    || echo "warning: could not install nodejs/npm/flatpak — install them manually if the build fails" >&2
else
  echo "warning: pacman not found — install the Tauri Linux deps manually:" >&2
  echo "  webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool" >&2
  echo "  plus nodejs npm flatpak" >&2
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "==> installing Rust via rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
command -v cargo >/dev/null 2>&1 || { echo "cargo still not found on PATH — open a new shell and rerun" >&2; exit 1; }

[ "$MODE" = "deps" ] && { echo "Dependencies installed."; exit 0; }

[ -d node_modules ] || npm install
[ -d frontend/node_modules ] || npm install --prefix frontend

if [ "$MODE" = "dev" ]; then
  exec "$ROOT/scripts/dev.sh"
fi

exec "$ROOT/scripts/build.sh"
