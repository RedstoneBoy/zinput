use std::{
    net::{SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::Result;
use crc::{crc32, Hasher32};
use crossbeam_channel::{Receiver, RecvError, Sender};
use dashmap::DashMap;
use dsu_protocol::{
    types::{
        BatteryStatus, Button as DButton, ConnectionType, Model, Protocol, Registration, State,
    },
    ControllerData, ControllerInfo as DsuControllerInfo, MessageRef, ProtocolVersionInfo,
};
use parking_lot::Mutex;
use zinput_engine::{
    device::component::{
        controller::{Button, Controller},
        motion::Motion,
    },
    DeviceView,
};
use zinput_engine::{
    eframe::{self, egui},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
};

const T: &'static str = "frontend:dsus";

const DSU_SERVER_ID: u32 = 0xAAEE00;
const DSU_CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Dsus {
    inner: Mutex<Inner>,
}

impl Dsus {
    pub fn new() -> Self {
        Dsus {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for Dsus {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine);
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status.lock().clone()
    }

    fn name(&self) -> &str {
        "dsus"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Frontend
    }

    fn update_gui(&self, ctx: &egui::Context, frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }
}

struct Inner {
    device: Sender<(usize, Option<Uuid>)>,
    device_recv: Receiver<(usize, Option<Uuid>)>,

    stop: Arc<AtomicBool>,

    engine: Option<Arc<Engine>>,

    selected_devices: [Option<Uuid>; 4],
    controllers: Arc<Mutex<[bool; 4]>>,

    handle1: Option<JoinHandle<()>>,
    handle2: Option<JoinHandle<()>>,

    status: Arc<Mutex<PluginStatus>>,
}

impl Inner {
    fn new() -> Self {
        let (device, device_recv) = crossbeam_channel::unbounded();

        Inner {
            device,
            device_recv,

            stop: Arc::new(AtomicBool::new(false)),

            engine: None,

            selected_devices: [None; 4],
            controllers: Arc::new(Mutex::new([false; 4])),

            handle1: None,
            handle2: None,

            status: Arc::new(Mutex::new(PluginStatus::Stopped)),
        }
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>) {
        self.engine = Some(engine.clone());
        let conn = Arc::new(
            match || -> Result<UdpSocket> {
                let conn = UdpSocket::bind("0.0.0.0:26760")?;
                conn.set_read_timeout(Some(Duration::from_secs(1)))?;
                conn.set_write_timeout(Some(Duration::from_secs(1)))?;
                Ok(conn)
            }() {
                Ok(v) => v,
                Err(err) => {
                    log::error!(target: T, "failed to initialize dsus frontend: {}", err);
                    return;
                }
            },
        );
        let clients = Arc::new(DashMap::new());

        *self.status.lock() = PluginStatus::Running;
        self.stop.store(false, Ordering::Release);

        self.handle1 = Some(std::thread::spawn(new_dsus_thread(Thread {
            engine,
            device_change: self.device_recv.clone(),
            stop: self.stop.clone(),

            conn: conn.clone(),
            clients: clients.clone(),
            status: self.status.clone(),
        })));

        self.handle2 = Some(std::thread::spawn(new_dsus_query_thread(QueryThread {
            conn,
            clients,
            controllers: self.controllers.clone(),
            status: self.status.clone(),
            stop: self.stop.clone(),
        })));
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);

        if let Some(handle) = std::mem::replace(&mut self.handle1, None) {
            match handle.join() {
                Ok(()) => {}
                Err(_) => log::error!(target: T, "error joining dsus thread"),
            }
        }

        if let Some(handle) = std::mem::replace(&mut self.handle2, None) {
            match handle.join() {
                Ok(()) => {}
                Err(_) => log::error!(target: T, "error joining dsus query thread"),
            }
        }

        *self.status.lock() = PluginStatus::Stopped;
    }

    fn update_gui(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        if let Some(engine) = self.engine.clone() {
            for i in 0..self.selected_devices.len() {
                egui::ComboBox::from_label(format!("Dsus Controller {}", i + 1))
                    .selected_text(
                        self.selected_devices[i]
                            .and_then(|id| engine.get_device(&id))
                            .map_or("[None]".to_owned(), |view| view.info().name.clone()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(&mut self.selected_devices[i], None, "[None]")
                            .clicked()
                        {
                            self.device.send((i, None)).unwrap();
                            self.controllers.lock()[i] = false;
                        }
                        for entry in engine.devices() {
                            if ui
                                .selectable_value(
                                    &mut self.selected_devices[i],
                                    Some(*entry.uuid()),
                                    &entry.info().name,
                                )
                                .clicked()
                            {
                                self.device.send((i, Some(*entry.uuid()))).unwrap();
                                self.controllers.lock()[i] = true;
                            }
                        }
                    });
            }
        }
    }
}

struct DsuClient {
    last_update: Instant,
    packet_count: u32,
    slots: [bool; 4],
}

impl DsuClient {
    fn new() -> Self {
        DsuClient {
            last_update: Instant::now(),
            packet_count: 0,
            slots: [false; 4],
        }
    }
}

struct QueryThread {
    conn: Arc<UdpSocket>,
    clients: Arc<DashMap<SocketAddr, DsuClient>>,
    controllers: Arc<Mutex<[bool; 4]>>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
}

fn new_dsus_query_thread(thread: QueryThread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();

        match dsus_query_thread(thread) {
            Ok(()) => log::info!(target: T, "dsus query thread closed"),
            Err(e) => {
                log::error!(target: T, "dsus query thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("dsus query thread crashed: {}", e));
            }
        }
    }
}

fn dsus_query_thread(thread: QueryThread) -> Result<()> {
    const TIMEOUT_KIND: std::io::ErrorKind = {
        #[cfg(target_os = "windows")]
        {
            std::io::ErrorKind::TimedOut
        }
        #[cfg(target_os = "linux")]
        {
            std::io::ErrorKind::WouldBlock
        }
    };

    let QueryThread {
        conn,
        clients,
        controllers,
        stop,
        ..
    } = thread;

    let mut to_remove = Vec::new();

    let mut buf = [0u8; 100];
    let mut crc = crc32::Digest::new(crc32::IEEE);

    while !stop.load(Ordering::Acquire) {
        {
            let now = Instant::now();
            for client in clients.iter() {
                if now - client.value().last_update > DSU_CLIENT_TIMEOUT {
                    to_remove.push(client.key().clone());
                }
            }

            for addr in &to_remove {
                clients.remove(addr);
            }

            to_remove.clear();
        }

        let (amt, client_addr) = match conn.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) if e.kind() == TIMEOUT_KIND => {
                continue;
            }
            Err(e) => {
                log::warn!(target: T, "failed to receive data: {}", e);
                continue;
            }
        };

        crc.reset();

        let message = match MessageRef::parse(&buf[..amt], &mut crc) {
            Ok(v) => v,
            Err(e) => {
                log::warn!(target: T, "{}", e);
                continue;
            }
        };

        crc.reset();

        match message {
            MessageRef::RequestProtocolVersionInfo(_) => {
                let response =
                    ProtocolVersionInfo::new(DSU_SERVER_ID, Protocol::Version1001, &mut crc);

                conn.send_to(&*response, client_addr)?;
            }
            MessageRef::RequestControllerInfo(msg) => {
                let mut response = DsuControllerInfo::new(
                    DSU_SERVER_ID,
                    0,
                    State::Disconnected,
                    Model::FullGyro,
                    ConnectionType::NotApplicable,
                    [0; 6],
                    BatteryStatus::Charged,
                    &mut crc,
                );

                crc.reset();

                let slots = match msg.slots() {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(target: T, "failed to parse controller info request: {}", e);
                        continue;
                    }
                };

                for &slot in slots {
                    let is_connected = controllers.lock()[slot as usize];

                    response.controller_header_mut().set_slot(slot);
                    if is_connected {
                        response.controller_header_mut().set_state(State::Connected);
                    } else {
                        response
                            .controller_header_mut()
                            .set_state(State::Disconnected);
                    }

                    response.update_crc(&mut crc);
                    crc.reset();

                    conn.send_to(&*response, client_addr)?;
                }
            }
            MessageRef::RequestControllerData(msg) => {
                let reg = match msg.registration() {
                    Ok(reg) => reg,
                    Err(_) => continue,
                };
                let new_client = match clients.get_mut(&client_addr) {
                    Some(mut entry) => {
                        entry.value_mut().last_update = Instant::now();
                        match reg {
                            Registration::AllControllers => {
                                entry.value_mut().slots = [true; 4];
                            }
                            Registration::SlotBased => {
                                entry.value_mut().slots[msg.slot() as usize & 0b11] = true;
                            }
                            Registration::MacBased => {
                                // todo
                                log::warn!(target: T, "client requested mac-based registration");
                            }
                        }
                        false
                    }
                    None => true,
                };

                if new_client {
                    log::info!(target: T, "inserting new client (addr {})", client_addr);
                    let mut client = DsuClient::new();
                    match reg {
                        Registration::AllControllers => {
                            client.slots = [true; 4];
                        }
                        Registration::SlotBased => {
                            client.slots[msg.slot() as usize & 0b11] = true;
                        }
                        Registration::MacBased => {
                            // todo
                            log::warn!(target: T, "client requested mac-based registration");
                        }
                    }
                    clients.insert(client_addr, client);
                }
            }
            _ => {
                log::warn!(target: T, "received server packet");
                continue;
            }
        }
    }

    Ok(())
}

