use std::{ops::ControlFlow, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use hidcon::steam_controller::{
    Button as HidButton, Controller as HidController, DISABLE_LIZARD_MODE, ENABLE_MOTION, EP_IN,
    PRODUCT_ID_WIRELESS, VENDOR_ID,
};

use rusb::{Device, DeviceHandle, GlobalContext};
use zinput_engine::{
    device::component::{
        controller::{Button, Controller, ControllerInfo},
        motion::{Motion, MotionInfo},
        touch_pad::{TouchPad, TouchPadInfo, TouchPadShape},
    },
    Engine,
};

use super::{
    device_thread::{DeviceDriver, DeviceThread},
    util::{self, UsbExt},
    UsbDriver,
};

pub(super) fn driver() -> UsbDriver {
    UsbDriver {
        filter: Box::new(filter),
        thread: <DeviceThread<SCDriver>>::new,
    }
}

fn filter(dev: &rusb::Device<rusb::GlobalContext>) -> bool {
    dev.device_descriptor()
        .ok()
        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID_WIRELESS)
        .unwrap_or(false)
}

crate::device_bundle!(DeviceBundle (owned),
    controller: Controller,
    motion: Motion,
    touch_pad: TouchPad[2],
);

struct SCDriver {
    adaptor_id: u64,
    engine: Arc<Engine>,
    packet: [u8; 64],
    bundle: DeviceBundle<'static>,
    controller: HidController,
}

impl DeviceDriver for SCDriver {
    const NAME: &'static str = "Steam Controller";

    fn new(engine: &Arc<Engine>, adaptor_id: u64) -> Result<Self> {
        let bundle = DeviceBundle::new(
            engine.clone(),
            format!("Steam Controller {}", adaptor_id),
            None,
            true,
            [sc_controller_info()],
            [MotionInfo::new(true, true)],
            [
                TouchPadInfo::new(TouchPadShape::Circle, true),
                TouchPadInfo::new(TouchPadShape::Circle, true),
            ],
        )?;

        Ok(SCDriver {
            adaptor_id,
            engine: engine.clone(),
            packet: [0; 64],
            bundle,
            controller: Default::default(),
        })
    }

    fn open_device(
        &mut self,
        device: &Device<GlobalContext>,
    ) -> Result<DeviceHandle<GlobalContext>> {
        let mut handle = device.open().context("failed to open device")?;

        match handle.set_auto_detach_kernel_driver(true) {
            Ok(()) => {}
            Err(rusb::Error::NotSupported) => {}
            Err(err) => {
                Err(err).context("failed to auto-detach kernel drivers")?;
            }
        }

        let iface = device.find_interface(util::hid_filter)?;

        handle
            .claim_interface(iface)
            .context("failed to claim interface")?;

        Ok(handle)
    }

    fn initialize(&mut self, handle: &mut DeviceHandle<GlobalContext>) -> Result<()> {
        let id = handle
            .device()
            .device_descriptor()
            .and_then(|desc| handle.read_serial_number_string_ascii(&desc))
            .ok()
            .map(|mut serial| {
                serial.insert_str(0, "pa_switch/");
                serial
            });
        
        self.bundle = DeviceBundle::new(
            self.engine.clone(),
            format!("Steam Controller {}", self.adaptor_id),
            id,
            true,
            [sc_controller_info()],
            [MotionInfo::new(true, true)],
            [
                TouchPadInfo::new(TouchPadShape::Circle, true),
                TouchPadInfo::new(TouchPadShape::Circle, true),
            ],
        )?;

        handle.write_control(
            0x21,
            0x09,
            0x300,
            1,
            &DISABLE_LIZARD_MODE,
            Duration::from_secs(3),
        )?;

        handle.write_control(
            0x21,
            0x09,
            0x0300,
            1,
            &ENABLE_MOTION,
            Duration::from_secs(3),
        )?;

        Ok(())
    }

    fn update(&mut self, handle: &mut DeviceHandle<GlobalContext>) -> Result<ControlFlow<()>> {
        let size = match handle.read_interrupt(EP_IN, &mut self.packet, Duration::from_millis(2000))
        {
            Ok(size) => size,
            Err(rusb::Error::Timeout) => return Ok(ControlFlow::Continue(())),
            Err(rusb::Error::NoDevice) => return Ok(ControlFlow::Break(())),
            Err(err) => return Err(err.into()),
        };
        if size != 64 {
            return Ok(ControlFlow::Continue(()));
        }

        self.controller.update(&self.packet)?;

        self.update_controller();
        self.update_touch_pads();
        self.update_motion();

        self.bundle.update();

        Ok(ControlFlow::Continue(()))
    }
}

