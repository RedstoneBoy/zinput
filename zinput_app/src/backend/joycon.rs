use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use hidapi::HidApi;
use parking_lot::Mutex;
use zinput_engine::{
    device::component::{
        controller::{Button, Controller, ControllerInfo},
        motion::{Motion, MotionInfo},
    },
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

const VENDOR_ID: u16 = 0x057E;
const PID_JOYCON_L: u16 = 0x2006;
const PID_JOYCON_R: u16 = 0x2007;
// const PID_JOYCON_PRO: u16 = 0x2009;
// const PID_JOYCON_CHARGEGRIP: u16 = 0x2007;

// TODO: Differentiate between usb and bluetooth
// TODO: Callibration for useful analog data
// TODO: Implement bluetooth pro controller
// TODO: Implement usb pro controller
// TODO: Implement usb joycon left and right (charge grip)

const T: &'static str = "backend:joycon";

const STANDARD_FULL_MODE: [u8; 12] = [
    0x01, // Report: Rumble and Subcommand
    0x00, // Packet Number
    0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, // Rumble: Neutral
    0x03, 0x30, // Subcommand: Standard Full Mode
];

pub struct Joycon {
    inner: Mutex<Inner>,
}

impl Joycon {
    pub fn new() -> Self {
        Joycon {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for Joycon {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "joycon"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

struct Inner {
    hidapi: Option<Arc<HidApi>>,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    stop: Arc<AtomicBool>,
    status: PluginStatus,
}

impl Inner {
    fn new() -> Self {
        Inner {
            hidapi: None,
            handles: Arc::new(Mutex::new(Vec::new())),
            stop: Arc::new(AtomicBool::new(false)),
            status: PluginStatus::Running,
        }
    }

    fn init(&mut self, api: Arc<Engine>) {
        log::info!(target: T, "driver initializing...");

        self.status = PluginStatus::Running;
        self.stop = Arc::new(AtomicBool::new(false));

        match || -> Result<Arc<HidApi>> {
            let hidapi = Arc::new(HidApi::new()?);

            let next_id = Arc::new(AtomicU64::new(0));

            for hid_info in hidapi.device_list() {
                if hid_info.vendor_id() != VENDOR_ID {
                    continue;
                }

                let joy_type = match hid_info.product_id() {
                    PID_JOYCON_L => JoyconType::Left,
                    PID_JOYCON_R => JoyconType::Right,
                    _ => continue,
                };

                let handle = std::thread::spawn(new_controller_thread(
                    ControllerThread {
                        api: api.clone(),
                        hidapi: hidapi.clone(),
                        stop: self.stop.clone(),
                        id: next_id.fetch_add(1, Ordering::SeqCst),
                        hid_info: hid_info.clone(),
                        joy_type,
                    }
                ));
                self.handles.lock().push(handle);
            }

            Ok(hidapi)
        }() {
            Ok(hidapi) => {
                log::info!(target: T, "driver initalized");
                self.hidapi = Some(hidapi);
            }
            Err(err) => {
                log::error!(target: T, "driver failed to initalize: {:#}", err);
                self.status = PluginStatus::Error("driver failed to initialize".to_owned());
            }
        };
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for handle in std::mem::replace(&mut *self.handles.lock(), Vec::new()) {
            let _ = handle.join();
        }
        self.stop.store(false, Ordering::Release);
        self.status = PluginStatus::Stopped;
    }

    fn status(&self) -> PluginStatus {
        self.status.clone()
    }
}

struct ControllerThread {
    api: Arc<Engine>,
    hidapi: Arc<HidApi>,
    stop: Arc<AtomicBool>,

    id: u64,
    hid_info: hidapi::DeviceInfo,
    joy_type: JoyconType,
}

fn new_controller_thread(thread: ControllerThread) -> impl FnOnce() {
    move || {
        let id = thread.id;
        log::info!(target: T, "controller found, id: {}", id);
        match controller_thread(thread) {
            Ok(()) => log::info!(target: T, "controller thread {} closed", id),
            Err(err) => log::error!(target: T, "controller thread {} crashed: {:#}", id, err),
        }
    }
}

fn controller_thread(
    ControllerThread {
        api,
        hidapi,
        stop,
        id,
        hid_info,
        joy_type,
    }: ControllerThread
) -> Result<()> {
    let joycon = hid_info.open_device(&*hidapi).context("failed to open device")?;
    joycon.write(&STANDARD_FULL_MODE)
        .context("failed to set controller to standard full mode")?;

    let mut bundle = JoyconBundle::new(id, joy_type, &*api);

    let mut buf = [0u8; 49];

    while !stop.load(Ordering::Acquire) {
        let read = joycon.read(&mut buf)?;

        if read != 49 {
            continue;
        }

        if buf[0] != 0x30 {
            continue;
        }

        bundle.update(&buf)?;
    }

    Ok(())
}

crate::device_bundle!(DeviceBundle,
    controller: Controller,
    motion: Motion,
);

struct JoyconBundle<'a> {
    bundle: DeviceBundle<'a>,
}

impl<'a> JoyconBundle<'a> {
    fn new(id: u64, joy_type: JoyconType, api: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            api,
            format!("{} (id {})", joy_type, id + 1),
            [match joy_type {
                JoyconType::Left => joycon_l_info(),
                JoyconType::Right => joycon_r_info(),
            }],
            [MotionInfo::new(true, true)],
        );

        JoyconBundle {
            bundle,
        }
    }

    fn update(&mut self, data: &[u8; 49]) -> Result<()> {
        let buttons = [data[3], data[4], data[5]];
        let left_stick = Self::parse_stick([data[6], data[7], data[8]]);
        let right_stick = Self::parse_stick([data[9], data[10], data[11]]);

        self.update_controller(buttons, left_stick, right_stick);

        let mut motions = [MotionData::default(); 3];

        for i in 0..3 {
            let data = &data[(13 + i * 12)..][..12];

            for j in 0..3 {
                motions[i].accel[j] = i16::from_le_bytes([data[j * 2], data[j * 2 + 1]]);
                motions[i].gyro[j] = i16::from_le_bytes([data[6 + j * 2], data[6 + j * 2 + 1]]);
            }
        }

        self.update_motion(motions);

        self.bundle.update()?;

        Ok(())
    }

    fn update_controller(&mut self, buttons: [u8; 3], lstick: (u16, u16), rstick: (u16, u16)) {
        macro_rules! convert {
            ($out:expr => $($jbutton:expr, $button:expr);* $(;)?) => {
                $(if $jbutton.is_pressed(buttons) {
                    $button.set_pressed($out);
                })*
            }
        }

        let mut new_buttons = 0;

        convert!(&mut new_buttons =>
            JButton::A,       Button::A;
            JButton::B,       Button::B;
            JButton::X,       Button::X;
            JButton::Y,       Button::Y;
            JButton::Up,      Button::Up;
            JButton::Down,    Button::Down;
            JButton::Left,    Button::Left;
            JButton::Right,   Button::Right;
            JButton::L,       Button::L1;
            JButton::R,       Button::R1;
            JButton::ZL,      Button::L2;
            JButton::ZR,      Button::R2;
            JButton::LSL,     Button::L3;
            JButton::RSR,     Button::R3;
            JButton::LSR,     Button::L4;
            JButton::RSL,     Button::R4;
            JButton::LStick,  Button::LStick;
            JButton::RStick,  Button::RStick;
            JButton::Plus,    Button::Start;
            JButton::Minus,   Button::Select;
            JButton::Home,    Button::Home;
            JButton::Capture, Button::Capture;
        );

        self.bundle.controller[0].buttons = new_buttons;

        todo!("stick data after callibration")
    }

    fn update_motion(
        &mut self,
        data: [MotionData; 3],
    ) {
        todo!("motion")
    }

    fn parse_stick(data: [u8; 3]) -> (u16, u16) {
        (
            data[0] as u16 | ((data[1] as u16 & 0xF) << 8),
            ((data[1] as u16) >> 4) | ((data[2] as u16) << 4),
        )
    }
}

macro_rules! buttons {
    ($($button:expr),* $(,)?) => {{
        let mut buttons = 0;
        $($button.set_pressed(&mut buttons);)*
        buttons
    }};
}

fn joycon_l_info() -> ControllerInfo {
    ControllerInfo {
        buttons: buttons!(
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
            Button::Select,
            Button::L1,
            Button::L2,
            Button::L3,
            Button::L4,
            Button::LStick,
            Button::Capture,
        ),
        analogs: 0,
    }
        .with_lstick()
}

fn joycon_r_info() -> ControllerInfo {
    ControllerInfo {
        buttons: buttons!(
            Button::A,
            Button::B,
            Button::X,
            Button::Y,
            Button::Start,
            Button::R1,
            Button::R2,
            Button::R3,
            Button::R4,
            Button::RStick,
            Button::Home,
        ),
        analogs: 0,
    }
        .with_rstick()
}

#[derive(Copy, Clone, Default)]
struct MotionData {
    accel: [i16; 3],
    gyro: [i16; 3],
}

enum JoyconType {
    Left,
    Right,
}

impl std::fmt::Display for JoyconType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            JoyconType::Left => write!(f, "Joycon Left"),
            JoyconType::Right => write!(f, "Joycon Right"),
        }
    }
}

