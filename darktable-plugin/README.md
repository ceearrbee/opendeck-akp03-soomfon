# Darktable Plugin for OpenDeck

OpenDeck plugin that controls darktable through darktable's Lua DBus endpoint.

## Support Matrix

- Platform: Linux
- Target runtime: OpenDeck Flatpak + darktable Flatpak
- Transport: `flatpak-spawn --host gdbus`

This release does not target Windows or macOS.

## Actions

### Darktable Adjust (`st.lynx.plugins.darktable.adjust`)

- Controllers: Keypad, Encoder
- Encoder rotate: increment/decrement action path
- Encoder press: reset action path
- Keypad press: one adjustment step
- Settings:
  - `path`: darktable action path, for example `iop/exposure/exposure`
  - `step`: adjustment step size (`1.0` default)

### Darktable Toggle (`st.lynx.plugins.darktable.toggle`)

- Controllers: Keypad
- Executes toggle-style paths (for example `views/darkroom/overexposed/toggle`)
- Maintains local OpenDeck state (`0/1`) for active/inactive icon variants
- Settings:
  - `path`: darktable action path

## Requirements

1. OpenDeck running in Flatpak
2. darktable Flatpak installed (`org.darktable.Darktable`)
3. darktable started before use so `org.darktable.service` is available on session DBus

## Install

1. Build plugin binary:
   ```sh
   cargo build --release --manifest-path darktable-plugin/Cargo.toml
   ```
2. Install `manifest.json`, `pi.html`, `assets/`, and binary `opendeck-darktable` into plugin directory `st.lynx.plugins.darktable.sdPlugin`.
3. Import/select the provided profile for your device.

## Troubleshooting

OpenDeck plugin logs:

- `~/.var/app/me.amankhanna.opendeck/data/opendeck/logs/plugins/st.lynx.plugins.darktable.sdPlugin.log`

Common errors:

- `service-unavailable`: darktable is not running; start with:
  ```sh
  flatpak run org.darktable.Darktable
  ```
- `spawn-error`: `flatpak-spawn` missing in runtime
- `timeout`: DBus call timed out; verify darktable is responsive
- `lua-error`: invalid path or Lua action call failed

## Known Limitations

- Toggle visual state is local to plugin context; it is not a full readback of darktable internal state.
- View-switch actions in darktable can be context-sensitive; this release focuses on reliable adjust/toggle action behavior.

## Release Gate

Run before publishing:

```sh
./darktable-plugin/release-check.sh
```
