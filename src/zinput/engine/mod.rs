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
        touch_pad::TouchPad, Component, ComponentData,
    },
    device::DeviceInfo,
    Event,
    InvalidComponentIdError,
};

macro_rules! engine_struct {
    ($struct_name:ident ; $($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            pub struct $struct_name {
                devices: DashMap<Uuid, DeviceInfo>,
                $([< $field_name s >]: DashMap<Uuid, Component<$ctype>>,)*

                event_channel: Sender<Event>,
            }

            impl Engine {
                pub fn new(event_channel: Sender<Event>) -> Self {
                    Engine {
                        devices: DashMap::new(),
                        $([< $field_name s >]: DashMap::new(),)*

                        event_channel,
                    }
                }

                $(pub fn [< has_ $field_name >](&self, id: &Uuid) -> bool {
                    self.[< $field_name s >].contains_key(id)
                })*

                $(pub fn [< get_ $field_name >](&self, id: &Uuid) -> Option<Ref<Uuid, Component<$ctype>>> {
                    self.[< $field_name s >].get(id)
                })*
            }

            impl Engine {
                pub fn new_device(&self, info: DeviceInfo) -> Uuid {
                    let id = Uuid::new_v4();
                    self.devices.insert(id, info.clone());
                    match self.event_channel.send(Event::DeviceAdded(id, info)) {
                        Ok(()) => {},
                        Err(_) => {}
                    }
                    id
                }

                pub fn remove_device(&self, id: &Uuid) {
                    self.devices.remove(id);
                    match self.event_channel.send(Event::DeviceRemoved(*id)) {
                        Ok(()) => {},
                        Err(_) => {}
                    }
                }

                $(pub fn [< new_ $field_name >](&self, info: <$ctype as ComponentData>::Info) -> Uuid {
                    let id = Uuid::new_v4();
                    self.[< $field_name s >].insert(id, Component::new(info));
                    id
                })*

                $(pub fn [< update_ $field_name >](&self, id: &Uuid, data: &$ctype) -> Result<(), InvalidComponentIdError> {
                    let mut component = self.[< $field_name s >].get_mut(id).ok_or(InvalidComponentIdError)?;

                    component.data.update(data);

                    match self.event_channel.send(Event::ComponentUpdate(*id)) {
                        Ok(()) => {}
                        Err(_) => {}
                    }

                    Ok(())
                })*

                $(pub fn [< remove_ $field_name >](&self, id: &Uuid) {
                    self.[< $field_name s >].remove(id);
                })*
            }
        }
    };
}

engine_struct!(Engine;
    controller: Controller,
    motion: Motion,

    analog: Analogs,
    button: Buttons,
    touch_pad: TouchPad,
);

impl Engine {
    pub fn devices(&self) -> impl Iterator<Item = RefMulti<Uuid, DeviceInfo>> {
        self.devices.iter()
    }
    /*
        pub fn controllers(&self) -> impl Iterator<Item = RefMulti<Uuid, Component<Controller>>> {
            self.controllers.iter()
        }

        pub fn motions(&self) -> impl Iterator<Item = RefMulti<Uuid, Component<Motion>>> {
            self.motions.iter()
        }

        pub fn touch_pads(&self) -> impl Iterator<Item = RefMulti<Uuid, Component<TouchPad>>> {
            self.touch_pads.iter()
        }
    */
    pub fn has_device(&self, id: &Uuid) -> bool {
        self.devices.contains_key(id)
    }

    pub fn get_device(&self, id: &Uuid) -> Option<Ref<Uuid, DeviceInfo>> {
        self.devices.get(id)
    }
}
