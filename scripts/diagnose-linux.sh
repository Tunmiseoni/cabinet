#!/bin/bash
# diagnose-linux — blank-window diagnosis for The Cabinet on Linux (AppImage).
#
# A non-technical user can run it with a single pasted command:
#   curl -fsSLO https://raw.githubusercontent.com/Tunmiseoni/the-cabinet/main/scripts/diagnose-linux.sh
#   bash diagnose-linux.sh
#
# It is READ-ONLY toward the system: no sudo, no installs. It finds the AppImage,
# records the graphics environment, tests the known launch workarounds (including
# the fix for the AppImage EGL_BAD_PARAMETER bug), and asks a yes/no after each.
#
# The report is written to the CURRENT DIRECTORY (where you ran the command), so
# it lands next to the AppImage or the terminal's folder. If that is not
# writable it falls back to the AppImage's folder, then to $HOME. The exact path
# is printed at the start and at the end.
#
# It never touches your emulators or their installs.
set -uo pipefail

REPO_SLUG="Tunmiseoni/the-cabinet"
APPIMAGE_API="https://api.github.com/repos/${REPO_SLUG}/releases/latest"
ORIG_PWD="$(pwd)"
OUT_DIR=""
REPORT=""
SHOT_DIR=""
APPIMAGE=""
EXTRACTED_APPDIR=""
ATTEMPTS=0
PASSED=""
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/the-cabinet-diag.XXXXXX")"

cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

# If the script is piped (curl | bash), stdin is the script, not the keyboard.
# Reattach to the terminal so the yes/no prompts work. Tests set the override.
if [ "${THE_CABINET_DIAG_NONINTERACTIVE:-0}" != 1 ] && [ ! -t 0 ] && [ -r /dev/tty ]; then
  exec < /dev/tty
fi

# ---------------------------------------------------------------- helpers

say()  { printf '%s\n' "$*"; }
head1() { printf '\n== %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

pause() {
  if [ -t 0 ]; then
    printf '    press Enter to continue...'
    read -r _ || true
  else
    say "    (non-interactive input: continuing)"
  fi
}

ask_yn() {
  local prompt="$1" answer
  while true; do
    printf '    %s [y/n] ' "$prompt"
    if ! read -r answer; then
      answer="n"
      say ""
    fi
    case "$answer" in
      y|Y|yes|YES) return 0 ;;
      n|N|no|NO)   return 1 ;;
      *) say "    please answer y or n." ;;
    esac
  done
}

cap() {
  local label="$1"; shift
  {
    printf '\n--- %s ---\n' "$label"
    "$@" 2>&1 || true
  } >> "$REPORT"
}

# ---------------------------------------------------------------- output location

resolve_output_dir() {
  local cand
  for cand in "$ORIG_PWD" "$(dirname "$APPIMAGE" 2>/dev/null)" "$HOME"; do
    [ -n "$cand" ] && [ -d "$cand" ] && [ -w "$cand" ] || continue
    OUT_DIR="$cand"
    break
  done
  [ -n "$OUT_DIR" ] || OUT_DIR="$HOME"
  REPORT="$OUT_DIR/the-cabinet-report.txt"
  SHOT_DIR="$OUT_DIR/the-cabinet-report-shots"
}

# ---------------------------------------------------------------- find appimage

find_appimage() {
  local dir f
  for dir in "$ORIG_PWD" "$HOME/Downloads" "$HOME/Applications" "$HOME/.local/bin" "$HOME"; do
    [ -d "$dir" ] || continue
    for f in \
      "$dir"/The.Cabinet_*_amd64.AppImage \
      "$dir"/The.Cabinet_*_x86_64.AppImage \
      "$dir"/the-cabinet.AppImage \
      "$dir"/The.Cabinet*.AppImage; do
      if [ -f "$f" ]; then
        APPIMAGE="$(cd "$(dirname "$f")" && pwd)/$(basename "$f")"
        return 0
      fi
    done
  done
  return 1
}

