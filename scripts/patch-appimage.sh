#!/bin/bash
# patch-appimage — strip the over-bundled display-stack libraries from the Linux
# AppImage so WebKit can create an EGL display on newer Mesa (25+).
#
# ============================================================================
# TEMPORARY WORKAROUND — READ THIS BEFORE CHANGING ANYTHING
# ============================================================================
#
# WHY THIS EXISTS
#   The Tauri AppImage bundler (linuxdeploy) sweeps the build machine's
#   Wayland/X11 client libraries into the bundle and puts them ahead of the
#   host's via LD_LIBRARY_PATH. On a Mesa 25+ host (e.g. CachyOS, Mesa 26) Mesa
#   negotiates EGL against the older bundled `libwayland-client` and fails:
#
#       Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
#
#   WebKitWebProcess dies before a window appears, and no `WEBKIT_*` flag
#   (DISABLE_DMABUF_RENDERER / DISABLE_COMPOSITING_MODE) helps. Verified on the
#   CachyOS machine: removing the libraries below makes the app render.
#
#   Upstream: tauri-apps/tauri#15976 (root cause + minimal fix), #15665. The
#   official fix is PR #15662, adding `bundle.linux.appimage.excludeLibraries`.
#
# HOW TO REMOVE THIS WORKAROUND (once upstream lands)
#   1. Bump `@tauri-apps/cli` (package.json) and the `tauri` crate (Cargo.toml)
#      to a release that contains PR #15662.
#   2. Add to src-tauri/tauri.conf.json under "bundle":
#          "linux": { "appimage": { "excludeLibraries": [
#            "libwayland-*.so*", "libxkbcommon*.so*", "libxcb-*.so*",
#            "libXau.so*", "libXdmcp.so*"
#          ] } }
#   3. Delete this file (scripts/patch-appimage.sh).
#
#   The two call sites are guarded and become no-ops once this file is gone:
#     - scripts/tauri-build.sh  (used by release.yml's tauriScript)
#     - scripts/build.sh
#   You may also delete those guarded lines, but it is not required. No
#   tauri.conf.json / Cargo.toml change has to be reverted to use the config
#   option — the config is just added when the upstream fix is available.
#
# BEHAVIOUR
#   * No-op (exit 0) on non-Linux, or when no AppDir is found, or when the
#     culprit libraries are already absent (future Tauri). Safe to leave wired.
#   * Otherwise it removes them from the built AppDir and repacks it with
#     appimagetool (a pure packer — it does not re-resolve/re-add libraries the
#     way re-running linuxdeploy would).
#   * Because repacking changes the AppImage's bytes, the updater signature Tauri
#     wrote beforehand no longer matches. When a signing key is present (CI) the
#     repacked AppImage is re-signed so the in-app updater accepts it; otherwise
#     the stale `.sig` is removed rather than shipped wrong.
#
# ENV OVERRIDES (testing / unusual layouts)
#   THE_CABINET_BUNDLE_DIR  bundle/appimage directory to patch
#   THE_CABINET_APPDIR      AppDir to patch (skips discovery)
#   APPIMAGETOOL            appimagetool binary to use (skips the download)
# ============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log() { printf 'patch-appimage: %s\n' "$*"; }

# The display/GPU-stack libraries that must come from the host, not the bundle.
# Extra arguments (e.g. -delete) are forwarded to find.
culprit_find() {
  local dir="$1"; shift
  find "$dir/usr/lib" -maxdepth 2 \
    \( -name 'libwayland-*.so*' -o -name 'libxkbcommon*.so*' \
       -o -name 'libxcb-*.so*' -o -name 'libXau.so*' -o -name 'libXdmcp.so*' \) \
    "$@" 2>/dev/null
}

case "$(uname -s)" in
  Linux) ;;
  *) log "not Linux; nothing to do"; exit 0 ;;
esac

# ---------------------------------------------------------------- locate AppDir
BUNDLE_DIR="${THE_CABINET_BUNDLE_DIR:-}"
if [ -z "$BUNDLE_DIR" ]; then
  BUNDLE_DIR="$(find "$ROOT/src-tauri/target" -type d -path '*/release/bundle/appimage' 2>/dev/null | head -1)"
fi
if [ -z "$BUNDLE_DIR" ] || [ ! -d "$BUNDLE_DIR" ]; then
  log "no bundle/appimage directory found; nothing to do"
  exit 0
fi

