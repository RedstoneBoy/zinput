use std::sync::Arc;

use parking_lot::{RwLock, RwLockReadGuard};
use zinput_device::DeviceInfo;

mod device;

pub use self::device::{DeviceHandle, DeviceView};
use self::device::InternalDevice;

pub struct Engine {
    devices: RwLock<Vec<Arc<InternalDevice>>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            devices: Default::default(),
        }
    }

    pub fn new_device(&self, info: DeviceInfo) -> Result<DeviceHandle, DeviceAlreadyExists> {
        self.release_devices();

        let internal = InternalDevice::new(info);
        let handle = DeviceHandle::new(internal.clone())
            .ok_or(DeviceAlreadyExists)?;
        
        self.devices
            .write()
            .push(internal);
        
        Ok(handle)
    }

    pub fn devices(&self) -> Devices {
        self.release_devices();

        Devices { lock: self.devices.read() }
    }

    fn release_devices(&self) {
        let mut devices = self.devices.write();
        let mut i = 0;
        while i < devices.len() {
            if devices[i].should_remove() {
                devices.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceAlreadyExists;

impl std::fmt::Display for DeviceAlreadyExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "device already exists")
    }
}

impl std::error::Error for DeviceAlreadyExists {}

pub struct Devices<'a> {
    lock: RwLockReadGuard<'a, Vec<Arc<InternalDevice>>>,
}

impl<'a> Devices<'a> {
    pub fn get(&self, index: usize) -> Option<DeviceView> {
        self.lock.get(index)
            .map(|int| DeviceView::new(int.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item=&DeviceInfo> {
        let iter = self.lock
            .iter()
            .map(|int| int.info());
        
        iter
    }
}