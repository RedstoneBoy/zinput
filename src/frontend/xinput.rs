use std::{collections::HashSet, sync::Arc};

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
    signals: Arc<Signals>,
}

impl XInput {
    pub fn new() -> Self {
        XInput {
            inner: Mutex::new(Inner::new()),
            signals: Arc::new(Signals::new()),
        }
    }
}

impl Frontend for XInput {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine, self.signals.clone());
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

    fn on_component_update(&self, id: &Uuid) {
        if self.signals.listen_update.lock().contains(id) && !self.signals.update.0.is_full() {
            // unwrap: the channel cannot become disconnected as it is Arc-owned by Self
            self.signals.update.0.send(*id).unwrap();
        }
    }
}

struct Inner {
    device: Sender<(usize, Option<Uuid>)>,
    device_recv: Option<Receiver<(usize, Option<Uuid>)>>,
    engine: Option<Arc<Engine>>,

    selected_devices: [Option<Uuid>; 4],
}

impl Inner {
    fn new() -> Self {
        let (device, device_recv) = crossbeam_channel::unbounded();
        Inner {
            device,
            device_recv: Some(device_recv),
            engine: None,

            selected_devices: [None; 4],
        }
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>, signals: Arc<Signals>) {
        self.engine = Some(engine.clone());
        std::thread::spawn(new_xinput_thread(Thread {
            engine,
            device_change: std::mem::replace(&mut self.device_recv, None).unwrap(),
            signals,
        }));
    }

    fn update_gui(
        &mut self,
        _ctx: &eframe::egui::CtxRef,
        _frame: &mut eframe::epi::Frame<'_>,
        ui: &mut eframe::egui::Ui,
    ) {
        if let Some(engine) = self.engine.clone() {
            for i in 0..self.selected_devices.len() {
                egui::ComboBox::from_label(format!("XInput Controller {}", i + 1))
                    .selected_text(
                        self.selected_devices[i]
                            .and_then(|id| engine.get_device(&id))
                            .map_or("[None]".to_owned(), |dev| dev.name.clone()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(&mut self.selected_devices[i], None, "[None]")
                            .clicked()
                        {
                            self.device.send((i, None)).unwrap();
                        }
                        for device_ref in engine.devices() {
                            if ui
                                .selectable_value(
                                    &mut self.selected_devices[i],
                                    Some(*device_ref.key()),
                                    &device_ref.name,
                                )
                                .clicked()
                            {
                                self.device.send((i, Some(*device_ref.key()))).unwrap();
                            }
                        }
                    });
            }
        }
    }
}

struct Signals {
    listen_update: Mutex<HashSet<Uuid>>,
    update: (Sender<Uuid>, Receiver<Uuid>),
}

impl Signals {
    fn new() -> Self {
        Signals {
            listen_update: Mutex::new(HashSet::new()),
            update: crossbeam_channel::bounded(4),
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_change: Receiver<(usize, Option<Uuid>)>,
    signals: Arc<Signals>,
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
        signals,
    } = thread;

    let mut vigem = Vigem::new();
    vigem.connect()?;

    let mut ids: [Option<(Uuid, Uuid, Target)>; 4] = [None, None, None, None];

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    Ok((idx, Some(device_id))) => {
                        if let Some((_, cid, target)) = &mut ids[idx] {
                            vigem.target_remove(target)?;
                            signals.listen_update.lock().remove(cid);
                        }

                        if let Some(controller_id) = engine.get_device(&device_id)
                            .and_then(|device| device.controller)
                        {
                            let mut target = Target::new(vigem::TargetType::Xbox360);
                            vigem.target_add(&mut target)?;
                            ids[idx] = Some((device_id, controller_id, target));
                            signals.listen_update.lock().insert(controller_id);
                        }
                    }
                    Ok((idx, None)) => {
                        if let Some((_, cid, target)) = ids[idx].as_mut() {
                            vigem.target_remove(target)?;
                            signals.listen_update.lock().remove(cid);
                        }
                        ids[idx] = None;
                    }
                    Err(_) => {
                        // todo
                    }
                }
            },
            recv(signals.update.1) -> _ => {
                for bundle in &ids {
                    if let Some((_, cid, target)) = bundle.as_ref() {
                        let controller = match engine.get_controller(cid) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        update_target(&mut vigem, &target, &controller.data)?;
                    }
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

    // TODO: Fix triggers

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
                Button::Home => XButton::Guide,
            ),
            b_left_trigger: if Button::L2.is_pressed(data.buttons) {
                255
            } else {
                data.l2_analog
            },
            b_right_trigger: if Button::R2.is_pressed(data.buttons) {
                255
            } else {
                data.r2_analog
            },
            s_thumb_lx: (((data.left_stick_x as i32) - 128) * 256) as i16,
            s_thumb_ly: (((data.left_stick_y as i32) - 128) * 256) as i16,
            s_thumb_rx: (((data.right_stick_x as i32) - 128) * 256) as i16,
            s_thumb_ry: (((data.right_stick_y as i32) - 128) * 256) as i16,
        },
    )?;

    Ok(())
}
