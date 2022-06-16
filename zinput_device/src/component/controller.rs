use std::ops::BitOr;

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

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

#[derive(Clone, Deserialize, Serialize)]
pub struct ControllerConfig {
    pub left_stick: StickConfig,
    pub right_stick: StickConfig,
    pub l1_range: [u8; 2],
    pub r1_range: [u8; 2],
    pub l2_range: [u8; 2],
    pub r2_range: [u8; 2],
    #[serde(with = "BigArray")]
    pub remap: [u8; 64],
}

impl Default for ControllerConfig {
    fn default() -> Self {
        let mut remap = [0; 64];
        for i in 0..64 {
            remap[i] = i as u8;
        }

        ControllerConfig {
            left_stick: Default::default(),
            right_stick: Default::default(),
            l1_range: [0, 255],
            r1_range: [0, 255],
            l2_range: [0, 255],
            r2_range: [0, 255],
            remap,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StickConfig {
    pub deadzone: u8,

    pub samples: Option<[f32; 32]>,
}

impl StickConfig {
    fn configure(&self, x: u8, y: u8) -> [u8; 2] {
        if self.deadzone == 0 && self.samples.is_none() {
            return [x, y];
        }

        let dzf = self.deadzone as f32 / 255.0;
        let xf = (x as f32 - 127.5) / 127.5;
        let yf = (y as f32 - 127.5) / 127.5;
        let scalar = f32::sqrt(xf.powi(2) + yf.powi(2));

        let max = match &self.samples {
            Some(samples) => {
                let mut angle = f32::atan2(yf, xf);
                if angle < 0.0 {
                    angle = 2.0 * std::f32::consts::PI + angle;
                }
                Self::sample(samples, angle)
            }
            None => 1.0,
        };

        let range = max - dzf;

        if range <= 0.0 {
            return [128, 128];
        }

        let new_scalar = (scalar.clamp(dzf, max) - dzf) / range;

        let xf = (xf / scalar) * new_scalar;
        let yf = (yf / scalar) * new_scalar;

        let x = (xf * 127.5 + 127.5) as u8;
        let y = (yf * 127.5 + 127.5) as u8;

        [x, y]
    }

    fn sample(samples: &[f32; 32], angle: f32) -> f32 {
        fn index_to_angle(index: usize) -> f32 {
            (index as f32) * (std::f32::consts::PI * 2.0 / 32.0)
        }

        let (mut i1, mut i2) = (0, 0);
        let mut influence = 0.0;

        for i in 0..32 {
            let min_angle = index_to_angle(i);
            let max_angle = index_to_angle(i + 1);
            if min_angle <= angle && angle < max_angle {
                i1 = i;
                i2 = (i + 1) % 32;
                influence = (angle - min_angle) / (max_angle - min_angle);
                break;
            }
        }

        let v1 = samples[i1] * (1.0 - influence);
        let v2 = samples[i2] * influence;

        v1 + v2
    }
}

impl Default for StickConfig {
    fn default() -> Self {
        StickConfig {
            deadzone: 0,

            samples: None,
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
        let [lx, ly] = config
            .left_stick
            .configure(self.left_stick_x, self.left_stick_y);
        let [rx, ry] = config
            .right_stick
            .configure(self.right_stick_x, self.right_stick_y);
        self.left_stick_x = lx;
        self.left_stick_y = ly;
        self.right_stick_x = rx;
        self.right_stick_y = ry;
        self.l1_analog = configure_analog(self.l1_analog, config.l1_range);
        self.r1_analog = configure_analog(self.r1_analog, config.r1_range);
        self.l2_analog = configure_analog(self.l2_analog, config.l2_range);
        self.r2_analog = configure_analog(self.r2_analog, config.r2_range);

        let mut output_buttons = 0;
        for i in 0..64 {
            if self.buttons & (1 << i) != 0 {
                output_buttons |= 1 << config.remap[i];
            }
        }

        self.buttons = output_buttons;
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

    pub fn try_from_bit(bit: u8) -> Option<Self> {
        Self::BUTTONS.get(bit as usize).copied()
    }

    pub fn set_pressed(&self, buttons: &mut u64) {
        *buttons |= 1 << self.bit();
    }

    pub fn set_clear(&self, buttons: &mut u64) {
        *buttons &= !(1 << self.bit());
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
    (((f32::clamp(analog as f32, min, max) - min) / range) * 255.0) as u8
}
