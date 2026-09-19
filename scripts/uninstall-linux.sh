#!/bin/bash
# uninstall-linux — reverse a source build of The Cabinet on Linux (Arch/CachyOS).
#
# It reports what it would remove; nothing changes until you pass --apply.
# Package removal is reconstructed from /var/log/pacman.log so only what the
# source build actually installed (via scripts/setup-linux.sh) is targeted.
# Rust is removed only when ~/.rustup / ~/.cargo are newer than the run.
#
#   scripts/uninstall-linux.sh                       # dry run (report only)
#   scripts/uninstall-linux.sh --apply               # remove repo + deps + rust
#   scripts/uninstall-linux.sh --since 2026-09-19T14:00:00+0100 --apply
#   scripts/uninstall-linux.sh --apply --install-appimage
#
# Download it without cloning the repo:
#   curl -fsSLO https://raw.githubusercontent.com/Tunmiseoni/the-cabinet/main/scripts/uninstall-linux.sh
#   bash uninstall-linux.sh          # then re-run with --apply
set -euo pipefail

APPLY=0
PURGE_CFG=0
KEEP_RUST=0
KEEP_PKGS=0
INSTALL_APPIMAGE=0
SINCE=""
SINCE_EPOCH=""
KEEP_EXTRA=()

REPO_SLUG="Tunmiseoni/the-cabinet"
APPIMAGE_API="https://api.github.com/repos/${REPO_SLUG}/releases/latest"
APPIMAGE_URL=""

usage() {
  cat <<'EOF'
usage: uninstall-linux.sh [options]

Reverses a source build of The Cabinet (scripts/setup-linux.sh) on Arch/CachyOS.

  --apply             actually delete (default is a read-only report)
  --since <when>      the run date/time as ISObasic (e.g. 2026-09-19T14:00:00+0100);
                      defaults to the extracted repo dir's mtime
  --keep-rust         do not uninstall the Rust toolchain
  --keep-pkgs         do not remove any pacman packages
  --keep <pkg,...>    extra packages to preserve (flatpak is always preserved)
  --purge-cfg         also delete the app config (config/scores/room-ledger)
  --install-appimage  download the latest Linux AppImage to ~/.local/bin
  -h, --help          show this help

flatpak and com.fightcade.Fightcade are never touched.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --apply) APPLY=1 ;;
    --install-appimage) INSTALL_APPIMAGE=1 ;;
    --purge-cfg) PURGE_CFG=1 ;;
    --keep-rust) KEEP_RUST=1 ;;
    --keep-pkgs) KEEP_PKGS=1 ;;
    --since) SINCE="${2:-}"; shift ;;
    --keep) KEEP_EXTRA+=("${2:-}"); shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if [ -n "$SINCE" ]; then
  SINCE_EPOCH="$(date -d "$SINCE" +%s 2>/dev/null || true)"
  if [ -z "$SINCE_EPOCH" ]; then
    echo "error: could not parse --since '$SINCE'" >&2
    exit 2
  fi
fi

if [ "$APPLY" = 1 ]; then
  MODE="APPLY"
else
  MODE="DRY RUN"
fi
echo "==> The Cabinet Linux uninstall ($MODE)"
[ -n "$SINCE" ] && echo "    since: $SINCE"

run() {
  if [ "$APPLY" = 1 ]; then
    "$@"
  else
    printf '    would run: %s\n' "$*"
  fi
}

in_list() {
  local needle="$1"; shift
  local x
  for x in "$@"; do
    [ "$x" = "$needle" ] && return 0
  done
  return 1
}

dir_size() {
  du -sh "$1" 2>/dev/null | cut -f1 || echo "?"
}

# ---------------------------------------------------------------- repo + build

declare -a REPO_DIRS=()
find_repo_dirs() {
  local conf dir
  while IFS= read -r conf; do
    if grep -qE '"productName": *"(The Cabinet|cabinet)"' "$conf" 2>/dev/null; then
      dir="$(dirname "$(dirname "$conf")")"
      REPO_DIRS+=("$dir")
    fi
  done < <(find "$HOME" -maxdepth 5 \
    \( -name .git -o -name node_modules -o -name target -o -name .cache \
       -o -name Library -o -name .rustup -o -name .cargo -o -name .local \
       -o -name .npm -o -name Applications \) -prune -o \
    -type f -name tauri.conf.json -path '*/src-tauri/*' -print 2>/dev/null)
}

