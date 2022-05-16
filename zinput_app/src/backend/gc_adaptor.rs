use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use rusb::UsbContext;
use zinput_engine::device::component::controller::{Button, Controller, ControllerInfo};
use zinput_engine::{
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

const EP_IN: u8 = 0x81;
const EP_OUT: u8 = 0x02;

const VENDOR_ID: u16 = 0x057E;
const PRODUCT_ID: u16 = 0x0337;

const STATE_NORMAL: u8 = 0x10;
const STATE_WAVEBIRD: u8 = 0x20;

const T: &'static str = "backend:gc_adaptor";

pub struct GcAdaptor {
    inner: Mutex<GcAdaptorInner>,
}

impl GcAdaptor {
    pub fn new() -> Self {
        GcAdaptor {
            inner: Mutex::new(GcAdaptorInner::new()),
        }
    }
}

impl Plugin for GcAdaptor {
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
        "gc_adaptor"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

struct GcAdaptorInner {
    callback_registration: Option<rusb::Registration<rusb::GlobalContext>>,
    handles: Arc<Mutex<Vec<std::thread::JoinHandle<()>>>>,
    stop: Arc<AtomicBool>,
    status: PluginStatus,
}

impl GcAdaptorInner {
    fn new() -> Self {
        GcAdaptorInner {
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
                let handle = std::thread::spawn(new_adaptor_thread(
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
        let handle = std::thread::spawn(new_adaptor_thread(
            device,
            self.next_id.fetch_add(1, Ordering::SeqCst),
            self.stop.clone(),
            self.api.clone(),
        ));
        self.handles.lock().push(handle);
    }

    fn device_left(&mut self, _device: rusb::Device<rusb::GlobalContext>) {}
}

fn new_adaptor_thread(
    usb_dev: rusb::Device<rusb::GlobalContext>,
    id: u64,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> impl FnOnce() {
    move || {
        log::info!(target: T, "adaptor found, id: {}", id);
        match adaptor_thread(usb_dev, id, stop, api) {
            Ok(()) => log::info!(target: T, "adaptor thread {} closed", id),
            Err(err) => log::error!(target: T, "adaptor thread {} crashed: {:#}", id, err),
        }
    }
}

fn adaptor_thread(
    usb_dev: rusb::Device<rusb::GlobalContext>,
    id: u64,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> Result<()> {
    let mut adaptor = usb_dev.open().context("failed to open device")?;

    match adaptor.set_auto_detach_kernel_driver(true) {
        Ok(()) => {}
        Err(rusb::Error::NotSupported) => {}
        Err(err) => {
            Err(err).context("failed to auto-detach kernel drivers")?;
        }
    }

    let iface = usb_dev
        .active_config_descriptor()
        .context("failed to get active config descriptor")?
        .interfaces()
        .next()
        .context("failed to find available interface")?
        .number();

    adaptor
        .claim_interface(iface)
        .context("failed to claim interface")?;

    if adaptor.write_interrupt(EP_OUT, &[0x13], Duration::from_secs(5))
        .context("write interrupt error: is the correct driver installed for the device (i.e. using zadig)")?
        != 1
    {
        return Err(anyhow!("invalid size sent"));
    }

    let mut ctrls = Controllers::new(id, &*api);

    let mut payload = [0u8; 37];

    while !stop.load(Ordering::Acquire) {
        let size = match adaptor.read_interrupt(EP_IN, &mut payload, Duration::from_millis(2000)) {
            Ok(size) => size,
            Err(rusb::Error::NoDevice) => break,
            Err(err) => return Err(err.into()),
        };
        if size != 37 || payload[0] != 0x21 {
            continue;
        }

        ctrls.update(&payload[1..])?;
    }

    Ok(())
}

struct Controllers<'a> {
    api: &'a Engine,
    adaptor_id: u64,
    bundles: [Option<DeviceBundle<'a>>; 4],
}

impl<'a> Controllers<'a> {
    fn new(adaptor_id: u64, api: &'a Engine) -> Self {
        Controllers {
            api,
            adaptor_id,
            bundles: [None, None, None, None],
        }
    }

    fn update(&mut self, data: &[u8]) -> Result<()> {
        for i in 0..4 {
            let data = &data[(i * 9)..][..9];

            let is_active = data[0] & (STATE_NORMAL | STATE_WAVEBIRD);
            let is_active = is_active == STATE_NORMAL || is_active == STATE_WAVEBIRD;

            let bundle = match &mut self.bundles[i] {
                Some(_) if !is_active => {
                    // remove device
                    log::info!(
                        target: T,
                        "removing slot {} from adaptor {}",
                        i + 1,
                        self.adaptor_id
                    );

                    self.bundles[i] = None;
                    continue;
                }
                Some(bundle) => bundle,
                None if is_active => {
                    // add device
                    log::info!(
                        target: T,
                        "adding slot {} from adaptor {}",
                        i + 1,
                        self.adaptor_id
                    );

                    let bundle = DeviceBundle::new(
                        self.api,
                        format!(
                            "Gamecube Adaptor Slot {} (Adaptor {})",
                            i + 1,
                            self.adaptor_id
                        ),
                        [gc_controller_info()],
                    );

                    self.bundles[i] = Some(bundle);
                    self.bundles[i].as_mut().unwrap()
                }
                None => continue,
            };

            bundle.controller[0].buttons = convert_buttons(data[1], data[2]);
            bundle.controller[0].left_stick_x = data[3];
            bundle.controller[0].left_stick_y = data[4];
            bundle.controller[0].right_stick_x = data[5];
            bundle.controller[0].right_stick_y = data[6];
            bundle.controller[0].l2_analog = data[7];
            bundle.controller[0].r2_analog = data[8];

            bundle.update()?;
        }

        Ok(())
    }
}

crate::device_bundle!(DeviceBundle, controller: Controller,);

fn convert_buttons(data1: u8, data2: u8) -> u64 {
    enum GcButton {
        Start = 0,
        Z = 1,
        R = 2,
        L = 3,
        A = 8,
        B = 9,
        X = 10,
        Y = 11,
        Left = 12,
        Right = 13,
        Down = 14,
        Up = 15,
    }

    let buttons = ((data1 as u16) << 8) | (data2 as u16);

    let mut out = 0u64;

    macro_rules! convert {
        ($($gcbutton:expr, $button:expr);* $(;)?) => {
            $(if (buttons >> ($gcbutton as u16)) & 1 == 1 {
                $button.set_pressed(&mut out);
            })*
        }
    }

    convert!(
        GcButton::Start, Button::Start;
        GcButton::Z,     Button::R1;
        GcButton::R,     Button::R2;
        GcButton::L,     Button::L2;
        GcButton::A,     Button::A;
        GcButton::B,     Button::B;
        GcButton::X,     Button::X;
        GcButton::Y,     Button::Y;
        GcButton::Left,  Button::Left;
        GcButton::Right, Button::Right;
        GcButton::Down,  Button::Down;
        GcButton::Up,    Button::Up;
    );

    out
}

fn gc_controller_info() -> ControllerInfo {
    let mut info = ControllerInfo {
        buttons: 0,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick()
    .with_l2_analog()
    .with_r2_analog();

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
        Button::R1,
        Button::L2,
        Button::R2
    );

    info
}
