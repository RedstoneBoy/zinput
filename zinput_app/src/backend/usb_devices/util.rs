use anyhow::{Context, Result};
use rusb::{Device, InterfaceDescriptor, UsbContext};

pub trait UsbExt {
    fn find_interface<F>(&self, filter: F) -> Result<u8>
    where
        F: FnMut(&InterfaceDescriptor) -> bool;
}

impl<T: UsbContext> UsbExt for Device<T> {
    fn find_interface<'a, F>(&'a self, mut filter: F) -> Result<u8>
    where
        F: FnMut(&InterfaceDescriptor) -> bool,
    {
        let interface = self
            .active_config_descriptor()
            .context("failed to get active config descriptor")?
            .interfaces()
            .flat_map(|interface| interface.descriptors())
            .find(|desc| filter(desc))
            .context("failed to find interface descriptor")?
            .interface_number();

        Ok(interface)
    }
}

pub fn hid_filter(desc: &InterfaceDescriptor) -> bool {
    desc.class_code() == 3 && desc.sub_class_code() == 0 && desc.protocol_code() == 0
}