echo
echo "==> source repo + build artifacts"
find_repo_dirs
if [ "${#REPO_DIRS[@]}" -eq 0 ]; then
  echo "    none found (already removed?)"
else
  for dir in "${REPO_DIRS[@]}"; do
    echo "    ${dir}  ($(dir_size "$dir"))"
    if [ -z "$SINCE_EPOCH" ]; then
      SINCE_EPOCH="$(stat -c %Y "$dir" 2>/dev/null || echo 0)"
      [ "$SINCE_EPOCH" -gt 0 ] && echo "    inferred --since from its mtime"
    fi
    if [ -d "$dir/src-tauri/target" ]; then
      echo "      src-tauri/target  ($(dir_size "$dir/src-tauri/target"))"
    fi
    run rm -rf "$dir"
  done
fi

# Leftover temp files from running the app.
shopt -s nullglob
TMP_LEFTOVERS=(/tmp/cabinet-*)
shopt -u nullglob
if [ "${#TMP_LEFTOVERS[@]}" -gt 0 ]; then
  echo "    temp leftovers: ${TMP_LEFTOVERS[*]}"
  run rm -rf "${TMP_LEFTOVERS[@]}"
fi

# ---------------------------------------------------------------- pacman pkgs

declare -a PACMAN_PKGS=()
declare -a PACMAN_TX=()

detect_pacman() {
  local log="${PACMAN_LOG:-/var/log/pacman.log}"
  [ -r "$log" ] || return 0

  local line ts cmd pkg
  local tx_match=0 tx_ts="" tx_epoch=0
  local -a tx_pkgs=()
  local -a out_pkgs=()

  flush() {
    if [ "$tx_match" = 1 ] && [ "${#tx_pkgs[@]}" -gt 0 ]; then
      if [ -z "$SINCE_EPOCH" ] || [ "${tx_epoch:-0}" -ge "$SINCE_EPOCH" ]; then
        out_pkgs+=("${tx_pkgs[@]}")
        PACMAN_TX+=("$tx_ts")
      fi
    fi
    tx_pkgs=(); tx_match=0; tx_ts=""; tx_epoch=0
  }

  while IFS= read -r line; do
    case "$line" in
      *" [PACMAN] Running "*)
        flush
        ts="${line#*[}"; ts="${ts%%]*}"
        cmd="${line#* Running }"
        case "$cmd" in
          *appmenu-gtk-module* | *"nodejs npm flatpak"*) tx_match=1 ;;
        esac
        tx_ts="$ts"
        if [ -n "$SINCE_EPOCH" ]; then
          tx_epoch="$(date -d "$ts" +%s 2>/dev/null || echo 0)"
        fi
        ;;
      *" [ALPM] installed "*)
        pkg="${line##*ALPM] installed }"
        pkg="${pkg%% *}"; pkg="${pkg%%(*}"
        [ -n "$pkg" ] && tx_pkgs+=("$pkg")
        ;;
    esac
  done < "$log"
  flush

  if [ "${#out_pkgs[@]}" -gt 0 ]; then
    PACMAN_PKGS=("${out_pkgs[@]}")
  fi
}

