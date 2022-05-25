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
use crossbeam_channel::{Receiver, RecvError, Sender};
use parking_lot::Mutex;
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};
use zinput_engine::{
    device::component::controller::{Button, Controller},
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

        device_send: Sender<(usize, Option<Uuid>)>,

        selected_devices: [Option<Uuid>; 4],
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

        let (device_send, device_recv) = crossbeam_channel::unbounded();

        let handle = std::thread::spawn(new_vigem_thread(Thread {
            engine: engine.clone(),
            device_recv,
            signals,
            status: status.clone(),
            stop: stop.clone(),
        }));

        *self = Inner::Init {
            engine,
            status,
            stop,
            handle,

            device_send,

            selected_devices: [None; 4],
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
            device_send,
            selected_devices,
            ..
        } = self
        else { return };

        for i in 0..selected_devices.len() {
            egui::ComboBox::from_label(format!("Vigem XBox Controller {}", i + 1))
                .selected_text(
                    selected_devices[i]
                        .and_then(|id| engine.get_device_info(&id))
                        .map_or("[None]".to_owned(), |dev| dev.name.clone()),
                )
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut selected_devices[i], None, "[None]")
                        .clicked()
                    {
                        device_send.send((i, None)).unwrap();
                    }
                    for device_ref in engine.devices() {
                        if ui
                            .selectable_value(
                                &mut selected_devices[i],
                                Some(*device_ref.id()),
                                &device_ref.name,
                            )
                            .clicked()
                        {
                            device_send.send((i, Some(*device_ref.id()))).unwrap();
                        }
                    }
                });
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
    device_recv: Receiver<(usize, Option<Uuid>)>,
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
                log::error!(target: T, "vigem thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("vigem thread crashed: {}", e));
            }
        }
    }
}

fn vigem_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_recv: device_change,
        signals,
        stop,
        ..
    } = thread;

    let vigem = Client::connect()?;

    let mut targets = [(); 4].map(|_| Xbox360Wired::new(&vigem, TargetId::XBOX360_WIRED));
    let mut ids: [Option<Uuid>; 4] = [None; 4];

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    // change controller[idx] to new controller
                    Ok((idx, Some(device_id))) => {
                        // remove old controller
                        if let Some(did) = &mut ids[idx] {
                            targets[idx].unplug().context("failed to unplug target being replaced")?;
                            signals.listen_update.lock().remove(did);
                        }

                        ids[idx] = Some(device_id);
                        targets[idx].plugin().context("failed to plugin target")?;
                        targets[idx].wait_ready().context("failed to wait on target")?;
                        signals.listen_update.lock().insert(device_id);
                    }
                    // remove controller[idx]
                    Ok((idx, None)) => {
                        if let Some(did) = ids[idx].as_mut() {
                            targets[idx].unplug().context("failed to unplug target")?;
                            signals.listen_update.lock().remove(did);
                        }
                        ids[idx] = None;
                    }
                    Err(RecvError) => {
                        // Sender dropped which means plugin is uninitialized
                        return Ok(());
                    }
                }
            },
            recv(signals.update.1) -> _ => {
                for i in 0..ids.len() {
                    if let Some(did) = &ids[i] {
                        let device = match engine.get_device(did) {
                            Some(device) => device,
                            None => continue,
                        };
                        let controller = match device.controllers.get(0) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        update_target(&mut targets[i], controller).with_context(|| format!("failed to update target {}", i))?;
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

fn update_target(target: &mut Xbox360Wired<&Client>, data: &Controller) -> Result<()> {
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
