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
    device::{component::{
        controller::{Button, Controller},
        motion::Motion,
    }, Device},
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

    let socket = UdpSocket::bind("0.0.0.0:0")
        .context("error binding socket")?;
    socket.set_nonblocking(true)
        .context("error setting socket to nonblocking")?;
    let sender = ZSender::new(socket);
    let mut buf = [0; BUFFER_SIZE];

    loop {
        crossbeam_channel::select! {
            recv(device_recv) -> device_recv => {
                let Ok(ids) = device_recv
                else { return Ok(()); }; // Sender dropped which means plugin is uninitialized

                if ids.len() < joysticks.len() {
                    joysticks.truncate(ids.len());
                } else if ids.len() > joysticks.len() {
                    for i in joysticks.len()..ids.len() {
                        let uinput_device = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(&uinput)
                            .context("failed to open uinput device")?;

                        let uinput_device = UInputHandle::new(uinput_device);

                        let Some(mut view) = engine.get_device(&ids[i])
                        else { anyhow::bail!("tried to get device with invalid uuid"); };
                        view.register_channel(update_send.clone());

                        joysticks.push(Joystick::new(view, uinput_device)?);
                    }
                }
            },
            recv(update_recv) -> uid => {
                let Ok(uid) = uid
                else { continue; };

                for joystick in &joysticks {
                    if joystick.view.uuid() != &uid { continue; };

                    joystick.update()?;
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

struct SwiConn {
    socket: UdpSocket,
    addr: SocketAddr,
    packet: SwiPacketBuffer,
    ctrls: [SwiController; 8],
}

impl SwiConn {
    fn new(address: &str) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").context("failed to bind socket")?;
        let addr = address.parse().context("invalid address")?;

        Ok(SwiConn {
            socket,
            addr,
            packet: Default::default(),
            ctrls: [
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ],
        })
    }

    fn send_data(&mut self) -> Result<()> {
        for i in 0..8 {
            self.packet.set_controller(i, &self.ctrls[i]);
        }

        self.socket
            .send_to(self.packet.sendable_buffer(), &self.addr)
            .context("failed to send swi packet")?;
        Ok(())
    }

    fn set_num_controllers(&mut self, num: usize) {
        self.packet.set_num_controllers(num);
    }

    fn update_controller(&mut self, num: usize, data: &Controller) {
        self.ctrls[num].number = num as u8;
        self.ctrls[num].buttons = [0, 0];
        for (from, to) in [
            (Button::A, SwiButton::A),
            (Button::B, SwiButton::B),
            (Button::X, SwiButton::X),
            (Button::Y, SwiButton::Y),
            (Button::Up, SwiButton::Up),
            (Button::Down, SwiButton::Down),
            (Button::Left, SwiButton::Left),
            (Button::Right, SwiButton::Right),
            (Button::Start, SwiButton::Plus),
            (Button::Select, SwiButton::Minus),
            (Button::LStick, SwiButton::LStick),
            (Button::RStick, SwiButton::RStick),
            (Button::L1, SwiButton::L),
            (Button::R1, SwiButton::R),
            (Button::L2, SwiButton::ZL),
            (Button::R2, SwiButton::ZR),
        ] {
            if from.is_pressed(data.buttons) {
                self.ctrls[num].set_pressed(to);
            }
        }

        self.ctrls[num].left_stick = [data.left_stick_x, data.left_stick_y];
        self.ctrls[num].right_stick = [data.right_stick_x, data.right_stick_y];
    }

    fn update_motion(&mut self, num: usize, data: &Motion) {
        self.ctrls[num].accelerometer = [data.accel_x, -data.accel_z, data.accel_y];
        self.ctrls[num].gyroscope = [
            data.gyro_pitch / GYRO_SCALE,
            -data.gyro_roll / GYRO_SCALE,
            data.gyro_yaw / GYRO_SCALE,
        ];
    }
}