# Drop any candidate that another installed package still depends on (outside
# the removal set) until the set is stable. flatpak is hard-excluded.
declare -a REMOVE_PKGS=()
prune_pacman() {
  command -v pacman >/dev/null 2>&1 || return 0
  local pkg dep deps keep changed=1
  local -a cur=("$@")
  local -a next=()

  while [ "$changed" = 1 ]; do
    changed=0
    next=()
    for pkg in "${cur[@]}"; do
      keep=1
      deps="$(pacman -Qi "$pkg" 2>/dev/null | awk -F': ' '/^Required By/{print $2}')"
      if [ -n "$deps" ] && [ "$deps" != "None" ]; then
        for dep in $deps; do
          if ! in_list "$dep" "${cur[@]}"; then
            keep=0
            echo "    keeping $pkg — still required by $dep"
            break
          fi
        done
      fi
      if [ "$keep" = 1 ]; then
        next+=("$pkg")
      else
        changed=1
      fi
    done
    cur=("${next[@]}")
    [ "${#cur[@]}" -eq 0 ] && break
  done

  if [ "${#cur[@]}" -gt 0 ]; then
    REMOVE_PKGS=("${cur[@]}")
  fi
}

echo
echo "==> pacman packages added by the setup run"
if [ "$KEEP_PKGS" = 1 ]; then
  echo "    skipped (--keep-pkgs)"
else
  detect_pacman
  if [ "${#PACMAN_PKGS[@]}" -eq 0 ]; then
    echo "    none detected in /var/log/pacman.log${SINCE:+ after $SINCE}"
  else
    for ts in "${PACMAN_TX[@]}"; do
      echo "    transaction @ $ts"
    done
    declare -a CANDIDATES=()
    for pkg in "${PACMAN_PKGS[@]}"; do
      if [ "$pkg" = "flatpak" ]; then
        echo "    preserving flatpak — FightCade runs from it"
        continue
      fi
      if in_list "$pkg" "${KEEP_EXTRA[@]:-}"; then
        echo "    preserving $pkg (--keep)"
        continue
      fi
      CANDIDATES+=("$pkg")
    done
    echo "    candidates: ${CANDIDATES[*]:-none}"
    if [ "${#CANDIDATES[@]}" -gt 0 ]; then
      prune_pacman "${CANDIDATES[@]}"
      if [ "${#REMOVE_PKGS[@]}" -gt 0 ]; then
        echo "    will remove: ${REMOVE_PKGS[*]}"
        if command -v pacman >/dev/null 2>&1; then
          echo "    pacman preview:"
          sudo pacman -Rns --print "${REMOVE_PKGS[@]}" 2>&1 | sed 's/^/      /' || true
        fi
        run sudo pacman -Rns --noconfirm "${REMOVE_PKGS[@]}"
      fi
    fi
  fi
fi

# ---------------------------------------------------------------- rust

echo
echo "==> Rust toolchain"
if [ "$KEEP_RUST" = 1 ]; then
  echo "    skipped (--keep-rust)"
elif [ ! -e "$HOME/.rustup" ] && [ ! -e "$HOME/.cargo" ]; then
  echo "    not present"
else
  rust_epoch=0
  for d in "$HOME/.rustup" "$HOME/.cargo"; do
    [ -e "$d" ] || continue
    e="$(stat -c %W "$d" 2>/dev/null || echo 0)"
    [ "$e" -gt 0 ] 2>/dev/null || e="$(stat -c %Y "$d" 2>/dev/null || echo 0)"
    [ "$e" -gt "$rust_epoch" ] 2>/dev/null && rust_epoch="$e"
  done
  echo "    ~/.rustup + ~/.cargo  (created $(date -d "@$rust_epoch" 2>/dev/null || echo unknown))"
  if [ -n "$SINCE_EPOCH" ] && [ "$rust_epoch" -gt 0 ] && [ "$rust_epoch" -lt "$SINCE_EPOCH" ] 2>/dev/null; then
    echo "    predates the setup run — leaving it alone (use --keep-rust to silence)"
  elif [ -z "$SINCE_EPOCH" ]; then
    echo "    no --since and no repo dir to infer one from — refusing to guess."
    echo "    pass --since <when> to remove it, or --keep-rust to silence."
  elif ! command -v rustup >/dev/null 2>&1; then
    echo "    rustup not on PATH — remove the directories manually if wanted"
  else
    grep -rl 'cargo/env' \
      "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile" "$HOME/.bash_profile" \
      "$HOME/.config/fish/config.fish" 2>/dev/null | sed 's/^/    shell profile: /' || true
    run rustup self uninstall -y
  fi
fi

