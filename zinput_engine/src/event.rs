use zinput_device::DeviceInfo;

use crate::util::Uuid;

#[derive(Clone)]
pub enum Event {
    DeviceUpdate(Uuid),
    DeviceAdded(Uuid, DeviceInfo),
    DeviceRemoved(Uuid),
    UsbConnected,
    UsbDisconnected,
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::DeviceUpdate(_) => EventKind::DeviceUpdate,
            Event::DeviceAdded(_, _) => EventKind::DeviceAdded,
            Event::DeviceRemoved(_) => EventKind::DeviceRemoved,
            Event::UsbConnected => EventKind::UsbConnected,
            Event::UsbDisconnected => EventKind::UsbDisconnected,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EventKind {
    DeviceUpdate,
    DeviceAdded,
    DeviceRemoved,
    UsbConnected,
    UsbDisconnected,
}
