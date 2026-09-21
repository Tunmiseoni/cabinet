#!/bin/bash
# publish-cores — upload the frozen FBNeo libretro cores to a published GitHub
# Release so the app can download the matching core per OS (spike §8).
#
# The cores are third-party binaries and NEVER enter the repository; this script
# uploads them as release assets on the public repo only. It is run by hand, on
# purpose, after re-freezing a core.
#
# Usage:
#   scripts/publish-cores.sh <cores-dir> [--repo owner/name] [--tag TAG] [--dry-run]
#
# <cores-dir> must contain the three collected cores named exactly:
#   fbneo_libretro-macos-arm64.dylib
#   fbneo_libretro-linux-x86_64.so
#   fbneo_libretro-windows-x86_64.dll
#
# Each is verified against the sha256 + GIT revision the app expects before
# anything is uploaded, then a parity-manifest.txt and CORE-NOTICE.txt are
# generated alongside the cores and uploaded with them.
#
# Undo: delete the release with `gh release delete retroarch-cores-v1 --repo
# owner/name --cleanup-tag` (or use `--dry-run` to never publish at all).
set -euo pipefail

REPO_DEFAULT="Tunmiseoni/the-cabinet"
TAG_DEFAULT="retroarch-cores-v1"
GIT_EXPECTED="GIT6bb3167"

REPO="$REPO_DEFAULT"
TAG="$TAG_DEFAULT"
DRY_RUN=0
CORES_DIR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -*) echo "unknown flag: $1" >&2; exit 1 ;;
    *) CORES_DIR="$1"; shift ;;
  esac
done

die() { echo "error: $*" >&2; exit 1; }

[ -n "$CORES_DIR" ] || die "usage: scripts/publish-cores.sh <cores-dir> [--repo owner/name] [--tag TAG] [--dry-run]"
[ -d "$CORES_DIR" ] || die "cores dir not found: $CORES_DIR"

# asset name -> expected sha256 (must match providers/retroarch/core.rs)
ASSETS=(
  "fbneo_libretro-macos-arm64.dylib:6472c6312fe6ad49a8001efabbdc4d2b542848a7c964d0a082cd736e01e8a4eb"
  "fbneo_libretro-linux-x86_64.so:a154a08d0f97ff1c66e6ec22a5c209854f31eb2ed7d831b2cb4c0101d48a2448"
  "fbneo_libretro-windows-x86_64.dll:0a92f3b61dba68b34df0a93afe1b24b98debb3c25dfe63bf21c02034fdfa1179"
)

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

nonempty() { [ -s "$1" ]; }

FILES=()
for entry in "${ASSETS[@]}"; do
  name="${entry%%:*}"
  want="${entry##*:}"
  path="$CORES_DIR/$name"
  [ -f "$path" ] || die "missing $path"
  nonempty "$path" || die "empty file: $path"

  got="$(sha256 "$path")"
  [ "$got" = "$want" ] || die "$name sha256 mismatch: expected $want, got $got (re-freeze the core?)"

  git="$(strings "$path" 2>/dev/null | grep -aoE 'GIT[0-9a-f]+' | sort -u | head -1 || true)"
  [ "$git" = "$GIT_EXPECTED" ] || die "$name reports '${git:-<none>}', expected $GIT_EXPECTED"

  echo "ok: $name  sha256=$got  $git"
  FILES+=("$path")
done

MANIFEST="$CORES_DIR/parity-manifest.txt"
NOTICE="$CORES_DIR/CORE-NOTICE.txt"

{
  echo "# The Cabinet frozen FBNeo libretro cores ($TAG)"
  echo "# sha256  asset"
  for entry in "${ASSETS[@]}"; do
    name="${entry%%:*}"
    echo "$(sha256 "$CORES_DIR/$name")  $name"
  done
  echo "# core revision: $GIT_EXPECTED"
} > "$MANIFEST"

cat > "$NOTICE" <<EOF
The Cabinet — frozen FBNeo libretro core notice
================================================

These release assets are FBNeo libretro cores, not part of The Cabinet (MIT).

- FBNeo libretro core: license declared in its .info as "Non-commercial".
  Upstream source: https://github.com/libretro/FBNeo
  Frozen revision: $GIT_EXPECTED
- RetroArch (the frontend that loads the core) is GPLv3 and is NOT distributed
  here; users install it separately.

The cores are provided so every machine in a netplay group runs the exact same
core revision and sha256 (see The Cabinet's parity gate). Redistribution is for
non-commercial use only. Source for the frozen revision is offered at the
upstream repository above.

Nothing here is a ROM. ROMs are never distributed.
EOF

echo "staged: $MANIFEST"
echo "staged: $NOTICE"

if [ "$DRY_RUN" = "1" ]; then
  echo
  echo "dry run — would publish to $REPO release '$TAG':"
  printf '  %s\n' "${FILES[@]}" "$MANIFEST" "$NOTICE"
  exit 0
fi

command -v gh >/dev/null 2>&1 || die "gh CLI not found (install it or use --dry-run)"

if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  echo "release $TAG exists — uploading with --clobber"
  gh release upload "$TAG" --repo "$REPO" --clobber "${FILES[@]}" "$MANIFEST" "$NOTICE"
else
  echo "creating published release $TAG on $REPO"
  gh release create "$TAG" --repo "$REPO" \
    --title "Frozen RetroArch cores ($TAG)" \
    --notes "Frozen FBNeo libretro cores for The Cabinet's RetroArch provider. Not the app installers — see the app releases. See CORE-NOTICE.txt for licensing." \
    "${FILES[@]}" "$MANIFEST" "$NOTICE"
fi

# A create interrupted mid-upload leaves a draft; make sure it is published so
# the app's download URL resolves (no-op when it is already public).
gh release edit "$TAG" --repo "$REPO" --draft=false --prerelease=false >/dev/null

echo
echo "done. assets:"
gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets[].name'
