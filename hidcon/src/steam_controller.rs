use std::convert::Infallible;

use crate::{
    common::{Stick, Acceleration, Gyroscope},
    util::buttons,
};

pub const VENDOR_ID: u16 = 0x28DE;
pub const PRODUCT_ID_WIRELESS: u16 = 0x1142;
pub const PRODUCT_ID_WIRED: u16 = 0x1042;

pub const EP_IN: u8 = 0x82;

pub const ENABLE_MOTION: [u8; 64] = [
    0x87, 0x15, 0x32, 0x84, 0x03, 0x18, 0x00, 0x00, 0x31, 0x02, 0x00, 0x08, 0x07, 0x00, 0x07, 0x07,
    0x00, 0x30, 0x18, 0x00, 0x2f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const DISABLE_LIZARD_MODE: [u8; 64] = [
    0x81, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const STATUS_INPUT: u8 = 0x1;

buttons! {
    Buttons, Button: u32 =>
    RPadTouch = 28,
    LPadTouch = 27,
    RClick    = 26,
    LClick    = 25,
    RGrip     = 24,
    LGrip     = 23,
    Start     = 22,
    Steam     = 21,
    Back      = 20,
    A         = 15,
    X         = 14,
    B         = 13,
    Y         = 12,
    Lb        = 11,
    Rb        = 10,
    Lt        = 9,
    Rt        = 8,
}

#[derive(Clone, Debug, Default)]
pub struct Controller {
    pub buttons: Buttons,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_pad: Stick<i16>,
    pub right_pad: Stick<i16>,
    pub acceleration: Acceleration<i16>,
    pub gyroscope: Gyroscope<i16>,
}

impl Controller {
    pub fn update(&mut self, packet: &[u8; 64]) -> Result<(), Infallible> {
        // if packet[3] != STATUS_INPUT { return Ok(()); }
        
        self.buttons.0 = u32::from_le_bytes(packet[7..11].try_into().unwrap());

        self.left_trigger = packet[11];
        self.right_trigger = packet[12];
        
        self.left_pad.x = i16::from_le_bytes([packet[16], packet[17]]);
        self.left_pad.y = i16::from_le_bytes([packet[18], packet[19]]);
        self.right_pad.x = i16::from_le_bytes([packet[20], packet[21]]);
        self.right_pad.y = i16::from_le_bytes([packet[22], packet[23]]);

        self.acceleration.x  = i16::from_le_bytes([packet[28], packet[29]]);
        self.acceleration.y  = i16::from_le_bytes([packet[30], packet[31]]);
        self.acceleration.z  = i16::from_le_bytes([packet[32], packet[33]]);
        self.gyroscope.pitch = i16::from_le_bytes([packet[34], packet[35]]);
        self.gyroscope.roll  = i16::from_le_bytes([packet[36], packet[37]]);
        self.gyroscope.yaw   = i16::from_le_bytes([packet[38], packet[39]]);

        Ok(())
    }
}