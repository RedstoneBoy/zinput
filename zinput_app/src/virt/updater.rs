use zinput_engine::{
    device::{component::ComponentKind, DeviceInfo},
    DeviceAlreadyExists, DeviceHandle, DeviceView, Engine,
};

pub trait Updater: Send + 'static {
    fn verify(&self, info: &[DeviceView]) -> Result<(), VerificationError>;
    fn create_output(&self, engine: &Engine) -> Result<DeviceHandle, DeviceAlreadyExists>;
    fn update(&self, view: &DeviceView, view_index: usize, out: &DeviceHandle);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationError {
    InvalidDeviceAmount {
        need: usize,
        got: usize,
    },
    InvalidComponentAmount {
        device_index: usize,
        kind: ComponentKind,
        need: usize,
        got: usize,
    },
}

impl std::error::Error for VerificationError {}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::InvalidDeviceAmount { need, got } => write!(f, "invalid number of devices (need {need}, got {got})"),
            VerificationError::InvalidComponentAmount {
                device_index,
                kind,
                need,
                got,
            } => write!(f, "invalid number of {kind} components for device index {device_index} (need {need}, got {got})"),
        }
    }
}