fn new_dsus_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();

        match dsus_thread(thread) {
            Ok(()) => log::info!(target: T, "dsus thread closed"),
            Err(e) => {
                log::error!(target: T, "dsus thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("dsus thread crashed: {}", e));
            }
        }
    }
}

struct Thread {
    engine: Arc<Engine>,

    device_change: Receiver<(usize, Option<Uuid>)>,
    stop: Arc<AtomicBool>,

    conn: Arc<UdpSocket>,
    clients: Arc<DashMap<SocketAddr, DsuClient>>,

    status: Arc<Mutex<PluginStatus>>,
}

fn dsus_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_change,
        stop,
        conn,
        clients,
        ..
    } = thread;

    let (update_send, update_recv) = crossbeam_channel::bounded(10);

    let mut server = Server::new(conn);

    let mut views: [Option<DeviceView>; 4] = [None, None, None, None];

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    Ok((idx, Some(device_id))) => {
                        views[idx] = engine.get_device(&device_id);
                        if let Some(view) = &mut views[idx] {
                            view.register_channel(update_send.clone());
                        }

                        server.set_connected(idx as u8, true);
                    }
                    Ok((idx, None)) => {
                        views[idx] = None;

                        server.set_connected(idx as u8, false);
                    }
                    Err(RecvError) => {
                        // Sender dropped which means plugin is uninitialized
                        return Ok(());
                    }
                }
            },
            recv(update_recv) -> uid => {
                let uid = match uid {
                    Ok(uid) => uid,
                    Err(RecvError) => {
                        // this thread owns a sender, receiver does not error
                        unreachable!();
                    }
                };

                for (i, view) in views.iter().filter_map(|view| view.as_ref()).enumerate() {
                    if &uid == view.uuid() {
                        let device = view.device();
                        match device.controllers.get(0) {
                            Some(controller) => server.update_controller(i as u8, controller),
                            None => {},
                        }
                        match device.motions.get(0) {
                            Some(motion) => server.update_motion(i as u8, motion),
                            None => {},
                        }
                    }
                }

                server.send_data(&clients)?;
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

struct Server {
    conn: Arc<UdpSocket>,
    dsu_data: [ControllerData; 4],
    started: Instant,
}

impl Server {
    fn new(conn: Arc<UdpSocket>) -> Self {
        let mut dsu_data = [
            ControllerData { bytes: [0; 100] },
            ControllerData { bytes: [0; 100] },
            ControllerData { bytes: [0; 100] },
            ControllerData { bytes: [0; 100] },
        ];
        let mut hasher = crc32::Digest::new(crc32::IEEE);
        for i in 0..4 {
            dsu_data[i] = ControllerData::new(
                DSU_SERVER_ID,
                i as u8,
                State::Disconnected,
                Model::FullGyro,
                ConnectionType::NotApplicable,
                [0; 6],
                BatteryStatus::Charged,
                false,
                &mut hasher,
            );
            hasher.reset();
        }

        Server {
            conn,
            dsu_data,
            started: Instant::now(),
        }
    }

    fn set_connected(&mut self, slot: u8, connected: bool) {
        let slot = slot as usize & 0b11;
        self.dsu_data[slot]
            .controller_header_mut()
            .set_state(if connected {
                State::Connected
            } else {
                State::Disconnected
            });
        self.dsu_data[slot].set_connected(connected);
    }

    fn update_controller(&mut self, slot: u8, data: &Controller) {
        macro_rules! translate {
            ($data:expr, $dsu:expr, $($from:expr => $to:expr $(=> $analog:ident)?),* $(,)?) => {{
                let mut buttons = dsu_protocol::types::Buttons::new();
                $(if $from.is_pressed($data) { buttons = buttons | $to; $($dsu.$analog(255);)? } else { $($dsu.$analog(0);)? })*
                buttons
            }};
        }

        let dsu_data = &mut self.dsu_data[slot as usize];

        dsu_data.set_analog_l1(data.l1_analog);
        dsu_data.set_analog_l2(data.l2_analog);
        dsu_data.set_analog_r2(data.r2_analog);
        dsu_data.set_analog_l2(data.l2_analog);

        let buttons = translate!(data.buttons, dsu_data,
            Button::A =>      DButton::A      => set_analog_a,
            Button::B =>      DButton::B      => set_analog_b,
            Button::X =>      DButton::X      => set_analog_x,
            Button::Y =>      DButton::Y      => set_analog_y,
            Button::Up =>     DButton::Up     => set_analog_dpad_up,
            Button::Down =>   DButton::Down   => set_analog_dpad_down,
            Button::Left =>   DButton::Left   => set_analog_dpad_left,
            Button::Right =>  DButton::Right  => set_analog_dpad_right,
            Button::Start =>  DButton::Start,
            Button::Select => DButton::Select,
            Button::L1 =>     DButton::L1     => set_analog_l1,
            Button::R1 =>     DButton::R1     => set_analog_r1,
            Button::L2 =>     DButton::L2     => set_analog_r2,
            Button::R2 =>     DButton::R2     => set_analog_l2,
            Button::LStick => DButton::LStick,
            Button::RStick => DButton::RStick,
        );
        dsu_data.set_buttons(buttons);
        dsu_data.set_ps_button(if Button::Home.is_pressed(data.buttons) {
            255
        } else {
            0
        });
        dsu_data.set_left_stick_x(data.left_stick_x);
        dsu_data.set_left_stick_y(data.left_stick_y);
        dsu_data.set_right_stick_x(data.right_stick_x);
        dsu_data.set_right_stick_y(data.right_stick_y);
    }

    fn update_motion(&mut self, slot: u8, data: &Motion) {
        let dsu_data = &mut self.dsu_data[slot as usize];

        dsu_data.set_motion_timestamp((Instant::now() - self.started).as_micros() as u64);

        dsu_data.set_accel_x(data.accel_x);
        dsu_data.set_accel_y(data.accel_y);
        dsu_data.set_accel_z(data.accel_z);
        dsu_data.set_gyro_pitch(data.gyro_pitch);
        dsu_data.set_gyro_yaw(data.gyro_yaw);
        dsu_data.set_gyro_roll(data.gyro_roll);
    }

    fn send_data(&mut self, clients: &DashMap<SocketAddr, DsuClient>) -> Result<()> {
        let mut hasher = crc32::Digest::new(crc32::IEEE);
        for mut client in clients.iter_mut() {
            client.value_mut().packet_count += 1;
            for (_, data) in self
                .dsu_data
                .iter_mut()
                .enumerate()
                .filter(|(slot, _)| client.value().slots[*slot])
            {
                data.set_packet_number(client.value().packet_count);
                data.update_crc(&mut hasher);
                hasher.reset();
                self.conn.send_to(&**data, client.key())?;
            }
        }

        Ok(())
    }
}
