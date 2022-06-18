use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use zinput_engine::device::DeviceInfo;
use zinput_engine::{util::Uuid, DeviceAlreadyExists, DeviceView, Engine};

mod ast;
mod vm;
mod device;
mod updater;

pub use self::device::VDevice;
pub use self::updater::{Updater, VerificationError};

pub struct VirtualDevices {
    engine: Arc<Engine>,

    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,

    send: Sender<Uuid>,
}

impl VirtualDevices {
    pub fn new(engine: Arc<Engine>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));

        let (thread, send) = Thread::new(stop.clone());
        let shared = thread.shared.clone();

        std::thread::spawn(updater_thread(thread));

        VirtualDevices {
            engine,

            shared,
            stop,

            send,
        }
    }

    pub fn new_device(
        &mut self,
        mut views: Vec<DeviceView>,
        updater: Box<dyn Updater>,
    ) -> Result<VDevice, VirtualDeviceError> {
        let mut shared = self.shared.lock();

        updater.verify(&views)?;
        let out = updater.create_output(&self.engine)?;
        let name = out.view().info().name.clone();

        let device = shared.devices.len();

        for view in 0..views.len() {
            views[view].register_channel(self.send.clone());

            shared
                .recv_map
                .insert(*views[view].uuid(), RecvDest { device, view });
        }

        let vdev = VDevice::new(name, views, out, updater);

        Ok(vdev)
    }
}

impl Drop for VirtualDevices {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

struct Thread {
    shared: Arc<Mutex<Shared>>,
    recv: Receiver<Uuid>,
    stop: Arc<AtomicBool>,
}

impl Thread {
    fn new(stop: Arc<AtomicBool>) -> (Self, Sender<Uuid>) {
        let (send, recv) = crossbeam_channel::unbounded();

        (
            Thread {
                shared: Default::default(),
                recv,
                stop,
            },
            send,
        )
    }
}

#[derive(Default)]
struct Shared {
    devices: Vec<VDevice>,
    recv_map: HashMap<Uuid, RecvDest>,
}

struct RecvDest {
    device: usize,
    view: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VirtualDeviceError {
    VerificationError(VerificationError),
    DeviceAlreadyExists(DeviceAlreadyExists),
}

impl From<VerificationError> for VirtualDeviceError {
    fn from(err: VerificationError) -> Self {
        VirtualDeviceError::VerificationError(err)
    }
}

impl From<DeviceAlreadyExists> for VirtualDeviceError {
    fn from(err: DeviceAlreadyExists) -> Self {
        VirtualDeviceError::DeviceAlreadyExists(err)
    }
}

impl std::error::Error for VirtualDeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VirtualDeviceError::VerificationError(err) => Some(err),
            VirtualDeviceError::DeviceAlreadyExists(err) => Some(err),
        }
    }
}

impl std::fmt::Display for VirtualDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VirtualDeviceError::VerificationError(err) => write!(f, "{err}"),
            VirtualDeviceError::DeviceAlreadyExists(err) => write!(f, "{err}"),
        }
    }
}

fn updater_thread(thread: Thread) -> impl FnOnce() {
    move || {
        let Thread { shared, recv, stop } = thread;

        loop {
            crossbeam_channel::select! {
                recv(recv) -> recv => {
                    let shared = shared.lock();

                    let Ok(id) = recv
                    else { break; };

                    let Some(dest) = shared.recv_map.get(&id)
                    else { continue; };

                    let Some(device) = shared.devices.get(dest.device)
                    else { continue; };

                    device.update(dest.view);

                    drop(shared);
                }
                default(Duration::from_secs(1)) => {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                }
            }
        }
    }
}
