use image::load_from_memory_with_format;
use mirajazz::{device::Device, error::MirajazzError};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS,
    mappings::{COL_COUNT, CandidateDevice, ENCODER_COUNT, KEY_COUNT, ROW_COUNT},
};

pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    log::info!("Running device task for {:?}", candidate);

    let device = async || -> Result<Device, MirajazzError> {
        let device = connect(&candidate).await?;
        device.set_brightness(50).await?;
        device.clear_all_button_images().await?;
        device.flush().await?;
        Ok(device)
    }().await;

    let device: Device = match device {
        Ok(device) => device,
        Err(err) => {
            log::error!("Failed to initialize device {}: {}", candidate.id, err);
            handle_error(&candidate.id, err).await;
            return;
        }
    };

    log::info!("Registering device {} with OpenDeck", candidate.id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let res = outbound
            .register_device(
                candidate.id.clone(),
                candidate.kind.human_name(),
                ROW_COUNT as u8,
                COL_COUNT as u8,
                ENCODER_COUNT as u8,
                7, // Type 7 = Stream Deck+ (Enables Encoders in UI)
            )
            .await;

        match res {
            Ok(_) => {
                crate::file_log("REGISTRATION SUCCESS (Type 7)");
                log::info!("Registration successful (Type 7)");
            }
            Err(e) => {
                crate::file_log(&format!("REGISTRATION FAILED: {:?}", e));
                log::error!("Registration failed: {:?}", e);
            }
        }
    }

    DEVICES.write().await.insert(candidate.id.clone(), device);

    let _ = device_events_task(&candidate, token).await;

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        let _ = device.shutdown().await;
    }
}

pub async fn handle_error(id: &String, err: MirajazzError) -> bool {
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        log::debug!("Recoverable error for {}: {}", id, err);
        return true;
    }
    log::error!("Fatal device error for {}: {}", id, err);
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
        candidate.kind.protocol_version(),
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await
}

async fn device_events_task(candidate: &CandidateDevice, token: CancellationToken) -> Result<(), MirajazzError> {
    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(crate::inputs::process_input),
        None => return Ok(()),
    };
    drop(devices_lock);

    loop {
        let updates = tokio::select! {
            _ = token.cancelled() => break,
            res = reader.read(None) => match res {
                Ok(updates) => updates,
                Err(e) => {
                    if !handle_error(&candidate.id, e).await { break; }
                    continue;
                }
            }
        };

        for update in updates {
            log::debug!("Device update: {:?}", update);
            let id = candidate.id.clone();
            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                use mirajazz::state::DeviceStateUpdate;
                match update {
                    DeviceStateUpdate::ButtonDown(key) => {
                        let _ = outbound.key_down(id, key).await;
                        crate::midi::button_down(key).await;
                    }
                    DeviceStateUpdate::ButtonUp(key) => {
                        let _ = outbound.key_up(id, key).await;
                        crate::midi::button_up(key).await;
                    }
                    DeviceStateUpdate::EncoderDown(encoder) => {
                        let _ = outbound.encoder_down(id, encoder).await;
                        crate::midi::encoder_down(encoder).await;
                    }
                    DeviceStateUpdate::EncoderUp(encoder) => {
                        let _ = outbound.encoder_up(id, encoder).await;
                        crate::midi::encoder_up(encoder).await;
                    }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        let _ = outbound.encoder_change(id, encoder, val as i16).await;
                        crate::midi::encoder_twist(encoder, val as i16).await;
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
        let image = evt.image.ok_or(MirajazzError::BadData)?;
        let key = evt.position.ok_or(MirajazzError::BadData)?;
        let data = data_url::DataUrl::process(&image).map_err(|_| MirajazzError::BadData)?;
        let (body, _) = data.decode_to_vec().map_err(|_| MirajazzError::BadData)?;
        let img = load_from_memory_with_format(&body, image::ImageFormat::Jpeg)
            .map_err(|_| MirajazzError::BadData)?;
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
