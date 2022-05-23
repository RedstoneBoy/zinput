use std::{sync::atomic::Ordering, time::Duration};

use anyhow::{anyhow, Context, Result};
use hidcon::gc_adaptor::{
    Button as HidButton, Buttons as HidButtons, Device as HidDevice, EP_IN, EP_OUT, INITIALIZE,
    PRODUCT_ID, VENDOR_ID,
};
use zinput_engine::{
    device::component::controller::{Button, Controller, ControllerInfo},
    Engine,
};

use super::{util::UsbExt, ThreadData, UsbDriver};

const T: &'static str = "backend:usb_devices:gc_adaptor";

pub(super) fn driver() -> UsbDriver {
    UsbDriver {
        filter: Box::new(filter),
        thread: new_adaptor_thread,
    }
}

fn filter(dev: &rusb::Device<rusb::GlobalContext>) -> bool {
    dev.device_descriptor()
        .ok()
        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
        .unwrap_or(false)
}

fn new_adaptor_thread(data: ThreadData) -> Box<dyn FnOnce() + Send> {
    Box::new(move || {
        let id = data.device_id;

        log::info!(target: T, "adaptor found, id: {}", id);
        match adaptor_thread(data) {
            Ok(()) => log::info!(target: T, "adaptor thread {} closed", id),
            Err(err) => log::error!(target: T, "adaptor thread {} crashed: {:#}", id, err),
        }
    })
}

fn adaptor_thread(
    ThreadData {
        device,
        device_id,
        stop,
        engine,
    }: ThreadData,
) -> Result<()> {
    let mut adaptor = device.open().context("failed to open device")?;

    match adaptor.set_auto_detach_kernel_driver(true) {
        Ok(()) => {}
        Err(rusb::Error::NotSupported) => {}
        Err(err) => {
            Err(err).context("failed to auto-detach kernel drivers")?;
        }
    }

    let iface = device.find_interface(|_| true)?;

    adaptor
        .claim_interface(iface)
        .context("failed to claim interface")?;

    if adaptor.write_interrupt(EP_OUT, &INITIALIZE, Duration::from_secs(5))
        .context("write interrupt error: is the correct driver installed for the device (i.e. using zadig)")?
        != 1
    {
        return Err(anyhow!("invalid size sent"));
    }

    let mut ctrls = Controllers::new(device_id, &*engine);

    let mut payload = [0u8; 37];

    while !stop.load(Ordering::Acquire) {
        let size = match adaptor.read_interrupt(EP_IN, &mut payload, Duration::from_millis(2000)) {
            Ok(size) => size,
            Err(rusb::Error::NoDevice) => break,
            Err(err) => return Err(err.into()),
        };
        if size != 37 {
            continue;
        }

        ctrls.update(&payload)?;
    }

    Ok(())
}

struct Controllers<'a> {
    engine: &'a Engine,
    device_id: u64,
    bundles: [Option<DeviceBundle<'a>>; 4],
    device: HidDevice,
}

impl<'a> Controllers<'a> {
    fn new(device_id: u64, api: &'a Engine) -> Self {
        Controllers {
            engine: api,
            device_id,
            bundles: [None, None, None, None],
            device: HidDevice::default(),
        }
    }

    fn update(&mut self, packet: &[u8; 37]) -> Result<()> {
        self.device.update(&packet)?;

        for i in 0..4 {
            let is_active = self.device.controllers[i].is_some();

            let bundle = match &mut self.bundles[i] {
                Some(_) if !is_active => {
                    // remove device
                    log::info!(
                        target: T,
                        "removing slot {} from adaptor {}",
                        i + 1,
                        self.device_id
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
                        self.device_id
                    );

                    let bundle = DeviceBundle::new(
                        self.engine,
                        format!(
                            "Gamecube Adaptor Slot {} (Adaptor {})",
                            i + 1,
                            self.device_id
                        ),
                        [gc_controller_info()],
                    );

                    self.bundles[i] = Some(bundle);
                    self.bundles[i].as_mut().unwrap()
                }
                None => continue,
            };

            if let Some(controller) = &self.device.controllers[i] {
                bundle.controller[0].buttons = convert_buttons(controller.buttons);
                bundle.controller[0].left_stick_x = controller.left_stick.x;
                bundle.controller[0].left_stick_y = controller.left_stick.y;
                bundle.controller[0].right_stick_x = controller.right_stick.x;
                bundle.controller[0].right_stick_y = controller.right_stick.y;
                bundle.controller[0].l2_analog = controller.left_trigger;
                bundle.controller[0].r2_analog = controller.right_trigger;

                bundle.update()?;
            }
        }

        Ok(())
    }
}

crate::device_bundle!(DeviceBundle, controller: Controller);

fn convert_buttons(buttons: HidButtons) -> u64 {
    let mut out = 0u64;

    macro_rules! convert {
        ($($hidbutton:expr, $button:expr);* $(;)?) => {
            $(if buttons.is_pressed($hidbutton) {
                $button.set_pressed(&mut out);
            })*
        }
    }

    convert!(
        HidButton::Start, Button::Start;
        HidButton::Z,     Button::R1;
        HidButton::R,     Button::R2;
        HidButton::L,     Button::L2;
        HidButton::A,     Button::A;
        HidButton::B,     Button::B;
        HidButton::X,     Button::X;
        HidButton::Y,     Button::Y;
        HidButton::Left,  Button::Left;
        HidButton::Right, Button::Right;
        HidButton::Down,  Button::Down;
        HidButton::Up,    Button::Up;
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
