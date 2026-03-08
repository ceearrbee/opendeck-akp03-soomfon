#!/usr/bin/env bash
set -euo pipefail

echo "[1/4] Build hardware plugin"
cargo build --release

echo "[2/4] Validate manifest ids/version/platform"
grep -q '"PluginUUID": "st.lynx.plugins.opendeck-akp03"' manifest.json
grep -q '"UUID": "st.lynx.plugins.opendeck-akp03.buttontest"' manifest.json
grep -q '"UUID": "st.lynx.plugins.opendeck-akp03.knobtest"' manifest.json
grep -q '"Platform": "linux"' manifest.json
if grep -q '"Platform": "windows"\|"Platform": "mac"\|"Platform": "macos"' manifest.json; then
  echo "manifest contains non-linux platform entries"
  exit 1
fi

manifest_version=$(grep -o '"Version": "[^"]*"' manifest.json | head -n1 | sed -E 's/.*"([^"]+)"/\1/')
cargo_version=$(grep -E '^version = "' Cargo.toml | head -n1 | sed -E 's/version = "([^"]+)"/\1/')
if [[ "$manifest_version" != "$cargo_version" ]]; then
  echo "version mismatch: manifest=$manifest_version cargo=$cargo_version"
  exit 1
fi

echo "[3/4] Validate README public contract"
grep -qi 'Platform: Linux' README.md
grep -qi 'OPENDECK_ENABLE_MIDI=1' README.md
grep -qi 'Button Test' README.md

echo "[4/4] Ensure tracked build artifacts are absent"
if git ls-files | grep -q '^darktable-plugin/target/\|^target/'; then
  echo "tracked target artifacts are still present in git index"
  exit 1
fi

echo "Release checks passed"
