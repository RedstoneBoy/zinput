use std::convert::TryInto;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusb::UsbContext;
use uuid::Uuid;

use crate::api::component::controller::{Button, Controller, ControllerInfo};
use crate::api::component::motion::{Motion, MotionInfo};
use crate::api::component::touch_pad::{TouchPad, TouchPadInfo, TouchPadShape};
use crate::api::device::DeviceInfo;
use crate::api::{Plugin, PluginKind, PluginStatus};
use crate::zinput::engine::Engine;

const EP_IN: u8 = 0x82;

const VENDOR_ID: u16 = 0x28DE;
const PRODUCT_ID: u16 = 0x1142;

const ENABLE_MOTION: [u8; 64] = [
    0x87, 0x15, 0x32, 0x84, 0x03, 0x18, 0x00, 0x00, 0x31, 0x02, 0x00, 0x08, 0x07, 0x00, 0x07, 0x07,
    0x00, 0x30, 0x18, 0x00, 0x2f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const T: &'static str = "backend:steam_controller";

pub struct SteamController {
    inner: Mutex<Inner>,
}

impl SteamController {
    pub fn new() -> Self {
        SteamController {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for SteamController {
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
        "steam_controller"
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
    let mut sc = usb_dev.open().context("failed to open device")?;
    let iface = usb_dev
        .config_descriptor(0)
        .context("failed to get active config descriptor")?
        .interfaces()
        .find(|inter| {
            inter.descriptors().any(|desc| {
                desc.class_code() == 3 && desc.sub_class_code() == 0 && desc.protocol_code() == 0
            })
        })
        .context("failed to find available interface")?
        .number();
    sc.claim_interface(iface)
        .context("failed to claim interface")?;

    sc.write_control(
        0x21,
        0x09,
        0x0300,
        1,
        &ENABLE_MOTION,
        Duration::from_secs(3),
    )?;

    let mut bundle = SCBundle::new(id, api);

    let mut buf = [0u8; 64];

    while !stop.load(Ordering::Acquire) {
        let size = match sc.read_interrupt(EP_IN, &mut buf, Duration::from_millis(2000)) {
            Ok(size) => size,
            Err(rusb::Error::Timeout) => continue,
            Err(rusb::Error::NoDevice) => break,
            Err(err) => return Err(err.into()),
        };
        if size != 64 {
            continue;
        }

        bundle.update(&buf)?;
    }

    Ok(())
}

struct SCBundle {
    api: Arc<Engine>,
    adaptor_id: u64,
    device_id: Uuid,
    controller_id: Uuid,
    motion_id: Uuid,
    touch_left_id: Uuid,
    touch_right_id: Uuid,
    controller: Controller,
    motion: Motion,
    touch_left: TouchPad,
    touch_right: TouchPad,
}

impl SCBundle {
    fn new(adaptor_id: u64, api: Arc<Engine>) -> Self {
        let controller_id = api.new_controller(sc_controller_info());
        let motion_id = api.new_motion(MotionInfo::new(true, true));
        let touch_left_id = api.new_touch_pad(TouchPadInfo::new(TouchPadShape::Circle, true));
        let touch_right_id = api.new_touch_pad(TouchPadInfo::new(TouchPadShape::Circle, true));

        let device_id = api.new_device(
            DeviceInfo::new(format!("Steam Controller (Adaptor {})", adaptor_id))
                .with_controller(controller_id)
                .with_motion(motion_id)
                .with_touch_pad(touch_left_id)
                .with_touch_pad(touch_right_id),
        );

        SCBundle {
            api,
            adaptor_id,
            device_id,
            controller_id,
            motion_id,
            touch_left_id,
            touch_right_id,
            controller: Default::default(),
            motion: Default::default(),
            touch_left: Default::default(),
            touch_right: Default::default(),
        }
    }

    fn update(&mut self, data: &[u8; 64]) -> Result<()> {
        let buttons = u32::from_le_bytes(data[7..11].try_into().unwrap());
        let ltrig = data[11];
        let rtrig = data[12];
        let lpad_x = i16::from_le_bytes(data[16..18].try_into().unwrap());
        let lpad_y = i16::from_le_bytes(data[18..20].try_into().unwrap());
        let rpad_x = i16::from_le_bytes(data[20..22].try_into().unwrap());
        let rpad_y = i16::from_le_bytes(data[22..24].try_into().unwrap());

        self.update_controller(buttons, ltrig, rtrig, lpad_x, lpad_y, rpad_x, rpad_y);
        self.update_touch_pads(buttons, lpad_x, lpad_y, rpad_x, rpad_y);

        // TODO: Motion
        let accelx = i16::from_le_bytes(data[28..30].try_into().unwrap());
        let accely = i16::from_le_bytes(data[30..32].try_into().unwrap());
        let accelz = i16::from_le_bytes(data[32..34].try_into().unwrap());
        let gpitch = i16::from_le_bytes(data[34..36].try_into().unwrap());
        let groll = i16::from_le_bytes(data[36..38].try_into().unwrap());
        let gyaw = i16::from_le_bytes(data[38..40].try_into().unwrap());

        self.update_motion(accelx, accely, accelz, gpitch, groll, gyaw);

        // let q1 = i16::from_le_bytes(data[40..42].try_into().unwrap());
        // let q2 = i16::from_le_bytes(data[42..44].try_into().unwrap());
        // let q3 = i16::from_le_bytes(data[44..46].try_into().unwrap());
        // let q4 = i16::from_le_bytes(data[46..48].try_into().unwrap());

        self.api
            .update_controller(&self.controller_id, &self.controller)?;
        self.api.update_motion(&self.motion_id, &self.motion)?;

        self.api
            .update_touch_pad(&self.touch_left_id, &self.touch_left)?;
        self.api
            .update_touch_pad(&self.touch_right_id, &self.touch_right)?;
        Ok(())
    }

    fn update_controller(
        &mut self,
        buttons: u32,
        ltrig: u8,
        rtrig: u8,
        lpad_x: i16,
        lpad_y: i16,
        rpad_x: i16,
        rpad_y: i16,
    ) {
        macro_rules! convert {
            ($out:expr => $($scbutton:expr, $button:expr $(, if $guard:expr)?);* $(;)?) => {
                $(if $scbutton.is_pressed(buttons) $(&& $guard)? {
                    $button.set_pressed($out);
                })*
            }
        }

        let lpad_touch = SCButton::LPadTouch.is_pressed(buttons);

        let mut new_buttons = if lpad_touch {
            self.controller.buttons & (1 << Button::LStick.bit())
        } else {
            self.controller.buttons
                & ((1 << Button::Up.bit())
                    | (1 << Button::Down.bit())
                    | (1 << Button::Left.bit())
                    | (1 << Button::Right.bit()))
        };

        convert!(&mut new_buttons =>
            SCButton::RClick, Button::RStick;
            SCButton::LClick, Button::LStick, if !lpad_touch;
            SCButton::LClick, Button::Up,    if lpad_touch && lpad_y >= 0 && lpad_y  > lpad_x && lpad_y  > -lpad_x;
            SCButton::LClick, Button::Down,  if lpad_touch && lpad_y <  0 && -lpad_y > lpad_x && -lpad_y > -lpad_x;
            SCButton::LClick, Button::Left,  if lpad_touch && lpad_x <  0 && -lpad_x > lpad_y && -lpad_x > -lpad_y;
            SCButton::LClick, Button::Right, if lpad_touch && lpad_x >= 0 && lpad_x  > lpad_y && lpad_x  > -lpad_y;
            SCButton::RGrip,  Button::R3;
            SCButton::LGrip,  Button::L3;
            SCButton::Start,  Button::Start;
            SCButton::Steam,  Button::Home;
            SCButton::Back,   Button::Select;
            SCButton::A,      Button::A;
            SCButton::X,      Button::X;
            SCButton::B,      Button::B;
            SCButton::Y,      Button::Y;
            SCButton::Lb,     Button::L1;
            SCButton::Rb,     Button::R1;
            SCButton::Lt,     Button::L2;
            SCButton::Rt,     Button::R2;
        );

        self.controller.buttons = new_buttons;

        self.controller.l2_analog = ltrig;
        self.controller.r2_analog = rtrig;
        self.controller.right_stick_x = ((rpad_x / 256) + 128) as u8;
        self.controller.right_stick_y = ((rpad_y / 256) + 128) as u8;
        if !lpad_touch {
            self.controller.left_stick_x = ((lpad_x / 256) + 128) as u8;
            self.controller.left_stick_y = ((lpad_y / 256) + 128) as u8;
        }
    }

    fn update_touch_pads(
        &mut self,
        buttons: u32,
        lpad_x: i16,
        lpad_y: i16,
        rpad_x: i16,
        rpad_y: i16,
    ) {
        self.touch_right.touch_x = (rpad_x as i32 - i16::MIN as i32) as u32 as u16;
        self.touch_right.touch_y = (rpad_y as i32 - i16::MIN as i32) as u32 as u16;
        self.touch_right.pressed = SCButton::RClick.is_pressed(buttons);
        self.touch_right.touched = SCButton::RPadTouch.is_pressed(buttons);

        if SCButton::LPadTouch.is_pressed(buttons) {
            self.touch_left.touched = true;
            self.touch_left.pressed = SCButton::LClick.is_pressed(buttons);
            self.touch_left.touch_x = (lpad_x as i32 - i16::MIN as i32) as u32 as u16;
            self.touch_left.touch_y = (lpad_y as i32 - i16::MIN as i32) as u32 as u16;
        } else {
            self.touch_left.pressed = false;
            self.touch_left.touched = false;
            self.touch_left.touch_x = u16::MAX / 2;
            self.touch_left.touch_y = u16::MAX / 2;
        }
    }

    fn update_motion(
        &mut self,
        accelx: i16,
        accely: i16,
        accelz: i16,
        gpitch: i16,
        groll: i16,
        gyaw: i16,
    ) {
        const ACCEL_SCALE: f32 = 2.0 / 32768.0;
        const GYRO_SCALE: f32 = 2000.0 / 32768.0;

        self.motion.accel_x = accelx as f32 * -ACCEL_SCALE;
        self.motion.accel_y = accelz as f32 * -ACCEL_SCALE;
        self.motion.accel_z = accely as f32 * ACCEL_SCALE;
        self.motion.gyro_pitch = gpitch as f32 * GYRO_SCALE;
        self.motion.gyro_roll = groll as f32 * GYRO_SCALE;
        self.motion.gyro_yaw = gyaw as f32 * GYRO_SCALE;
    }
}

impl Drop for SCBundle {
    fn drop(&mut self) {
        self.api.remove_device(&self.device_id);
        self.api.remove_controller(&self.controller_id);
        self.api.remove_motion(&self.motion_id);
        self.api.remove_touch_pad(&self.touch_left_id);
        self.api.remove_touch_pad(&self.touch_right_id);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SCButton {
    RPadTouch = 28,
    LPadTouch = 27,
    RClick = 26,
    LClick = 25,
    RGrip = 24,
    LGrip = 23,
    Start = 22,
    Steam = 21,
    Back = 20,
    A = 15,
    X = 14,
    B = 13,
    Y = 12,
    Lb = 11,
    Rb = 10,
    Lt = 9,
    Rt = 8,
}

impl SCButton {
    #[inline(always)]
    pub fn is_pressed(&self, buttons: u32) -> bool {
        (buttons >> *self as u32) & 1 != 0
    }
}

fn sc_controller_info() -> ControllerInfo {
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
        Button::Select,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::L3,
        Button::R3,
        Button::LStick,
        Button::RStick,
        Button::Home,
    );

    info
}
