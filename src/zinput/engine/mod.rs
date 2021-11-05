use std::ops::Deref;

use crossbeam_channel::Sender;
use dashmap::{
    mapref::{multiple::RefMulti, one::Ref},
    DashMap,
};
use paste::paste;
use uuid::Uuid;

use crate::api::{
    component::{
        analogs::Analogs, buttons::Buttons, controller::Controller, motion::Motion,
        touch_pad::TouchPad, ComponentData,
    },
    device::{Device, DeviceInfo},
    ComponentUpdateError, Event,
};

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

    pub fn get_device<'a>(&'a self, id: &Uuid) -> Option<DeviceRef<'a>> {
        self.devices.get(id).map(DeviceRef)
    }
}

macro_rules! engine_components {
    ($($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            impl Engine {
                pub fn new_device(&self, info: DeviceInfo) -> Uuid {
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

                $(pub fn [< update_ $field_name >](&self, device_id: &Uuid, index: usize, data: &$ctype) -> Result<(), ComponentUpdateError> {
                    let mut device = self.devices.get_mut(device_id)
                        .ok_or(ComponentUpdateError::InvalidDeviceId)?;

                    let component = device
                        .value_mut()
                        .[< $field_name s >]
                        .get_mut(index)
                        .ok_or(ComponentUpdateError::InvalidIndex)?;

                    component.update(data);
                    
                    match self.event_channel.send(Event::DeviceUpdate(*device_id)) {
                        Ok(()) => {}
                        Err(_) => {}
                    }

                    Ok(())
                })*
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

pub struct DeviceRef<'a>(Ref<'a, Uuid, Device>);

impl<'a> Deref for DeviceRef<'a> {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
