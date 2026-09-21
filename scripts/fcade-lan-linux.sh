#!/bin/bash
# fcade-lan-linux — direct-connect FightCade FBNeo over Tailscale (Linux)
# Mirror of the macOS launcher. Prefers the FightCade Flatpak; falls back to a
# native or system-Wine install.
set -euo pipefail

FLATPAK_APP="com.fightcade.Fightcade"
FLATPAK_DATA="$HOME/.var/app/$FLATPAK_APP/data"
APP_FB="/app/fightcade/Fightcade/emulator/fbneo"   # path inside the Flatpak sandbox

have_flatpak() {
  command -v flatpak >/dev/null 2>&1 && flatpak info "$FLATPAK_APP" >/dev/null 2>&1
}

# --- select mode + ROM dir ----------------------------------------------------
if have_flatpak; then
  MODE="flatpak"
  ROM_DIR="$FLATPAK_DATA/ROMs/fbneo"
else
  MODE="native"
  find_fc() {
    local c
    for c in "${FC_DIR:-}" "$HOME/.fightcade" "$HOME/fightcade" "$HOME/Fightcade" \
             "$HOME/fightcade2" "$HOME/.local/share/fightcade" "$HOME/Games"/* \
             /opt/fightcade /opt/FightCade2; do
      [ -n "$c" ] && [ -d "$c/emulator/fbneo" ] && { echo "$c"; return 0; }
    done
    return 1
  }
  FC="$(find_fc || true)"
  [ -n "$FC" ] || { echo "FightCade not found. Install the Flatpak (com.fightcade.Fightcade) or set FC_DIR=/path/to/fightcade" >&2; exit 1; }
  FB_DIR="$FC/emulator/fbneo"
  ROM_DIR="$FB_DIR/ROMs"

  # --- locate emulator + runner (native vs Wine-wrapped) ----------------------
  if [ -x "$FB_DIR/fcadefbneo" ]; then
    EMU="$FB_DIR/fcadefbneo"; RUNNER=()
  elif [ -e "$FB_DIR/fcadefbneo.exe" ]; then
    EMU="$FB_DIR/fcadefbneo.exe"
    for w in wine wine64; do command -v "$w" >/dev/null 2>&1 && { RUNNER=("$w"); break; }; done
    [ "${#RUNNER[@]}" -gt 0 ] || { echo "fcadefbneo.exe found but wine is not installed" >&2; exit 1; }
  else
    echo "No fcadefbneo / fcadefbneo.exe under $FB_DIR" >&2; exit 1
  fi
fi

TS_IP="$(command -v tailscale >/dev/null 2>&1 && tailscale ip -4 2>/dev/null | head -1 || true)"

# --- UI (zenity if present, else plain prompts) --------------------------------
if command -v zenity >/dev/null 2>&1; then
  PEER_IP="$(zenity --entry --title="FightCade LAN" --text="Peer Tailscale IP (your friend):" --entry-text="100.64.0.1")" || exit 1
  ROM="$(ls "$ROM_DIR/"*.zip 2>/dev/null | sed 's#.*/##;s/\.zip$//' | \
        zenity --list --title="FightCade LAN" --text="Choose ROM" --column="ROM")" || exit 1
  SIDE_LABEL="$(zenity --list --title="FightCade LAN" --text="Choose your side" --column="Side" P1 P2)" || exit 1
  [ "$SIDE_LABEL" = "P1" ] && SIDE=0 || SIDE=1
else
  printf 'Peer Tailscale IP [100.64.0.1]: '; read -r PEER_IP; PEER_IP="${PEER_IP:-100.64.0.1}"
  echo "ROMs:"; ls "$ROM_DIR/"*.zip 2>/dev/null | sed 's#.*/##;s/\.zip$//' | nl
  printf 'ROM short name [sfiii3nr1]: '; read -r ROM; ROM="${ROM:-sfiii3nr1}"
  printf 'Your side (P1/P2) [P1]: '; read -r S; S="${S:-P1}"
  [ "$S" = "P1" ] && SIDE=0 || SIDE=1
fi
[ -n "$PEER_IP" ] && [ -n "$ROM" ] || exit 1

if [ "$SIDE" = "0" ]; then LOCAL=7001; PEER=7000; else LOCAL=7000; PEER=7001; fi
ARG="quark:direct,$ROM,$LOCAL,$PEER_IP,$PEER,$SIDE,0"

echo "Mode $MODE  |  Local $TS_IP  |  Peer $PEER_IP  |  side $SIDE"

if [ "$MODE" = "flatpak" ]; then
  INNER=". /app/bin/get-wine-prefix; cd $APP_FB; export WINEDEBUG=-all; exec /app/fightcade/Resources/wine.sh fcadefbneo.exe \"$ARG\" -w"
  echo "Exec: flatpak run --command=/bin/sh $FLATPAK_APP -c '$INNER'"
  exec flatpak run --command=/bin/sh "$FLATPAK_APP" -c "$INNER"
else
  echo "Exec: ${RUNNER[*]} $EMU $ARG -w"
  cd "$FB_DIR"
  WINEDEBUG=-all "${RUNNER[@]}" "$EMU" "$ARG" -w
fi
