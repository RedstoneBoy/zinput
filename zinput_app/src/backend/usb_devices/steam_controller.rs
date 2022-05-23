use std::{convert::TryInto, sync::atomic::Ordering, time::Duration};

use anyhow::{Context, Result};
use zinput_engine::{
    device::component::{
        controller::{Button, Controller, ControllerInfo},
        motion::{Motion, MotionInfo},
        touch_pad::{TouchPad, TouchPadInfo, TouchPadShape},
    },
    Engine,
};

use super::{
    util::{self, UsbExt},
    ThreadData, UsbDriver,
};

const EP_IN: u8 = 0x82;

const VENDOR_ID: u16 = 0x28DE;
const PRODUCT_ID: u16 = 0x1142;

const DISABLE_LIZARD: [u8; 64] = [
    0x81, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const ENABLE_MOTION: [u8; 64] = [
    0x87, 0x15, 0x32, 0x84, 0x03, 0x18, 0x00, 0x00, 0x31, 0x02, 0x00, 0x08, 0x07, 0x00, 0x07, 0x07,
    0x00, 0x30, 0x18, 0x00, 0x2f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const T: &'static str = "backend:steam_controller";

pub(super) fn driver() -> UsbDriver {
    UsbDriver {
        filter: Box::new(filter),
        thread: new_controller_thread,
    }
}

fn filter(dev: &rusb::Device<rusb::GlobalContext>) -> bool {
    dev.device_descriptor()
        .ok()
        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
        .unwrap_or(false)
}

fn new_controller_thread(data: ThreadData) -> Box<dyn FnOnce() + Send> {
    Box::new(move || {
        let id = data.device_id;

        log::info!(target: T, "controller found, id: {}", id);
        match controller_thread(data) {
            Ok(()) => log::info!(target: T, "controller thread {} closed", id),
            Err(err) => log::error!(target: T, "controller thread {} crashed: {:#}", id, err),
        }
    })
}

fn controller_thread(
    ThreadData {
        device,
        device_id,
        stop,
        engine,
    }: ThreadData,
) -> Result<()> {
    let mut sc = device.open().context("failed to open device")?;

    match sc.set_auto_detach_kernel_driver(true) {
        Ok(()) => {}
        Err(rusb::Error::NotSupported) => {}
        Err(err) => {
            Err(err).context("failed to auto-detach kernel drivers")?;
        }
    }

    let iface = device.find_interface(util::hid_filter)?;

    sc.claim_interface(iface)
        .context("failed to claim interface")?;

    sc.write_control(
        0x21,
        0x09,
        0x300,
        1,
        &DISABLE_LIZARD,
        Duration::from_secs(3),
    )?;

    sc.write_control(
        0x21,
        0x09,
        0x0300,
        1,
        &ENABLE_MOTION,
        Duration::from_secs(3),
    )?;

    let mut bundle = SCBundle::new(device_id, &*engine);

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

crate::device_bundle!(DeviceBundle,
    controller: Controller,
    motion: Motion,
    touch_pad: TouchPad[2],
);

struct SCBundle<'a> {
    bundle: DeviceBundle<'a>,
}

impl<'a> SCBundle<'a> {
    fn new(adaptor_id: u64, engine: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            engine,
            format!("Steam Controller (Adaptor {})", adaptor_id),
            [sc_controller_info()],
            [MotionInfo::new(true, true)],
            [
                TouchPadInfo::new(TouchPadShape::Circle, true),
                TouchPadInfo::new(TouchPadShape::Circle, true),
            ],
        );

        SCBundle { bundle }
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

        self.bundle.update()?;

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
            self.bundle.controller[0].buttons & (1 << Button::LStick.bit())
        } else {
            self.bundle.controller[0].buttons
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
            SCButton::A,      Button::B;
            SCButton::X,      Button::Y;
            SCButton::B,      Button::A;
            SCButton::Y,      Button::X;
            SCButton::Lb,     Button::L1;
            SCButton::Rb,     Button::R1;
            SCButton::Lt,     Button::L2;
            SCButton::Rt,     Button::R2;
        );

        self.bundle.controller[0].buttons = new_buttons;

        self.bundle.controller[0].l2_analog = ltrig;
        self.bundle.controller[0].r2_analog = rtrig;
        self.bundle.controller[0].right_stick_x = ((rpad_x / 256) + 128) as u8;
        self.bundle.controller[0].right_stick_y = ((rpad_y / 256) + 128) as u8;
        if !lpad_touch {
            self.bundle.controller[0].left_stick_x = ((lpad_x / 256) + 128) as u8;
            self.bundle.controller[0].left_stick_y = ((lpad_y / 256) + 128) as u8;
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
        self.bundle.touch_pad[1].touch_x = (rpad_x as i32 - i16::MIN as i32) as u32 as u16;
        self.bundle.touch_pad[1].touch_y = (rpad_y as i32 - i16::MIN as i32) as u32 as u16;
        self.bundle.touch_pad[1].pressed = SCButton::RClick.is_pressed(buttons);
        self.bundle.touch_pad[1].touched = SCButton::RPadTouch.is_pressed(buttons);

        if SCButton::LPadTouch.is_pressed(buttons) {
            self.bundle.touch_pad[0].touched = true;
            self.bundle.touch_pad[0].pressed = SCButton::LClick.is_pressed(buttons);
            self.bundle.touch_pad[0].touch_x = (lpad_x as i32 - i16::MIN as i32) as u32 as u16;
            self.bundle.touch_pad[0].touch_y = (lpad_y as i32 - i16::MIN as i32) as u32 as u16;
        } else {
            self.bundle.touch_pad[0].pressed = false;
            self.bundle.touch_pad[0].touched = false;
            self.bundle.touch_pad[0].touch_x = u16::MAX / 2;
            self.bundle.touch_pad[0].touch_y = u16::MAX / 2;
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

        self.bundle.motion[0].accel_x = accelx as f32 * -ACCEL_SCALE;
        self.bundle.motion[0].accel_y = accelz as f32 * -ACCEL_SCALE;
        self.bundle.motion[0].accel_z = accely as f32 * ACCEL_SCALE;
        self.bundle.motion[0].gyro_pitch = gpitch as f32 * GYRO_SCALE;
        self.bundle.motion[0].gyro_roll = groll as f32 * GYRO_SCALE;
        self.bundle.motion[0].gyro_yaw = gyaw as f32 * GYRO_SCALE;
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
