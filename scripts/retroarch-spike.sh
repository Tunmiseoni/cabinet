#!/bin/bash
# retroarch-spike — Phase 0 RetroArch netplay/spectator spike harness.
#
# See docs/07-retroarch-spike.md. macOS + Linux (bash); Windows friends follow
# the documented commands instead. Nothing here writes to the real RetroArch
# config: every role gets its own copy under $RETROARCH_SPIKE_DIR.
#
# Usage:
#   scripts/retroarch-spike.sh parity [manifest]
#   scripts/retroarch-spike.sh smoke [frames]
#   scripts/retroarch-spike.sh host [port]
#   scripts/retroarch-spike.sh client <host-ip> [port]
#   scripts/retroarch-spike.sh spectator <host-ip> [port]
#   scripts/retroarch-spike.sh spectator2 <host-ip> [port]
#   scripts/retroarch-spike.sh measure [seconds]
#   scripts/retroarch-spike.sh status
#   scripts/retroarch-spike.sh stop
#
# Overrides: RETROARCH_SPIKE_DIR, RETROARCH_BIN, RETROARCH_CORE, RETROARCH_ROM.
set -euo pipefail

WORK="${RETROARCH_SPIKE_DIR:-${TMPDIR:-/tmp}/retroarch-spike}"
PORT_DEFAULT=55435
FRAMES_DEFAULT=600
MEASURE_DEFAULT=15

# --- per-OS defaults --------------------------------------------------------
case "$(uname -s)" in
  Darwin)
    OS_TAG="macOS arm64"
    RA_BIN_DEFAULT="/Applications/RetroArch.app/Contents/MacOS/RetroArch"
    RA_CFG_DEFAULT="$HOME/Library/Application Support/RetroArch/config/retroarch.cfg"
    CORE_DIR_DEFAULT="$HOME/Library/Application Support/RetroArch/cores"
    CORE_NAME="fbneo_libretro.dylib"
    ROM_DEFAULT="/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs/sfiii3nr1.zip"
    ;;
  Linux)
    OS_TAG="Linux $(uname -m)"
    RA_BIN_DEFAULT="$(command -v retroarch || true)"
    RA_CFG_DEFAULT="${XDG_CONFIG_HOME:-$HOME/.config}/retroarch/retroarch.cfg"
    CORE_DIR_DEFAULT="$HOME/.config/retroarch/cores"
    CORE_NAME="fbneo_libretro.so"
    ROM_DEFAULT="$HOME/.var/app/com.fightcade.Fightcade/data/ROMs/fbneo/sfiii3nr1.zip"
    ;;
  *)
    echo "unsupported OS $(uname -s) — use the documented commands on Windows" >&2
    exit 1
    ;;
esac

RA_BIN="${RETROARCH_BIN:-$RA_BIN_DEFAULT}"
RA_CFG="${RETROARCH_CFG:-$RA_CFG_DEFAULT}"
CORE="${RETROARCH_CORE:-$CORE_DIR_DEFAULT/$CORE_NAME}"
ROM="${RETROARCH_ROM:-$ROM_DEFAULT}"

# The standalone macOS Tailscale app bundles the GUI + CLI and infers which to
# run from the environment; force CLI mode so spike pings don't spawn icons.
[ "$(uname -s)" = "Darwin" ] && export TAILSCALE_BE_CLI=1

# --- helpers ----------------------------------------------------------------
die() { echo "error: $*" >&2; exit 1; }

require_bin() {
  [ -n "${RA_BIN:-}" ] && [ -x "$RA_BIN" ] || die "RetroArch binary not found (set RETROARCH_BIN); tried: ${RA_BIN:-<none>}"
  [ -f "$CORE" ] || die "core not found: $CORE"
  [ -f "$ROM" ] || die "ROM not found: $ROM (set RETROARCH_ROM)"
}

prepare_base() {
  mkdir -p "$WORK"
  if [ ! -f "$WORK/base.cfg" ]; then
    if [ -f "$RA_CFG" ]; then cp "$RA_CFG" "$WORK/base.cfg"; else : > "$WORK/base.cfg"; fi
  fi
}

