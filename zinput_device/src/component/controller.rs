use std::ops::BitOr;

use super::ComponentData;

#[derive(Clone, PartialEq, Eq)]
pub struct ControllerInfo {
    pub buttons: u64,
    pub analogs: u8,
}

impl ControllerInfo {
    pub const fn with_lstick(mut self) -> Self {
        self.analogs |= 1 << 0;
        self
    }

    pub const fn with_rstick(mut self) -> Self {
        self.analogs |= 1 << 1;
        self
    }

    pub const fn with_l1_analog(mut self) -> Self {
        self.analogs |= 1 << 2;
        self
    }

    pub const fn with_r1_analog(mut self) -> Self {
        self.analogs |= 1 << 3;
        self
    }

    pub const fn with_l2_analog(mut self) -> Self {
        self.analogs |= 1 << 4;
        self
    }

    pub const fn with_r2_analog(mut self) -> Self {
        self.analogs |= 1 << 5;
        self
    }
}

impl Default for ControllerInfo {
    fn default() -> Self {
        ControllerInfo {
            buttons: 0,
            analogs: 0,
        }
    }
}

#[derive(Clone)]
pub struct ControllerConfig {
    pub left_stick: StickConfig,
    pub right_stick: StickConfig,
    pub l1_range: [u8; 2],
    pub r1_range: [u8; 2],
    pub l2_range: [u8; 2],
    pub r2_range: [u8; 2],
}

impl Default for ControllerConfig {
    fn default() -> Self {
        ControllerConfig {
            left_stick: Default::default(),
            right_stick: Default::default(),
            l1_range: [0, 255],
            r1_range: [0, 255],
            l2_range: [0, 255],
            r2_range: [0, 255],
        }
    }
}

#[derive(Copy, Clone)]
pub struct StickConfig {
    pub deadzone_squared: u16,
    // TODO: FIXME completely incorrect
    pub x_range: [u8; 2],
    pub y_range: [u8; 2],
}

impl StickConfig {
    fn configure(&self, x: u8, y: u8) -> [u8; 2] {
        let x = configure_analog(x, self.x_range);
        let y = configure_analog(y, self.y_range);
        if (x as f32 - 127.5).powi(2) + (y as f32 - 127.5).powi(2) <= (self.deadzone_squared as f32) {
            return [0, 0];
        }

        [x, y]
    }
}

impl Default for StickConfig {
    fn default() -> Self {
        StickConfig {
            deadzone_squared: 0,
            x_range: [0, 255],
            y_range: [0, 255],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Controller {
    pub buttons: u64,
    pub left_stick_x: u8,
    pub left_stick_y: u8,
    pub right_stick_x: u8,
    pub right_stick_y: u8,
    pub l1_analog: u8,
    pub r1_analog: u8,
    pub l2_analog: u8,
    pub r2_analog: u8,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            buttons: 0,
            left_stick_x: 127,
            left_stick_y: 127,
            right_stick_x: 127,
            right_stick_y: 127,
            l1_analog: 0,
            r1_analog: 0,
            l2_analog: 0,
            r2_analog: 0,
        }
    }
}

impl ComponentData for Controller {
    type Config = ControllerConfig;
    type Info = ControllerInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }
    
    fn configure(&mut self, config: &Self::Config) {
        let [lx, ly] = config.left_stick.configure(self.left_stick_x, self.left_stick_y);
        let [rx, ry] = config.right_stick.configure(self.right_stick_x, self.right_stick_y);
        self.left_stick_x = lx;
        self.left_stick_y = ly;
        self.right_stick_x = rx;
        self.right_stick_y = ry;
        self.l1_analog = configure_analog(self.l1_analog, config.l1_range);
        self.r1_analog = configure_analog(self.r1_analog, config.r1_range);
        self.l2_analog = configure_analog(self.l2_analog, config.l2_range);
        self.r2_analog = configure_analog(self.r2_analog, config.r2_range);
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Button {
    A,
    B,
    X,
    Y,
    Up,
    Down,
    Left,
    Right,
    Start,
    Select,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    L4,
    R4,
    LStick,
    RStick,
    Home,
    Capture,
}

impl Button {
    pub const BUTTONS: [Button; 22] = {
        use Button::*;
        [
            A, B, X, Y, Up, Down, Left, Right, Start, Select, L1, R1, L2, R2, L3, R3, L4, R4,
            LStick, RStick, Home, Capture,
        ]
    };

    pub const fn bit(&self) -> u64 {
        match self {
            Button::A => 0,
            Button::B => 1,
            Button::X => 2,
            Button::Y => 3,
            Button::Up => 4,
            Button::Down => 5,
            Button::Left => 6,
            Button::Right => 7,
            Button::Start => 8,
            Button::Select => 9,
            Button::L1 => 10,
            Button::R1 => 11,
            Button::L2 => 12,
            Button::R2 => 13,
            Button::L3 => 14,
            Button::R3 => 15,
            Button::L4 => 16,
            Button::R4 => 17,
            Button::LStick => 18,
            Button::RStick => 19,
            Button::Home => 20,
            Button::Capture => 21,
        }
    }

    pub fn set_pressed(&self, buttons: &mut u64) {
        *buttons |= 1 << self.bit();
    }

    pub fn is_pressed(&self, buttons: u64) -> bool {
        buttons & (1 << self.bit()) != 0
    }
}

impl BitOr for Button {
    type Output = u64;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.bit() | rhs.bit()
    }
}

impl BitOr<Button> for u64 {
    type Output = u64;

    fn bitor(self, rhs: Button) -> u64 {
        self | rhs.bit()
    }
}

impl std::fmt::Display for Button {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Button::*;
        write!(
            f,
            "{}",
            match *self {
                A => "A",
                B => "B",
                X => "X",
                Y => "Y",
                Up => "Up",
                Down => "Down",
                Left => "Left",
                Right => "Right",
                Start => "Start",
                Select => "Select",
                L1 => "L1",
                R1 => "R1",
                L2 => "L2",
                R2 => "R2",
                L3 => "L3",
                R3 => "R3",
                L4 => "L4",
                R4 => "R4",
                LStick => "LStick",
                RStick => "RStick",
                Home => "Home",
                Capture => "Capture",
            }
        )
    }
}

fn configure_analog(analog: u8, range: [u8; 2]) -> u8 {
    let min = range[0] as f32;
    let max = range[1] as f32;
    let range = max - min;
    ((f32::clamp(analog as f32, min, max) / range) * 255.0) as u8
}