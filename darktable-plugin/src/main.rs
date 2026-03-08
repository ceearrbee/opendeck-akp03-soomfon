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

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ToggleSettings {
    #[serde(default = "default_toggle_path")]
    path: String,
}

impl Default for ToggleSettings {
    fn default() -> Self {
        Self {
            path: default_toggle_path(),
        }
    }
}

fn default_path() -> String { "iop/exposure/exposure".to_string() }
fn default_step() -> f32 { 1.0 }
fn default_toggle_path() -> String { "views/darkroom/overexposed/toggle".to_string() }

fn escape_lua_str(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\'', "\\'")
}

fn normalize_action_path(path: &str) -> &str {
    if path == "__selected__" { "" } else { path }
}

fn normalize_toggle_path(path: &str) -> &str {
    let normalized = normalize_action_path(path);
    normalized.strip_suffix("/toggle").unwrap_or(normalized)
}

fn lua_bootstrap() -> &'static str {
    "local darktable = require 'darktable'; \
     local function ensure_darkroom() \
       if darktable.gui.current_view().id ~= 'darkroom' then \
         local first = darktable.collection[1]; \
         if first ~= nil then darktable.gui.selection({first}) end; \
         darktable.gui.current_view(darktable.gui.views.darkroom); \
       end; \
     end;"
}

impl ActionEventHandler for DarktablePlugin {
    async fn key_down(&self, event: KeyEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        match event.action.as_str() {
            "st.lynx.plugins.darktable.switchview" => {
                log::info!("key_down switchview context={}", event.context);
                let _ = send_lua(&format!(
                    "{} \
                     if darktable.gui.current_view().id == 'darkroom' then \
                       darktable.gui.action('global/switch views/lighttable'); \
                     else \
                       ensure_darkroom(); \
                     end; \
                     return darktable.gui.current_view().id",
                    lua_bootstrap()
                ));
            }
            "st.lynx.plugins.darktable.toggle" => {
                let settings: ToggleSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                let normalized = normalize_toggle_path(&settings.path);
                let path = escape_lua_str(normalized);
                log::info!(
                    "key_down toggle path={} normalized={} context={}",
                    settings.path, normalized, event.context
                );
                let _ = send_lua(&format!(
                    "{} \
                     if string.sub('{}', 1, 15) == 'views/darkroom/' then ensure_darkroom() end; \
                     darktable.gui.action('{}', '', 'toggle', 1); \
                     return darktable.gui.current_view().id",
                    lua_bootstrap(),
                    path,
                    path
                ));
            }
            "st.lynx.plugins.darktable.adjust" => {
                let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                let path = escape_lua_str(normalize_action_path(&settings.path));
                let needs_darkroom = path.is_empty() || path.starts_with("iop/");
                if event.payload.controller == "Encoder" {
                    // Encoder push resets current control.
                    log::info!("key_down encoder-reset path={} context={}", settings.path, event.context);
                    let _ = send_lua(&format!(
                        "{} {} darktable.gui.action('{}', '', 'reset'); return darktable.gui.current_view().id",
                        lua_bootstrap(),
                        if needs_darkroom { "ensure_darkroom();" } else { "" },
                        path
                    ));
                } else {
                    if settings.step == 0.0 {
                        log::info!("key_down keypad-reset path={} context={}", settings.path, event.context);
                        let _ = send_lua(&format!(
                            "{} {} darktable.gui.action('{}', '', 'reset'); return darktable.gui.current_view().id",
                            lua_bootstrap(),
                            if needs_darkroom { "ensure_darkroom();" } else { "" },
                            path
                        ));
                        return Ok(());
                    }
                    // Keypad press performs one adjustment step.
                    let effect = if settings.step >= 0.0 { "up" } else { "down" };
                    let speed = settings.step.abs().round().max(1.0) as i32;
                    log::info!(
                        "key_down keypad-adjust path={} effect={} speed={} context={}",
                        settings.path, effect, speed, event.context
                    );
                    let _ = send_lua(&format!(
                        "{} {} darktable.gui.action('{}', '', '{}', {}); return darktable.gui.current_view().id",
                        lua_bootstrap(),
                        if needs_darkroom { "ensure_darkroom();" } else { "" },
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
            let path = escape_lua_str(normalize_action_path(&settings.path));
            let needs_darkroom = path.is_empty() || path.starts_with("iop/");
            log::info!("dial_down reset path={} context={}", settings.path, event.context);
            let _ = send_lua(&format!(
                "{} {} darktable.gui.action('{}', '', 'reset'); return darktable.gui.current_view().id",
                lua_bootstrap(),
                if needs_darkroom { "ensure_darkroom();" } else { "" },
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
                let path = escape_lua_str(normalize_action_path(&settings.path));
                let needs_darkroom = path.is_empty() || path.starts_with("iop/");
                log::info!(
                    "dial_rotate path={} effect={} speed={} context={} ticks={}",
                    settings.path, effect, speed, event.context, event.payload.ticks
                );
                let _ = send_lua(&format!(
                    "{} {} darktable.gui.action('{}', '', '{}', {}); return darktable.gui.current_view().id",
                    lua_bootstrap(),
                    if needs_darkroom { "ensure_darkroom();" } else { "" },
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
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.trim().is_empty() {
                log::info!("darktable Lua output: {}", stdout.trim());
            }
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