impl SCDriver {
    fn update_controller(&mut self) {
        let buttons = self.controller.buttons;
        let lpad_x = self.controller.left_pad.x;
        let lpad_y = self.controller.left_pad.y;
        let rpad_x = self.controller.right_pad.x;
        let rpad_y = self.controller.right_pad.y;
        let ltrig = self.controller.left_trigger;
        let rtrig = self.controller.right_trigger;

        macro_rules! convert {
            ($out:expr => $($hidbutton:expr, $button:expr $(, if $guard:expr)?);* $(;)?) => {
                $(if buttons.is_pressed($hidbutton) $(&& $guard)? {
                    $button.set_pressed($out);
                })*
            }
        }

        let lpad_touch = buttons.is_pressed(HidButton::LPadTouch);

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
            HidButton::RClick, Button::RStick;
            HidButton::LClick, Button::LStick, if !lpad_touch;
            HidButton::LClick, Button::Up,    if lpad_touch && lpad_y > 8192;
            HidButton::LClick, Button::Down,  if lpad_touch && lpad_y < -8192;
            HidButton::LClick, Button::Left,  if lpad_touch && lpad_x < -8192;
            HidButton::LClick, Button::Right, if lpad_touch && lpad_x > 8192;
            HidButton::RGrip,  Button::R3;
            HidButton::LGrip,  Button::L3;
            HidButton::Start,  Button::Start;
            HidButton::Steam,  Button::Home;
            HidButton::Back,   Button::Select;
            HidButton::A,      Button::B;
            HidButton::X,      Button::Y;
            HidButton::B,      Button::A;
            HidButton::Y,      Button::X;
            HidButton::Lb,     Button::L1;
            HidButton::Rb,     Button::R1;
            HidButton::Lt,     Button::L2;
            HidButton::Rt,     Button::R2;
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

    fn update_touch_pads(&mut self) {
        let buttons = self.controller.buttons;
        let lpad_x = self.controller.left_pad.x;
        let lpad_y = self.controller.left_pad.y;
        let rpad_x = self.controller.right_pad.x;
        let rpad_y = self.controller.right_pad.y;

        self.bundle.touch_pad[1].touch_x = (rpad_x as i32 - i16::MIN as i32) as u32 as u16;
        self.bundle.touch_pad[1].touch_y = (rpad_y as i32 - i16::MIN as i32) as u32 as u16;
        self.bundle.touch_pad[1].pressed = buttons.is_pressed(HidButton::RClick);
        self.bundle.touch_pad[1].touched = buttons.is_pressed(HidButton::RPadTouch);

        if buttons.is_pressed(HidButton::LPadTouch) {
            self.bundle.touch_pad[0].touched = true;
            self.bundle.touch_pad[0].pressed = buttons.is_pressed(HidButton::LClick);
            self.bundle.touch_pad[0].touch_x = (lpad_x as i32 - i16::MIN as i32) as u32 as u16;
            self.bundle.touch_pad[0].touch_y = (lpad_y as i32 - i16::MIN as i32) as u32 as u16;
        } else {
            self.bundle.touch_pad[0].pressed = false;
            self.bundle.touch_pad[0].touched = false;
            self.bundle.touch_pad[0].touch_x = u16::MAX / 2;
            self.bundle.touch_pad[0].touch_y = u16::MAX / 2;
        }
    }

    fn update_motion(&mut self) {
        const ACCEL_SCALE: f32 = 2.0 / 32768.0;
        const GYRO_SCALE: f32 = 2000.0 / 32768.0;

        self.bundle.motion[0].accel_x = self.controller.acceleration.x as f32 * -ACCEL_SCALE;
        self.bundle.motion[0].accel_y = self.controller.acceleration.y as f32 * -ACCEL_SCALE;
        self.bundle.motion[0].accel_z = self.controller.acceleration.z as f32 * ACCEL_SCALE;
        self.bundle.motion[0].gyro_pitch = self.controller.gyroscope.pitch as f32 * GYRO_SCALE;
        self.bundle.motion[0].gyro_roll = self.controller.gyroscope.roll as f32 * GYRO_SCALE;
        self.bundle.motion[0].gyro_yaw = self.controller.gyroscope.yaw as f32 * GYRO_SCALE;
    }
}

fn sc_controller_info() -> ControllerInfo {
    use Button::*;

    ControllerInfo {
        buttons: 0
            | A
            | B
            | X
            | Y
            | Up
            | Down
            | Left
            | Right
            | Start
            | Select
            | L1
            | R1
            | L2
            | R2
            | L3
            | R3
            | LStick
            | RStick
            | Home,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick()
    .with_l2_analog()
    .with_r2_analog()
}
