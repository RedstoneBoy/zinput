use std::{ops::ControlFlow, sync::Arc, time::Duration};

use anyhow::Result;
use hidcon::powera_pro_controller::{
    Button as HidButton, Buttons as HidButtons, Controller as HidController, DPad, EP_IN,
    PRODUCT_ID, VENDOR_ID,
};
use rusb::{DeviceHandle, GlobalContext};
use zinput_engine::{
    device::component::controller::{Button, Controller, ControllerInfo},
    Engine,
};

use super::{
    device_thread::{DeviceDriver, DeviceThread},
    UsbDriver,
};

pub(super) fn driver() -> UsbDriver {
    UsbDriver {
        filter: Box::new(filter),
        thread: <DeviceThread<PADriver>>::new,
    }
}

fn filter(dev: &rusb::Device<GlobalContext>) -> bool {
    dev.device_descriptor()
        .ok()
        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
        .unwrap_or(false)
}

crate::device_bundle!(DeviceBundle(owned), controller: Controller);

struct PADriver {
    id: u64,
    engine: Arc<Engine>,
    packet: [u8; 8],
    bundle: DeviceBundle<'static>,
    controller: HidController,
}

impl DeviceDriver for PADriver {
    const NAME: &'static str = "PowerA Wired Pro Controller";

    fn new(engine: &Arc<Engine>, id: u64) -> Result<Self> {
        let bundle = DeviceBundle::new(
            engine.clone(),
            format!("PowerA Wired Pro Controller {}", id),
            None,
            [controller_info()],
        )?;

        Ok(PADriver {
            id,
            engine: engine.clone(),
            packet: [0; 8],
            bundle,
            controller: Default::default(),
        })
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
            format!("PowerA Wired Pro Controller {}", self.id),
            id,
            [controller_info()],
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
        if size != 8 {
            return Ok(ControlFlow::Continue(()));
        }

        self.controller.update(&self.packet)?;

        self.bundle.controller[0].buttons =
            convert_buttons(self.controller.buttons, self.controller.dpad);
        self.bundle.controller[0].l2_analog =
            (self.controller.buttons.is_pressed(HidButton::L2) as u8) * 255;
        self.bundle.controller[0].r2_analog =
            (self.controller.buttons.is_pressed(HidButton::R2) as u8) * 255;
        self.bundle.controller[0].left_stick_x = self.controller.left_stick.x;
        self.bundle.controller[0].left_stick_y = 255 - self.controller.left_stick.y;
        self.bundle.controller[0].right_stick_x = self.controller.right_stick.x;
        self.bundle.controller[0].right_stick_y = 255 - self.controller.right_stick.y;

        self.bundle.update();

        Ok(ControlFlow::Continue(()))
    }
}

fn convert_buttons(buttons: HidButtons, dpad: DPad) -> u64 {
    let mut out = 0u64;

    macro_rules! convert {
        ($($hidbutton:expr, $button:expr);* $(;)?) => {
            $(if buttons.is_pressed($hidbutton) {
                $button.set_pressed(&mut out);
            })*
        }
    }

    convert!(
        HidButton::Y,       Button::Y;
        HidButton::B,       Button::B;
        HidButton::A,       Button::A;
        HidButton::X,       Button::X;
        HidButton::L1,      Button::L1;
        HidButton::R1,      Button::R1;
        HidButton::L2,      Button::L2;
        HidButton::R2,      Button::R2;
        HidButton::Select,  Button::Select;
        HidButton::Start,   Button::Start;
        HidButton::LStick,  Button::LStick;
        HidButton::RStick,  Button::RStick;
        HidButton::Home,    Button::Home;
        HidButton::Capture, Button::Capture;
    );

    if dpad.up() {
        Button::Up.set_pressed(&mut out);
    }
    if dpad.down() {
        Button::Down.set_pressed(&mut out);
    }
    if dpad.left() {
        Button::Left.set_pressed(&mut out);
    }
    if dpad.right() {
        Button::Right.set_pressed(&mut out);
    }

    out
}

fn controller_info() -> ControllerInfo {
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
            | LStick
            | RStick
            | Home
            | Capture,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick()
}
