![Plugin Icon](assets/icon.png)

# OpenDeck Soomfon SE Plugin

Hardware plugin for Soomfon Stream Controller SE with OpenDeck events and Linux virtual MIDI output.

## OpenDeck version

Requires OpenDeck 2.5.0 or newer

## Supported device

- Soomfon Stream Controller SE (1500:3001)

## Platform support

- Linux: Guaranteed, if stuff breaks - I'll probably catch it before public release
- Mac: Best effort, no tests before release, things may break, but I probably have means to fix them
- Windows: Zero effort, no tests before release, if stuff breaks - too bad, it's up to you to contribute fixes

## Installation

1. Download an archive from [releases](https://github.com/4ndv/opendeck-akp03/releases)
2. In OpenDeck: Plugins -> Install from file
3. Download [udev rules](./40-opendeck-akp03.rules) and install them by copying into `/etc/udev/rules.d/` and running `sudo udevadm control --reload-rules`
4. Unplug and plug again the device, restart OpenDeck

## Control mapping

The plugin exposes Soomfon controls as OpenDeck `3x3` keypad + `3` encoders:

- Physical keys are positions `0..8`.
- Encoder press controls are `0..2`.
- Encoder turns are `0..2`.

## Virtual MIDI output (Linux)

This plugin now also creates a virtual MIDI output port on Linux:

- Port name: `OpenDeck Soomfon SE MIDI`
- Channel: `1`
- Buttons `0..8`: MIDI Note `36..44` (Note On/Off)
- Encoder press `0..2`: MIDI Note `80..82` (Note On/Off)
- Encoder turn `0..2`: MIDI CC `16..18` (relative mode)
  - Clockwise: `1..63`
  - Counter-clockwise: `127..65`

## Adding new devices

Read [this wiki page](https://github.com/4ndv/opendeck-akp03/wiki/Adding-support-for-new-devices) for more information.

## Building

### Prerequisites

You'll need:

- A Linux OS of some sort
- Rust 1.87 and up with `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` targets installed
- gcc with Windows support
- Docker
- [just](https://just.systems)

On Arch Linux:

```sh
sudo pacman -S just mingw-w64-gcc mingw-w64-binutils
```

Adding rust targets:

```sh
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-gnu
```

### Preparing environment

```sh
$ just prepare
```

This will build docker image for macOS crosscompilation

### Building a release package

```sh
$ just package
```

## Acknowledgments

This plugin is heavily based on work by contributors of [elgato-streamdeck](https://github.com/streamduck-org/elgato-streamdeck) crate
