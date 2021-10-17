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

    pub fn set_num_controllers(&mut self, num: usize) {
        self.buf[0] = u8::min(num as u8, 8);
    }

    pub fn set_controller(&mut self, index: usize, ctrl: &SwiController) {
        let index = usize::min(index, 7);
        let buf = &mut self.buf[(1 + 31 * index)..];
        buf[0] = ctrl.number;
        buf[1] = ctrl.buttons[0];
        buf[2] = ctrl.buttons[1];
        buf[3] = ctrl.left_stick[0];
        buf[4] = ctrl.left_stick[1];
        buf[5] = ctrl.right_stick[0];
        buf[6] = ctrl.right_stick[1];
        write_float(&mut buf[7..], ctrl.accelerometer[0]);
        write_float(&mut buf[11..], ctrl.accelerometer[1]);
        write_float(&mut buf[15..], ctrl.accelerometer[2]);
        write_float(&mut buf[19..], ctrl.gyroscope[0]);
        write_float(&mut buf[23..], ctrl.gyroscope[1]);
        write_float(&mut buf[27..], ctrl.gyroscope[2]);
    }

    pub fn full_buffer(&mut self) -> &mut [u8; PACKET_SIZE] {
        &mut self.buf
    }

    pub fn sendable_buffer(&self) -> &[u8] {
        &self.buf[0..(1 + 31 * self.num_controllers())]
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
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

    pub fn set_pressed(&mut self, button: SwiButton) {
        let buttons = (self.buttons[0] as u16) | ((self.buttons[1] as u16) << 8);
        let buttons = buttons | (1 << button as u16);
        self.buttons[0] = buttons as u8;
        self.buttons[1] = (buttons >> 8) as u8;
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

fn write_float(to: &mut [u8], float: f32) {
    for i in 0..4 {
        to[i] = float.to_le_bytes()[i];
    }
}