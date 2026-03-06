use async_hid::{
    AsyncHidWrite, Device as HidDevice, DeviceId, DeviceInfo as HidDeviceInfo, DeviceReader,
    DeviceWriter, HidBackend,
};
use futures_lite::{Stream, StreamExt};
use image::DynamicImage;
use std::{
    collections::{HashMap, HashSet},
    convert::identity,
    str::{from_utf8, Utf8Error},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::Mutex;

use crate::{
    error::MirajazzError,
    images::convert_image_with_format,
    state::{DeviceState, DeviceStateReader},
    types::{DeviceInput, DeviceLifecycleEvent, ImageFormat},
};

pub fn new_hid_backend() -> HidBackend {
    HidBackend::default()
}

#[derive(Debug, Clone)]
pub struct DeviceQuery {
    usage_page: u16,
    usage_id: u16,
    vendor_id: u16,
    product_id: u16,
}

impl DeviceQuery {
    pub const fn new(usage_page: u16, usage_id: u16, vendor_id: u16, product_id: u16) -> Self {
        Self { usage_page, usage_id, vendor_id, product_id }
    }
}

fn check_device(device: HidDevice, queries: &[DeviceQuery]) -> Option<HidDevice> {
    if queries.is_empty() { return Some(device); }
    if !queries.iter().any(|query| {
        device.matches(query.usage_page, query.usage_id, query.vendor_id, query.product_id)
    }) { return None; }
    Some(device)
}

pub async fn list_devices(queries: &[DeviceQuery]) -> Result<HashSet<HidDevice>, MirajazzError> {
    let backend = HidBackend::default();
    let mut devices = backend.enumerate().await?;
    let mut matched = HashSet::new();
    while let Some(d) = devices.next().await { if let Some(matched_device) = check_device(d, queries) { matched.insert(matched_device); } }
    Ok(matched)
}

pub struct DeviceWatcher {
    initialized: bool,
    id_map: Arc<Mutex<HashMap<DeviceId, HidDeviceInfo>>>,
    connected: Arc<Mutex<HashSet<HidDeviceInfo>>>,
}

impl DeviceWatcher {
    pub fn new() -> Self {
        Self {
            initialized: false,
            id_map: Arc::new(Mutex::new(HashMap::new())),
            connected: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn watch<'a>(
        &'a mut self,
        queries: &'a [DeviceQuery],
    ) -> Result<impl Stream<Item = DeviceLifecycleEvent> + Send + Unpin + use<'a>, MirajazzError>
    {
        let backend = HidBackend::default();
        if self.initialized { return Err(MirajazzError::WatcherAlreadyInitialized); }
        self.initialized = true;
        let mut already_connected = backend.enumerate().await?;
        let mut map = self.id_map.lock().await;
        let mut connected = self.connected.lock().await;
        while let Some(device) = already_connected.next().await {
            if let Some(matched) = check_device(device, queries) {
                map.insert(matched.id.clone(), matched.clone());
                connected.insert(matched.clone());
            }
        }
        drop(map);
        drop(connected);
        let watcher = backend
            .watch()?
            .then(|e| async {
                match e {
                    async_hid::DeviceEvent::Connected(device_id) => {
                        let device = HidBackend::default().query_devices(&device_id).await.unwrap().into_iter().next()?;
                        let info = device.clone();
                        self.id_map.lock().await.insert(device_id, info.clone());
                        let new = self.connected.lock().await.insert(info.clone());
                        if new { Some(DeviceLifecycleEvent::Connected(info)) } else { None }
                    }
                    async_hid::DeviceEvent::Disconnected(device_id) => {
                        let info = self.id_map.lock().await.remove(&device_id)?;
                        let existed = self.connected.lock().await.remove(&info);
                        if existed { Some(DeviceLifecycleEvent::Disconnected(info)) } else { None }
                    }
                }
            })
            .filter_map(identity);
        Ok(Box::pin(watcher))
    }
}

pub fn extract_str(bytes: &[u8]) -> Result<String, Utf8Error> {
    Ok(from_utf8(bytes)?.replace('\0', "").to_string())
}

struct ImageCache {
    key: u8,
    image_data: Vec<u8>,
}

pub struct Device {
    pub vid: u16,
    pub pid: u16,
    pub serial_number: String,
    protocol_version: usize,
    key_count: usize,
    encoder_count: usize,
    packet_size: usize,
    reader: Arc<Mutex<DeviceReader>>,
    writer: Arc<Mutex<DeviceWriter>>,
    image_cache: Mutex<Vec<ImageCache>>,
    initialized: AtomicBool,
}

impl Device {
    pub async fn connect(
        dev: &HidDeviceInfo,
        _protocol_version: usize,
        key_count: usize,
        encoder_count: usize,
    ) -> Result<Device, MirajazzError> {
        let protocol_version = 3;
        let backend = HidBackend::default();
        let devices = backend.query_devices(&dev.id).await?;
        let device = devices.into_iter().next();
        let device = match device {
            Some(device) => device,
            None => return Err(MirajazzError::DeviceNotFoundError),
        };
        let serial_number = device.serial_number.clone().unwrap_or_else(|| "8730DB781721".to_string());
        let (reader, writer) = device.open().await?;
        Ok(Device {
            vid: device.vendor_id,
            pid: device.product_id,
            serial_number,
            protocol_version,
            key_count,
            encoder_count,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            packet_size: 1024,
            image_cache: Mutex::new(vec![]),
            initialized: false.into(),
        })
    }
}

impl Device {
    pub fn key_count(&self) -> usize { self.key_count }
    pub fn encoder_count(&self) -> usize { self.encoder_count }
    pub fn serial_number(&self) -> &String { &self.serial_number }

    async fn initialize(&self) -> Result<(), MirajazzError> {
        if self.initialized.load(Ordering::Acquire) { return Ok(()); }
        self.initialized.store(true, Ordering::Release);
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x44, 0x49, 0x53, 0x00, 0x00];
        self.write_extended_data(&mut buf).await?;
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x4c, 0x49, 0x47, 0x00, 0x00, 0x32];
        self.write_extended_data(&mut buf).await?;
        self.send_connect().await?;
        Ok(())
    }

    async fn send_connect(&self) -> Result<(), MirajazzError> {
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x43, 0x4F, 0x4E, 0x4E, 0x45, 0x43, 0x54, 0x00, 0x00];
        self.write_extended_data(&mut buf).await?;
        Ok(())
    }

    pub async fn reset(&self) -> Result<(), MirajazzError> {
        self.initialize().await?;
        self.set_brightness(100).await?;
        self.clear_all_button_images().await?;
        Ok(())
    }

    pub async fn set_brightness(&self, percent: u8) -> Result<(), MirajazzError> {
        self.initialize().await?;
        let percent = percent.clamp(0, 100);
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x4c, 0x49, 0x47, 0x00, 0x00, percent];
        self.write_extended_data(&mut buf).await?;
        Ok(())
    }

    async fn send_image(&self, key: u8, image_data: &[u8]) -> Result<(), MirajazzError> {
        let mut buf = vec![
            0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x42, 0x41, 0x54, 0x00, 0x00,
            (image_data.len() >> 8) as u8, image_data.len() as u8, key + 1,
        ];
        self.write_extended_data(&mut buf).await?;
        self.write_image_data_reports(image_data).await?;
        Ok(())
    }

    pub async fn write_image(&self, key: u8, image_data: &[u8]) -> Result<(), MirajazzError> {
        let cache_entry = ImageCache { key, image_data: image_data.to_vec() };
        self.image_cache.lock().await.push(cache_entry);
        Ok(())
    }

    pub async fn clear_button_image(&self, key: u8) -> Result<(), MirajazzError> {
        self.initialize().await?;
        let mut buf = vec![
            0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x43, 0x4c, 0x45, 0x00, 0x00, 0x00,
            if key == 0xff { 0xff } else { key + 1 },
        ];
        self.write_extended_data(&mut buf).await?;
        Ok(())
    }

    pub async fn clear_all_button_images(&self) -> Result<(), MirajazzError> {
        self.initialize().await?;
        self.clear_button_image(0xFF).await?;
        if self.protocol_version >= 2 {
            let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x53, 0x54, 0x50];
            self.write_extended_data(&mut buf).await?;
        }
        Ok(())
    }

    pub async fn set_button_image(&self, key: u8, image_format: ImageFormat, image: DynamicImage) -> Result<(), MirajazzError> {
        self.initialize().await?;
        let image_data = convert_image_with_format(image_format, image).await?;
        self.write_image(key, &image_data).await?;
        Ok(())
    }

    pub async fn sleep(&self) -> Result<(), MirajazzError> {
        self.initialize().await?;
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x48, 0x41, 0x4e];
        self.write_extended_data(&mut buf).await?;
        Ok(())
    }

    pub async fn keep_alive(&self) -> Result<(), MirajazzError> {
        self.initialize().await?;
        self.send_connect().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), MirajazzError> {
        self.initialize().await?;
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x43, 0x4c, 0x45, 0x00, 0x00, 0x44, 0x43];
        self.write_extended_data(&mut buf).await?;
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x48, 0x41, 0x4E];
        self.write_extended_data(&mut buf).await?;
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), MirajazzError> {
        let mut cache = self.image_cache.lock().await;
        self.initialize().await?;
        if cache.is_empty() { return Ok(()); }
        for image in cache.iter() { self.send_image(image.key, &image.image_data).await?; }
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x53, 0x54, 0x50];
        self.write_extended_data(&mut buf).await?;
        cache.clear();
        Ok(())
    }

    pub fn get_reader(&self, process_input: fn(u8, u8) -> Result<DeviceInput, MirajazzError>) -> Arc<DeviceStateReader> {
        Arc::new(DeviceStateReader {
            protocol_version: self.protocol_version,
            reader: self.reader.clone(),
            states: Mutex::new(DeviceState {
                buttons: vec![false; self.key_count],
                encoders: vec![false; self.encoder_count],
            }),
            process_input,
        })
    }

    async fn write_image_data_reports(&self, image_data: &[u8]) -> Result<(), MirajazzError> {
        let image_report_length = self.packet_size + 1;
        let image_report_payload_length = self.packet_size;
        let mut page_number = 0;
        let mut bytes_remaining = image_data.len();
        let mut buf: Vec<u8> = Vec::with_capacity(image_report_length);
        while bytes_remaining > 0 {
            let this_length = bytes_remaining.min(image_report_payload_length);
            let bytes_sent = page_number * image_report_payload_length;
            buf.clear();
            buf.push(0x00);
            buf.extend(&image_data[bytes_sent..bytes_sent + this_length]);
            buf.resize(image_report_length, 0);
            self.write_data(&buf).await?;
            bytes_remaining -= this_length;
            page_number += 1;
        }
        Ok(())
    }

    pub async fn write_data(&self, payload: &[u8]) -> Result<(), MirajazzError> {
        Ok(self.writer.lock().await.write_output_report(&payload).await?)
    }

    pub async fn write_extended_data(&self, payload: &mut Vec<u8>) -> Result<(), MirajazzError> {
        payload.resize(1 + self.packet_size, 0);
        self.write_data(payload).await
    }

    pub async fn set_mode(&self, mode: u8) -> Result<(), MirajazzError> {
        let mut buf = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x4D, 0x4F, 0x44, 0x00, 0x00, 0x30 + mode];
        self.write_extended_data(&mut buf).await
    }
}
