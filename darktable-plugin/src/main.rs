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

fn default_path() -> String { "lib/exposure/exposure".to_string() }
fn default_step() -> f32 { 0.1 }

impl ActionEventHandler for DarktablePlugin {
    async fn key_down(&self, event: KeyEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        match event.action.as_str() {
            "st.lynx.plugins.darktable.switchview" => {
                send_lua("darktable.gui.action('views/darkroom/lighttable', 'toggle', 0)");
            }
            "st.lynx.plugins.darktable.adjust" => {
                // Reset on push
                let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                send_lua(&format!("darktable.gui.action('{}', 'set', 0)", settings.path));
            }
            _ => {}
        }
        Ok(())
    }

    async fn dial_rotate(&self, event: DialRotateEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if event.action == "st.lynx.plugins.darktable.adjust" {
            let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
            let ticks = event.payload.ticks as f32;
            let amount = ticks * settings.step;
            send_lua(&format!("darktable.gui.action('{}', 'inc', {})", settings.path, amount));
        }
        Ok(())
    }
}

impl GlobalEventHandler for DarktablePlugin {}

fn send_lua(cmd: &str) {
    let _ = Command::new("dbus-send")
        .args([
            "--type=method_call",
            "--dest=org.darktable.service",
            "/org/darktable/service/Remote",
            "org.darktable.service.Remote.Lua",
            &format!("string:{}", cmd),
        ])
        .spawn();
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