APPDIR="${THE_CABINET_APPDIR:-}"
if [ -z "$APPDIR" ]; then
  APPDIR="$(find "$BUNDLE_DIR" -maxdepth 1 -type d -name '*.AppDir' 2>/dev/null | head -1)"
fi
if [ -z "$APPDIR" ] || [ ! -d "$APPDIR" ]; then
  log "no .AppDir found under $BUNDLE_DIR; nothing to do"
  exit 0
fi

# ---------------------------------------------------------------- count culprits
BEFORE="$(culprit_find "$APPDIR" | wc -l | tr -d ' ')"
if [ "$BEFORE" = "0" ]; then
  log "bundle already excludes the display-stack libraries; nothing to do"
  log "(upstream fix appears to be in effect — see the removal notes in this file)"
  exit 0
fi

log "removing $BEFORE over-bundled display-stack libraries from $(basename "$APPDIR")"
culprit_find "$APPDIR" | sed 's/^/patch-appimage:   /'
culprit_find "$APPDIR" -delete 2>/dev/null || true

AFTER="$(culprit_find "$APPDIR" | wc -l | tr -d ' ')"
if [ "$AFTER" != "0" ]; then
  log "error: $AFTER libraries still present after removal"
  exit 1
fi

# ---------------------------------------------------------------- repack
case "$(uname -m)" in
  x86_64)         TOOL_ARCH="x86_64" ;;
  aarch64|arm64)  TOOL_ARCH="aarch64" ;;
  i686|i386)      TOOL_ARCH="i686" ;;
  armv7l|armhf)   TOOL_ARCH="armhf" ;;
  *) log "unsupported architecture $(uname -m)"; exit 1 ;;
esac

if [ -n "${APPIMAGETOOL:-}" ] && [ -x "${APPIMAGETOOL:-}" ]; then
  TOOL="$APPIMAGETOOL"
else
  CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/the-cabinet"
  TOOL="$CACHE/appimagetool-$TOOL_ARCH.AppImage"
  if [ ! -x "$TOOL" ]; then
    mkdir -p "$CACHE"
    log "downloading appimagetool ($TOOL_ARCH) to $TOOL"
    ok=0
    for url in \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$TOOL_ARCH.AppImage" \
      "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-$TOOL_ARCH.AppImage"; do
      if curl -fsSL -o "$TOOL" "$url"; then ok=1; break; fi
    done
    [ "$ok" = 1 ] || { log "error: could not download appimagetool"; exit 1; }
    chmod +x "$TOOL"
  fi
fi

# Reuse the existing AppImage's filename so the release asset name is unchanged.
EXISTING="$(find "$BUNDLE_DIR" -maxdepth 1 -type f -name '*.AppImage' 2>/dev/null | head -1)"
if [ -n "$EXISTING" ]; then
  OUTPUT="$EXISTING"
else
  BASE="$(basename "$APPDIR" .AppDir)"
  OUTPUT="$BUNDLE_DIR/${BASE// /.}.AppImage"
fi
rm -f "$OUTPUT"

ARCH_ENV="$TOOL_ARCH"
log "repacking $(basename "$OUTPUT")"
if ! APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH_ENV" "$TOOL" --appimage-extract-and-run --no-appstream "$APPDIR" "$OUTPUT"; then
  log "error: appimagetool failed"
  exit 1
fi

[ -f "$OUTPUT" ] || { log "error: expected output $OUTPUT was not produced"; exit 1; }
chmod +x "$OUTPUT"

# Tauri signed the AppImage before the repack, so its .sig is now stale.
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
  TAURI_BIN="$ROOT/node_modules/.bin/tauri"
  if [ ! -x "$TAURI_BIN" ]; then
    log "error: Tauri CLI not found at $TAURI_BIN; cannot re-sign the repacked AppImage"
    exit 1
  fi
  log "re-signing $(basename "$OUTPUT")"
  rm -f "$OUTPUT.sig"
  if ! TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD-}" "$TAURI_BIN" signer sign "$OUTPUT"; then
    log "error: re-signing the repacked AppImage failed"
    exit 1
  fi
  [ -f "$OUTPUT.sig" ] || { log "error: expected signature $OUTPUT.sig was not produced"; exit 1; }
else
  log "warning: no signing key set; dropping the stale .sig for the repacked AppImage"
  rm -f "$OUTPUT.sig"
fi

log "wrote $OUTPUT"
