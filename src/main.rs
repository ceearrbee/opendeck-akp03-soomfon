use device::{handle_error, handle_set_image};
use mirajazz::device::Device;
use openaction::*;
use std::{collections::HashMap, process::exit, sync::LazyLock};
use tokio::sync::{Mutex, RwLock};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use watcher::watcher_task;

#[cfg(not(target_os = "windows"))]
use tokio::signal::unix::{SignalKind, signal};

mod device;
mod inputs;
mod mappings;
mod midi;
mod watcher;

pub static DEVICES: LazyLock<RwLock<HashMap<String, Device>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TOKENS: LazyLock<RwLock<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TRACKER: LazyLock<Mutex<TaskTracker>> = LazyLock::new(|| Mutex::new(TaskTracker::new()));

pub fn file_log(msg: &str) {
    log::info!("{}", msg);
}

struct GlobalEventHandler {}
impl openaction::GlobalEventHandler for GlobalEventHandler {
    async fn plugin_ready(
        &self,
        _outbound: &mut openaction::OutboundEventManager,
    ) -> EventHandlerResult {
        midi::init().await;
        let tracker = TRACKER.lock().await.clone();
        let token = CancellationToken::new();
        tracker.spawn(watcher_task(token.clone()));
        TOKENS.write().await.insert("_watcher_task".to_string(), token);
        log::info!("Plugin initialized");
        Ok(())
    }

    async fn set_image(
        &self,
        event: SetImageEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        if event.controller == Some("Encoder".to_string()) { return Ok(()); }
        let id = event.device.clone();
        if let Some(device) = DEVICES.read().await.get(&event.device) {
            handle_set_image(device, event)
                .await
                .map_err(async |err| handle_error(&id, err).await)
                .ok();
        }
        Ok(())
    }

    async fn set_brightness(
        &self,
        event: SetBrightnessEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        let id = event.device.clone();
        if let Some(device) = DEVICES.read().await.get(&event.device) {
            device.set_brightness(event.brightness).await
                .map_err(async |err| handle_error(&id, err).await)
                .ok();
        }
        Ok(())
    }
}

struct ActionEventHandler {}
impl openaction::ActionEventHandler for ActionEventHandler {}

async fn shutdown() {
    let tokens = TOKENS.write().await;
    for (_, token) in tokens.iter() { token.cancel(); }
}

async fn connect() {
    if let Err(error) = init_plugin(GlobalEventHandler {}, ActionEventHandler {}).await {
        log::error!("Failed to initialize plugin: {}", error);
        exit(1);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn sigterm() -> Result<(), Box<dyn std::error::Error>> {
    let mut sig = signal(SignalKind::terminate())?;
    sig.recv().await;
    Ok(())
}

#[cfg(target_os = "windows")]
async fn sigterm() -> Result<(), Box<dyn std::error::Error>> {
    std::future::pending::<()>().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stdout,
        simplelog::ColorChoice::Never,
    ).ok();

    tokio::select! {
        _ = connect() => {},
        _ = sigterm() => {},
    }
    shutdown().await;
    let tracker = TRACKER.lock().await.clone();
    tracker.close();
    tracker.wait().await;
    Ok(())
}
