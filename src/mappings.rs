use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "s6";

pub const ROW_COUNT: usize = 3;
pub const COL_COUNT: usize = 3;
pub const KEY_COUNT: usize = 9;
pub const ENCODER_COUNT: usize = 3;

#[derive(Debug, Clone)]
pub enum Kind {
    SoomfonSE,
}

pub const SOOMFON_VID: u16 = 0x1500;
pub const SOOMFON_SE_PID: u16 = 0x3001;

pub const SOOMFON_SE_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, SOOMFON_VID, SOOMFON_SE_PID);

pub const QUERIES: [DeviceQuery; 1] = [SOOMFON_SE_QUERY];

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        if vid == SOOMFON_VID && pid == SOOMFON_SE_PID {
            Some(Kind::SoomfonSE)
        } else {
            None
        }
    }

    /// There is no point relying on manufacturer/device names reported by the USB stack,
    /// so we return custom names for all the kinds of devices
    pub fn human_name(&self) -> String {
        match &self {
            Self::SoomfonSE => "Soomfon Stream Controller SE",
        }
        .to_string()
    }

    /// Returns protocol version for device
    pub fn protocol_version(&self) -> usize {
        3
    }

    #[allow(dead_code)]
    pub fn image_format(&self) -> ImageFormat {
        ImageFormat {
            mode: ImageMode::JPEG,
            size: (60, 60),
            rotation: ImageRotation::Rot90,
            mirror: ImageMirroring::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub kind: Kind,
}
