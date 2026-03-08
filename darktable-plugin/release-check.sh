#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "[1/3] Build darktable plugin"
cargo build --release --manifest-path darktable-plugin/Cargo.toml

echo "[2/3] Validate darktable manifest fields"
manifest="darktable-plugin/manifest.json"
grep -q '"PluginUUID": "st.lynx.plugins.darktable"' "$manifest"
grep -q '"UUID": "st.lynx.plugins.darktable.adjust"' "$manifest"
grep -q '"UUID": "st.lynx.plugins.darktable.toggle"' "$manifest"
if grep -q '"UUID": "st.lynx.plugins.darktable.switchview"' "$manifest"; then
  echo "switchview action must not be present in public manifest"
  exit 1
fi

echo "[3/3] Validate docs mention flatpak target"
grep -qi 'OpenDeck Flatpak' darktable-plugin/README.md
grep -qi 'darktable Flatpak' darktable-plugin/README.md

echo "Release checks passed"
