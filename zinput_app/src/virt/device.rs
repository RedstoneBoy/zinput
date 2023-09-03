use zinput_engine::{DeviceHandle, DeviceView};

pub struct VDevice {
    name: String,
    views: Vec<DeviceView>,
    output_handle: DeviceHandle,
}

unsafe impl Send for VDevice {}

impl VDevice {
    pub fn new(name: String, views: Vec<DeviceView>, output_handle: DeviceHandle) -> Self {
        VDevice {
            name,
            views,
            output_handle,
        }
    }

    pub fn update(&mut self, _input_index: usize) {}

    pub fn name(&self) -> &str {
        &self.name
    }
}
