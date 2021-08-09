use crossbeam_channel::{Receiver, Sender};
use dashmap::{
    mapref::{multiple::RefMulti, one::Ref},
    DashMap,
};
use uuid::Uuid;

use crate::api::{
    component::{
        controller::{Controller, ControllerInfo},
        motion::{Motion, MotionInfo},
        Component, ComponentData,
    },
    device::DeviceInfo,
    InvalidComponentIdError, ZInputApi,
};

pub struct Engine {
    devices: DashMap<Uuid, DeviceInfo>,
    controllers: DashMap<Uuid, Component<Controller>>,
    motions: DashMap<Uuid, Component<Motion>>,
    updates: DashMap<Uuid, Vec<Sender<()>>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            devices: DashMap::new(),
            controllers: DashMap::new(),
            motions: DashMap::new(),
            updates: DashMap::new(),
        }
    }

    pub fn devices(&self) -> impl Iterator<Item = RefMulti<Uuid, DeviceInfo>> {
        self.devices.iter()
    }

    pub fn controllers(&self) -> impl Iterator<Item = RefMulti<Uuid, Component<Controller>>> {
        self.controllers.iter()
    }

    pub fn motions(&self) -> impl Iterator<Item = RefMulti<Uuid, Component<Motion>>> {
        self.motions.iter()
    }

    pub fn has_device(&self, id: &Uuid) -> bool {
        self.devices.contains_key(id)
    }

    pub fn has_controller(&self, id: &Uuid) -> bool {
        self.controllers.contains_key(id)
    }

    pub fn has_motion(&self, id: &Uuid) -> bool {
        self.motions.contains_key(id)
    }

    pub fn get_device(&self, id: &Uuid) -> Option<Ref<Uuid, DeviceInfo>> {
        self.devices.get(id)
    }

    pub fn get_controller(&self, id: &Uuid) -> Option<Ref<Uuid, Component<Controller>>> {
        self.controllers.get(id)
    }

    pub fn get_motion(&self, id: &Uuid) -> Option<Ref<Uuid, Component<Motion>>> {
        self.motions.get(id)
    }

    pub fn add_update_channel(&self, id: &Uuid) -> Receiver<()> {
        let (send, recv) = crossbeam_channel::bounded(1);
        self.updates.entry(*id).or_insert(Vec::new()).push(send);
        recv
    }
}

impl ZInputApi for Engine {
    fn new_controller(&self, info: ControllerInfo) -> Uuid {
        let id = Uuid::new_v4();
        self.controllers.insert(id, Component::new(info));
        id
    }

    fn new_motion(&self, info: MotionInfo) -> Uuid {
        let id = Uuid::new_v4();
        self.motions.insert(id, Component::new(info));
        id
    }

    fn new_device(&self, info: DeviceInfo) -> Uuid {
        let id = Uuid::new_v4();
        self.devices.insert(id, info);
        id
    }

    fn update_controller(
        &self,
        id: &Uuid,
        data: &Controller,
    ) -> Result<(), InvalidComponentIdError> {
        let mut component = self
            .controllers
            .get_mut(id)
            .ok_or(InvalidComponentIdError)?;

        component.data.update(data);

        match self.updates.get(id) {
            Some(senders) => {
                for sender in senders.value() {
                    if sender.is_empty() {
                        sender.send(()).unwrap();
                    }
                }
            }
            None => {}
        }

        Ok(())
    }

    fn update_motion(&self, id: &Uuid, data: &Motion) -> Result<(), InvalidComponentIdError> {
        let mut component = self.motions.get_mut(id).ok_or(InvalidComponentIdError)?;

        component.data.update(data);

        match self.updates.get(id) {
            Some(senders) => {
                for sender in senders.value() {
                    if sender.is_empty() {
                        sender.send(()).unwrap();
                    }
                }
            }
            None => {}
        }

        Ok(())
    }

    fn remove_controller(&self, id: &Uuid) {
        self.controllers.remove(id);
    }

    fn remove_motion(&self, id: &Uuid) {
        self.motions.remove(id);
    }

    fn remove_device(&self, id: &Uuid) {
        self.devices.remove(id);
    }
}
