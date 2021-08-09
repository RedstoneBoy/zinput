use std::{
    convert::TryInto,
    net::{SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use eframe::{egui, epi};
use parking_lot::Mutex;
use swi_protocol::{SwiButton, SwiPacket};

use crate::api::component::{
    controller::{Button, Controller, ControllerInfo},
    motion::{Motion, MotionInfo},
};
use crate::api::device::DeviceInfo;
use crate::api::{Backend, BackendStatus, ZInputApi};

const T: &'static str = "backend:swi";

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

impl Backend for Swi {
    fn init(&self, zinput_api: Arc<dyn ZInputApi + Send + Sync>) {
        self.inner.lock().init(zinput_api)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> BackendStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "swi"
    }

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, ui: &mut egui::Ui) {
        let mut inner = self.inner.lock();
        ui.label(format!("Current Address: {}", inner.old_address));
        ui.text_edit_singleline(&mut inner.address);
    }
}

struct Inner {
    handle: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<BackendStatus>>,
    address: String,
    old_address: String,
}

impl Inner {
    fn new() -> Self {
        Inner {
            handle: None,
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(BackendStatus::Running)),
            address: "0.0.0.0:26780".to_owned(),
            old_address: "0.0.0.0:26780".to_owned(),
        }
    }

    fn init(&mut self, api: Arc<dyn ZInputApi + Send + Sync>) {
        *self.status.lock() = BackendStatus::Running;
        self.stop = Arc::new(AtomicBool::new(false));
        self.handle = Some(std::thread::spawn(swi_thread(
            self.address.clone(),
            self.status.clone(),
            self.stop.clone(),
            api,
        )));
        self.old_address = self.address.clone();
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = std::mem::replace(&mut self.handle, None) {
            match handle.join() {
                Ok(()) => (),
                Err(_) => log::info!(target: T, "driver panicked"),
            }
        }
        *self.status.lock() = BackendStatus::Stopped;
    }

    fn status(&self) -> BackendStatus {
        self.status.lock().clone()
    }
}

impl Drop for Swi {
    fn drop(&mut self) {
        self.stop();
    }
}

fn swi_thread(
    address: String,
    status: Arc<Mutex<BackendStatus>>,
    stop: Arc<AtomicBool>,
    api: Arc<dyn ZInputApi + Send + Sync>,
) -> impl FnOnce() {
    move || {
        log::info!(target: T, "driver initialized");

        match swi(address, stop, api) {
            Ok(()) => {
                log::info!(target: T, "driver stopped");
                *status.lock() = BackendStatus::Stopped;
            }
            Err(err) => {
                log::error!(target: T, "driver crashed: {:#}", err);
                *status.lock() = BackendStatus::Error(format!("driver crashed: {:#}", err));
            }
        }
    }
}

fn swi(address: String, stop: Arc<AtomicBool>, api: Arc<dyn ZInputApi>) -> Result<()> {
    const TIMEOUT_KIND: std::io::ErrorKind = {
        #[cfg(target_os = "windows")]
        {
            std::io::ErrorKind::TimedOut
        }
        #[cfg(target_os = "unix")]
        {
            std::io::ErrorKind::WouldBlock
        }
    };
    let mut switch_conn =
        SwitchConnection::new(address.parse()?).context("failed to connect to switch")?;

    switch_conn
        .conn
        .set_read_timeout(Some(Duration::from_secs(1)))?;

    let controller_id = api.new_controller(ControllerInfo::default());
    let motion_id = api.new_motion(MotionInfo::default());
    let device_id = api.new_device(
        DeviceInfo::new(format!("Swi Controller"))
            .with_controller(controller_id)
            .with_motion(motion_id),
    );

    let mut controller = Controller::default();
    let mut motion = Motion::default();

    while !stop.load(Ordering::Acquire) {
        match switch_conn.receive_data() {
            Ok(()) => (),
            Err(err) if err.kind() == TIMEOUT_KIND => {
                continue;
            }
            Err(err) => {
                return Err(err).context("failed to receive switch data");
            }
        }

        switch_conn.write_controller(&mut controller);
        switch_conn.write_motion(&mut motion);

        api.update_controller(&controller_id, &controller)?;
        api.update_motion(&motion_id, &motion)?;
    }

    api.remove_controller(&controller_id);
    api.remove_motion(&motion_id);
    api.remove_device(&device_id);

    Ok(())
}

struct SwitchConnection {
    conn: UdpSocket,
    data: SwiPacket,
}

impl SwitchConnection {
    fn new(addr: SocketAddr) -> Result<SwitchConnection> {
        let conn = UdpSocket::bind(addr).context("failed to bind address")?;

        Ok(SwitchConnection {
            conn,
            data: SwiPacket::new(),
        })
    }

    pub fn receive_data(&mut self) -> std::io::Result<()> {
        let (amt, _) = self.conn.recv_from(&mut *self.data)?;

        if amt != 34 {
            log::info!(target: T, "received incomplete switch data");
            return Ok(());
        }

        Ok(())
    }

    fn write_controller(&self, ctrl: &mut Controller) {
        let swi_buttons = self.data.buttons();
        let mut buttons = 0;

        macro_rules! translate {
            ($data:expr, $($from:expr => $to:expr),* $(,)?) => {
                $(if $data.is_pressed($from) { $to.set_pressed(&mut buttons); })*
            };
        }

        translate!(swi_buttons,
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

        ctrl.buttons = buttons;

        ctrl.left_stick_x = self.data[2];
        ctrl.left_stick_y = self.data[3];
        ctrl.right_stick_x = self.data[4];
        ctrl.right_stick_y = self.data[5];
    }

    fn write_motion(&self, motion: &mut Motion) {
        motion.accel_x = f32::from_le_bytes(self.data[6..10].try_into().unwrap());
        motion.accel_y = f32::from_le_bytes(self.data[10..14].try_into().unwrap());
        motion.accel_z = f32::from_le_bytes(self.data[14..18].try_into().unwrap());
        motion.gyro_pitch = f32::from_le_bytes(self.data[18..22].try_into().unwrap());
        motion.gyro_roll = f32::from_le_bytes(self.data[22..26].try_into().unwrap());
        motion.gyro_yaw = f32::from_le_bytes(self.data[26..30].try_into().unwrap());
    }
}