write_role() {
  local role="$1" extra="${2:-}"
  mkdir -p "$WORK/$role/saves" "$WORK/$role/states"
  cat > "$WORK/$role/overrides.cfg" <<EOF
config_save_on_exit = "false"
video_fullscreen = "false"
pause_nonactive = "false"
netplay_nat_traversal = "false"
netplay_public_announce = "false"
netplay_check_frames = "600"
netplay_ping_show = "true"
netplay_allow_slaves = "true"
netplay_require_slaves = "false"
netplay_max_connections = "8"
savefile_directory = "$WORK/$role/saves"
savestate_directory = "$WORK/$role/states"
netplay_nickname = "spike-$role"
$extra
EOF
}

launch_bg() {
  local role="$1" extra="${2:-}"; shift 2
  prepare_base
  write_role "$role" "$extra"
  "$RA_BIN" -c "$WORK/base.cfg" --appendconfig "$WORK/$role/overrides.cfg" \
    -L "$CORE" "$ROM" --verbose --log-file "$WORK/$role/retroarch.log" \
    "$@" >/dev/null 2>&1 &
  echo $! > "$WORK/$role/$role.pid"
  echo "started $role (pid $(cat "$WORK/$role/$role.pid")) — log: $WORK/$role/retroarch.log"
}

spike_pids() { pgrep -f "$WORK/base.cfg" 2>/dev/null || true; }

# --- commands ---------------------------------------------------------------
cmd_parity() {
  local manifest="${1:-}"
  local ra_ver core_git core_sha rom_sha
  ra_ver="$("$RA_BIN" --version 2>/dev/null | grep -m1 -aE '^Version:' || "$RA_BIN" --version 2>/dev/null | head -1)"
  core_git="$(strings "$CORE" | grep -aoE 'GIT[0-9a-f]+' | sort -u | head -1)"
  core_sha="$(shasum -a 256 "$CORE" | awk '{print $1}')"
  rom_sha="$(shasum -a 256 "$ROM" | awk '{print $1}')"
  echo "OS:          $OS_TAG"
  echo "RetroArch:   $ra_ver"
  echo "Binary:      $RA_BIN"
  echo "Core:        $CORE"
  echo "Core GIT:    $core_git"
  echo "Core sha256: $core_sha"
  echo "ROM:         $ROM"
  echo "ROM sha256:  $rom_sha"
  if [ -n "$manifest" ]; then
    [ -f "$manifest" ] || die "manifest not found: $manifest"
    local want_git want_sha
    want_git="$(grep -aoE 'GIT[0-9a-f]+' "$manifest" | sort -u | head -1)"
    want_sha="$(grep " $(basename "$CORE")" "$manifest" | awk '{print $1}' | head -1)"
    echo
    echo "Manifest GIT:    $want_git"
    echo "Manifest sha256: $want_sha"
    if [ "$core_git" = "$want_git" ] && [ "$core_sha" = "$want_sha" ]; then
      echo "PARITY OK (core revision + sha256 match)"
    else
      echo "PARITY MISMATCH (core revision or sha256 differs)"
    fi
  fi
}

cmd_smoke() {
  local frames="${1:-$FRAMES_DEFAULT}"
  require_bin
  prepare_base
  write_role smoke
  local log="$WORK/smoke/retroarch.log"
  echo "loading core + ROM for $frames frames (~$((frames / 60))s)..."
  "$RA_BIN" -c "$WORK/base.cfg" --appendconfig "$WORK/smoke/overrides.cfg" \
    -L "$CORE" "$ROM" --verbose --log-file "$log" --max-frames "$frames" >/dev/null 2>&1 || true
  echo "log: $log"
  echo "--- result ---"
  if grep -qiE 'error|failed|fatal' "$log"; then echo "errors/failures present:"; grep -iE 'error|failed|fatal' "$log" | head -20; else echo "no error/failed lines"; fi
  echo "--- content/core lines ---"
  grep -iE 'content|loading|fbneo|FinalBurn|CRC|rom' "$log" | head -25 || true
}

