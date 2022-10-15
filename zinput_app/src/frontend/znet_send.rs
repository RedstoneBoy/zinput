use std::{
    net::{SocketAddr, UdpSocket},
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
use zinput_engine::{
    device::{
        component::{
            controller::{Button, Controller},
            motion::Motion,
        },
        components,
        Device,
    },
    DeviceView,
};
use zinput_engine::{
    eframe::{self, egui},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
};
use znet::Sender as ZSender;

const T: &'static str = "frontend:znet_send";

const DEFAULT_ADDRESS: &'static str = "10.0.0.176:26810";

const BUFFER_SIZE: usize = 4096;

pub struct ZNet {
    inner: Mutex<Inner>,
}

impl ZNet {
    pub fn new() -> Self {
        ZNet {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for ZNet {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine);
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "znet_send"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Frontend
    }

    fn update_gui(&self, ctx: &egui::Context, frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }
}

#[derive(Clone)]
struct Gui {
    old_address: String,
    address: String,
}

impl Gui {
    fn new() -> Self {
        Gui {
            old_address: DEFAULT_ADDRESS.to_owned(),
            address: DEFAULT_ADDRESS.to_owned(),
        }
    }
}

enum Inner {
    Uninit {
        gui: Gui,
    },
    Init {
        engine: Arc<Engine>,
        status: Arc<Mutex<PluginStatus>>,
        stop: Arc<AtomicBool>,
        handle: JoinHandle<()>,

        device_send: Sender<Vec<Uuid>>,
        selected: Vec<Uuid>,

        gui: Gui,
    },
}

impl Inner {
    fn new() -> Self {
        Inner::Uninit { gui: Gui::new() }
    }

    fn gui(&mut self) -> &mut Gui {
        match self {
            Inner::Uninit { gui } => gui,
            Inner::Init { gui, .. } => gui,
        }
    }

    fn init(&mut self, engine: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }

        let gui = match self {
            Inner::Uninit { gui } => gui.clone(),
            _ => unreachable!(),
        };

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));

        let (device_send, device_recv) = crossbeam_channel::unbounded();

        let handle = std::thread::spawn(new_znet_thread(Thread {
            engine: engine.clone(),
            device_recv,
            status: status.clone(),
            stop: stop.clone(),
            addr: gui.address.clone(),
        }));

        *self = Inner::Init {
            engine,
            status,
            stop,
            handle,

            device_send,
            selected: Vec::new(),

            gui,
        };
    }

    fn stop(&mut self) {
        let gui = self.gui().clone();

        match std::mem::replace(self, Inner::Uninit { gui }) {
            Inner::Uninit { .. } => {}
            Inner::Init {
                handle,
                stop,
                status,
                ..
            } => {
                stop.store(true, Ordering::SeqCst);

                match handle.join() {
                    Ok(()) => (),
                    Err(_) => log::info!(target: T, "driver panicked"),
                }

                *status.lock() = PluginStatus::Stopped;
            }
        }
    }

    fn status(&self) -> PluginStatus {
        match self {
            Inner::Uninit { .. } => PluginStatus::Stopped,
            Inner::Init { status, .. } => status.lock().clone(),
        }
    }

    fn update_gui(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        match self {
            Inner::Uninit { gui } => {
                ui.label(format!("Receiver Address: {}", gui.old_address));
                ui.text_edit_singleline(&mut gui.address);
            }
            Inner::Init {
                engine,
                device_send,
                selected,
                gui,
                ..
            } => {
                #[derive(PartialEq, Eq)]
                enum Action {
                    Remove(usize),
                    Change(usize, Uuid),
                    Add(Uuid),
                }

                let mut action = None;

                for i in 0..selected.len() {
                    if action.is_some() {
                        break;
                    }

                    egui::ComboBox::from_label(format!("ZNet Controller {}", i + 1))
                        .selected_text(match engine.get_device(&selected[i]) {
                            Some(view) => view.info().name.clone(),
                            None => {
                                action = Some(Action::Remove(i));
                                break;
                            }
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut action, Some(Action::Remove(i)), "[None]");
                            for entry in engine.devices() {
                                ui.selectable_value(
                                    &mut action,
                                    Some(Action::Change(i, *entry.uuid())),
                                    &entry.info().name,
                                );
                            }
                        });
                }

                if selected.len() < 4 && action.is_none() {
                    egui::ComboBox::from_label(format!(
                        "ViGEm XBox Controller {}",
                        selected.len() + 1
                    ))
                    .selected_text("[None]")
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut action, None, "[None]");
                        for entry in engine.devices() {
                            ui.selectable_value(
                                &mut action,
                                Some(Action::Add(*entry.uuid())),
                                &entry.info().name,
                            );
                        }
                    });
                }

                if let Some(action) = action {
                    match action {
                        Action::Remove(i) => {
                            selected.remove(i);
                        }
                        Action::Change(i, id) => selected[i] = id,
                        Action::Add(id) => selected.push(id),
                    }

                    device_send.send(selected.clone()).unwrap();
                }
            }
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_recv: Receiver<Vec<Uuid>>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    addr: String,
}

fn new_znet_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();
        match znet_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "stopped");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(e) => {
                log::error!(target: T, "crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("crashed: {}", e));
            }
        }
    }
}

fn znet_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_recv,
        stop,
        addr,
        ..
    } = thread;

    let (update_send, update_recv) = crossbeam_channel::bounded(10);

    let socket = UdpSocket::bind("0.0.0.0:0").context("error binding socket")?;
    socket
        .set_nonblocking(true)
        .context("error setting socket to nonblocking")?;
    socket
        .connect(&addr)
        .context(format!("error connecting to '{addr}'"))?;
    let sender = ZSender::new(socket);
    let mut buffer = [0; BUFFER_SIZE];

    let mut devices = Vec::<Device>::new();
    let mut names = Vec::<[u8; 16]>::new();
    let mut views = Vec::<DeviceView>::new();

    loop {
        crossbeam_channel::select! {
            recv(device_recv) -> device_recv => {
                let Ok(ids) = device_recv
                else { return Ok(()); }; // Sender dropped which means plugin is uninitialized

                if ids.len() < views.len() {
                    devices.truncate(ids.len());
                    names.truncate(ids.len());
                    views.truncate(ids.len());
                } else if ids.len() > views.len() {
                    for i in views.len()..ids.len() {
                        let Some(mut view) = engine.get_device(&ids[i])
                        else { anyhow::bail!("tried to get device with invalid uuid"); };
                        view.register_channel(update_send.clone());

                        devices.push(view.device().clone());
                        let mut name = [0u8; 16];
                        for (i, b) in format!("zcon {i}").as_bytes().iter().enumerate() {
                            name[i] = *b;
                        }
                        names.push(name);
                        views.push(view);
                    }
                }
            },
            recv(update_recv) -> uid => {
                let Ok(uid) = uid
                else { continue; };

                for (out, view) in devices
                    .iter_mut()
                    .zip(views.iter())
                {
                    if view.uuid() != &uid {
                        continue;
                    }

                    let input = view.device();

                    macro_rules! update_device {
                        ($($cname:ident : $cty:ty),* $(,)?) => {
                            paste::paste! {
                                $(
                                for (i, data) in input.[< $cname s >].iter().enumerate() {
                                    out.[< $cname s >][i] = data.clone();
                                }
                                )*
                            }
                        }
                    }

                    components!(data update_device);
                }

                match sender.send(&devices, &names, &mut buffer) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        log::warn!(target: T, "error sending packet: {e}");
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
