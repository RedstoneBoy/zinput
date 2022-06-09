use std::{sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}}, ops::Deref};

use crossbeam_channel::{Sender, TrySendError};
use index_map::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard, Mutex};
use paste::paste;
use uuid::Uuid;
use zinput_device::{DeviceInfo, Device, DeviceMut};

pub struct DeviceHandle {
    internal: Arc<InternalDevice>,
}

impl DeviceHandle {
    pub(super) fn new(internal: Arc<InternalDevice>) -> Option<Self> {
        internal.handle.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| DeviceHandle { internal })
    }

    pub fn update<F>(&self, updater: F)
    where
        F: for<'a> FnOnce(DeviceMut<'a>),
    {
        updater(self.internal.device.write().as_mut());

        self.internal.channels.lock().retain(|_, channel| {
            match channel.try_send(self.internal.uuid) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        assert!(self.internal.handle.load(Ordering::Acquire) == true, "DeviceHandle was already dropped");

        self.internal.handle.store(false, Ordering::Release);
    }
}

pub struct DeviceView {
    internal: Arc<InternalDevice>,

    channel: Option<usize>,
}

impl DeviceView {
    pub(super) fn new(internal: Arc<InternalDevice>) -> Self {
        internal.views.fetch_add(1, Ordering::AcqRel);
        DeviceView { internal, channel: None }
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.internal.info
    }

    pub fn device(&self) -> DeviceRead {
        DeviceRead {
            lock: self.internal.device.read(),
        }
    }

    pub fn uuid(&self) -> &Uuid {
        &self.internal.uuid
    }

    pub fn register_channel(&mut self, channel: Sender<Uuid>) {
        if let Some(channel) = self.channel.take() {
            self.internal.channels.lock().remove(channel);
        }

        let channel = self.internal.channels.lock().insert(channel);
        self.channel = Some(channel);
    }
}

impl Clone for DeviceView {
    fn clone(&self) -> Self {
        DeviceView::new(self.internal.clone())
    }
}

impl Drop for DeviceView {
    fn drop(&mut self) {
        assert!(self.internal.views.load(Ordering::Acquire) > 0, "number of DeviceViews was incorrect");

        if let Some(channel) = self.channel.take() {
            self.internal.channels.lock().remove(channel);
        }

        self.internal.views.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct DeviceRead<'a> {
    lock: RwLockReadGuard<'a, Device>,
}

impl<'a> Deref for DeviceRead<'a> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.lock.deref()
    }
}

pub(super) struct InternalDevice {
    pub(super) uuid: Uuid,

    pub(super) handle: AtomicBool,
    views: AtomicUsize,

    info: DeviceInfo,
    device: RwLock<Device>,

    channels: Mutex<IndexMap<Sender<Uuid>>>,
}

macro_rules! internal_device_components {
    ($($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            impl InternalDevice {
                pub(super) fn new(info: DeviceInfo, uuid: Uuid) -> Arc<Self> {
                    let device = Device {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };
                    let device = RwLock::new(device);

                    Arc::new(InternalDevice {
                        uuid,

                        handle: AtomicBool::new(false),
                        views: AtomicUsize::new(0),

                        info,
                        device,

                        channels: Mutex::default(),
                    })
                }

                pub(super) fn info(&self) -> &DeviceInfo {
                    &self.info
                }

                pub(super) fn should_remove(&self) -> bool {
                    (self.handle.load(Ordering::Acquire) == false)
                        && (self.views.load(Ordering::Acquire) == 0)
                }
            }
        }
    };
}

internal_device_components!(
    controller: Controller,
    motion: Motion,
    analog: Analogs,
    button: Buttons,
    touch_pad: TouchPad,
);