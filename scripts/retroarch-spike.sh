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
#   scripts/retroarch-spike.sh hostspec [port]
#   scripts/retroarch-spike.sh solo
#   scripts/retroarch-spike.sh client <host-ip> [port]
#   scripts/retroarch-spike.sh client2 <host-ip> [port]
#   scripts/retroarch-spike.sh spectator <host-ip> [port]
#   scripts/retroarch-spike.sh spectator2 <host-ip> [port]
#   scripts/retroarch-spike.sh send <role> <command>
#   scripts/retroarch-spike.sh memdump <role> <addr> <len> <file>
#   scripts/retroarch-spike.sh memdiff <fileA> <fileB>
#   scripts/retroarch-spike.sh savestate <role> <name>
#   scripts/retroarch-spike.sh statediff <a.state> <b.state>
#   scripts/retroarch-spike.sh measure [seconds]
#   scripts/retroarch-spike.sh status
#   scripts/retroarch-spike.sh stop
#
# Every role gets RetroArch's network command interface on its own loopback port
# (host 55355, client 55356, spectator 55357, spectator2 55358) so the lobby
# spike can drive NETPLAY_GAME_WATCH / READ_CORE_MEMORY locally
# (docs/10-lobby-spike.md).
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
    ROM_DEFAULT="$HOME/ROMs/sfiii3nr1.zip"
    ;;
  Linux)
    OS_TAG="Linux $(uname -m)"
    RA_BIN_DEFAULT="$(command -v retroarch || true)"
    RA_CFG_DEFAULT="${XDG_CONFIG_HOME:-$HOME/.config}/retroarch/retroarch.cfg"
    CORE_DIR_DEFAULT="$HOME/.config/retroarch/cores"
    CORE_NAME="fbneo_libretro.so"
    ROM_DEFAULT="$HOME/ROMs/sfiii3nr1.zip"
    ;;
  *)
    echo "unsupported OS $(uname -s) — use the documented commands on Windows" >&2
    exit 1
    ;;
esac

# Prefer the app-managed frozen core (docs/07-retroarch-spike.md §2); the
# RetroArch cores dir may still hold the pre-rebuild buildbot file.
MANAGED_CORE="$HOME/Library/Application Support/com.the-cabinet.app/cores/macos-arm64/$CORE_NAME"
if [ "$(uname -s)" = "Darwin" ] && [ -f "$MANAGED_CORE" ]; then
  CORE_DEFAULT="$MANAGED_CORE"
else
  CORE_DEFAULT="$CORE_DIR_DEFAULT/$CORE_NAME"
fi

RA_BIN="${RETROARCH_BIN:-$RA_BIN_DEFAULT}"
RA_CFG="${RETROARCH_CFG:-$RA_CFG_DEFAULT}"
CORE="${RETROARCH_CORE:-$CORE_DEFAULT}"
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

# Per-role loopback command port — one emulator per port on this machine.
cmd_port_for() {
  case "$1" in
    host|hostspec) echo 55355 ;;
    client)        echo 55356 ;;
    spectator)     echo 55357 ;;
    spectator2)    echo 55358 ;;
    client2)       echo 55359 ;;
    solo)          echo 55360 ;;
    *)             echo 55355 ;;
  esac
}

