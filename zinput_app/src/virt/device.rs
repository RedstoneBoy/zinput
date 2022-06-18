use zinput_engine::{DeviceView, DeviceHandle};

use super::updater::Updater;

pub struct VDevice {
    name: String,
    
    views: Vec<DeviceView>,
    out: DeviceHandle,
    updater: Box<dyn Updater>,
}

impl VDevice {
    pub(super) fn new(name: String, views: Vec<DeviceView>, out: DeviceHandle, updater: Box<dyn Updater>) -> Self {
        VDevice {
            name,

            views,
            out,
            updater,
        }
    }

    pub(super) fn update(&self, view_index: usize) {
        let view = &self.views[view_index];
        self.updater.update(view, view_index, &self.out);
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}