# ---------------------------------------------------------------- caches

echo
echo "==> caches"
if [ -d "$HOME/.cache/tauri" ]; then
  echo "    ~/.cache/tauri  ($(dir_size "$HOME/.cache/tauri"))"
  run rm -rf "$HOME/.cache/tauri"
else
  echo "    ~/.cache/tauri not present"
fi
if [ -d "$HOME/.npm" ]; then
  if in_list "nodejs" "${REMOVE_PKGS[@]:-}" || in_list "npm" "${REMOVE_PKGS[@]:-}"; then
    echo "    ~/.npm  ($(dir_size "$HOME/.npm"))  (node/npm were installed by the run)"
    run rm -rf "$HOME/.npm"
  else
    echo "    ~/.npm present — node/npm looked pre-existing, leaving it"
  fi
fi

# ---------------------------------------------------------------- config

echo
echo "==> app config + ledger"
declare -a CFG_DIRS=("$HOME/.config/com.the-cabinet.app" "$HOME/.config/com.cabinet.app")
found_cfg=0
for d in "${CFG_DIRS[@]}"; do
  if [ -d "$d" ]; then
    found_cfg=1
    echo "    $d  ($(dir_size "$d"))"
  fi
done
if [ "$found_cfg" = 0 ]; then
  echo "    none"
elif [ "$PURGE_CFG" = 1 ]; then
  for d in "${CFG_DIRS[@]}"; do
    [ -d "$d" ] && run rm -rf "$d"
  done
else
  echo "    kept (scores/ledger); pass --purge-cfg to delete"
fi

# ---------------------------------------------------------------- appimage

install_appimage() {
  echo
  echo "==> Linux AppImage"
  if ! command -v curl >/dev/null 2>&1; then
    echo "    curl not found — install curl first" >&2
    return 1
  fi
  local url="$APPIMAGE_URL"
  if [ -z "$url" ]; then
    url="$(curl -fsSL "$APPIMAGE_API" 2>/dev/null \
      | tr ',' '\n' \
      | grep -o '"browser_download_url": *"[^"]*\.AppImage"' \
      | head -1 \
      | sed 's/.*"browser_download_url": *"//;s/"$//')"
  fi
  if [ -z "$url" ]; then
    echo "    could not find an AppImage on the latest release" >&2
    return 1
  fi

  local bin_dir="$HOME/.local/bin"
  local dest="$bin_dir/the-cabinet.AppImage"
  local desktop="$HOME/.local/share/applications/the-cabinet.desktop"
  echo "    release asset: $url"
  echo "    destination:   $dest"

  if [ "$APPLY" != 1 ]; then
    echo "    would download and chmod +x $dest"
    echo "    would write $desktop"
    return 0
  fi

  mkdir -p "$bin_dir" "$(dirname "$desktop")"
  curl -fL --progress-bar -o "$dest" "$url"
  chmod +x "$dest"
  cat > "$desktop" <<EOF
[Desktop Entry]
Type=Application
Name=The Cabinet
Comment=FightCade direct-connect lobby over Tailscale
Exec=$dest %U
Terminal=false
Categories=Game;Network;
EOF
  echo "    installed. Ensure $bin_dir is on PATH, then run: the-cabinet.AppImage"
  if ! ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; then
    echo "    note: FUSE2 not found — either 'sudo pacman -S fuse2' or run"
    echo "          $dest --appimage-extract-and-run"
  fi
}

[ "$INSTALL_APPIMAGE" = 1 ] && install_appimage || true

# ---------------------------------------------------------------- summary

echo
echo "==> summary"
echo "    mode: $MODE"
[ "$APPLY" = 1 ] || echo "    nothing was changed — re-run with --apply to remove"
if command -v flatpak >/dev/null 2>&1; then
  if flatpak info com.fightcade.Fightcade >/dev/null 2>&1; then
    echo "    flatpak FightCade: still installed"
  fi
fi
for tool in cargo rustup node npm; do
  if command -v "$tool" >/dev/null 2>&1; then
    echo "    still on PATH: $tool ($(command -v "$tool"))"
  fi
done
