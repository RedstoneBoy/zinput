use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use vigem::{Target, Vigem, XButton, XUSBReport};
use zinput_engine::device::component::controller::{Button, Controller};
use zinput_engine::{
    eframe::{egui, epi},
    event::{Event, EventKind},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
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

impl Plugin for XInput {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine, self.signals.clone());
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status.lock().clone()
    }

    fn name(&self) -> &str {
        "xinput"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Frontend
    }

    fn events(&self) -> &[EventKind] {
        &[EventKind::DeviceUpdate]
    }

    fn update_gui(
        &self,
        ctx: &egui::CtxRef,
        frame: &mut epi::Frame<'_>,
        ui: &mut egui::Ui,
    ) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }

    fn on_event(&self, event: &Event) {
        match event {
            Event::DeviceUpdate(id) => {
                if self.signals.listen_update.lock().contains(id)
                    && !self.signals.update.0.is_full()
                {
                    // unwrap: the channel cannot become disconnected as it is Arc-owned by Self
                    self.signals.update.0.send(*id).unwrap();
                }
            }
            _ => {}
        }
    }
}

struct Inner {
    device: Sender<(usize, Option<Uuid>)>,
    device_recv: Receiver<(usize, Option<Uuid>)>,
    engine: Option<Arc<Engine>>,

    selected_devices: [Option<Uuid>; 4],

    status: Arc<Mutex<PluginStatus>>,

    stop: Arc<AtomicBool>,

    handle: Option<JoinHandle<()>>,
}

impl Inner {
    fn new() -> Self {
        let (device, device_recv) = crossbeam_channel::unbounded();
        Inner {
            device,
            device_recv,
            engine: None,

            selected_devices: [None; 4],

            status: Arc::new(Mutex::new(PluginStatus::Stopped)),

            stop: Arc::new(AtomicBool::new(false)),

            handle: None,
        }
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>, signals: Arc<Signals>) {
        self.engine = Some(engine.clone());

        *self.status.lock() = PluginStatus::Running;
        self.stop.store(false, Ordering::Release);

        self.handle = Some(std::thread::spawn(new_xinput_thread(Thread {
            engine,
            device_change: self.device_recv.clone(),
            signals,
            status: self.status.clone(),
            stop: self.stop.clone(),
        })));
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);

        if let Some(handle) = std::mem::replace(&mut self.handle, None) {
            match handle.join() {
                Ok(()) => {}
                Err(_) => log::error!(target: T, "error joining xinput thread"),
            }
        }
    }

    fn update_gui(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        ui: &mut egui::Ui,
    ) {
        if let Some(engine) = self.engine.clone() {
            for i in 0..self.selected_devices.len() {
                egui::ComboBox::from_label(format!("XInput Controller {}", i + 1))
                    .selected_text(
                        self.selected_devices[i]
                            .and_then(|id| engine.get_device_info(&id))
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
                                    Some(*device_ref.id()),
                                    &device_ref.name,
                                )
                                .clicked()
                            {
                                self.device.send((i, Some(*device_ref.id()))).unwrap();
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
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
}

fn new_xinput_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();
        match xinput_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "xinput thread closed");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(e) => {
                log::error!(target: T, "xinput thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("xinput thread crashed: {}", e));
            }
        }
    }
}

fn xinput_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_change,
        signals,
        stop,
        ..
    } = thread;

    let mut vigem = Vigem::new();
    vigem.connect()?;

    let mut ids: [Option<(Uuid, Target)>; 4] = [None, None, None, None];

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    Ok((idx, Some(device_id))) => {
                        if let Some((did, target)) = &mut ids[idx] {
                            vigem.target_remove(target)?;
                            signals.listen_update.lock().remove(did);
                        }

                        let mut target = Target::new(vigem::TargetType::Xbox360);
                        vigem.target_add(&mut target)?;
                        ids[idx] = Some((device_id, target));
                        signals.listen_update.lock().insert(device_id);
                    }
                    Ok((idx, None)) => {
                        if let Some((did, target)) = ids[idx].as_mut() {
                            vigem.target_remove(target)?;
                            signals.listen_update.lock().remove(did);
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
                    if let Some((did, target)) = bundle.as_ref() {
                        let device = match engine.get_device(did) {
                            Some(device) => device,
                            None => continue,
                        };
                        let controller = match device.controllers.get(0) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        update_target(&mut vigem, &target, controller)?;
                    }
                }
            }
            default(Duration::from_secs(1)) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
            }
        }
    }

    Ok(())
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
