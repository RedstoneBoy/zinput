use std::{sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}}, ops::Deref, time::Duration};

use parking_lot::{RwLock, RwLockReadGuard, Condvar, Mutex};
use paste::paste;
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

    pub fn update<F>(&mut self, updater: F)
    where
        F: for<'a> FnOnce(DeviceMut<'a>),
    {
        updater(self.internal.device.write().as_mut());

        // match self.event_channel.send(Event::DeviceUpdate(*id)) {
        //     Ok(()) => {}
        //     Err(_) => {}
        // }
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
}

impl DeviceView {
    pub(super) fn new(internal: Arc<InternalDevice>) -> Self {
        internal.views.fetch_add(1, Ordering::AcqRel);
        DeviceView { internal }
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.internal.info
    }

    pub fn device(&self) -> DeviceRead {
        DeviceRead {
            lock: self.internal.device.read(),
        }
    }

    pub fn wait(&mut self, timeout: Duration) -> Result<DeviceRead, Timeout> {
        self.internal.update_signal.wait(timeout)?;
        Ok(self.device())
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
    handle: AtomicBool,
    views: AtomicUsize,

    info: DeviceInfo,
    device: RwLock<Device>,

    update_signal: Signal,
}

macro_rules! internal_device_components {
    ($($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            impl InternalDevice {
                pub(super) fn new(info: DeviceInfo) -> Arc<Self> {
                    let device = Device {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };
                    let device = RwLock::new(device);

                    Arc::new(InternalDevice {
                        handle: AtomicBool::new(false),
                        views: AtomicUsize::new(0),

                        info,
                        device,

                        update_signal: Signal::new(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timeout;

struct Signal {
    cvar: Condvar,
    mutex: Mutex<()>,
}

impl Signal {
    fn new() -> Self {
        Signal { cvar: Condvar::new(), mutex: Mutex::new(()) }
    }

    fn send(&self) {
        let _ = self.mutex.lock();
        self.cvar.notify_all();
    }

    fn wait(&self, timeout: Duration) -> Result<(), Timeout> {
        let mut guard = self.mutex.lock();
        if self.cvar.wait_for(&mut guard, timeout).timed_out() {
            Err(Timeout)
        } else {
            Ok(())
        }
    }
}