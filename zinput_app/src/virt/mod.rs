use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bindlang::backend_cranelift::Program;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use zinput_engine::DeviceHandle;
use zinput_engine::device::DeviceMutFfi;
use zinput_engine::{util::Uuid, DeviceView, Engine};

mod device;

use self::device::VDevice;

#[derive(Copy, Clone)]
pub struct VDeviceHandle(Uuid);

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

    pub fn insert(
        &self,
        out: DeviceHandle,
        mut views: Vec<DeviceView>,
    ) -> VDeviceHandle {
        let mut shared = self.shared.lock();

        let name = out.view().info().name.clone();

        let device = Uuid::new_v4();

        for view in 0..views.len() {
            views[view].register_channel(self.send.clone());

            shared
                .recv_map
                .entry(*views[view].uuid())
                .or_default()
                .push(RecvDest { device, view });
        }

        let vdev = VDevice::new(name, views, out);
        shared.devices.insert(device, vdev);

        VDeviceHandle(device)
    }

    pub fn remove(
        &mut self,
        handle: VDeviceHandle,
    ) {
        let mut shared = self.shared.lock();

        for dests in shared.recv_map.values_mut() {
            let mut i = 0;
            while i < dests.len() {
                if dests[i].device == handle.0 {
                    dests.swap_remove(i);
                } else {
                    i += 1;
                }
            }
        }

        shared.devices.remove(&handle.0);
    }

    pub fn set_program(
        &mut self,
        handle: VDeviceHandle,
        program: Option<Program<DeviceMutFfi>>,
    ) {
        let mut shared = self.shared.lock();
        let Some(device) = shared.devices.get_mut(&handle.0)
        else { return; };

        device.set_program(program);
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
    devices: HashMap<Uuid, VDevice>,
    recv_map: HashMap<Uuid, Vec<RecvDest>>,
}

#[derive(Copy, Clone)]
struct RecvDest {
    device: Uuid,
    view: usize,
}

fn updater_thread(thread: Thread) -> impl FnOnce() {
    move || {
        let Thread { shared, recv, stop } = thread;
        let mut recv_dests = Vec::new();

        loop {
            crossbeam_channel::select! {
                recv(recv) -> recv => {
                    let mut shared = shared.lock();

                    let Ok(id) = recv
                    else { break; };

                    let Some(dests) = shared.recv_map.get(&id)
                    else { continue; };

                    recv_dests.clone_from(&dests);

                    for dest in &recv_dests {
                        let Some(device) = shared.devices.get_mut(&dest.device)
                        else { continue; };
    
                        device.update(dest.view);
                    }                    

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
