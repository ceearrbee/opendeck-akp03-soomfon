![Plugin Icon](assets/icon.png)

# OpenDeck Soomfon SE Plugin

Hardware plugin for Soomfon Stream Controller SE (`1500:3001`) with OpenDeck device events and optional Linux virtual MIDI output.

## Support Matrix

- Platform: Linux
- OpenDeck: 2.5.0+
- Device: Soomfon Stream Controller SE (`1500:3001`)

This release does not target Windows or macOS.

## Installation

1. Download plugin archive from releases.
2. In OpenDeck: `Plugins -> Install from file`.
3. Install udev rules from [`40-opendeck-akp03.rules`](./40-opendeck-akp03.rules):
   ```sh
   sudo cp 40-opendeck-akp03.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   ```
4. Replug device and restart OpenDeck.

## Control Mapping

Exposed as OpenDeck `3x3` keypad + `3` encoders:

- Keys: `0..8`
- Encoder press: `0..2`
- Encoder rotate: `0..2`

## MIDI Bridge (Linux, Optional)

MIDI bridge is disabled by default.

Enable explicitly:

```sh
OPENDECK_ENABLE_MIDI=1 flatpak run me.amankhanna.opendeck
```

Behavior when enabled:

- Port: `OpenDeck Soomfon SE MIDI`
- Channel: `1`
- Buttons `0..8` -> Notes `36..44` (on/off)
- Encoder press `0..2` -> Notes `80..82` (on/off)
- Encoder rotate `0..2` -> CC `16..18` (relative)

## Included Actions

- `Button Test` (`st.lynx.plugins.opendeck-akp03.buttontest`)
- `Knob Test` (`st.lynx.plugins.opendeck-akp03.knobtest`)

These are intentionally visible for hardware diagnostics.

## Troubleshooting

OpenDeck logs:

- `~/.var/app/me.amankhanna.opendeck/data/opendeck/logs/opendeck.log`
- `~/.var/app/me.amankhanna.opendeck/data/opendeck/logs/plugins/st.lynx.plugins.opendeck-akp03.sdPlugin.log`

Common issues:

- Device not discovered: confirm udev rules and replug.
- No MIDI output: verify `OPENDECK_ENABLE_MIDI=1` is set.
- Input events missing: check plugin log for registration/device lifecycle errors.

## Build and Release

Build:

```sh
cargo build --release
```

Run release checks:

```sh
./release-check.sh
```

Package plugin (Linux artifact):

```sh
just package-linux
```
