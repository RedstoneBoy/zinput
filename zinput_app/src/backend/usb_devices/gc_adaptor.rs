use std::{ops::ControlFlow, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use hidcon::gc_adaptor::{
    Button as HidButton, Buttons as HidButtons, Device as HidDevice, EP_IN, EP_OUT, INITIALIZE,
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

const T: &'static str = "backend:usb_devices:gc_adaptor";

pub(super) fn driver() -> UsbDriver {
    UsbDriver {
        filter: Box::new(filter),
        thread: <DeviceThread<GCDriver>>::new,
    }
}

fn filter(dev: &rusb::Device<rusb::GlobalContext>) -> bool {
    dev.device_descriptor()
        .ok()
        .map(|desc| desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID)
        .unwrap_or(false)
}

crate::device_bundle!(DeviceBundle(owned), controller: Controller);

struct GCDriver {
    packet: [u8; 37],
    engine: Arc<Engine>,
    device_id: u64,
    bundles: [Option<DeviceBundle<'static>>; 4],
    ids: [Option<String>; 4],
    device: HidDevice,
}

impl DeviceDriver for GCDriver {
    const NAME: &'static str = "Gamecube Adaptor";

    fn new(engine: &Arc<Engine>, device_id: u64) -> Self {
        GCDriver {
            packet: [0; 37],
            engine: engine.clone(),
            device_id,
            bundles: [None, None, None, None],
            ids: [None, None, None, None],
            device: HidDevice::default(),
        }
    }

    fn initialize(&mut self, handle: &mut DeviceHandle<GlobalContext>) -> Result<()> {
        if handle.write_interrupt(EP_OUT, &INITIALIZE, Duration::from_secs(5))
            .context("write interrupt error: is the correct driver installed for the device (i.e. using zadig)")?
            != 1
        {
            return Err(anyhow!("invalid size sent"));
        }

        let serial = handle
            .device()
            .device_descriptor()
            .and_then(|desc| handle.read_serial_number_string_ascii(&desc))
            .ok();

        self.ids = [0, 1, 2, 3].map(|i| {
            serial
                .as_ref()
                .map(|serial| format!("gc_adaptor/{}/{}", serial, i))
        });

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
        if size != 37 {
            return Ok(ControlFlow::Continue(()));
        }

        self.device.update(&self.packet)?;

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
                    return Ok(ControlFlow::Continue(()));
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
                        self.engine.clone(),
                        format!("Gamecube Adaptor {} Slot {}", self.device_id, i + 1),
                        self.ids[i].clone(),
                        [gc_controller_info()],
                    );

                    self.bundles[i] = Some(bundle);
                    self.bundles[i].as_mut().unwrap()
                }
                None => return Ok(ControlFlow::Continue(())),
            };

            if let Some(controller) = &self.device.controllers[i] {
                bundle.controller[0].buttons = convert_buttons(controller.buttons);
                // TODO: Change back
                bundle.controller[0].left_stick_x = ((((controller.left_stick.x as f32) - 127.0) * 1.25) + 127.0) as u8;
                bundle.controller[0].left_stick_y = ((((controller.left_stick.y as f32) - 127.0) * 1.25) + 127.0) as u8;
                bundle.controller[0].right_stick_x = controller.right_stick.x;
                bundle.controller[0].right_stick_y = controller.right_stick.y;
                bundle.controller[0].l2_analog = controller.left_trigger;
                bundle.controller[0].r2_analog = controller.right_trigger;

                bundle.update()?;
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}

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
    use Button::*;

    ControllerInfo {
        buttons: A | B | X | Y | Up | Down | Left | Right | Start | R1 | L2 | R2,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick()
    .with_l2_analog()
    .with_r2_analog()
}
