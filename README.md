# OpenDeck AKP03 (Soomfon SE Fork)

This is a fork of [4ndv/opendeck-akp03](https://github.com/4ndv/opendeck-akp03) specifically patched for the Soomfon SE hardware.

# OpenDeck AKP03 - Soomfon SE Patched

This is a modified version of the AKP03 plugin for OpenDeck, specifically patched to support the **Soomfon SE** (HID 1500:3001) physical knobs and buttons.

## Breakthrough Features
- **Full Knob Support**: Successfully mapped the non-standard HID packets (`0x90`, `0x50`, `0x60` codes) to OpenDeck `EncoderTwist` events.
- **Stream Deck + Emulation**: Registers as **Type 7** (SD+) with a 4x2 grid and 4 encoders to force the OpenDeck UI to display dial controls.
- **Mirajazz Patch**: Includes a localized and patched version of `mirajazz` that removes protocol version assertions and ACK-prefix filtering, which originally blocked Soomfon packets.
- **Fedora Silverblue Ready**: Optimized build and deployment scripts for Toolbox/Flatpak environments.

## HID Logic
| Control | CCW Code | CW Code | Press Code |
|---------|----------|---------|------------|
| Knob 1  | 0x90     | 0x91    | 0x33       |
| Knob 2  | 0x50     | 0x51    | 0x35       |
| Knob 3  | 0x60     | 0x61    | 0x34       |

## Build & Deploy (Fedora Silverblue)
1. Build in toolbox:
   ```bash
   cargo build --release
   ```
2. Deploy to host OpenDeck Flatpak:
   ```bash
   # Use the absolute host path from within toolbox (/run/host/...)
   DEST="/run/host/var/home/crb/.var/app/me.amankhanna.opendeck/config/opendeck/plugins/st.lynx.plugins.opendeck-akp03.sdPlugin/opendeck-akp03-linux"
   cp target/release/opendeck-akp03 "$DEST"
   ```
3. Ensure `40-opendeck-akp03.rules` is installed in `/etc/udev/rules.d/` for HID permissions.