cmd_host()     { local port="${1:-$PORT_DEFAULT}"; require_bin; launch_bg host "netplay_ip_port = \"$port\"" "-H" "--port" "$port" "--nick" "spike-host"; }
cmd_client()   { local ip="${1:?usage: client <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg client "" "-C" "$ip" "--port" "$port" "--nick" "spike-p2"; }
cmd_spectator(){ local ip="${1:?usage: spectator <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg spectator "netplay_start_as_spectator = \"true\"" "-C" "$ip" "--port" "$port" "--nick" "spike-spec"; }
cmd_spectator2(){ local ip="${1:?usage: spectator2 <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg spectator2 "netplay_start_as_spectator = \"true\"" "-C" "$ip" "--port" "$port" "--nick" "spike-spec2"; }

cmd_status() {
  echo "workdir: $WORK"
  local pids; pids="$(spike_pids)"
  if [ -z "$pids" ]; then echo "no spike RetroArch instances running"; return; fi
  echo "spike pids: $pids"
  for p in $pids; do
    printf '  pid %s cpu=%%%s rss=%sKB\n' "$p" "$(ps -o %cpu= -p "$p" | tr -d ' ')" "$(ps -o rss= -p "$p" | tr -d ' ')"
  done
  echo "--- netplay listener/connections (port $PORT_DEFAULT) ---"
  lsof -nP -iTCP:"$PORT_DEFAULT" 2>/dev/null || echo "(none)"
}

cmd_measure() {
  local secs="${1:-$MEASURE_DEFAULT}"
  local pids; pids="$(spike_pids)"
  [ -n "$pids" ] || die "no spike instances running (start host/client first)"
  local pid; pid="$(echo "$pids" | head -1)"
  echo "sampling nettop for pid $pid over ${secs}s..."
  # CSV: bytes_in,bytes_out per process summary. Loopback traffic is under 'lo0'.
  nettop -n -m tcp -p "$pid" -x -L "$secs" -s 1 -P > "$WORK/measure-nettop.csv" 2>/dev/null || true
  echo "nettop -> $WORK/measure-nettop.csv"
  nettop -n -m tcp -p "$pid" -x -l "$secs" -s 1 -P > "$WORK/measure-nettop.txt" 2>/dev/null || true
  echo "nettop table -> $WORK/measure-nettop.txt"
  lsof -nP -iTCP:"$PORT_DEFAULT" > "$WORK/measure-lsof.txt" 2>/dev/null || true
  ps -o pid,%cpu,rss,command -p $pids > "$WORK/measure-ps.txt" 2>/dev/null || true
  echo "--- nettop (first + last sample) ---"; sed -n '1p;$p' "$WORK/measure-nettop.txt" 2>/dev/null || true
  echo "--- process ---"; cat "$WORK/measure-ps.txt" 2>/dev/null || true
}

cmd_stop() {
  local pids; pids="$(spike_pids)"
  if [ -z "$pids" ]; then echo "nothing to stop"; return; fi
  kill $pids 2>/dev/null || true
  sleep 1
  pids="$(spike_pids)"
  [ -n "$pids" ] && kill -9 $pids 2>/dev/null || true
  echo "stopped spike instances"
}

case "${1:-}" in
  parity)    shift; cmd_parity "$@" ;;
  smoke)     shift; cmd_smoke "$@" ;;
  host)      shift; cmd_host "$@" ;;
  client)    shift; cmd_client "$@" ;;
  spectator) shift; cmd_spectator "$@" ;;
  spectator2) shift; cmd_spectator2 "$@" ;;
  measure)   shift; cmd_measure "$@" ;;
  status)    shift; cmd_status "$@" ;;
  stop)      shift; cmd_stop "$@" ;;
  *) sed -n '2,20p' "$0"; exit 1 ;;
esac