write_role() {
  local role="$1" extra="${2:-}"
  # macOS: pin MoltenVK. RetroArch's Metal driver is broken on Apple Silicon +
  # macOS 26 (~8 fps; docs/07 §14 F22); a menu-selected driver wouldn't persist
  # anyway (config_save_on_exit=false), so the inherited config can lie.
  local video_driver=""
  [ "$(uname -s)" = "Darwin" ] && video_driver='video_driver = "vulkan"'
  mkdir -p "$WORK/$role/saves" "$WORK/$role/states"
  # Bind both keyboard prefixes. Netplay samples a participant's local input from the *first*
  # local device (`input_player1_*`) regardless of the player slot it is assigned
  # (docs/10-lobby-spike.md L8), so a spectator later promoted into a vacated slot needs the
  # player-1 binds too; the player-2 copy is the pre-existing per-slot bind.
  cat > "$WORK/$role/overrides.cfg" <<EOF
config_save_on_exit = "false"
video_fullscreen = "false"
$video_driver
pause_nonactive = "false"
netplay_nat_traversal = "false"
netplay_public_announce = "false"
netplay_check_frames = "600"
netplay_ping_show = "true"
netplay_allow_slaves = "true"
netplay_require_slaves = "false"
netplay_max_connections = "8"
network_cmd_enable = "true"
network_cmd_port   = "$(cmd_port_for "$role")"
savestate_file_compression = "false"
input_player1_up = "up"
input_player1_down = "down"
input_player1_left = "left"
input_player1_right = "right"
input_player1_a = "x"
input_player1_b = "z"
input_player1_x = "s"
input_player1_y = "a"
input_player1_l = "q"
input_player1_r = "w"
input_player1_start = "enter"
input_player1_select = "rshift"
input_player2_up = "up"
input_player2_down = "down"
input_player2_left = "left"
input_player2_right = "right"
input_player2_a = "x"
input_player2_b = "z"
input_player2_x = "s"
input_player2_y = "a"
input_player2_l = "q"
input_player2_r = "w"
input_player2_start = "enter"
input_player2_select = "rshift"
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
# solo — one instance with the command socket and no netplay at all (L17 experiments).
cmd_solo()     { require_bin; launch_bg solo ""; }
cmd_hostspec() {
  local port="${1:-$PORT_DEFAULT}"; require_bin
  local extra="netplay_ip_port = \"$port\""
  extra="$extra"$'\n'"netplay_start_as_spectator = \"true\""
  launch_bg hostspec "$extra" "-H" "--port" "$port" "--nick" "spike-hostspec"
}
cmd_client()   { local ip="${1:?usage: client <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg client "" "-C" "$ip" "--port" "$port" "--nick" "spike-p2"; }
cmd_client2()  { local ip="${1:?usage: client2 <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg client2 "" "-C" "$ip" "--port" "$port" "--nick" "spike-p1"; }
cmd_spectator(){ local ip="${1:?usage: spectator <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg spectator "netplay_start_as_spectator = \"true\"" "-C" "$ip" "--port" "$port" "--nick" "spike-spec"; }
cmd_spectator2(){ local ip="${1:?usage: spectator2 <host-ip> [port]}"; local port="${2:-$PORT_DEFAULT}"; require_bin; launch_bg spectator2 "netplay_start_as_spectator = \"true\"" "-C" "$ip" "--port" "$port" "--nick" "spike-spec2"; }

# send <role> <COMMAND...> — drive the role's loopback command socket.
cmd_send() {
  local role="${1:?usage: send <role> <command>}"; shift
  [ "$#" -gt 0 ] || die "no command given"
  local port; port="$(cmd_port_for "$role")"
  printf '%s\n' "$*" | nc -u -w1 127.0.0.1 "$port" || true
}

# memdump <role> <addr> <len> <file> — chunked READ_CORE_RAM -> raw binary.
# READ_CORE_RAM (retro_get_memory_data / system RAM) works with FBNeo, unlike
# READ_CORE_MEMORY which needs a libretro memory map FBNeo does not define.
# One UDP socket (replies cap at 336 bytes each); nc would burn ~1s per request.
cmd_memdump() {
  local role="${1:?usage: memdump <role> <addr> <len> <file>}"
  local addr="${2:?}" len="${3:?}" out="${4:?}"
  local port; port="$(cmd_port_for "$role")"
  python3 - "$port" "$addr" "$len" "$out" <<'PY'
import socket, sys
port, addr, length, out = int(sys.argv[1]), int(sys.argv[2], 0), int(sys.argv[3], 0), sys.argv[4]
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(2.0)
data, off, CH = bytearray(), 0, 256
while off < length:
    n = min(CH, length - off)
    want = addr + off
    s.sendto(("READ_CORE_RAM 0x%08X %d\n" % (want, n)).encode(), ("127.0.0.1", port))
    resp = None
    for _ in range(20):
        try:
            r = s.recv(65535).decode(errors="replace").split()
        except socket.timeout:
            break
        try:
            got = int(r[1], 16)
        except (IndexError, ValueError):
            continue
        if got == want:
            resp = r; break
    if resp is None:
        sys.stderr.write("memdump: no reply at 0x%08X\n" % want); break
    if len(resp) < 3 or resp[2] == "-1":
        sys.stderr.write("memdump: error at 0x%08X: %s\n" % (want, " ".join(resp))); break
    data += bytes(int(b, 16) for b in resp[2:])
    off += n
open(out, "wb").write(bytes(data))
print("memdump: %d bytes from 0x%X via port %d -> %s" % (len(data), addr, port, out))
PY
}

# savestate <role> <name> — SAVE_STATE then copy the newest state to snapshots/<name>.state
cmd_savestate() {
  local role="${1:?usage: savestate <role> <name>}"
  local name="${2:?usage: savestate <role> <name>}"
  local port; port="$(cmd_port_for "$role")"
  printf 'SAVE_STATE\n' | nc -u -w1 127.0.0.1 "$port" >/dev/null 2>&1 || true
  sleep 1
  local src; src="$(ls -t "$WORK/$role/states"/*/*.state 2>/dev/null | head -1)"
  [ -n "$src" ] || die "no state written by $role"
  mkdir -p "$WORK/snapshots"
  cp "$src" "$WORK/snapshots/$name.state"
  echo "savestate: $role -> $WORK/snapshots/$name.state ($(wc -c < "$src" | tr -d ' ') bytes)"
}

# statediff <a.state> <b.state> [rambase] [len] — diff only the system-RAM region
# of two uncompressed RASTATE snapshots and report RAM addresses that changed.
# The RAM region starts at state offset 0x214 for this core (FBNeo CPS3, GIT6bb3167).
cmd_statediff() {
  local a="${1:?usage: statediff <a.state> <b.state> [rambase] [len]}"
  local b="${2:?}"
  local base="${3:-0x214}" len="${4:-0x80000}"
  python3 - "$a" "$b" "$base" "$len" <<'PY'
import sys
a = open(sys.argv[1], "rb").read(); b = open(sys.argv[2], "rb").read()
base = int(sys.argv[3], 0); ln = int(sys.argv[4], 0)
ra, rb = a[base:base+ln], b[base:base+ln]
if len(ra) != ln or len(rb) != ln:
    print("statediff: short read (a=%d b=%d want=%d)" % (len(ra), len(rb), ln)); sys.exit(1)
inc, total = [], 0
for i in range(ln):
    if ra[i] != rb[i]:
        total += 1
        if rb[i] == ra[i] + 1: inc.append((i, ra[i], rb[i]))
print("statediff: %d differing bytes in RAM region, %d are +1" % (total, len(inc)))
for i, o, v in inc[:80]:
    print("  RAM 0x%06X: %d -> %d" % (i, o, v))
if len(inc) > 80: print("  ... (%d more +1)" % (len(inc) - 80))
PY
}

# memdiff <fileA> <fileB> — byte offsets where two memdumps differ (old -> new).
cmd_memdiff() {
  local a="${1:?usage: memdiff <fileA> <fileB>}" b="${2:?}"
  local sa sb
  sa="$(wc -c < "$a" | tr -d ' ')"; sb="$(wc -c < "$b" | tr -d ' ')"
  [ "$sa" = "$sb" ] || { echo "memdiff: size mismatch ($sa vs $sb)"; return 1; }
  echo "memdiff: $sa bytes compared"
  local report
  report="$(cmp -l "$a" "$b" 2>/dev/null | perl -ane '
    my $off=$F[0]-1; my $o=oct($F[1]); my $v=oct($F[2]);
    $inc++ if $v==$o+1;
    if ($shown<40) { printf "  0x%06X: %d -> %d%s\n",$off,$o,$v,($v==$o+1?" (+1)":""); $shown++ }
    $total++;
    END { printf "  %d differing bytes, %d of them +1 (shown up to 40)\n",$total,$inc }
  ' || true)"
  if [ -z "$report" ]; then echo "  (no differences)"; else printf '%s\n' "$report"; fi
}

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
  echo "--- command sockets (UDP 55355-55359) ---"
  lsof -nP -iUDP:55355-55359 2>/dev/null || echo "(none)"
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
  hostspec)  shift; cmd_hostspec "$@" ;;
  solo)      shift; cmd_solo "$@" ;;
  client)    shift; cmd_client "$@" ;;
  client2)   shift; cmd_client2 "$@" ;;
  spectator) shift; cmd_spectator "$@" ;;
  spectator2) shift; cmd_spectator2 "$@" ;;
  send)      shift; cmd_send "$@" ;;
  memdump)   shift; cmd_memdump "$@" ;;
  memdiff)   shift; cmd_memdiff "$@" ;;
  savestate) shift; cmd_savestate "$@" ;;
  statediff) shift; cmd_statediff "$@" ;;
  measure)   shift; cmd_measure "$@" ;;
  status)    shift; cmd_status "$@" ;;
  stop)      shift; cmd_stop "$@" ;;
  *) sed -n '2,20p' "$0"; exit 1 ;;
esac
