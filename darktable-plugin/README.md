# Darktable Plugin for OpenDeck

This plugin provides direct integration with Darktable via its DBus Lua interface.

## Actions

### Darktable Adjust (Knob/Button)
- **Rotation:** Increments or decrements a Darktable slider.
- **Press:** Resets the slider to 0.
- **Settings:**
    - `Slider Path`: The internal Darktable path (e.g., `lib/exposure/exposure`).
    - `Step Size`: How much to change the value per tick (e.g., `0.1`).

### Switch View (Button)
- Toggles between the Darkroom and Lighttable views.

## Installation

The plugin is already installed in your OpenDeck Flatpak directory.

## Pre-configured Profile

A profile named **"Darktable"** has been created for your Soomfon SE. You can select it in the OpenDeck UI.
- **Large Knob:** Exposure
- **Small Knob 1:** Contrast
- **Small Knob 2:** Saturation
- **Top Left Button:** Toggle View
