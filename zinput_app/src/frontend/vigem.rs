use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use vigem_client::{
    Client, DS4Report, DualShock4Wired, TargetId, XButtons, XGamepad, Xbox360Wired,
};
use zinput_engine::{
    device::{component::controller::{Button, Controller}, DeviceInfo},
    eframe::{egui, epi},
    event::{Event, EventKind},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
};

const T: &'static str = "frontend:vigem";

pub struct Vigem {
    inner: Mutex<Inner>,
    signals: Arc<Signals>,
}

impl Vigem {
    pub fn new() -> Self {
        Vigem {
            inner: Mutex::new(Inner::new()),
            signals: Arc::new(Signals::new()),
        }
    }
}

impl Plugin for Vigem {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine, self.signals.clone());
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "vigem"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Frontend
    }

    fn events(&self) -> &[EventKind] {
        &[EventKind::DeviceUpdate]
    }

    fn update_gui(&self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>, ui: &mut egui::Ui) {
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
            Event::DeviceAdded(id, info) => self.inner.lock().device_added(id, info),
            Event::DeviceRemoved(id) => self.inner.lock().device_removed(id),
            _ => {}
        }
    }
}

enum Inner {
    Uninit,
    Init {
        engine: Arc<Engine>,
        status: Arc<Mutex<PluginStatus>>,
        stop: Arc<AtomicBool>,
        handle: JoinHandle<()>,

        xbox_send: Sender<Vec<Uuid>>,
        selected_xbox: Vec<Uuid>,
        xbox_ids: Vec<Option<String>>,

        ds4_send: Sender<Vec<Uuid>>,
        selected_ds4: Vec<Uuid>,
    },
}