enum JButton {
    A,
    B,
    X,
    Y,
    Up,
    Down,
    Left,
    Right,
    L,
    R,
    ZL,
    ZR,
    RSL,
    RSR,
    LSL,
    LSR,
    LStick,
    RStick,
    Plus,
    Minus,
    Home,
    Capture,
}

impl JButton {
    fn is_pressed(self, buttons: [u8; 3]) -> bool {
        let (i, bit) = match self {
            JButton::Y       => (0, 0),
            JButton::X       => (0, 1),
            JButton::B       => (0, 2),
            JButton::A       => (0, 3),
            JButton::RSR     => (0, 4),
            JButton::RSL     => (0, 5),
            JButton::R       => (0, 6),
            JButton::ZR      => (0, 7),
            JButton::Minus   => (1, 0),
            JButton::Plus    => (1, 1),
            JButton::RStick  => (1, 2),
            JButton::LStick  => (1, 3),
            JButton::Home    => (1, 4),
            JButton::Capture => (1, 5),
            JButton::Down    => (2, 0),
            JButton::Up      => (2, 1),
            JButton::Right   => (2, 2),
            JButton::Left    => (2, 3),
            JButton::LSR     => (2, 4),
            JButton::LSL     => (2, 5),
            JButton::L       => (2, 6),
            JButton::ZL      => (2, 7),
        };

        buttons[i] & (1 << bit) != 0
    }
}