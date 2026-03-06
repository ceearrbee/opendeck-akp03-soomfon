use async_hid::{AsyncHidRead, DeviceReader};
use futures_lite::FutureExt;
use std::{iter::zip, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time};

use crate::{error::MirajazzError, types::DeviceInput};

#[derive(Copy, Clone, Debug, Hash)]
pub enum DeviceStateUpdate {
    ButtonDown(u8),
    ButtonUp(u8),
    EncoderDown(u8),
    EncoderUp(u8),
    EncoderTwist(u8, i8),
}

#[derive(Default)]
pub struct DeviceState {
    pub buttons: Vec<bool>,
    pub encoders: Vec<bool>,
}

pub struct DeviceStateReader {
    pub protocol_version: usize,
    pub reader: Arc<Mutex<DeviceReader>>,
    pub states: Mutex<DeviceState>,
    pub process_input: fn(u8, u8) -> Result<DeviceInput, MirajazzError>,
}

impl DeviceStateReader {
    pub fn supports_both_states(&self) -> bool {
        self.protocol_version > 2
    }

    pub async fn raw_read_data(&self, _length: usize) -> Result<Vec<u8>, MirajazzError> {
        let mut buf = vec![0u8; 1024];
        let size = self.reader.lock().await.read_input_report(&mut buf).await?;
        eprintln!("MIRAJAZZ: Read {} bytes", size);
        Ok(buf[..size].to_vec())
    }

    pub async fn raw_read_data_with_timeout(
        &self,
        _length: usize,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, MirajazzError> {
        let mut buf = vec![0u8; 1024];
        let size = self
            .reader
            .lock()
            .await
            .read_input_report(&mut buf)
            .or(async {
                time::sleep(timeout).await;
                Ok(0)
            })
            .await?;

        if size == 0 {
            return Ok(None);
        }
        eprintln!("MIRAJAZZ: Read {} bytes (timeout mode)", size);
        Ok(Some(buf[..size].to_vec()))
    }

    pub async fn read_input(
        &self,
        _timeout: Option<Duration>,
        process_input: fn(u8, u8) -> Result<DeviceInput, MirajazzError>,
    ) -> Result<DeviceInput, MirajazzError> {
        let data_opt = self.raw_read_data_with_timeout(512, Duration::from_millis(500)).await?;

        if data_opt.is_none() {
            return Ok(DeviceInput::NoData);
        }

        let data = data_opt.unwrap();
        eprintln!("MIRAJAZZ RAW: {:02X?}", &data[..data.len().min(16)]);

        let state = if self.supports_both_states() {
            if data.len() > 10 { data[10] } else { 0x01 }
        } else {
            0x1u8
        };

        let id = if data.len() > 9 { data[9] } else { 0x00 };

        Ok(process_input(id, state)?)
    }

    pub async fn read(
        &self,
        timeout: Option<Duration>,
    ) -> Result<Vec<DeviceStateUpdate>, MirajazzError> {
        let input = self.read_input(timeout, self.process_input).await?;
        Ok(self.input_to_updates(input).await)
    }

    async fn input_to_updates(&self, input: DeviceInput) -> Vec<DeviceStateUpdate> {
        let mut my_states = self.states.lock().await;
        let mut updates = vec![];

        match input {
            DeviceInput::ButtonStateChange(buttons) => {
                for (index, (their, mine)) in
                    zip(buttons.iter(), my_states.buttons.iter()).enumerate()
                {
                    if !self.supports_both_states() {
                        if *their {
                            updates.push(DeviceStateUpdate::ButtonDown(index as u8));
                            updates.push(DeviceStateUpdate::ButtonUp(index as u8));
                        }
                    } else if their != mine {
                        if *their {
                            updates.push(DeviceStateUpdate::ButtonDown(index as u8));
                        } else {
                            updates.push(DeviceStateUpdate::ButtonUp(index as u8));
                        }
                    }
                }
                my_states.buttons = buttons;
            }
            DeviceInput::EncoderStateChange(encoders) => {
                for (index, (their, mine)) in
                    zip(encoders.iter(), my_states.encoders.iter()).enumerate()
                {
                    if !self.supports_both_states() {
                        if *their {
                            updates.push(DeviceStateUpdate::EncoderDown(index as u8));
                            updates.push(DeviceStateUpdate::EncoderUp(index as u8));
                        }
                    } else if *their != *mine {
                        if *their {
                            updates.push(DeviceStateUpdate::EncoderDown(index as u8));
                        } else {
                            updates.push(DeviceStateUpdate::EncoderUp(index as u8));
                        }
                    }
                }
                my_states.encoders = encoders;
            }
            DeviceInput::EncoderTwist(twist) => {
                for (index, change) in twist.iter().enumerate() {
                    if *change != 0 {
                        updates.push(DeviceStateUpdate::EncoderTwist(index as u8, *change));
                    }
                }
            }
            DeviceInput::ButtonDown(key) => {
                if key < my_states.buttons.len() as u8 {
                    my_states.buttons[key as usize] = true;
                }
                updates.push(DeviceStateUpdate::ButtonDown(key));
            }
            DeviceInput::ButtonUp(key) => {
                if key < my_states.buttons.len() as u8 {
                    my_states.buttons[key as usize] = false;
                }
                updates.push(DeviceStateUpdate::ButtonUp(key));
            }
            DeviceInput::EncoderDown(encoder) => {
                if encoder < my_states.encoders.len() as u8 {
                    my_states.encoders[encoder as usize] = true;
                }
                updates.push(DeviceStateUpdate::EncoderDown(encoder));
            }
            DeviceInput::EncoderUp(encoder) => {
                if encoder < my_states.encoders.len() as u8 {
                    my_states.encoders[encoder as usize] = false;
                }
                updates.push(DeviceStateUpdate::EncoderUp(encoder));
            }
            DeviceInput::SingleEncoderTwist(encoder, val) => {
                updates.push(DeviceStateUpdate::EncoderTwist(encoder, val));
            }
            _ => {}
        }
        drop(my_states);
        updates
    }
}
