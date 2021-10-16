use std::{convert::TryInto, ops::{Deref, DerefMut}};

pub const MAX_CONTROLLERS: usize = 8;
pub const PACKET_SIZE: usize = 1 + 31 * MAX_CONTROLLERS;

pub struct SwiPacketBuffer {
    buf: [u8; PACKET_SIZE],
}

impl Default for SwiPacketBuffer {
    fn default() -> Self {
        SwiPacketBuffer {
            buf: [0; PACKET_SIZE],
        }
    }
}

impl SwiPacketBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn num_controllers(&self) -> usize {
        usize::min(self.buf[0] as usize, MAX_CONTROLLERS)
    }

    pub fn controller(&self, index: usize) -> Option<SwiController> {
        if index >= self.num_controllers() {
            return None;
        }

        let buf = &self.buf[1 + 31 * index..];
        Some(SwiController {
            number: buf[0],
            buttons: [buf[1], buf[2]],
            left_stick: [buf[3], buf[4]],
            right_stick: [buf[5], buf[6]],
            accelerometer: [
                f32::from_le_bytes(buf[7..11].try_into().unwrap()),
                f32::from_le_bytes(buf[11..15].try_into().unwrap()),
                f32::from_le_bytes(buf[15..19].try_into().unwrap()),
            ],
            gyroscope: [
                f32::from_le_bytes(buf[19..23].try_into().unwrap()),
                f32::from_le_bytes(buf[23..27].try_into().unwrap()),
                f32::from_le_bytes(buf[27..31].try_into().unwrap()),
            ],
        })
    }
}

impl Deref for SwiPacketBuffer {
    type Target = [u8; PACKET_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl DerefMut for SwiPacketBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buf
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwiController {
    pub number: u8,
    pub buttons: [u8; 2],
    pub left_stick: [u8; 2],
    pub right_stick: [u8; 2],
    pub accelerometer: [f32; 3],
    pub gyroscope: [f32; 3],
}

impl SwiController {
    pub fn is_pressed(&self, button: SwiButton) -> bool {
        let buttons = (self.buttons[0] as u16) | ((self.buttons[1] as u16) << 8);
        (buttons >> (button as u16)) & 1 == 1
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SwiButton {
    Minus = 0,
    LStick = 1,
    RStick = 2,
    Plus = 3,
    Up = 4,
    Right = 5,
    Down = 6,
    Left = 7,
    ZL = 8,
    ZR = 9,
    L = 10,
    R = 11,
    Y = 12,
    B = 13,
    A = 14,
    X = 15,
}