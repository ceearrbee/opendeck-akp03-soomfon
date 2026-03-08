use device::{handle_error, handle_set_image};
use mirajazz::device::Device;
use openaction::*;
use std::{collections::HashMap, process::exit, sync::LazyLock};
use tokio::sync::{Mutex, RwLock};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use watcher::watcher_task;
use crate::mappings::{COL_COUNT, DEVICE_NAMESPACE, ENCODER_COUNT, KEY_COUNT, ROW_COUNT};

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
        log::info!(
            "Hardware plugin preflight: namespace={} keys={} rows={} cols={} encoders={}",
            DEVICE_NAMESPACE, KEY_COUNT, ROW_COUNT, COL_COUNT, ENCODER_COUNT
        );
        midi::init().await;
        let tracker = TRACKER.lock().await.clone();
        let token = CancellationToken::new();
        let watcher_token = token.clone();
        tracker.spawn(async move {
            if let Err(err) = watcher_task(watcher_token).await {
                log::error!("Watcher task stopped with error: {}", err);
            }
        });
        TOKENS.write().await.insert("_watcher_task".to_string(), token);
        log::info!("Hardware watcher task started");
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
            if let Err(err) = handle_set_image(device, event).await {
                log::error!("set_image failed for device {}: {}", id, err);
                let _ = handle_error(&id, err).await;
            }
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
            if let Err(err) = device.set_brightness(event.brightness).await {
                log::error!(
                    "set_brightness failed for device {} brightness={}: {}",
                    id, event.brightness, err
                );
                let _ = handle_error(&id, err).await;
            }
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
        log::error!("Failed to initialize hardware plugin websocket session: {}", error);
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
