use std::{sync::atomic::Ordering, time::Duration};

use anyhow::{Context, Result};
use zinput_engine::{
    device::component::controller::{Button, Controller, ControllerInfo},
    Engine,
};

use super::{util::UsbExt, ThreadData, UsbDriver};

const EP_IN: u8 = 0x81;

const VENDOR_ID: u16 = 0x20D6;
const PRODUCT_ID: u16 = 0xA713;

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
}

impl<'a> PABundle<'a> {
    fn new(adaptor_id: u64, engine: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            engine,
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
