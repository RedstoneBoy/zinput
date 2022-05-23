use std::convert::Infallible;

use crate::{common::Stick, util::buttons};

pub const VENDOR_ID: u16 = 0x20D6;
pub const PRODUCT_ID: u16 = 0xA713;

pub const EP_IN: u8 = 0x81;

buttons! {
    Buttons, Button: u16 =>
    Y       = 0,
    B       = 1,
    A       = 2,
    X       = 3,
    L1      = 4,
    R1      = 5,
    L2      = 6,
    R2      = 7,
    Select  = 8,
    Start   = 9,
    LStick  = 10,
    RStick  = 11,
    Home    = 12,
    Capture = 13,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DPad(u8);

impl Default for DPad {
    fn default() -> Self {
        DPad(0xFF)
    }
}

impl DPad {
    pub fn up(self) -> bool {
        self.0 == 7 || self.0 == 0 || self.0 == 1
    }

    pub fn right(self) -> bool {
        self.0 == 1 || self.0 == 2 || self.0 == 3
    }

    pub fn down(self) -> bool {
        self.0 == 3 || self.0 == 4 || self.0 == 5
    }

    pub fn left(self) -> bool {
        self.0 == 5 || self.0 == 6 || self.0 == 7
    }
}

#[derive(Clone, Debug, Default)]
pub struct Controller {
    pub buttons: Buttons,
    pub dpad: DPad,
    pub left_stick: Stick<u8>,
    pub right_stick: Stick<u8>,
}

impl Controller {
    pub fn update(&mut self, packet: &[u8; 8]) -> Result<(), Infallible> {
        self.buttons.0 = u16::from_le_bytes([packet[0], packet[1]]);
        self.dpad.0 = packet[2];
        self.left_stick.x = packet[3];
        self.left_stick.y = packet[4];
        self.right_stick.x = packet[5];
        self.right_stick.y = packet[6];

        Ok(())
    }
}
