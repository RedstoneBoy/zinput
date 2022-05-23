use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use zinput_engine::{
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

mod gc_adaptor;
mod pa_switch;
mod util;

const T: &'static str = "backend:usb_devices";

struct UsbDriver {
    filter: Box<dyn Fn(&rusb::Device<rusb::GlobalContext>) -> bool + Send>,
    thread: fn(ThreadData) -> Box<dyn FnOnce() + Send>,
}

struct DriverData {
    driver: UsbDriver,
    device_id: u64,
}

struct ThreadData {
    device_id: u64,
    device: rusb::Device<rusb::GlobalContext>,
    stop: Arc<AtomicBool>,
    engine: Arc<Engine>,
}

pub struct UsbDevices {
    inner: Mutex<Inner>,
}

impl UsbDevices {
    pub fn new() -> Self {
        UsbDevices {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for UsbDevices {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "usb_devices"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

enum Inner {
    Uninit,
    Init {
        drivers: Arc<Mutex<Vec<DriverData>>>,
        handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<PluginStatus>>,
    },
}

impl Inner {
    fn new() -> Self {
        Inner::Uninit
    }

    fn init(&mut self, engine: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }

        match self.init_inner(engine) {
            Ok(()) => log::info!(target: T, "driver initialized"),
            Err(err) => {
                log::error!(target: T, "driver failed to initialize: {:#}", err);
                if let Inner::Init { status, .. } = self {
                    *status.lock() = PluginStatus::Error(format!("error initializing driver"));
                }
            }
        }
    }

    fn init_inner(&mut self, engine: Arc<Engine>) -> Result<()> {
        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));

        let drivers = vec![
            gc_adaptor::driver(),
            pa_switch::driver(),
        ];
        let mut drivers: Vec<DriverData> = drivers
            .into_iter()
            .map(|driver| DriverData {
                driver,
                device_id: 0,
            })
            .collect();

        let mut handles = Vec::new();

        for usb_device in rusb::devices()
            .context("failed to find usb devices")?
            .iter()
        {
            for driver_data in &mut drivers {
                if (driver_data.driver.filter)(&usb_device) {
                    let device_id = driver_data.device_id;
                    driver_data.device_id += 1;
                    let handle = std::thread::spawn((driver_data.driver.thread)(ThreadData {
                        device_id,
                        device: usb_device,
                        stop: stop.clone(),
                        engine: engine.clone(),
                    }));

                    handles.push(handle);

                    break;
                }
            }
        }

        let drivers = Arc::new(Mutex::new(drivers));

        let handles = Arc::new(Mutex::new(handles));

        *self = Inner::Init {
            drivers,
            handles,
            stop,
            status,
        };

        Ok(())
    }

    fn stop(&mut self) {
        match std::mem::replace(self, Inner::Uninit) {
            Inner::Uninit => {}
            Inner::Init {
                handles,
                stop,
                status,
                ..
            } => {
                stop.store(true, Ordering::Release);

                let mut handles = handles.lock();
                for handle in std::mem::replace(&mut *handles, Vec::new()) {
                    match handle.join() {
                        Ok(()) => (),
                        Err(_) => log::info!(target: T, "a usb driver panicked"),
                    }
                }

                *status.lock() = PluginStatus::Stopped;
            }
        }
    }

    fn status(&self) -> PluginStatus {
        match self {
            Inner::Uninit => PluginStatus::Stopped,
            Inner::Init { status, .. } => status.lock().clone(),
        }
    }
}
