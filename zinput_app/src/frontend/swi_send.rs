use std::{
    collections::HashSet,
    net::{SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, RecvError};
use parking_lot::Mutex;
use swi_packet::{SwiButton, SwiController, SwiPacketBuffer};
use zinput_engine::device::component::{
    controller::{Button, Controller},
    motion::Motion,
};
use zinput_engine::{
    eframe::{egui, epi},
    event::{Event, EventKind},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
};

const T: &'static str = "frontend:swi_send";

const DEFAULT_ADDRESS: &'static str = "10.0.0.177:26780";

// Rotations Per Second -> Degrees Per Second
const GYRO_SCALE: f32 = 360.0;

pub struct Swi {
    inner: Mutex<Inner>,
    signals: Arc<Signals>,
}

impl Swi {
    pub fn new() -> Self {
        Swi {
            inner: Mutex::new(Inner::new()),
            signals: Arc::new(Signals::new()),
        }
    }
}

impl Plugin for Swi {
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
        "swi_send"
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

#[derive(Clone)]
struct Gui {
    old_address: String,
    address: String,

    selected_devices: [Option<Uuid>; 8],
}

impl Gui {
    fn new() -> Self {
        Gui {
            old_address: DEFAULT_ADDRESS.to_owned(),
            address: DEFAULT_ADDRESS.to_owned(),

            selected_devices: [None; 8],
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

        device_send: Sender<(usize, Option<Uuid>)>,

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

    fn init(&mut self, engine: Arc<Engine>, signals: Arc<Signals>) {
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

        let handle = std::thread::spawn(new_swi_thread(Thread {
            engine: engine.clone(),
            device_recv,
            signals,
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

    fn update_gui(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        ui: &mut egui::Ui,
    ) {
        match self {
            Inner::Uninit { gui } => {
                ui.label(format!("Switch Address: {}", gui.old_address));
                ui.text_edit_singleline(&mut gui.address);
            }
            Inner::Init {
                engine,
                device_send,
                gui,
                ..
            } => {
                for i in 0..gui.selected_devices.len() {
                    egui::ComboBox::from_label(format!("Swi Controller {}", i + 1))
                        .selected_text(
                            gui.selected_devices[i]
                                .and_then(|id| engine.get_device_info(&id))
                                .map_or("[None]".to_owned(), |dev| dev.name.clone()),
                        )
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut gui.selected_devices[i], None, "[None]")
                                .clicked()
                            {
                                device_send.send((i, None)).unwrap();
                            }
                            for device_ref in engine.devices() {
                                if ui
                                    .selectable_value(
                                        &mut gui.selected_devices[i],
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
            update: crossbeam_channel::bounded(8),
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_recv: Receiver<(usize, Option<Uuid>)>,
    signals: Arc<Signals>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    addr: String,
}

fn new_swi_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();
        match swi_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "swi thread closed");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(e) => {
                log::error!(target: T, "swi thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("swi thread crashed: {}", e));
            }
        }
    }
}

fn swi_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_recv: device_change,
        signals,
        stop,
        addr,
        ..
    } = thread;

    let mut conn = SwiConn::new(&addr)?;

    let mut dids: [Option<Uuid>; 8] = [None; 8];

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    Ok((idx, Some(device_id))) => {
                        if let Some(did) = &dids[idx] {
                            signals.listen_update.lock().remove(did);
                            dids[idx] = None;
                        }

                        dids[idx] = Some(device_id);
                        signals.listen_update.lock().insert(device_id);
                    }
                    Ok((idx, None)) => {
                        if let Some(did) = &dids[idx] {
                            signals.listen_update.lock().remove(did);
                            dids[idx] = None;
                        }
                    }
                    Err(RecvError) => {
                        // Sender dropped which means plugin is uninitialized
                        return Ok(());
                    }
                }
            },
            recv(signals.update.1) -> uid => {
                let uid = match uid {
                    Ok(uid) => uid,
                    Err(RecvError) => {
                        // Sender can't be dropped since we own it
                        unreachable!()
                    }
                };

                for (i, did) in dids.iter().filter_map(|did| did.as_ref()).enumerate() {
                    if &uid == did {
                        let device = match engine.get_device(did) {
                            Some(device) => device,
                            None => continue,
                        };
                        match device.controllers.get(0) {
                            Some(controller) => conn.update_controller(i, controller),
                            None => {},
                        }
                        match device.motions.get(0) {
                            Some(motion) => conn.update_motion(i, motion),
                            None => {},
                        }
                    }
                }

                conn.set_num_controllers(0);
                for i in (0..8).rev() {
                    if dids[i].is_some() {
                        conn.set_num_controllers(i + 1);
                        break;
                    }
                }

                conn.send_data()?;
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
