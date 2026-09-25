#!/bin/bash
# clean — remove regenerable build output.
#
#   scripts/clean.sh          build artifacts (Rust target, Vite dist, generated schemas)
#   scripts/clean.sh --deps   also remove installed dependencies (node_modules)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WITH_DEPS=0
for arg in "$@"; do
  case "$arg" in
    --deps) WITH_DEPS=1 ;;
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
