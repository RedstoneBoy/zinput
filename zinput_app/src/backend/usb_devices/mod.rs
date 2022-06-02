use std::{
    collections::HashMap,
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

use self::hotplug::HotPlug;

mod device_thread;
mod gc_adaptor;
mod hotplug;
mod pa_switch;
mod steam_controller;
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
        scan_context: Arc<Mutex<ScanContext>>,
        hotplug: Option<HotPlug>,

        status: Arc<Mutex<PluginStatus>>,
    },
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
struct UsbDeviceId {
    bus_number: u8,
    address: u8,
    port: u8,
}

struct ScanContext {
    drivers: Vec<DriverData>,
    handles: HashMap<UsbDeviceId, JoinHandle<()>>,
    to_remove: Vec<UsbDeviceId>,

    stop: Arc<AtomicBool>,
    engine: Arc<Engine>,
}

impl ScanContext {
    fn new(drivers: Vec<DriverData>, stop: Arc<AtomicBool>, engine: Arc<Engine>) -> Self {
        ScanContext {
            drivers,
            handles: HashMap::new(),
            to_remove: Vec::new(),

            stop,
            engine,
        }
    }
    fn scan_devices(&mut self) -> Result<()> {
        self.to_remove.clear();

        for (id, handle) in &self.handles {
            if handle.is_finished() {
                self.to_remove.push(*id);
            }
        }

        for id in &self.to_remove {
            let _ = self.handles.remove(id).unwrap().join();
        }

        for usb_device in rusb::devices()
            .context("failed to find usb devices")?
            .iter()
        {
            let id = UsbDeviceId {
                bus_number: usb_device.bus_number(),
                address: usb_device.address(),
                port: usb_device.port_number(),
            };

            if self.handles.contains_key(&id) {
                continue;
            }

            for driver_data in &mut self.drivers {
                if (driver_data.driver.filter)(&usb_device) {
                    let device_id = driver_data.device_id;
                    driver_data.device_id += 1;

                    let handle = std::thread::spawn((driver_data.driver.thread)(ThreadData {
                        device_id,
                        device: usb_device,
                        stop: self.stop.clone(),
                        engine: self.engine.clone(),
                    }));

                    self.handles.insert(id, handle);

                    break;
                }
            }
        }

        Ok(())
    }
}

fn hotplug_function(scan_context: Arc<Mutex<ScanContext>>) -> impl FnMut() + Send + 'static {
    move || match scan_context.lock().scan_devices() {
        Ok(()) => {}
        Err(e) => {
            log::warn!(target: T, "failed to scan devices: {}", e);
        }
    }
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
            steam_controller::driver(),
        ];
        let drivers: Vec<DriverData> = drivers
            .into_iter()
            .map(|driver| DriverData {
                driver,
                device_id: 0,
            })
            .collect();

        let mut scan_context = ScanContext::new(drivers, stop.clone(), engine.clone());
        scan_context
            .scan_devices()
            .context("failed to scan devices")?;

        let scan_context = Arc::new(Mutex::new(scan_context));

        let hotplug = HotPlug::register(hotplug_function(scan_context.clone()));

        let hotplug = match hotplug {
            Ok(v) => Some(v),
            Err(e) => {
                log::warn!(target: T, "failed to register hotplug: {}", e);
                None
            }
        };

        *self = Inner::Init {
            scan_context,
            status,

            hotplug,
        };

        Ok(())
    }

    fn stop(&mut self) {
        match std::mem::replace(self, Inner::Uninit) {
            Inner::Uninit => {}
            Inner::Init {
                scan_context,
                status,
                hotplug,
                ..
            } => {
                drop(hotplug);

                let mut scan_context = scan_context.lock();

                scan_context.stop.store(true, Ordering::Release);

                for (_, handle) in std::mem::replace(&mut scan_context.handles, HashMap::new()) {
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
