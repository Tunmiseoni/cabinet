#!/bin/bash
# fcade-lan-macos — direct-connect FightCade FBNeo over Tailscale (macOS)
# Bypasses FightCade matchmaking (and therefore CGNAT) by using the emulator's
# built-in quark:direct mode. Friend-only.
set -euo pipefail

FC_APP="${FC_APP:-/Applications/FightCade2.app}"
FB_DIR="$FC_APP/Contents/MacOS/emulator/fbneo"
WINE="$FC_APP/Contents/Resources/wine/bin/wine32on64"
PREFIX="$FC_APP/Contents/Resources/.wine32"
TS="/Applications/Tailscale.app/Contents/MacOS/Tailscale"

die() { osascript -e "display dialog \"$1\" buttons {\"OK\"} default button 1 with icon stop" >/dev/null 2>&1 || echo "$1" >&2; exit 1; }

[ -x "$WINE" ]  || die "Wine binary not found at $WINE"
[ -d "$FB_DIR" ] || die "Emulator dir not found at $FB_DIR"

MY_IP="$("$TS" ip -4 2>/dev/null | head -1 || true)"
[ -n "$MY_IP" ] || die "Tailscale is not running (no IPv4). Start Tailscale and retry."

# Build ROM list from the ROMs dir (short names, no .zip)
ROMS=()
while IFS= read -r f; do
  [ -n "$f" ] && ROMS+=("${f%.zip}")
done < <(ls "$FB_DIR/ROMs/"*.zip 2>/dev/null | sed 's#.*/##')

ask() { osascript -e "text returned of (display dialog \"$1\" default answer \"$2\" buttons {\"OK\"} default button 1)" 2>/dev/null; }

TITLE="FightCade LAN ($MY_IP)"
PEER_IP="$(ask "Peer Tailscale IP (your friend):" "100.64.0.2")" || exit 1
[ -n "$PEER_IP" ] || exit 1

if [ "${#ROMS[@]}" -gt 0 ]; then
  APPLESCRIPT_LIST=$(printf '"%s",' "${ROMS[@]}"; echo -n '')
  ROM="$(osascript <<OSA
set opts to {${APPLESCRIPT_LIST%,}}
set r to choose from list opts with prompt "Choose ROM" default items {item 1 of opts} with title "$TITLE"
if r is false then return ""
return item 1 of r
OSA
)" || exit 1
else
  ROM="$(ask "ROM short name (e.g. sfiii3nr1):" "sfiii3nr1")" || exit 1
fi
[ -n "$ROM" ] || exit 1

SIDE="$(osascript <<'OSA'
set r to display dialog "Choose your side:" buttons {"P1", "P2", "Cancel"} default button 1 with title "FightCade LAN"
if button returned of r is "P1" then return "0"
if button returned of r is "P2" then return "1"
return ""
OSA
)" || exit 1
[ -n "$SIDE" ] || exit 1

if [ "$SIDE" = "0" ]; then LOCAL=7001; PEER=7000; else LOCAL=7000; PEER=7001; fi

cd "$FB_DIR"
ARG="quark:direct,$ROM,$LOCAL,$PEER_IP,$PEER,$SIDE,0"
osascript -e "display notification \"$ARG -w\" with title \"Launching FBNeo\"" >/dev/null 2>&1 || true
echo "Local $MY_IP  |  Peer $PEER_IP  |  side $SIDE"
echo "Exec: fcadefbneo.exe $ARG -w"
WINEPREFIX="$PREFIX" WINEDEBUG=-all "$WINE" "fcadefbneo.exe" "$ARG" -w
