use openaction::*;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Default, Clone)]
struct DarktablePlugin;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AdjustSettings {
    #[serde(default = "default_path")]
    path: String,
    #[serde(default = "default_step")]
    step: f32,
}

impl Default for AdjustSettings {
    fn default() -> Self {
        Self {
            path: default_path(),
            step: default_step(),
        }
    }
}

fn default_path() -> String { "iop/exposure/exposure".to_string() }
fn default_step() -> f32 { 1.0 }

fn escape_lua_str(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\'', "\\'")
}

impl ActionEventHandler for DarktablePlugin {
    async fn key_down(&self, event: KeyEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        match event.action.as_str() {
            "st.lynx.plugins.darktable.switchview" => {
                log::info!("key_down switchview context={}", event.context);
                let _ = send_lua("local darktable = require 'darktable'; darktable.gui.action('global/switch views/darkroom'); return 'ok'");
            }
            "st.lynx.plugins.darktable.adjust" => {
                let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                let path = escape_lua_str(&settings.path);
                if event.payload.controller == "Encoder" {
                    // Encoder push resets current control.
                    log::info!("key_down encoder-reset path={} context={}", settings.path, event.context);
                    let _ = send_lua(&format!(
                        "local darktable = require 'darktable'; darktable.gui.action('{}', '', 'reset'); return 'ok'",
                        path
                    ));
                } else {
                    // Keypad press performs one adjustment step.
                    let effect = if settings.step >= 0.0 { "up" } else { "down" };
                    let speed = settings.step.abs().round().max(1.0) as i32;
                    log::info!(
                        "key_down keypad-adjust path={} effect={} speed={} context={}",
                        settings.path, effect, speed, event.context
                    );
                    let _ = send_lua(&format!(
                        "local darktable = require 'darktable'; darktable.gui.action('{}', '', '{}', {}); return 'ok'",
                        path, effect, speed
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn dial_down(&self, event: DialPressEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if event.action == "st.lynx.plugins.darktable.adjust" {
            let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
            let path = escape_lua_str(&settings.path);
            log::info!("dial_down reset path={} context={}", settings.path, event.context);
            let _ = send_lua(&format!(
                "local darktable = require 'darktable'; darktable.gui.action('{}', '', 'reset'); return 'ok'",
                path
            ));
        }
        Ok(())
    }

    async fn dial_rotate(&self, event: DialRotateEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if event.action == "st.lynx.plugins.darktable.adjust" {
            let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
            let ticks = event.payload.ticks as f32;
            let movement = (ticks * settings.step).round() as i32;
            if movement != 0 {
                let effect = if movement > 0 { "up" } else { "down" };
                let speed = movement.abs();
                let path = escape_lua_str(&settings.path);
                log::info!(
                    "dial_rotate path={} effect={} speed={} context={} ticks={}",
                    settings.path, effect, speed, event.context, event.payload.ticks
                );
                let _ = send_lua(&format!(
                    "local darktable = require 'darktable'; darktable.gui.action('{}', '', '{}', {}); return 'ok'",
                    path, effect, speed
                ));
            }
        }
        Ok(())
    }
}

impl GlobalEventHandler for DarktablePlugin {}

fn send_lua(cmd: &str) -> bool {
    // Verified endpoint (Flatpak darktable):
    // dest=org.darktable.service, path=/darktable, method=org.darktable.service.Remote.Lua
    let result = Command::new("flatpak-spawn")
        .args([
            "--host",
            "gdbus",
            "call",
            "--session",
            "--timeout",
            "1",
            "--dest",
            "org.darktable.service",
            "--object-path",
            "/darktable",
            "--method",
            "org.darktable.service.Remote.Lua",
            cmd,
        ])
        .output();

    match result {
        Ok(out) => {
            if out.status.success() {
                log::debug!("darktable Lua OK: {}", cmd);
                true
            } else {
                log::error!(
                    "darktable Lua FAILED: status={:?}, stderr={}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stderr)
                );
                false
            }
        }
        Err(err) => {
            log::error!("Failed to dispatch darktable Lua command: {} ({})", cmd, err);
            false
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    simplelog::TermLogger::init(
        log::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    ).ok();

    log::info!("Darktable Plugin starting...");

    let handler = DarktablePlugin;
    
    init_plugin(handler.clone(), handler).await
}