impl Inner {
    fn new() -> Self {
        Inner::Uninit
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>, signals: Arc<Signals>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));

        let (xbox_send, xbox_recv) = crossbeam_channel::unbounded();
        let (ds4_send, ds4_recv) = crossbeam_channel::unbounded();

        let handle = std::thread::spawn(new_vigem_thread(Thread {
            engine: engine.clone(),
            xbox_recv,
            ds4_recv,
            signals,
            status: status.clone(),
            stop: stop.clone(),
        }));

        *self = Inner::Init {
            engine,
            status,
            stop,
            handle,

            xbox_send,
            selected_xbox: Vec::new(),
            xbox_ids: Vec::new(),

            ds4_send,
            selected_ds4: Vec::new(),
        };
    }

    fn stop(&mut self) {
        match std::mem::replace(self, Inner::Uninit) {
            Inner::Uninit => {}
            Inner::Init {
                handle,
                status,
                stop,
                ..
            } => {
                stop.store(true, Ordering::Release);

                match handle.join() {
                    Ok(()) => {}
                    Err(_) => log::info!(target: T, "driver panicked"),
                }

                *status.lock() = PluginStatus::Stopped;
            }
        }
    }

    fn status(&self) -> PluginStatus {
        match self {
            Inner::Uninit => PluginStatus::Stopped,
            Inner::Init { status, .. } => status.lock().clone(),
        }
    }

    fn update_gui(&mut self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, ui: &mut egui::Ui) {
        let Inner::Init {
            engine,
            xbox_send,
            selected_xbox,
            ds4_send,
            selected_ds4,
            ..
        } = self
        else { return };

        #[derive(PartialEq, Eq)]
        enum Action {
            Remove(usize),
            Change(usize, Uuid),
            Add(Uuid),
        }

        let mut action = None;

        // XBox Controllers

        for i in 0..selected_xbox.len() {
            if action.is_some() {
                break;
            }
            egui::ComboBox::from_label(format!("ViGEm XBox Controller {}", i + 1))
                .selected_text(match engine.get_device_info(&selected_xbox[i]) {
                    Some(info) => info.name.clone(),
                    None => {
                        action = Some(Action::Remove(i));
                        break;
                    }
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut action, Some(Action::Remove(i)), "[None]");
                    for device_ref in engine.devices() {
                        ui.selectable_value(
                            &mut action,
                            Some(Action::Change(i, *device_ref.id())),
                            &device_ref.name,
                        );
                    }
                });
        }

        if selected_xbox.len() < 4 && action.is_none() {
            egui::ComboBox::from_label(format!(
                "ViGEm XBox Controller {}",
                selected_xbox.len() + 1
            ))
            .selected_text("[None]")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut action, None, "[None]");
                for device_ref in engine.devices() {
                    ui.selectable_value(
                        &mut action,
                        Some(Action::Add(*device_ref.id())),
                        &device_ref.name,
                    );
                }
            });
        }

        if let Some(action) = action {
            match action {
                Action::Remove(i) => {
                    selected_xbox.remove(i);
                }
                Action::Change(i, id) => selected_xbox[i] = id,
                Action::Add(id) => selected_xbox.push(id),
            }

            xbox_send.send(selected_xbox.clone()).unwrap();
        }

        ui.separator();

        // DS4 Controllers

        action = None;

        for i in 0..selected_ds4.len() {
            if action.is_some() {
                break;
            }
            egui::ComboBox::from_label(format!("ViGEm DS4 Controller {}", i + 1))
                .selected_text(match engine.get_device_info(&selected_ds4[i]) {
                    Some(info) => info.name.clone(),
                    None => {
                        action = Some(Action::Remove(i));
                        break;
                    }
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut action, Some(Action::Remove(i)), "[None]");
                    for device_ref in engine.devices() {
                        ui.selectable_value(
                            &mut action,
                            Some(Action::Change(i, *device_ref.id())),
                            &device_ref.name,
                        );
                    }
                });
        }

        if selected_ds4.len() < u16::MAX as usize - 1 && action.is_none() {
            egui::ComboBox::from_label(format!("ViGEm DS4 Controller {}", selected_ds4.len() + 1))
                .selected_text("[None]")
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut action, None, "[None]");
                    for device_ref in engine.devices() {
                        ui.selectable_value(
                            &mut action,
                            Some(Action::Add(*device_ref.id())),
                            &device_ref.name,
                        );
                    }
                });
        }

        if let Some(action) = action {
            match action {
                Action::Remove(i) => {
                    selected_ds4.remove(i);
                }
                Action::Change(i, id) => selected_ds4[i] = id,
                Action::Add(id) => selected_ds4.push(id),
            }

            ds4_send.send(selected_ds4.clone()).unwrap();
        }
    }

    fn device_added(&mut self, id: &Uuid, info: &DeviceInfo) {
        let Inner::Init {
            engine,
            xbox_send,
            selected_xbox,
            ds4_send,
            selected_ds4,
            ..
        } = self
        else { return };
    }

    fn device_removed(&mut self, id: &Uuid) {
        let Inner::Init {
            engine,
            xbox_send,
            selected_xbox,
            ds4_send,
            selected_ds4,
            ..
        } = self
        else { return };
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
    xbox_recv: Receiver<Vec<Uuid>>,
    ds4_recv: Receiver<Vec<Uuid>>,
    signals: Arc<Signals>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
}

fn new_vigem_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();
        match vigem_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "vigem thread closed");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(e) => {
                log::error!(target: T, "vigem thread crashed: {:?}", e);
                *status.lock() = PluginStatus::Error(format!("vigem thread crashed: {}", e));
            }
        }
    }
}

