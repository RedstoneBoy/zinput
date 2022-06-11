use std::{
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use swi_packet::{SwiButton, SwiController, SwiPacketBuffer};
use zinput_engine::{
    device::component::{
        controller::{Button, ControllerInfo},
        motion::MotionInfo,
    },
    eframe::{self, egui},
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

const T: &'static str = "backend:swi_recv";

const DEFAULT_PORT: &'static str = "26780";

// Rotations Per Second -> Degrees Per Second
const GYRO_SCALE: f32 = 360.0;

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

pub struct Swi {
    inner: Mutex<Inner>,
}

impl Swi {
    pub fn new() -> Self {
        Swi {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for Swi {
    fn init(&self, zinput_api: Arc<Engine>) {
        self.inner.lock().init(zinput_api)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "swi_recv"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }

    fn update_gui(&self, _ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        let mut inner = self.inner.lock();
        ui.label(format!("Port: {}", inner.gui().old_port));
        ui.text_edit_singleline(&mut inner.gui().port);
    }
}

#[derive(Clone)]
struct Gui {
    old_port: String,
    port: String,
}

impl Gui {
    fn new() -> Self {
        Gui {
            old_port: DEFAULT_PORT.to_owned(),
            port: DEFAULT_PORT.to_owned(),
        }
    }
}

enum Inner {
    Uninit {
        gui: Gui,
    },
    Init {
        handle: JoinHandle<()>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<PluginStatus>>,
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

    fn init(&mut self, api: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }
        let gui = self.gui().clone();

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = std::thread::spawn(swi_thread(
            gui.port.clone(),
            status.clone(),
            stop.clone(),
            api,
        ));

        *self = Inner::Init {
            handle,
            stop,
            status,
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
}

impl Drop for Swi {
    fn drop(&mut self) {
        self.stop();
    }
}

fn swi_thread(
    port: String,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> impl FnOnce() {
    move || {
        log::info!(target: T, "driver initialized");

        match swi(port, stop, api) {
            Ok(()) => {
                log::info!(target: T, "driver stopped");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(err) => {
                log::error!(target: T, "driver crashed: {:#}", err);
                *status.lock() = PluginStatus::Error(format!("driver crashed: {:#}", err));
            }
        }
    }
}

fn swi(port: String, stop: Arc<AtomicBool>, api: Arc<Engine>) -> Result<()> {
    let mut swi_conn = SwiConn::new(&*api, port).context("failed to create swi connection")?;

    swi_conn
        .conn
        .set_read_timeout(Some(Duration::from_secs(1)))?;

    while !stop.load(Ordering::Acquire) {
        match swi_conn.receive_data() {
            Ok(()) => (),
            Err(err) if err.kind() == TIMEOUT_KIND => {
                continue;
            }
            Err(err) => {
                return Err(err).context("failed to receive swi data");
            }
        }

        swi_conn.update()?;
    }

    Ok(())
}

struct SwiConn<'a> {
    api: &'a Engine,
    conn: UdpSocket,
    packet: SwiPacketBuffer,
    devices: [Option<DeviceBundle<'a>>; 8],
}

impl<'a> SwiConn<'a> {
    fn new(api: &'a Engine, port: String) -> Result<SwiConn<'a>> {
        let port: u16 = port.parse().context("port is not a valid number")?;
        let conn =
            UdpSocket::bind(format!("0.0.0.0:{}", port)).context("failed to bind address")?;

        Ok(SwiConn {
            api,
            conn,
            packet: SwiPacketBuffer::new(),
            devices: [None, None, None, None, None, None, None, None],
        })
    }

    fn receive_data(&mut self) -> std::io::Result<()> {
        let (amt, _) = self.conn.recv_from(self.packet.full_buffer())?;

        if amt < 1 {
            log::info!(target: T, "received incomplete swi packet data");
            return Ok(());
        }

        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        let mut connected = [false; 8];

        for i in 0..self.packet.num_controllers() {
            if let Some(data) = self.packet.controller(i) {
                let ctrl_num = data.number as usize & 0b111;
                connected[ctrl_num] = true;

                let api = self.api;

                let bundle = match &mut self.devices[ctrl_num] {
                    Some(dev) => dev,
                    None => {
                        self.devices[ctrl_num] = Some(DeviceBundle::new(
                            api,
                            format!("Swi Controller {}", ctrl_num),
                            // TODO: ID
                            None,
                            false,
                            [controller_info()],
                            [MotionInfo::new(true, true)],
                        )?);
                        self.devices[ctrl_num].as_mut().unwrap()
                    }
                };

                bundle.update_data(&data);
            }
        }

        for i in 0..8 {
            if !connected[i] {
                self.devices[i] = None;
            }
        }

        Ok(())
    }
}

crate::device_bundle! {
    DeviceBundle,
    controller: zinput_engine::device::component::controller::Controller,
    motion: zinput_engine::device::component::motion::Motion,
}

impl<'a> DeviceBundle<'a> {
    fn update_data(&mut self, from: &SwiController) {
        let mut buttons = 0;

        macro_rules! translate {
            ($data:expr, $($from:expr => $to:expr),* $(,)?) => {
                $(if $data.is_pressed($from) { $to.set_pressed(&mut buttons); })*
            };
        }

        translate!(from,
            SwiButton::Minus  => Button::Select,
            SwiButton::LStick => Button::LStick,
            SwiButton::RStick => Button::RStick,
            SwiButton::Plus   => Button::Start,
            SwiButton::Up     => Button::Up,
            SwiButton::Right  => Button::Right,
            SwiButton::Down   => Button::Down,
            SwiButton::Left   => Button::Left,
            SwiButton::ZL     => Button::L2,
            SwiButton::ZR     => Button::R2,
            SwiButton::L      => Button::L1,
            SwiButton::R      => Button::R1,
            SwiButton::Y      => Button::Y,
            SwiButton::B      => Button::B,
            SwiButton::A      => Button::A,
            SwiButton::X      => Button::X,
        );

        let ctrl = &mut self.controller[0];

        ctrl.buttons = buttons;

        ctrl.left_stick_x = from.left_stick[0];
        ctrl.left_stick_y = from.left_stick[1];
        ctrl.right_stick_x = from.right_stick[0];
        ctrl.right_stick_y = from.right_stick[1];

        let motion = &mut self.motion[0];

        motion.accel_x = from.accelerometer[0];
        motion.accel_y = from.accelerometer[2];
        motion.accel_z = -from.accelerometer[1];

        motion.gyro_pitch = from.gyroscope[0] * GYRO_SCALE;
        motion.gyro_roll = from.gyroscope[1] * -GYRO_SCALE;
        motion.gyro_yaw = from.gyroscope[2] * GYRO_SCALE;

        self.update();
    }
}

fn controller_info() -> ControllerInfo {
    let mut info = ControllerInfo {
        buttons: 0,
        analogs: 0,
    }
    .with_lstick()
    .with_rstick();

    macro_rules! for_buttons {
        ($($button:expr),* $(,)?) => {
            $($button.set_pressed(&mut info.buttons);)*
        }
    }
    for_buttons!(
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::Start,
        Button::Select,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::LStick,
        Button::RStick,
        Button::Home,
    );

    info
}
