#!/bin/bash
# clean — remove regenerable build output.
#
#   scripts/clean.sh          build artifacts (Rust target, Vite dist, generated schemas)
#   scripts/clean.sh --deps   also remove installed dependencies (node_modules)
#   scripts/clean.sh --wine   also stop stray Wine/emulator processes from live tests
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WITH_DEPS=0
WITH_WINE=0
for arg in "$@"; do
  case "$arg" in
    --deps) WITH_DEPS=1 ;;
    --wine) WITH_WINE=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

echo "==> build artifacts"
rm -rf src-tauri/target src-tauri/gen frontend/dist frontend/.vite
echo "removed src-tauri/{target,gen}, frontend/{dist,.vite}"

if [ "$WITH_DEPS" = "1" ]; then
  echo "==> dependencies"
  rm -rf node_modules frontend/node_modules
  echo "removed node_modules, frontend/node_modules (run npm install to restore)"
fi

if [ "$WITH_WINE" = "1" ]; then
  echo "==> stray Wine/emulator processes"
  if pgrep -f 'FightCade2.app/Contents/MacOS/Fightcade' >/dev/null 2>&1; then
    echo "FightCade is running — not touching Wine processes" >&2
  else
    pkill -f 'fcadefbneo' 2>/dev/null || true
    "/Applications/FightCade2.app/Contents/Resources/wine/bin/wineserver" -k 2>/dev/null || true
    sleep 1
    pkill -9 -f 'wine32on64-preloader' 2>/dev/null || true
    echo "stopped stray Wine/emulator processes"
  fi
fi