download_appimage() {
  have curl || { say "    curl not found; cannot download."; return 1; }
  local url
  url="$(curl -fsSL "$APPIMAGE_API" 2>/dev/null \
    | tr ',' '\n' \
    | grep -o '"browser_download_url": *"[^"]*\.AppImage"' \
    | head -1 \
    | sed 's/.*"browser_download_url": *"//;s/"$//')"
  [ -n "$url" ] || { say "    could not find an AppImage on the latest release."; return 1; }
  local dest="$HOME/Downloads/$(basename "$url")"
  say "    downloading $(basename "$url") ..."
  mkdir -p "$HOME/Downloads"
  curl -fL --progress-bar -o "$dest" "$url" || return 1
  chmod +x "$dest"
  APPIMAGE="$dest"
}

# ---------------------------------------------------------------- facts

collect_facts() {
  head1 "Collecting system information"
  {
    say "The Cabinet Linux diagnosis report (v2)"
    say "generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    say "appimage:  $APPIMAGE"
  } > "$REPORT"

  cap "kernel"                    uname -srmo
  cap "os-release"                cat /etc/os-release
  cap "packages (pacman)"         pacman -Q webkit2gtk-4.1 gtk3 glib2 glibc mesa fuse2 2>/dev/null
  cap "vga"                       sh -c "lspci 2>/dev/null | grep -i -E 'vga|3d|display'"
  cap "nvidia-smi"                nvidia-smi
  cap "glxinfo"                   sh -c "glxinfo -B 2>/dev/null"
  cap "fuse"                      sh -c "ldconfig -p 2>/dev/null | grep -E 'libfuse\\.so\\.2'"
  cap "system libwayland-client"  sh -c "ldconfig -p 2>/dev/null | grep -E 'libwayland-client'"

  {
    say ""
    say "--- relevant environment ---"
    for v in XDG_SESSION_TYPE WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP \
             DESKTOP_SESSION GDK_BACKEND GTK_THEME \
             WEBKIT_DISABLE_DMABUF_RENDERER WEBKIT_DISABLE_COMPOSITING_MODE \
             LIBGL_ALWAYS_SOFTWARE LIBGL_ALWAYS_INDIRECT LD_PRELOAD; do
      eval "value=\${$v:-}"
      [ -n "$value" ] && say "$v=$value"
    done
    true
  } >> "$REPORT"
}

appimage_inventory() {
  head1 "Inspecting the AppImage contents"
  local extract="$WORKDIR/extract"
  mkdir -p "$extract"
  if ( cd "$extract" && "$APPIMAGE" --appimage-extract >/dev/null 2>&1 ); then
    EXTRACTED_APPDIR="$extract/squashfs-root"
    local root="$EXTRACTED_APPDIR"
    {
      say ""
      say "--- appimage inventory ---"
      say "bundled libwebkit:"
      find "$root/usr/lib" -maxdepth 2 -name 'libwebkit*' 2>/dev/null | sed "s|$root||" || true
      say "bundled webkit helpers:"
      find "$root/usr/lib" -path '*webkit2gtk-4.1*' -maxdepth 4 2>/dev/null | sed "s|$root||" || true
      say "bundled display-stack libraries (the EGL_BAD_PARAMETER culprits):"
      bundled_culprits "$root" | sed "s|$root||" || true
      say "all bundled .so files:"
      find "$root/usr/lib" -maxdepth 2 \( -name '*.so' -o -name '*.so.*' \) 2>/dev/null \
        | sed "s|$root||" | sort || true
      if [ -f "$root/AppRun" ]; then
        say "AppRun exported vars:"
        grep -E '^[[:space:]]*export ' "$root/AppRun" 2>/dev/null || true
      fi
    } >> "$REPORT"
    say "    done."
  else
    say "    could not extract the AppImage."
    say "    could not extract (FUSE missing?)" >> "$REPORT"
  fi
}

# Prints the bundled display-stack libs that break newer Mesa via EGL.
bundled_culprits() {
  local root="$1"
  find "$root/usr/lib" -maxdepth 2 \
    \( -name 'libwayland-*.so*' -o -name 'libxkbcommon*.so*' \
       -o -name 'libxcb-*.so*' -o -name 'libXau.so*' -o -name 'libXdmcp.so*' \) \
    2>/dev/null | sort
}

# ---------------------------------------------------------------- attempts

screenshot() {
  local name="$1"
  mkdir -p "$SHOT_DIR"
  local out="$SHOT_DIR/$name.png"
  if have spectacle; then spectacle -b -n -o "$out" >/dev/null 2>&1 && return 0; fi
  if have gnome-screenshot; then gnome-screenshot -f "$out" >/dev/null 2>&1 && return 0; fi
  if have xfce4-screenshooter; then xfce4-screenshooter -f -s "$SHOT_DIR" >/dev/null 2>&1 && return 0; fi
  if have grim; then grim "$out" >/dev/null 2>&1 && return 0; fi
  if have scrot; then scrot "$out" >/dev/null 2>&1 && return 0; fi
  if have import; then import -window root "$out" >/dev/null 2>&1 && return 0; fi
  return 1
}

# window_present — best effort; only meaningful for the forced X11 (XWayland) app.
window_present() {
  local name="$1"
  if have xdotool; then
    [ -n "$(xdotool search --name "The Cabinet" 2>/dev/null | head -1)" ] && { echo "yes"; return; }
  fi
  if have wmctrl; then
    wmctrl -l 2>/dev/null | grep -qi "The Cabinet" && { echo "yes"; return; }
  fi
  if have xwininfo; then
    xwininfo -root -tree 2>/dev/null | grep -qi "The Cabinet" && { echo "yes"; return; }
  fi
  echo "unknown"
}

# run_program <program> <label> [env...] -- [args...]
run_program() {
  local program="$1" label="$2"; shift 2
  local -a envs=()
  while [ "$1" != "--" ]; do envs+=("$1"); shift; done
  shift
  ATTEMPTS=$((ATTEMPTS + 1))
  local log="$WORKDIR/attempt-$ATTEMPTS.log"
  local envs_shown=""
  [ "${#envs[@]}" -gt 0 ] && envs_shown="${envs[*]}"

  say ""
  say "    Attempt $ATTEMPTS: $label"
  if [ -n "$envs_shown" ]; then
    say "      (with $envs_shown)"
  fi
  say "    A window should open. Wait a few seconds, then return here."

  # Clear the inherited EXIT trap inside the subshell so a failing launch does
  # not run cleanup and delete WORKDIR mid-run.
  if [ "${#envs[@]}" -gt 0 ]; then
    ( trap - EXIT; exec env "${envs[@]}" "$program" "$@" ) > "$log" 2>&1 &
  else
    ( trap - EXIT; exec "$program" "$@" ) > "$log" 2>&1 &
  fi
  local pid=$!
  sleep 12

  local alive="no"
  kill -0 "$pid" 2>/dev/null && alive="yes"
  local webprocs
  webprocs="$(pgrep -c -f 'WebKitWebProcess' 2>/dev/null || echo 0)"
  local egl="no"
  grep -q 'EGL_BAD_PARAMETER' "$log" 2>/dev/null && egl="yes"
  local window
  window="$(window_present "The Cabinet")"
  screenshot "attempt-$ATTEMPTS" && say "      saved a screenshot" || true

  {
    printf '\n--- attempt %d: %s ---\n' "$ATTEMPTS" "$label"
    printf 'program: %s\n' "$program"
    [ -n "$envs_shown" ] && printf 'env: %s\n' "$envs_shown"
    printf 'args: %s\n' "$*"
    printf 'main process alive after 12s: %s\n' "$alive"
    printf 'WebKitWebProcess count: %s\n' "$webprocs"
    printf 'EGL_BAD_PARAMETER seen: %s\n' "$egl"
    printf 'window found by title: %s\n' "$window"
    printf 'relevant stderr:\n'
    grep -a -i -E 'webkit|gtk|egl|gl |gbm|drm|gdk|wayland|x11|error|warning|fatal|segmentation|abort' "$log" 2>/dev/null | tail -n 40 || true
    printf 'tail:\n'
    tail -n 20 "$log" 2>/dev/null || true
  } >> "$REPORT"

  if ask_yn "Did The Cabinet appear correctly (not blank/white)?"; then
    PASSED="$label"
  fi
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}

# Removes the display-stack libs from an extracted AppDir so the host's Mesa can
# negotiate EGL (the upstream-verified fix for EGL_BAD_PARAMETER).
strip_culprit_libs() {
  local root="$1" before after
  before="$(bundled_culprits "$root" | wc -l | tr -d ' ')"
  find "$root/usr/lib" -maxdepth 2 \
    \( -name 'libwayland-*.so*' -o -name 'libxkbcommon*.so*' \
       -o -name 'libxcb-*.so*' -o -name 'libXau.so*' -o -name 'libXdmcp.so*' \) \
    -delete 2>/dev/null || true
  after="$(bundled_culprits "$root" | wc -l | tr -d ' ')"
  printf '\n--- stripped %d display-stack libraries from the extracted AppDir ---\n' \
    "$((before - after))" >> "$REPORT"
  say "    stripped $((before - after)) display-stack libraries from the test copy."
}

find_system_wayland_client() {
  local path
  path="$(ldconfig -p 2>/dev/null | grep -E 'libwayland-client\.so\.0' | awk '{print $NF; exit}')"
  if [ -z "$path" ]; then
    for path in /usr/lib/libwayland-client.so.0 /usr/lib64/libwayland-client.so.0 \
                /usr/lib/x86_64-linux-gnu/libwayland-client.so.0; do
      [ -f "$path" ] && break
      path=""
    done
  fi
  printf '%s' "$path"
}

run_attempts() {
  head1 "Testing the launch workarounds (3 attempts)"
  say "    Each attempt opens The Cabinet, waits, then asks you a yes/no question."
  pause

  # 1) Baseline: confirms the bug is still present with the plain launch.
  run_program "$APPIMAGE" "plain launch (baseline)" --

  # 2) Preload the host's libwayland-client ahead of the bundled one.
  local wl
  wl="$(find_system_wayland_client)"
  if [ -n "$wl" ]; then
    run_program "$APPIMAGE" "host libwayland-client preloaded (LD_PRELOAD)" \
      DESKTOPINTEGRATION=1 "LD_PRELOAD=$wl" --
  else
    say ""
    say "    Skipping the LD_PRELOAD attempt (no system libwayland-client found)."
    printf '\n--- LD_PRELOAD attempt skipped (no system libwayland-client) ---\n' >> "$REPORT"
  fi

  # 3) The real fix: run the extracted AppDir with the bundled display libs removed.
  if [ -n "$EXTRACTED_APPDIR" ] && [ -x "$EXTRACTED_APPDIR/AppRun" ]; then
    strip_culprit_libs "$EXTRACTED_APPDIR"
    run_program "$EXTRACTED_APPDIR/AppRun" "bundled display libs stripped (the candidate fix)" --
  else
    say ""
    say "    Skipping the stripped-AppDir attempt (could not extract the AppImage)."
    printf '\n--- stripped-AppDir attempt skipped (no extracted AppRun) ---\n' >> "$REPORT"
  fi

  {
    say ""
    say "--- result ---"
    if [ -n "$PASSED" ]; then
      say "working variant: $PASSED"
    else
      say "no variant rendered correctly"
    fi
    say "attempts run: $ATTEMPTS"
  } >> "$REPORT"
}

# ---------------------------------------------------------------- main

say "==> The Cabinet Linux blank-window diagnosis (v2)"
say "    This only reads your system and writes one report file. Nothing is"
say "    installed or changed outside a temporary folder. Your emulators are"
say "    never touched."

if find_appimage; then
  say "    found: $APPIMAGE"
else
  say "    no AppImage found in the current folder, Downloads, Applications,"
  say "    .local/bin or home."
  if ask_yn "Download the latest AppImage now?"; then
    download_appimage || { say "    could not download; stopping."; exit 1; }
  else
    say "    stopping. Download the AppImage, then run this script again."
    exit 0
  fi
fi

resolve_output_dir
say "    report will be saved to: $REPORT"

collect_facts
appimage_inventory
rm -f "$SHOT_DIR"/attempt-*.png
run_attempts

if [ -d "$SHOT_DIR" ] && [ -n "$(ls -A "$SHOT_DIR" 2>/dev/null)" ]; then
  printf '\nscreenshots: %s\n' "$SHOT_DIR" >> "$REPORT"
fi

head1 "Done"
if [ -n "$PASSED" ]; then
  say "    The Cabinet worked with: $PASSED"
else
  say "    None of the variants made the window appear."
fi
say "    Report saved to: $REPORT"
say "    To read or send it, run:"
say "      cat '$REPORT'"