fn vigem_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        xbox_recv,
        ds4_recv,
        signals,
        stop,
        ..
    } = thread;

    let vigem = Client::connect()?;

    let mut ds4_targets = Vec::<(Uuid, DualShock4Wired<_>)>::new();
    let mut xbox_targets = Vec::<(Uuid, Xbox360Wired<_>)>::new();

    loop {
        crossbeam_channel::select! {
            recv(xbox_recv) -> xbox_recv => {
                let Ok(xbox_ids) = xbox_recv
                else { return Ok(()); }; // Sender dropped which means plugin is uninitialized

                if xbox_ids.len() < xbox_targets.len() {
                    for (_, xbox_target) in &mut xbox_targets[xbox_ids.len()..] {
                        xbox_target.unplug().context("failed to unplug xbox target")?;
                    }
                    for _ in xbox_ids.len()..xbox_targets.len() {
                        xbox_targets.pop();
                    }
                } else if xbox_ids.len() > xbox_targets.len() {
                    for i in xbox_targets.len()..xbox_ids.len() {
                        let mut xbox = Xbox360Wired::new(&vigem, TargetId::XBOX360_WIRED);
                        xbox.plugin().context("failed to plugin xbox target")?;
                        xbox.wait_ready().context("xbox target failed to ready")?;
                        xbox_targets.push((xbox_ids[i], xbox));
                    }
                }

                {
                    let mut listen_to = signals.listen_update.lock();
                    listen_to.clear();
                    for (id, _) in &xbox_targets {
                        listen_to.insert(*id);
                    }
                    for (id, _) in &ds4_targets {
                        listen_to.insert(*id);
                    }
                }
            },
            recv(ds4_recv) -> ds4_recv => {
                let Ok(ds4_ids) = ds4_recv
                else { return Ok(()); }; // Sender dropped which means plugin is uninitialized

                if ds4_ids.len() < ds4_targets.len() {
                    for (_, ds4_target) in &mut ds4_targets[ds4_ids.len()..] {
                        ds4_target.unplug().context("failed to unplug ds4 target")?;
                    }
                    for _ in ds4_ids.len()..ds4_targets.len() {
                        ds4_targets.pop();
                    }
                } else if ds4_ids.len() > ds4_targets.len() {
                    for i in ds4_targets.len()..ds4_ids.len() {
                        let mut ds4 = DualShock4Wired::new(&vigem, TargetId::DUALSHOCK4_WIRED);
                        ds4.plugin().context("failed to plugin ds4 target")?;
                        ds4.wait_ready().context("ds4 target failed to ready")?;
                        ds4_targets.push((ds4_ids[i], ds4));
                    }
                }

                {
                    let mut listen_to = signals.listen_update.lock();
                    listen_to.clear();
                    for (id, _) in &xbox_targets {
                        listen_to.insert(*id);
                    }
                    for (id, _) in &ds4_targets {
                        listen_to.insert(*id);
                    }
                }
            },
            recv(signals.update.1) -> rid => {
                if let Ok(rid) = rid {
                    for (id, target) in &mut xbox_targets {
                        if id != &rid { continue; }
                        let device = match engine.get_device(id) {
                            Some(device) => device,
                            None => continue,
                        };
                        let controller = match device.controllers.get(0) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        update_xbox_target(target, controller).with_context(|| "failed to update xbox target")?;
                    }

                    for (id, target) in &mut ds4_targets {
                        if id != &rid { continue; }
                        let device = match engine.get_device(id) {
                            Some(device) => device,
                            None => continue,
                        };
                        let controller = match device.controllers.get(0) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        update_ds4_target(target, controller).with_context(|| "failed to update ds4 target")?;
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

fn update_xbox_target(target: &mut Xbox360Wired<&Client>, data: &Controller) -> Result<()> {
    macro_rules! translate {
        ($data:expr, $($from:expr => $to:expr),* $(,)?) => {{
            XButtons {
                raw: 0 $(| if $from.is_pressed($data) { $to } else { 0 })*
            }
        }};
    }

    target.update(&XGamepad {
        buttons: translate!(data.buttons,
            Button::A => XButtons::A,
            Button::B => XButtons::B,
            Button::X => XButtons::X,
            Button::Y => XButtons::Y,
            Button::Up => XButtons::UP,
            Button::Down => XButtons::DOWN,
            Button::Left => XButtons::LEFT,
            Button::Right => XButtons::RIGHT,
            Button::Start => XButtons::START,
            Button::Select => XButtons::BACK,
            Button::L1 => XButtons::LB,
            Button::R1 => XButtons::RB,
            Button::LStick => XButtons::LTHUMB,
            Button::RStick => XButtons::RTHUMB,
            Button::Home => XButtons::GUIDE,
        ),
        left_trigger: if Button::L2.is_pressed(data.buttons) {
            255
        } else {
            data.l2_analog
        },
        right_trigger: if Button::R2.is_pressed(data.buttons) {
            255
        } else {
            data.r2_analog
        },
        thumb_lx: (((data.left_stick_x as i32) - 128) * 256) as i16,
        thumb_ly: (((data.left_stick_y as i32) - 128) * 256) as i16,
        thumb_rx: (((data.right_stick_x as i32) - 128) * 256) as i16,
        thumb_ry: (((data.right_stick_y as i32) - 128) * 256) as i16,
    })?;

    Ok(())
}

fn update_ds4_target(target: &mut DualShock4Wired<&Client>, data: &Controller) -> Result<()> {
    enum DS4Buttons {
        Square = 4,
        Cross = 5,
        Circle = 6,
        Triangle = 7,
        LB = 8,
        RB = 9,
        LT = 10,
        RT = 11,
        Share = 12,
        Options = 13,
        LStick = 14,
        RStick = 15,
    }

    enum DS4Special {
        PS = 0,
        // TouchPad = 1,
    }

    enum DS4DPad {
        None = 8,
        NW = 7,
        W = 6,
        SW = 5,
        S = 4,
        SE = 3,
        E = 2,
        NE = 1,
        N = 0,
    }

    macro_rules! translate {
        ($data:expr, $($from:expr => $to:expr),* $(,)?) => {{
            0 $(| if $from.is_pressed($data) { 1 << ($to as u16) } else { 0 })*
        }};
    }

    let dpad = match (
        Button::Up.is_pressed(data.buttons),
        Button::Right.is_pressed(data.buttons),
        Button::Down.is_pressed(data.buttons),
        Button::Left.is_pressed(data.buttons),
    ) {
        (true, false, false, false) => DS4DPad::N,
        (true, true, false, false) => DS4DPad::NE,
        (false, true, false, false) => DS4DPad::E,
        (false, true, true, false) => DS4DPad::SE,
        (false, false, true, false) => DS4DPad::S,
        (false, false, true, true) => DS4DPad::SW,
        (false, false, false, true) => DS4DPad::W,
        (true, false, false, true) => DS4DPad::NW,
        _ => DS4DPad::None,
    };

    let special = if Button::Home.is_pressed(data.buttons) {
        1 << (DS4Special::PS as u8)
    } else {
        0
    };

    target.update(&DS4Report {
        buttons: translate!(data.buttons,
            Button::A =>      DS4Buttons::Cross,
            Button::B =>      DS4Buttons::Circle,
            Button::X =>      DS4Buttons::Square,
            Button::Y =>      DS4Buttons::Triangle,
            Button::Start =>  DS4Buttons::Options,
            Button::Select => DS4Buttons::Share,
            Button::L1 =>     DS4Buttons::LB,
            Button::R1 =>     DS4Buttons::RB,
            Button::L2 =>     DS4Buttons::LT,
            Button::R2 =>     DS4Buttons::RT,
            Button::LStick => DS4Buttons::LStick,
            Button::RStick => DS4Buttons::RStick,
        ) | (dpad as u16),
        special,
        trigger_l: if Button::L2.is_pressed(data.buttons) {
            255
        } else {
            data.l2_analog
        },
        trigger_r: if Button::R2.is_pressed(data.buttons) {
            255
        } else {
            data.r2_analog
        },
        thumb_lx: data.left_stick_x,
        thumb_ly: data.left_stick_y,
        thumb_rx: data.right_stick_x,
        thumb_ry: data.right_stick_y,
    })?;

    Ok(())
}
