use std::ops::Deref;

use crossbeam_channel::Sender;
use dashmap::{
    mapref::{multiple::RefMulti, one::Ref},
    DashMap,
};
use paste::paste;
use zinput_device::{
    component::{
        analogs::Analogs, buttons::Buttons, controller::Controller, motion::Motion,
        touch_pad::TouchPad, ComponentData,
    },
    Device, DeviceInfo, DeviceMut,
};

use crate::util::Uuid;
use crate::event::Event;

pub struct Engine {
    device_info: DashMap<Uuid, DeviceInfo>,
    devices: DashMap<Uuid, Device>,

    event_channel: Sender<Event>,
}

impl Engine {
    pub fn new(event_channel: Sender<Event>) -> Self {
        Engine {
            device_info: DashMap::new(),
            devices: DashMap::new(),

            event_channel,
        }
    }

    pub fn new_device(&self, info: DeviceInfo) -> Uuid {
        self.new_device_internal(info)
    }

    pub fn remove_device(&self, id: &Uuid) {
        self.device_info.remove(id);
        self.devices.remove(id);
        match self.event_channel.send(Event::DeviceRemoved(*id)) {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    pub fn devices<'a>(&'a self) -> Devices<'a> {
        Devices(self.device_info.iter())
    }

    pub fn get_device_info<'a>(&'a self, id: &Uuid) -> Option<DeviceInfoRef<'a>> {
        self.device_info
            .get(id)
            .map(|r| DeviceInfoRef(DeviceInfoType::One(r)))
    }

    pub fn get_device<'a>(&'a self, id: &Uuid) -> Option<DeviceHandle<'a>> {
        self.devices.get(id).map(DeviceHandle)
    }

    pub fn update<F>(&self, id: &Uuid, updater: F) -> Result<(), ComponentUpdateError>
    where
        F: for<'a> FnOnce(DeviceMut<'a>),
    {
        let mut device = self
            .devices
            .get_mut(id)
            .ok_or(ComponentUpdateError::InvalidDeviceId)?;

        updater(device.as_mut());

        match self.event_channel.send(Event::DeviceUpdate(*id)) {
            Ok(()) => {}
            Err(_) => {}
        }

        Ok(())
    }
}

macro_rules! engine_components {
    ($($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            impl Engine {
                fn new_device_internal(&self, info: DeviceInfo) -> Uuid {
                    let id = Uuid::new_v4();

                    // TODO
                    let device = Device {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };

                    self.device_info.insert(id, info.clone());
                    self.devices.insert(id, device);
                    match self.event_channel.send(Event::DeviceAdded(id, info)) {
                        Ok(()) => {}
                        Err(_) => {}
                    }
                    id
                }
            }
        }
    };
}

engine_components!(
    controller: Controller,
    motion: Motion,
    analog: Analogs,
    button: Buttons,
    touch_pad: TouchPad,
);

pub struct Devices<'a>(dashmap::iter::Iter<'a, Uuid, DeviceInfo>);

impl<'a> Iterator for Devices<'a> {
    type Item = DeviceInfoRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|r| DeviceInfoRef(DeviceInfoType::Multi(r)))
    }
}

enum DeviceInfoType<'a> {
    One(Ref<'a, Uuid, DeviceInfo>),
    Multi(RefMulti<'a, Uuid, DeviceInfo>),
}

pub struct DeviceInfoRef<'a>(DeviceInfoType<'a>);

impl<'a> DeviceInfoRef<'a> {
    pub fn id(&self) -> &Uuid {
        match &self.0 {
            DeviceInfoType::One(r) => r.key(),
            DeviceInfoType::Multi(r) => r.key(),
        }
    }
}

impl<'a> Deref for DeviceInfoRef<'a> {
    type Target = DeviceInfo;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            DeviceInfoType::One(r) => r.deref(),
            DeviceInfoType::Multi(r) => r.deref(),
        }
    }
}

pub struct DeviceHandle<'a>(Ref<'a, Uuid, Device>);

impl<'a> Deref for DeviceHandle<'a> {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[derive(Debug)]
pub enum ComponentUpdateError {
    InvalidDeviceId,
    InvalidIndex,
}

impl std::error::Error for ComponentUpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for ComponentUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentUpdateError::InvalidDeviceId => write!(f, "invalid device id"),
            ComponentUpdateError::InvalidIndex => write!(f, "invalid index"),
        }
    }
}

/*
pub struct DeviceHandleMut<'a>(RefMut<'a, Uuid, Device>);

impl<'a> Deref for DeviceHandleMut<'a> {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a> DeviceHandleMut {
    pub fn as_mut(&mut self) -> DeviceMut {
        self.0.as_mut()
    }
}*/
