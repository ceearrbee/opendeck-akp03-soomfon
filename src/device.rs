use image::load_from_memory_with_format;
use mirajazz::{device::Device, error::MirajazzError};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS,
    mappings::{COL_COUNT, CandidateDevice, ENCODER_COUNT, KEY_COUNT, ROW_COUNT},
};

pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    crate::file_log(&format!("Starting device task for {}", candidate.id));

    let device_res = connect(&candidate).await;

    let device: Device = match device_res {
        Ok(device) => {
            crate::file_log("Connected successfully");
            device
        },
        Err(err) => {
            crate::file_log(&format!("Connection error: {:?}", err));
            handle_error(&candidate.id, err).await;
            return;
        }
    };

    crate::file_log("Initializing device...");
    if let Err(e) = device.set_brightness(50).await {
        crate::file_log(&format!("Init error (brightness): {:?}", e));
    }
    let _ = device.clear_all_button_images().await;
    let _ = device.flush().await;
    crate::file_log("Device initialized");

    crate::file_log(&format!("Registering device {} with OpenDeck", candidate.id));
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let res_legacy = outbound
            .register_device(
                candidate.id.clone(),
                candidate.kind.human_name(),
                ROW_COUNT as u8,
                COL_COUNT as u8,
                ENCODER_COUNT as u8,
                7, // Stream Deck+ class in OpenDeck
            )
            .await;

        if let Err(e) = &res_legacy {
            crate::file_log(&format!("Legacy registration failed: {:?}", e));
        } else {
            crate::file_log("Legacy registration sent");
        }

        let res = outbound
            .send_event(RegisterDeviceEvent {
                event: "registerDevice",
                payload: RegisterDevicePayload {
                    id: candidate.id.clone(),
                    name: candidate.kind.human_name(),
                    rows: ROW_COUNT as u8,
                    columns: COL_COUNT as u8,
                    encoders: ENCODER_COUNT as u8,
                    touchpoints: 0,
                    r#type: 7, // Stream Deck+ class in OpenDeck
                },
            })
            .await;

        
        match res {
            Ok(_) => crate::file_log("Extended registration sent"),
            Err(e) => crate::file_log(&format!("Extended registration failed: {:?}", e)),
        }
    }

    DEVICES.write().await.insert(candidate.id.clone(), device);

    let _ = device_events_task(&candidate, token).await;

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        let _ = device.shutdown().await;
    }
}

#[derive(Serialize)]
struct RegisterDeviceEvent {
    event: &'static str,
    payload: RegisterDevicePayload,
}

#[derive(Serialize)]
struct RegisterDevicePayload {
    id: String,
    name: String,
    rows: u8,
    columns: u8,
    encoders: u8,
    touchpoints: u8,
    r#type: u8,
}

pub async fn handle_error(id: &String, err: MirajazzError) -> bool {
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.deregister_device(id.clone()).await;
    }
    if let Some(token) = TOKENS.read().await.get(id) {
        token.cancel();
    }
    DEVICES.write().await.remove(id);
    false
}

pub async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    Device::connect(
        &candidate.dev,
        3,
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await
}

async fn device_events_task(candidate: &CandidateDevice, token: CancellationToken) -> Result<(), MirajazzError> {
    crate::file_log("Starting event loop...");
    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(crate::inputs::process_input),
        None => return Ok(()),
    };
    drop(devices_lock);

    let mut keep_alive_ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        let updates = tokio::select! {
            _ = token.cancelled() => {
                crate::file_log("Token cancelled");
                break;
            }
            _ = keep_alive_ticker.tick() => {
                if let Some(device) = DEVICES.read().await.get(&candidate.id) {
                    let _ = device.keep_alive().await;
                }
                continue;
            }
            res = reader.read(None) => {
                match res {
                    Ok(updates) => updates,
                    Err(e) => {
                        crate::file_log(&format!("Reader error: {:?}", e));
                        if !handle_error(&candidate.id, e).await { break; }
                        continue;
                    }
                }
            }
        };

        for update in updates {
            crate::file_log(&format!("UPDATE: {:?}", update));
            let id = candidate.id.clone();
            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                use mirajazz::state::DeviceStateUpdate;
                match update {
                    DeviceStateUpdate::ButtonDown(key) => { let _ = outbound.key_down(id, key).await; }
                    DeviceStateUpdate::ButtonUp(key) => { let _ = outbound.key_up(id, key).await; }
                    DeviceStateUpdate::EncoderDown(encoder) => { let _ = outbound.encoder_down(id, encoder).await; }
                    DeviceStateUpdate::EncoderUp(encoder) => { let _ = outbound.encoder_up(id, encoder).await; }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        let ticks = val as i16;
                        // Super-payload to cover all bases for rotation
                        let _ = outbound.send_event(serde_json::json!({
                            "event": "dialRotate",
                            "payload": {
                                "device": id,
                                "encoder": encoder,
                                "position": encoder,
                                "column": encoder,
                                "row": 0,
                                "ticks": ticks,
                                "value": ticks,
                                "delta": ticks,
                                "pressed": false
                            }
                        })).await;

                        // Also send encoderChange (OpenAction default) with multiple field names
                        let _ = outbound.send_event(serde_json::json!({
                            "event": "encoderChange",
                            "payload": {
                                "device": id.clone(),
                                "position": encoder,
                                "ticks": ticks,
                                "value": ticks,
                                "delta": ticks
                            }
                        })).await;
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    if evt.image.is_none() {
        if let Some(key) = evt.position { device.clear_button_image(key as u8).await?; }
        else { device.clear_all_button_images().await?; }
    } else {
        let image = evt.image.unwrap();
        let key = evt.position.unwrap();
        let data = data_url::DataUrl::process(&image).unwrap();
        let (body, _) = data.decode_to_vec().unwrap();
        let img = load_from_memory_with_format(&body, image::ImageFormat::Jpeg).unwrap();
        device.set_button_image(key as u8, mirajazz::types::ImageFormat {
            mode: mirajazz::types::ImageMode::JPEG,
            size: (60, 60),
            rotation: mirajazz::types::ImageRotation::Rot90,
            mirror: mirajazz::types::ImageMirroring::None,
        }, img).await?;
    }
    device.flush().await?;
    Ok(())
}
