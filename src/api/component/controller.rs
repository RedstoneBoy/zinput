use super::ComponentData;

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
    type Info = ControllerInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
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
}

impl Button {
    pub const BUTTONS: [Button; 21] = {
        use Button::*;
        [
            A, B, X, Y, Up, Down, Left, Right, Start, Select, L1, R1, L2, R2, L3, R3, L4, R4,
            LStick, RStick, Home,
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
        }
    }

    pub fn set_pressed(&self, buttons: &mut u64) {
        *buttons |= 1 << self.bit();
    }

    pub fn is_pressed(&self, buttons: u64) -> bool {
        buttons & (1 << self.bit()) != 0
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
            }
        )
    }
}
