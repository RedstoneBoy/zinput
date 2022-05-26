use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{bail, Context, Result};
use hidapi::{HidApi, HidDevice};
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
const PID_JOYCON_PRO: u16 = 0x2009;
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
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    stop: Arc<AtomicBool>,
    status: PluginStatus,
}

impl Inner {
    fn new() -> Self {
        Inner {
            handles: Arc::new(Mutex::new(Vec::new())),
            stop: Arc::new(AtomicBool::new(false)),
            status: PluginStatus::Running,
        }
    }

    fn init(&mut self, api: Arc<Engine>) {
        log::info!(target: T, "driver initializing...");

        self.status = PluginStatus::Running;
        self.stop = Arc::new(AtomicBool::new(false));

        match || -> Result<()> {
            let hidapi = Arc::new(HidApi::new()?);

            let next_id = Arc::new(AtomicU64::new(0));

            for hid_info in hidapi.device_list() {
                if hid_info.vendor_id() != VENDOR_ID {
                    continue;
                }

                let joy_type = match hid_info.product_id() {
                    PID_JOYCON_L => JoyconType::Left,
                    PID_JOYCON_R => JoyconType::Right,
                    PID_JOYCON_PRO => JoyconType::Pro,
                    _ => continue,
                };

                let handle = std::thread::spawn(new_controller_thread(ControllerThread {
                    api: api.clone(),
                    hidapi: hidapi.clone(),
                    stop: self.stop.clone(),
                    id: next_id.fetch_add(1, Ordering::SeqCst),
                    hid_info: hid_info.clone(),
                    joy_type,
                }));
                self.handles.lock().push(handle);
            }

            Ok(())
        }() {
            Ok(()) => {
                log::info!(target: T, "driver initalized");
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
    }: ControllerThread,
) -> Result<()> {
    let joycon = hid_info
        .open_device(&*hidapi)
        .context("failed to open device")?;

    let calibration = read_calibration(&joycon).context("failed to read calibration data")?;

    joycon
        .write(&STANDARD_FULL_MODE)
        .context("failed to set controller to standard full mode")?;

    let mut bundle = JoyconBundle::new(id, joy_type, calibration, &*api);

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

fn read_calibration(dev: &HidDevice) -> Result<Calibration> {
    fn spi_addr(offset: u32) -> u32 {
        0xF8000000 + offset
    }
    fn read_memory<'a, 'b>(
        dev: &'a HidDevice,
        addr: u32,
        size: u16,
        buf: &'b mut [u8],
    ) -> Result<&'b mut [u8]> {
        fn setup_memory_read(addr: u32, size: u16) -> [u8; 8] {
            let addr = addr.to_le_bytes();
            let size = size.to_le_bytes();
            let mut bytes = [
                0x71, addr[0], addr[1], addr[2], addr[3], size[0], size[1], 0x00,
            ];
            let mut sum = 0u8;
            let mut i = 0;
            while i < 7 {
                sum = sum.wrapping_add(bytes[i]);
                i += 1;
            }
            let sum = (0x100 - (sum as u16)) as u8;
            bytes[7] = sum;
            bytes
        }

        const MEMORY_READ: [u8; 3] = [0x72, 0x8E, 0x00];

        // CHECK: is this correct?
        if size > 0xF9 {
            bail!("a maximum of 0xF9 bytes can be read at once");
        }

        if buf.len() < size as usize + 9 {
            bail!(
                "buffer is not large enough: need {} bytes, got {}",
                size as usize + 9,
                buf.len()
            );
        }

        for i in 0..buf.len() {
            buf[i] = 0;
        }
        buf[0..3].copy_from_slice(&MEMORY_READ);

        let packet_setup = setup_memory_read(addr, size);
        dev.send_feature_report(&packet_setup)
            .context("failed to send feature report \"setup memory read\"")?;
        dev.get_feature_report(buf)
            .context("failed to get feature report \"read memory\"")?;

        if buf[0] != 0x72 {
            bail!(
                "feature report (read memory) returned wrong id: {:#X}",
                buf[0]
            );
        }
        let ret_addr = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        if ret_addr != addr {
            bail!(
                "feature report (read memory) returned wrong address: {:#X}",
                ret_addr
            );
        }
        let ret_size = u16::from_le_bytes([buf[5], buf[6]]);
        if ret_size != size {
            bail!(
                "feature report (read memory) returned wrong size: {:#X}",
                ret_size
            );
        }

        Ok(&mut buf[7..][..(size as usize)])
    }

    let mut buf = [0u8; 32];

    let mut lstick_data = [0; 9];
    let mut rstick_data = [0; 9];

    let mut user_stick_data = [0; 0x16];
    user_stick_data.copy_from_slice(
        read_memory(dev, spi_addr(0x8010), 0x16, &mut buf)
            .context("failed to read user stick calibration data")?,
    );

    let mut stick_data = [0; 0x12];
    stick_data.copy_from_slice(
        read_memory(dev, spi_addr(0x603D), 0x12, &mut buf)
            .context("failed to read stick calibration data")?,
    );

    if user_stick_data[0] == 0xB2 && user_stick_data[1] == 0xA1 {
        lstick_data.copy_from_slice(&user_stick_data[2..][..9]);
    } else {
        lstick_data.copy_from_slice(&stick_data[..9]);
    }
    if user_stick_data[11] == 0xB2 && user_stick_data[12] == 0xA1 {
        log::info!("joycon has user stick data right");
        rstick_data.copy_from_slice(&user_stick_data[13..][..9]);
    } else {
        rstick_data.copy_from_slice(&stick_data[9..]);
    }

    let mut deadzone_l = [0; 2];
    let mut deadzone_r = [0; 2];

    deadzone_l.copy_from_slice(
        read_memory(dev, spi_addr(0x6086 + 3), 2, &mut buf)
            .context("failed to read left stick deadzone data")?,
    );
    deadzone_r.copy_from_slice(
        read_memory(dev, spi_addr(0x6098 + 3), 2, &mut buf)
            .context("failed to read right stick deadzone data")?,
    );

    let lstick = StickCalibration::parse(lstick_data, deadzone_l, true);
    let rstick = StickCalibration::parse(rstick_data, deadzone_r, false);

    Ok(Calibration { lstick, rstick })
}

crate::device_bundle!(DeviceBundle, controller: Controller, motion: Motion);

struct JoyconBundle<'a> {
    bundle: DeviceBundle<'a>,
    calibration: Calibration,
}

impl<'a> JoyconBundle<'a> {
    fn new(id: u64, joy_type: JoyconType, calibration: Calibration, api: &'a Engine) -> Self {
        let bundle = DeviceBundle::new(
            api,
            format!("{} (id {})", joy_type, id + 1),
            [match joy_type {
                JoyconType::Left => joycon_l_info(),
                JoyconType::Right => joycon_r_info(),
                JoyconType::Pro => joycon_pro_info(),
            }],
            [MotionInfo::new(true, true)],
        );

        JoyconBundle {
            bundle,
            calibration,
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

    fn update_controller(&mut self, buttons: [u8; 3], lstick: [u16; 2], rstick: [u16; 2]) {
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

        let lstick = lstick.map(|v| v as f32);
        let rstick = rstick.map(|v| v as f32);

        let lstick = self.calibration.lstick.apply(lstick);
        let rstick = self.calibration.rstick.apply(rstick);

        let lstick = lstick.map(|v| (v * 255.0) as u8);
        let rstick = rstick.map(|v| (v * 255.0) as u8);

        self.bundle.controller[0].buttons = new_buttons;
        self.bundle.controller[0].left_stick_x = lstick[0];
        self.bundle.controller[0].left_stick_y = lstick[1];
        self.bundle.controller[0].right_stick_x = rstick[0];
        self.bundle.controller[0].right_stick_y = rstick[1];
    }

    fn update_motion(&mut self, _data: [MotionData; 3]) {
        // todo: motion
    }

    fn parse_stick(data: [u8; 3]) -> [u16; 2] {
        [
            data[0] as u16 | ((data[1] as u16 & 0xF) << 8),
            ((data[1] as u16) >> 4) | ((data[2] as u16) << 4),
        ]
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

fn joycon_pro_info() -> ControllerInfo {
    ControllerInfo {
        buttons: buttons!(
            Button::A,
            Button::B,
            Button::X,
            Button::Y,
            Button::Start,
            Button::R1,
            Button::R2,
            Button::RStick,
            Button::Home,
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
            Button::Select,
            Button::L1,
            Button::L2,
            Button::LStick,
            Button::Capture,
        ),
        analogs: 0,
    }
    .with_lstick()
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
    Pro,
}

impl std::fmt::Display for JoyconType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            JoyconType::Left => write!(f, "Joycon Left"),
            JoyconType::Right => write!(f, "Joycon Right"),
            JoyconType::Pro => write!(f, "Pro Controller"),
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
            JButton::Y => (0, 0),
            JButton::X => (0, 1),
            JButton::B => (0, 2),
            JButton::A => (0, 3),
            JButton::RSR => (0, 4),
            JButton::RSL => (0, 5),
            JButton::R => (0, 6),
            JButton::ZR => (0, 7),
            JButton::Minus => (1, 0),
            JButton::Plus => (1, 1),
            JButton::RStick => (1, 2),
            JButton::LStick => (1, 3),
            JButton::Home => (1, 4),
            JButton::Capture => (1, 5),
            JButton::Down => (2, 0),
            JButton::Up => (2, 1),
            JButton::Right => (2, 2),
            JButton::Left => (2, 3),
            JButton::LSR => (2, 4),
            JButton::LSL => (2, 5),
            JButton::L => (2, 6),
            JButton::ZL => (2, 7),
        };

        buttons[i] & (1 << bit) != 0
    }
}

#[derive(Debug)]
struct Calibration {
    lstick: StickCalibration,
    rstick: StickCalibration,
}

#[derive(Debug)]
struct StickCalibration {
    center: [f32; 2],
    omin: [f32; 2],
    omax: [f32; 2],
    deadzone: f32,
}

impl StickCalibration {
    fn parse(data: [u8; 9], deadzone_data: [u8; 2], is_left: bool) -> Self {
        let data = data.map(|v| v as u16);
        let v0 = (data[1] << 8) & 0xF00 | data[0];
        let v1 = (data[2] << 4) | (data[1] >> 4);
        let v2 = (data[4] << 8) & 0xF00 | data[3];
        let v3 = (data[5] << 4) | (data[4] >> 4);
        let v4 = (data[7] << 8) & 0xF00 | data[6];
        let v5 = (data[8] << 4) | (data[7] >> 4);

        let deadzone_data = deadzone_data.map(|v| v as u16);
        let deadzone = (deadzone_data[1] << 8) & 0xF00 | deadzone_data[0];
        let deadzone = deadzone as f32;
        let deadzone = deadzone * deadzone;

        let [omin, center, omax] = if is_left {
            [[v4, v5], [v2, v3], [v0, v1]]
        } else {
            [[v2, v3], [v0, v1], [v4, v5]]
        };

        let center = center.map(|v| v as f32);
        let omin = omin.map(|v| v as f32);
        let omax = omax.map(|v| v as f32);

        StickCalibration {
            center,
            omin,
            omax,
            deadzone,
        }
    }

    fn apply(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        let x = x - self.center[0];
        let y = y - self.center[1];

        if x * x + y * y < self.deadzone {
            return [0.5, 0.5];
        }

        // TODO: deadzone

        let x = x / if x > 0.0 { self.omax[0] } else { self.omin[0] };
        let y = y / if y > 0.0 { self.omax[1] } else { self.omin[1] };

        let x = x + 0.5;
        let y = y + 0.5;

        [x, y]
    }
}
