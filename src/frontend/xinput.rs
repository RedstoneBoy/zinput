use std::{sync::Arc, time::Duration};

use anyhow::Result;
use vigem::{Target, Vigem, XButton, XUSBReport};

use crate::{api::{Frontend, component::controller::{Button, Controller}}, zinput::engine::Engine};

const T: &'static str = "frontend:xinput";

pub struct XInput;

impl Frontend for XInput {
    fn init(&mut self, engine: Arc<Engine>) {
        std::thread::spawn(new_xinput_thread(engine));
    }
}

fn new_xinput_thread(engine: Arc<Engine>) -> impl FnOnce() {
    || {
        match xinput_thread(engine) {
            Ok(()) => log::info!(target: T, "xinput thread closed"),
            Err(e) => log::error!(target: T, "xinput thread crashed: {}", e),
        }
    }
}

fn xinput_thread(engine: Arc<Engine>) -> Result<()> {
    let mut vigem = Vigem::new();
    vigem.connect()?;

    let mut target = Target::new(vigem::TargetType::Xbox360);
    vigem.target_add(&mut target)?;

    loop {
        let controller_id;
        loop {
            let device = engine.devices().next();
            if let Some(device) = device {
                if let Some(new_controller_id) = &device.controller {
                    controller_id = *new_controller_id;
                    break;
                }
            }
        }

        let ticker = crossbeam_channel::tick(Duration::from_millis(16));
    
        loop {
            crossbeam_channel::select! {
                recv(ticker) -> _ => {
                    let controller = match engine.get_controller(&controller_id) {
                        Some(controller) => controller,
                        None => break,
                    };
                    
                    update_target(&mut vigem, &target, &controller.data)?;
                }
            }
        }
    }
}

fn update_target(vigem: &mut Vigem, target: &Target, data: &Controller) -> Result<()> {
    macro_rules! translate {
        ($data:expr, $($from:expr => $to:expr),* $(,)?) => {{
            XButton::empty()
            $(| if $from.is_pressed($data) { $to } else { XButton::Nothing })*
        }};
    }

    vigem.update(target, &XUSBReport {
        w_buttons: translate!(data.buttons,
            Button::A => XButton::A,
            Button::B => XButton::B,
            Button::X => XButton::X,
            Button::Y => XButton::Y,
            Button::Up => XButton::DpadUp,
            Button::Down => XButton::DpadDown,
            Button::Left => XButton::DpadLeft,
            Button::Right => XButton::DpadRight,
            Button::Start => XButton::Start,
            Button::Select => XButton::Back,
            Button::L1 => XButton::LeftShoulder,
            Button::R1 => XButton::RightShoulder,
            Button::LStick => XButton::LeftThumb,
            Button::RStick => XButton::RightThumb,
        ),
        b_left_trigger: if Button::L2.is_pressed(data.buttons) { 255 } else { 0 },
        b_right_trigger: if Button::R2.is_pressed(data.buttons) { 255 } else { 0 },
        s_thumb_lx: (((data.left_stick_x as i32) - 128) * 256) as i16,
        s_thumb_ly: (((data.left_stick_y as i32) - 128) * 256) as i16,
        s_thumb_rx: (((data.right_stick_x as i32) - 128) * 256) as i16,
        s_thumb_ry: (((data.right_stick_y as i32) - 128) * 256) as i16,
    })?;

    Ok(())
}