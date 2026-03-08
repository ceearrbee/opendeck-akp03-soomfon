#[cfg(target_os = "linux")]
use std::sync::LazyLock;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
use midir::{MidiOutput, MidiOutputConnection, os::unix::VirtualOutput};
#[cfg(target_os = "linux")]
use tokio::sync::Mutex;

#[cfg(target_os = "linux")]
const MIDI_CHANNEL: u8 = 0;
#[cfg(target_os = "linux")]
const BUTTON_BASE_NOTE: u8 = 36;
#[cfg(target_os = "linux")]
const ENCODER_PRESS_BASE_NOTE: u8 = 80;
#[cfg(target_os = "linux")]
const ENCODER_BASE_CC: u8 = 16;

#[cfg(target_os = "linux")]
struct MidiBridge {
    conn: MidiOutputConnection,
}

#[cfg(target_os = "linux")]
impl MidiBridge {
    fn send(&mut self, bytes: &[u8]) {
        if let Err(err) = self.conn.send(bytes) {
            crate::file_log(&format!("MIDI send error: {err}"));
        }
    }
}

#[cfg(target_os = "linux")]
static MIDI: LazyLock<Mutex<Option<MidiBridge>>> = LazyLock::new(|| Mutex::new(None));
#[cfg(target_os = "linux")]
static MIDI_ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
pub async fn init() {
    let enabled = std::env::var("OPENDECK_ENABLE_MIDI")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    MIDI_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        crate::file_log("MIDI bridge disabled (set OPENDECK_ENABLE_MIDI=1 to enable)");
        return;
    }

    let mut guard = MIDI.lock().await;
    if guard.is_some() {
        return;
    }

    let out = match MidiOutput::new("OpenDeck Soomfon SE MIDI") {
        Ok(v) => v,
        Err(err) => {
            crate::file_log(&format!("MIDI init failed (MidiOutput): {err}"));
            return;
        }
    };

    let conn = match out.create_virtual("OpenDeck Soomfon SE MIDI") {
        Ok(v) => v,
        Err(err) => {
            crate::file_log(&format!("MIDI init failed (create_virtual): {err}"));
            return;
        }
    };

    *guard = Some(MidiBridge { conn });
    crate::file_log("MIDI virtual output ready: OpenDeck Soomfon SE MIDI");
}

#[cfg(target_os = "linux")]
fn relative_cc_value(delta: i16) -> u8 {
    if delta >= 0 {
        delta.clamp(1, 63) as u8
    } else {
        let mag = (-delta).clamp(1, 63) as u8;
        128u8.saturating_sub(mag)
    }
}

#[cfg(target_os = "linux")]
pub async fn button_down(button: u8) {
    if !MIDI_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = MIDI.lock().await;
    if let Some(midi) = guard.as_mut() {
        midi.send(&[0x90 | MIDI_CHANNEL, BUTTON_BASE_NOTE.saturating_add(button), 127]);
    }
}

#[cfg(target_os = "linux")]
pub async fn button_up(button: u8) {
    if !MIDI_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = MIDI.lock().await;
    if let Some(midi) = guard.as_mut() {
        midi.send(&[0x80 | MIDI_CHANNEL, BUTTON_BASE_NOTE.saturating_add(button), 0]);
    }
}

#[cfg(target_os = "linux")]
pub async fn encoder_down(encoder: u8) {
    if !MIDI_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = MIDI.lock().await;
    if let Some(midi) = guard.as_mut() {
        midi.send(&[
            0x90 | MIDI_CHANNEL,
            ENCODER_PRESS_BASE_NOTE.saturating_add(encoder),
            127,
        ]);
    }
}

#[cfg(target_os = "linux")]
pub async fn encoder_up(encoder: u8) {
    if !MIDI_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = MIDI.lock().await;
    if let Some(midi) = guard.as_mut() {
        midi.send(&[
            0x80 | MIDI_CHANNEL,
            ENCODER_PRESS_BASE_NOTE.saturating_add(encoder),
            0,
        ]);
    }
}

#[cfg(target_os = "linux")]
pub async fn encoder_twist(encoder: u8, delta: i16) {
    if !MIDI_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = MIDI.lock().await;
    if let Some(midi) = guard.as_mut() {
        midi.send(&[
            0xB0 | MIDI_CHANNEL,
            ENCODER_BASE_CC.saturating_add(encoder),
            relative_cc_value(delta),
        ]);
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn init() {}
#[cfg(not(target_os = "linux"))]
pub async fn button_down(_button: u8) {}
#[cfg(not(target_os = "linux"))]
pub async fn button_up(_button: u8) {}
#[cfg(not(target_os = "linux"))]
pub async fn encoder_down(_encoder: u8) {}
#[cfg(not(target_os = "linux"))]
pub async fn encoder_up(_encoder: u8) {}
#[cfg(not(target_os = "linux"))]
pub async fn encoder_twist(_encoder: u8, _delta: i16) {}
