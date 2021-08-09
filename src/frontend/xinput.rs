use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use parking_lot::Mutex;
use uuid::Uuid;
use vigem::{Target, Vigem, XButton, XUSBReport};

use crate::{
    api::{
        component::controller::{Button, Controller},
        Frontend,
    },
    zinput::engine::Engine,
};

const T: &'static str = "frontend:xinput";

pub struct XInput {
    inner: Mutex<Inner>,
}

impl XInput {
    pub fn new() -> Self {
        XInput {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Frontend for XInput {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine);
    }

    fn name(&self) -> &str {
        "xinput"
    }

    fn update_gui(
        &self,
        ctx: &eframe::egui::CtxRef,
        frame: &mut eframe::epi::Frame<'_>,
        ui: &mut eframe::egui::Ui,
    ) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }
}

struct Inner {
    device: Sender<Uuid>,
    device_recv: Option<Receiver<Uuid>>,
    engine: Option<Arc<Engine>>,

    selected_device: Option<Uuid>,
}

impl Inner {
    fn new() -> Self {
        let (device, device_recv) = crossbeam_channel::unbounded();
        Inner {
            device,
            device_recv: Some(device_recv),
            engine: None,

            selected_device: None,
        }
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>) {
        self.engine = Some(engine.clone());
        std::thread::spawn(new_xinput_thread(Thread {
            engine,
            device_change: std::mem::replace(&mut self.device_recv, None).unwrap(),
        }));
    }

    fn update_gui(
        &mut self,
        _ctx: &eframe::egui::CtxRef,
        _frame: &mut eframe::epi::Frame<'_>,
        ui: &mut eframe::egui::Ui,
    ) {
        if let Some(engine) = self.engine.clone() {
            egui::ComboBox::from_label("Devices")
                .selected_text(
                    self.selected_device
                        .and_then(|id| engine.get_device(&id))
                        .map_or("".to_owned(), |dev| dev.name.clone()),
                )
                .show_ui(ui, |ui| {
                    for device_ref in engine.devices() {
                        if ui
                            .selectable_value(
                                &mut self.selected_device,
                                Some(*device_ref.key()),
                                &device_ref.name,
                            )
                            .clicked()
                        {
                            self.device.send(*device_ref.key()).unwrap();
                        }
                    }
                });
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_change: Receiver<Uuid>,
}

fn new_xinput_thread(thread: Thread) -> impl FnOnce() {
    || match xinput_thread(thread) {
        Ok(()) => log::info!(target: T, "xinput thread closed"),
        Err(e) => log::error!(target: T, "xinput thread crashed: {}", e),
    }
}

fn xinput_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_change,
    } = thread;

    let mut vigem = Vigem::new();
    vigem.connect()?;

    let mut target = Target::new(vigem::TargetType::Xbox360);
    vigem.target_add(&mut target)?;

    let mut device_id = None;

    loop {
        let cur_device_id = match device_id {
            Some(id) => id,
            None => loop {
                match device_change.try_recv() {
                    Ok(id) => {
                        break id;
                    }
                    Err(_) => {}
                }

                let device = engine.devices().next();
                if let Some(device) = device {
                    break *device.key();
                } else {
                    continue;
                }
            },
        };

        let controller_id = match engine
            .get_device(&cur_device_id)
            .and_then(|device| device.controller)
        {
            Some(id) => id,
            None => {
                device_id = None;
                continue;
            }
        };

        let update_recv = engine.add_update_channel(&controller_id);

        loop {
            crossbeam_channel::select! {
                recv(device_change) -> id => {
                    device_id = match id {
                        Ok(id) => Some(id),
                        Err(_) => None,
                    };
                    break;
                }
                recv(update_recv) -> _ => {
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

    vigem.update(
        target,
        &XUSBReport {
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
            b_left_trigger: if Button::L2.is_pressed(data.buttons) {
                255
            } else {
                0
            },
            b_right_trigger: if Button::R2.is_pressed(data.buttons) {
                255
            } else {
                0
            },
            s_thumb_lx: (((data.left_stick_x as i32) - 128) * 256) as i16,
            s_thumb_ly: (((data.left_stick_y as i32) - 128) * 256) as i16,
            s_thumb_rx: (((data.right_stick_x as i32) - 128) * 256) as i16,
            s_thumb_ry: (((data.right_stick_y as i32) - 128) * 256) as i16,
        },
    )?;

    Ok(())
}
