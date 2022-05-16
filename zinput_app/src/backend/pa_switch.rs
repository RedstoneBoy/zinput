use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusb::UsbContext;
use zinput_engine::device::component::{
    controller::{Button, Controller, ControllerInfo},
};
use zinput_engine::{
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

const EP_IN: u8 = 0x81;

const VENDOR_ID: u16 = 0x20D6;
const PRODUCT_ID: u16 = 0xA713;

const T: &'static str = "backend:pa_switch";

pub struct PASwitch {
    inner: Mutex<Inner>,
}

impl PASwitch {
    pub fn new() -> Self {
        PASwitch {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for PASwitch {
    fn init(&self, zinput_api: Arc<Engine>) {
        self.inner.lock().init(zinput_api)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "pa_switch"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

struct Inner {
    callback_registration: Option<rusb::Registration<rusb::GlobalContext>>,
    handles: Arc<Mutex<Vec<std::thread::JoinHandle<()>>>>,
    stop: Arc<AtomicBool>,
    status: PluginStatus,
}

impl Inner {
    fn new() -> Self {
        Inner {
            callback_registration: None,
            handles: Arc::new(Mutex::new(Vec::new())),
            stop: Arc::new(AtomicBool::new(false)),
            status: PluginStatus::Running,
        }
    }

    fn init(&mut self, api: Arc<Engine>) {
        log::info!(target: T, "driver initializing...");

        self.status = PluginStatus::Running;
        self.stop = Arc::new(AtomicBool::new(false));

        match || -> Result<()> {
            let next_id = Arc::new(AtomicU64::new(0));

            for usb_dev in rusb::devices()
                .context("failed to find devices")?
                .iter()
                .filter(|dev| {
                    dev.device_descriptor()
                        .ok()
                        .map(|desc| {
                            desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID
                        })
                        .unwrap_or(false)
                })
            {
                let handle = std::thread::spawn(new_controller_thread(
                    usb_dev,
                    next_id.fetch_add(1, Ordering::SeqCst),
                    self.stop.clone(),
                    api.clone(),
                ));
                self.handles.lock().push(handle);
            }

            if rusb::has_hotplug() {
                log::info!(
                    target: T,
                    "usb driver supports hotplug, registering callback handler"
                );
                self.callback_registration = rusb::GlobalContext {}
                    .register_callback(
                        Some(VENDOR_ID),
                        Some(PRODUCT_ID),
                        None,
                        Box::new(CallbackHandler {
                            api,
                            stop: self.stop.clone(),
                            next_id,
                            handles: self.handles.clone(),
                        }),
                    )
                    .map(Some)
                    .context("failed to register callback handler")?;
            } else {
                log::info!(target: T, "usb driver does not support hotplug");
            }

            Ok(())
        }() {
            Ok(()) => log::info!(target: T, "driver initalized"),
            Err(err) => {
                log::error!(target: T, "driver failed to initalize: {:#}", err);
                self.status = PluginStatus::Error("driver failed to initialize".to_owned());
            }
        }
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for handle in std::mem::replace(&mut *self.handles.lock(), Vec::new()) {
            let _ = handle.join();
        }
        self.stop.store(false, Ordering::Release);
        self.status = PluginStatus::Stopped;
    }

    fn status(&self) -> PluginStatus {
        self.status.clone()
    }
}

struct CallbackHandler {
    api: Arc<Engine>,
    stop: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl rusb::Hotplug<rusb::GlobalContext> for CallbackHandler {
    fn device_arrived(&mut self, device: rusb::Device<rusb::GlobalContext>) {
        let handle = std::thread::spawn(new_controller_thread(
            device,
            self.next_id.fetch_add(1, Ordering::SeqCst),
            self.stop.clone(),
            self.api.clone(),
        ));
        self.handles.lock().push(handle);
    }

    fn device_left(&mut self, _device: rusb::Device<rusb::GlobalContext>) {}
}

fn new_controller_thread(
    usb_dev: rusb::Device<rusb::GlobalContext>,
    id: u64,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> impl FnOnce() {
    move || {
        log::info!(target: T, "controller found, id: {}", id);
        match controller_thread(usb_dev, id, stop, api) {
            Ok(()) => log::info!(target: T, "controller thread {} closed", id),
            Err(err) => log::error!(target: T, "controller thread {} crashed: {:#}", id, err),
        }
    }
}

fn controller_thread(
    usb_dev: rusb::Device<rusb::GlobalContext>,
    id: u64,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> Result<()> {
    let mut pa = usb_dev.open().context("failed to open device")?;
    let iface = usb_dev
        .config_descriptor(0)
        .context("failed to get active config descriptor")?
        .interfaces()
        .next()
        .context("failed to find available interface")?
        .number();
    
    pa.claim_interface(iface)
        .context("failed to claim interface")?;

    let mut bundle = PABundle::new(id, &*api);

    let mut buf = [0u8; 8];

    while !stop.load(Ordering::Acquire) {
        let size = match pa.read_interrupt(EP_IN, &mut buf, Duration::from_millis(2000)) {
            Ok(size) => size,
            Err(rusb::Error::Timeout) => continue,
            Err(rusb::Error::NoDevice) => break,
            Err(err) => return Err(err.into()),
        };
        if size != 8 {
            continue;
        }

        bundle.update(&buf)?;
    }

    Ok(())
}

crate::device_bundle!(DeviceBundle,
    controller: Controller,
);

struct PABundle<'a> {
    bundle: DeviceBundle<'a>,
}

impl<'a> PABundle<'a> {
    fn new(adaptor_id: u64, api: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            api,
            format!("PowerA Wired Pro Controller (Adaptor {})", adaptor_id),
            [controller_info()],
        );

        PABundle { bundle }
    }

    fn update(&mut self, data: &[u8; 8]) -> Result<()> {
        macro_rules! convert {
            ($out:expr => $( ( $ine:expr ) $offset:expr , $button:expr );* $(;)?) => {
                $(if (($ine >> $offset) & 1) != 0 {
                    $button.set_pressed($out);
                })*
            }
        }

        let mut new_buttons = 0;
        convert!(&mut new_buttons =>
            (data[0]) 0, Button::Y;
            (data[0]) 1, Button::B;
            (data[0]) 2, Button::A;
            (data[0]) 3, Button::X;
            (data[0]) 4, Button::L1;
            (data[0]) 5, Button::R1;
            (data[0]) 6, Button::L2;
            (data[0]) 7, Button::R2;
            (data[1]) 0, Button::Select;
            (data[1]) 1, Button::Start;
            (data[1]) 2, Button::LStick;
            (data[1]) 3, Button::RStick;
            (data[1]) 4, Button::Home;
            (data[1]) 5, Button::Capture;
        );

        if data[2] == 7 || data[2] == 0 || data[2] == 1 {
            Button::Up.set_pressed(&mut new_buttons);
        }
        if data[2] == 1 || data[2] == 2 || data[2] == 3 {
            Button::Right.set_pressed(&mut new_buttons);
        }
        if data[2] == 3 || data[2] == 4 || data[2] == 5 {
            Button::Down.set_pressed(&mut new_buttons);
        }
        if data[2] == 5 || data[2] == 6 || data[2] == 7 {
            Button::Left.set_pressed(&mut new_buttons);
        }

        self.bundle.controller[0].buttons = new_buttons;
        self.bundle.controller[0].l2_analog = (Button::L2.is_pressed(new_buttons) as u8) * 255;
        self.bundle.controller[0].r2_analog = (Button::R2.is_pressed(new_buttons) as u8) * 255;
        self.bundle.controller[0].left_stick_x = data[3];
        self.bundle.controller[0].left_stick_y = 255 - data[4];
        self.bundle.controller[0].right_stick_x = data[5];
        self.bundle.controller[0].right_stick_y = 255 - data[6];

        self.bundle.update()?;

        Ok(())
    }
}

fn controller_info() -> ControllerInfo {
    let mut info = ControllerInfo {
        buttons: 0,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick();

    macro_rules! for_buttons {
        ($($button:expr),* $(,)?) => {
            $($button.set_pressed(&mut info.buttons);)*
        }
    }
    for_buttons!(
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::Start,
        Button::Select,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::LStick,
        Button::RStick,
        Button::Home,
        Button::Capture,
    );

    info
}
