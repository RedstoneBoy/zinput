use std::convert::Infallible;

use crate::{
    common::{Acceleration, Gyroscope, Stick},
    util::buttons,
};

pub const VENDOR_ID: u16 = 0x057E;
pub const PRODUCT_ID_JOYCON_L: u16 = 0x2006;
pub const PRODUCT_ID_JOYCON_R: u16 = 0x2007;

buttons! {
    Buttons, Button: u32 =>
    Y       = 0,
    X       = 1,
    B       = 2,
    A       = 3,
    RSR     = 4,
    RSL     = 5,
    R       = 6,
    ZR      = 7,
    Minus   = 8,
    Plus    = 9,
    RStick  = 10,
    LStick  = 11,
    Home    = 12,
    Capture = 13,
    Down    = 14,
    Up      = 15,
    Right   = 16,
    Left    = 17,
    LSR     = 18,
    LSL     = 19,
    L       = 20,
    ZL      = 21,
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub buttons: Buttons,
    pub left_stick: Stick<u16>,
    pub right_stick: Stick<u16>,
    pub acceleration: Acceleration<i16>,
    pub gyroscope: Gyroscope<i16>,
}

impl Controller {
    pub fn update(&mut self, packet: &[u8; 49]) -> Result<(), Infallible> {
        self.buttons.0 = u32::from_le_bytes([packet[3], packet[4], packet[5], 0]);
        self.left_stick = Self::parse_stick([packet[6], packet[7], packet[8]]);
        self.right_stick = Self::parse_stick([packet[9], packet[10], packet[11]]);

        todo!()
    }

    fn parse_stick(data: [u8; 3]) -> Stick<u16> {
        Stick {
            x: data[0] as u16 | ((data[1] as u16 & 0xF) << 8),
            y: ((data[1] as u16) >> 4) | ((data[2] as u16) << 4),
        }
    }
}
