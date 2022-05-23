use std::{sync::atomic::Ordering, time::Duration};

use anyhow::{Context, Result};
use hidcon::powera_pro_controller::{
    Button as HidButton, Buttons as HidButtons, Controller as HidController, DPad, EP_IN,
    PRODUCT_ID, VENDOR_ID,
};
use zinput_engine::{
    device::component::controller::{Button, Controller, ControllerInfo},
    Engine,
};

use super::{util::UsbExt, ThreadData, UsbDriver};

const T: &'static str = "backend:usb_devices:pa_switch";

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
        device_id,
        device,
        stop,
        engine,
    }: ThreadData,
) -> Result<()> {
    let mut pa = device.open().context("failed to open device")?;

    match pa.set_auto_detach_kernel_driver(true) {
        Ok(()) => {}
        Err(rusb::Error::NotSupported) => {}
        Err(err) => {
            Err(err).context("failed to auto-detach kernel drivers")?;
        }
    }

    let iface = device.find_interface(|_| true)?;

    pa.claim_interface(iface)
        .context("failed to claim interface")?;

    let mut bundle = PABundle::new(device_id, &*engine);

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

crate::device_bundle!(DeviceBundle, controller: Controller);

struct PABundle<'a> {
    bundle: DeviceBundle<'a>,
    controller: HidController,
}

impl<'a> PABundle<'a> {
    fn new(adaptor_id: u64, engine: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            engine,
            format!("PowerA Wired Pro Controller (Adaptor {})", adaptor_id),
            [controller_info()],
        );

        PABundle {
            bundle,
            controller: Default::default(),
        }
    }

    fn update(&mut self, packet: &[u8; 8]) -> Result<()> {
        self.controller.update(&packet)?;

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

        self.bundle.update()?;

        Ok(())
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
