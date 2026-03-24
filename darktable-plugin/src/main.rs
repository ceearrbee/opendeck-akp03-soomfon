use openaction::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
struct DarktablePlugin {
    toggle_states: Arc<RwLock<HashMap<String, u16>>>,
}

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

#[derive(Debug)]
enum LuaDispatchError {
    Spawn(String),
    Timeout,
    ServiceUnavailable(String),
    LuaError(String),
    Failed(i32, String),
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

fn lua_adjust_call(path: &str, effect: &str, speed: i32) -> String {
    if path.is_empty() {
        format!(
            "local ok = pcall(function() return darktable.gui.action('', '{}', {}) end); \
             if not ok then darktable.gui.action('', '', '{}', {}); end;",
            effect, speed, effect, speed
        )
    } else {
        format!(
            "darktable.gui.action('{}', '', '{}', {});",
            path, effect, speed
        )
    }
}

fn lua_reset_call(path: &str) -> String {
    if path.is_empty() {
        "local ok = pcall(function() return darktable.gui.action('', 'reset', 1) end); \
         if not ok then darktable.gui.action('', '', 'reset'); end;"
            .to_string()
    } else {
        format!("darktable.gui.action('{}', '', 'reset');", path)
    }
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

fn run_host_check(args: &[&str]) -> Result<(), String> {
    let out = Command::new("flatpak-spawn")
        .args(args)
        .output()
        .map_err(|e| format!("spawn error: {e}"))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn send_host_scroll(effect: &str, speed: i32) -> Result<&'static str, String> {
    let button = if effect == "up" { "4" } else { "5" };
    let mut errs = Vec::new();

    for tool in ["xdotool", "ydotool"] {
        let mut ok = true;
        for _ in 0..speed.max(1) {
            let out = Command::new("flatpak-spawn")
                .args(["--host", tool, "click", button])
                .output()
                .map_err(|e| e.to_string());
            match out {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    ok = false;
                    errs.push(format!(
                        "{tool}: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                    break;
                }
                Err(err) => {
                    ok = false;
                    errs.push(format!("{tool}: {err}"));
                    break;
                }
            }
        }
        if ok {
            return Ok(tool);
        }
    }

    Err(errs.join(" | "))
}

fn send_lua(cmd: &str) -> Result<String, LuaDispatchError> {
    let out = Command::new("flatpak-spawn")
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
        .output()
        .map_err(|e| LuaDispatchError::Spawn(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();

    if out.status.success() {
        Ok(stdout)
    } else if stderr.contains("Timeout was reached") {
        Err(LuaDispatchError::Timeout)
    } else if stderr.contains("ServiceUnknown")
        || stderr.contains("The name is not activatable")
        || stderr.contains("org.darktable.service")
    {
        Err(LuaDispatchError::ServiceUnavailable(stderr))
    } else if stderr.contains("LuaError") {
        Err(LuaDispatchError::LuaError(stderr))
    } else {
        Err(LuaDispatchError::Failed(out.status.code().unwrap_or(-1), stderr))
    }
}

impl DarktablePlugin {
    async fn init_toggle_state(&self, context: &str, state: u16) {
        let mut states = self.toggle_states.write().await;
        states.insert(context.to_string(), state.min(1));
    }

    async fn flip_toggle_state(&self, context: &str, fallback: u16) -> u16 {
        let mut states = self.toggle_states.write().await;
        let current = *states.entry(context.to_string()).or_insert(fallback.min(1));
        let next = if current == 0 { 1 } else { 0 };
        states.insert(context.to_string(), next);
        next
    }

    async fn dispatch_lua(&self, label: &str, context: &str, cmd: String) -> bool {
        match send_lua(&cmd) {
            Ok(stdout) => {
                if stdout.is_empty() {
                    log::info!("{label} OK context={context}");
                } else {
                    log::info!("{label} OK context={context} output={stdout}");
                }
                true
            }
            Err(LuaDispatchError::Timeout) => {
                log::error!("{label} timeout context={context}");
                false
            }
            Err(LuaDispatchError::ServiceUnavailable(err)) => {
                log::error!(
                    "{label} service-unavailable context={context}: {err}. Start darktable Flatpak first: flatpak run org.darktable.Darktable"
                );
                false
            }
            Err(LuaDispatchError::LuaError(err)) => {
                log::error!("{label} lua-error context={context}: {err}");
                false
            }
            Err(LuaDispatchError::Spawn(err)) => {
                log::error!(
                    "{label} spawn-error context={context}: {err}. Verify flatpak-spawn exists in OpenDeck runtime"
                );
                false
            }
            Err(LuaDispatchError::Failed(code, err)) => {
                log::error!("{label} failed context={context} code={code}: {err}");
                false
            }
        }
    }
}

impl ActionEventHandler for DarktablePlugin {
    async fn key_down(&self, event: KeyEvent, outbound: &mut OutboundEventManager) -> EventHandlerResult {
        match event.action.as_str() {
            "st.lynx.plugins.darktable.toggle" => {
                let settings: ToggleSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                let normalized = normalize_toggle_path(&settings.path);
                let path = escape_lua_str(normalized);
                let lua = format!(
                    "{} \
                     if string.sub('{}', 1, 15) == 'views/darkroom/' then ensure_darkroom() end; \
                     if string.find('{}', 'global/switch views/', 1, true) == 1 \
                        or string.find('{}', 'cycle overlay colors', 1, true) ~= nil then \
                       darktable.gui.action('{}'); \
                     else \
                       darktable.gui.action('{}', '', 'toggle', 1); \
                     end; \
                     return darktable.gui.current_view().id",
                    lua_bootstrap(),
                    path,
                    path,
                    path,
                    path,
                    path
                );

                log::info!(
                    "key_down toggle path={} normalized={} context={}",
                    settings.path,
                    normalized,
                    event.context
                );

                if self.dispatch_lua("toggle", &event.context, lua).await {
                    let next_state = self.flip_toggle_state(&event.context, event.payload.state).await;
                    if let Err(err) = outbound.set_state(event.context.clone(), next_state).await {
                        log::error!("set_state failed context={} state={next_state}: {err}", event.context);
                    }
                }
            }
            "st.lynx.plugins.darktable.adjust" => {
                let settings: AdjustSettings = serde_json::from_value(event.payload.settings).unwrap_or_default();
                let path = escape_lua_str(normalize_action_path(&settings.path));
                let needs_darkroom = path.is_empty() || path.starts_with("iop/");
                if event.payload.controller == "Encoder" {
                    log::info!("key_down encoder-reset path={} context={}", settings.path, event.context);
                    let reset = lua_reset_call(&path);
                    let lua = format!(
                        "{} {} {} return darktable.gui.current_view().id",
                        lua_bootstrap(),
                        if needs_darkroom { "ensure_darkroom();" } else { "" },
                        reset
                    );
                    let _ = self.dispatch_lua("adjust-reset", &event.context, lua).await;
                } else {
                    if settings.step == 0.0 {
                        log::info!("key_down keypad-reset path={} context={}", settings.path, event.context);
                        let reset = lua_reset_call(&path);
                        let lua = format!(
                            "{} {} {} return darktable.gui.current_view().id",
                            lua_bootstrap(),
                            if needs_darkroom { "ensure_darkroom();" } else { "" },
                            reset
                        );
                        let _ = self.dispatch_lua("adjust-reset", &event.context, lua).await;
                        return Ok(());
                    }

                    let effect = if settings.step >= 0.0 { "up" } else { "down" };
                    let speed = settings.step.abs().round().max(1.0) as i32;
                    let adjust = lua_adjust_call(&path, effect, speed);
                    log::info!(
                        "key_down keypad-adjust path={} effect={} speed={} context={}",
                        settings.path,
                        effect,
                        speed,
                        event.context
                    );
                    let lua = format!(
                        "{} {} {} return darktable.gui.current_view().id",
                        lua_bootstrap(),
                        if needs_darkroom { "ensure_darkroom();" } else { "" },
                        adjust
                    );
                    let _ = self.dispatch_lua("adjust-step", &event.context, lua).await;
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
            let reset = lua_reset_call(&path);
            log::info!("dial_down reset path={} context={}", settings.path, event.context);
            let lua = format!(
                "{} {} {} return darktable.gui.current_view().id",
                lua_bootstrap(),
                if needs_darkroom { "ensure_darkroom();" } else { "" },
                reset
            );
            let _ = self.dispatch_lua("dial-reset", &event.context, lua).await;
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
                if path.is_empty() {
                    match send_host_scroll(effect, speed) {
                        Ok(tool) => {
                            log::info!(
                                "dial_rotate selected-scroll tool={} effect={} speed={} context={} ticks={}",
                                tool,
                                effect,
                                speed,
                                event.context,
                                event.payload.ticks
                            );
                        }
                        Err(err) => {
                            log::error!(
                                "dial_rotate selected-scroll failed effect={} speed={} context={}: {}",
                                effect,
                                speed,
                                event.context,
                                err
                            );
                        }
                    }
                    return Ok(());
                }
                let adjust = lua_adjust_call(&path, effect, speed);
                log::info!(
                    "dial_rotate path={} effect={} speed={} context={} ticks={}",
                    settings.path,
                    effect,
                    speed,
                    event.context,
                    event.payload.ticks
                );
                let lua = format!(
                    "{} {} {} return darktable.gui.current_view().id",
                    lua_bootstrap(),
                    if needs_darkroom { "ensure_darkroom();" } else { "" },
                    adjust
                );
                let _ = self.dispatch_lua("dial-rotate", &event.context, lua).await;
            }
        }
        Ok(())
    }

    async fn will_appear(&self, event: AppearEvent, outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if event.action == "st.lynx.plugins.darktable.toggle" {
            let state = event.payload.state.min(1);
            self.init_toggle_state(&event.context, state).await;
            if let Err(err) = outbound.set_state(event.context.clone(), state).await {
                log::error!("will_appear set_state failed context={} state={state}: {err}", event.context);
            }
        }
        Ok(())
    }
}

impl GlobalEventHandler for DarktablePlugin {
    async fn plugin_ready(&self, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        log::info!("Darktable Plugin ready");

        if let Err(err) = run_host_check(&["--host", "which", "gdbus"]) {
            log::error!(
                "Host dependency missing: gdbus. Ensure it is installed on host. details={err}"
            );
        }

        match send_lua("return 'ready'") {
            Ok(_) => log::info!("Darktable DBus bridge is reachable"),
            Err(LuaDispatchError::ServiceUnavailable(err)) => {
                log::warn!(
                    "Darktable service is unavailable ({err}). Start darktable Flatpak first: flatpak run org.darktable.Darktable"
                );
            }
            Err(other) => {
                log::warn!("Darktable preflight check failed: {other:?}");
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    simplelog::TermLogger::init(
        log::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .ok();

    log::info!("Darktable Plugin starting...");

    let handler = DarktablePlugin::default();
    init_plugin(handler.clone(), handler).await
}
