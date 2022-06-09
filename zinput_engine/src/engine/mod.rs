use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;
use zinput_device::DeviceInfo;

mod device;

pub use self::device::{DeviceHandle, DeviceView};
use self::device::InternalDevice;

pub struct Engine {
    devices: DashMap<Uuid, Arc<InternalDevice>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            devices: Default::default(),
        }
    }

    pub fn new_device(&self, info: DeviceInfo) -> Result<DeviceHandle, DeviceAlreadyExists> {
        self.release_devices();

        let id = Uuid::new_v4();
        let internal = InternalDevice::new(info, id);
        let handle = DeviceHandle::new(internal.clone())
            .ok_or(DeviceAlreadyExists)?;
        
        self.devices.insert(id, internal);
        
        Ok(handle)
    }

    pub fn devices(&self) -> Devices {
        self.release_devices();

        Devices { iter: self.devices.iter() }
    }

    pub fn get_device(&self, uuid: &Uuid) -> Option<DeviceView> {
        self.release_devices();
        
        self.devices
            .get(uuid)
            .map(|int| DeviceView::new(int.value().clone()))
    }

    fn release_devices(&self) {
        self.devices.retain(|_, int| !int.should_remove());
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
    iter: dashmap::iter::Iter<'a, Uuid, Arc<InternalDevice>>,
}

impl<'a> Iterator for Devices<'a> {
    type Item = DeviceEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.iter.next()?;
        Some(DeviceEntry { entry })
    }
}

pub struct DeviceEntry<'a> {
    entry: dashmap::mapref::multiple::RefMulti<'a, Uuid, Arc<InternalDevice>>,
}

impl<'a> DeviceEntry<'a> {
    pub fn uuid(&self) -> &Uuid {
        self.entry.key()
    }

    pub fn info(&self) -> &DeviceInfo {
        self.entry.value().info()
    }
}