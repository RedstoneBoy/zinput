use crate::{
    common::Stick,
    util::buttons,
};

pub const VENDOR_ID: u16 = 0x057E;
pub const PRODUCT_ID: u16 = 0x0337;

pub const EP_IN: u8 = 0x81;
pub const EP_OUT: u8 = 0x02;

pub const INITIALIZE: [u8; 1] = [0x13];

const STATE_NORMAL: u8 = 0x10;
const STATE_WAVEBIRD: u8 = 0x20;

buttons! {
    Buttons, Button: u16 =>
    A     = 0,
    B     = 1,
    X     = 2,
    Y     = 3,
    Left  = 4,
    Right = 5,
    Down  = 6,
    Up    = 7,
    Start = 8,
    Z     = 9,
    R     = 10,
    L     = 11,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControllerState {
    Normal,
    Wavebird,
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub state: ControllerState,
    pub buttons: Buttons,
    pub left_stick: Stick<u8>,
    pub right_stick: Stick<u8>,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

impl Controller {
    fn parse(packet: [u8; 9]) -> Option<Self> {
        let state = if packet[0] & STATE_NORMAL != 0 {
            ControllerState::Normal
        } else if packet[0] & STATE_WAVEBIRD != 0 {
            ControllerState::Wavebird
        } else {
            return None;
        };

        Some(Controller {
            state,
            buttons: Buttons(packet[1] as u16 | ((packet[2] as u16) << 8)),
            left_stick: Stick { x: packet[3], y: packet[4] },
            right_stick: Stick { x: packet[5], y: packet[6] },
            left_trigger: packet[7],
            right_trigger: packet[8],
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Device {
    pub controllers: [Option<Controller>; 4],
}

impl Device {
    pub fn update(&mut self, packet: &[u8; 37]) -> Result<(), InvalidData> {
        if packet[0] != 0x21 { return Err(InvalidData); }
        for i in 0..4 {
            let packet: [u8; 9] = packet[1 + (i * 9)..][..9].try_into().unwrap();
            self.controllers[i] = Controller::parse(packet);
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct InvalidData;

impl std::error::Error for InvalidData {

}

impl std::fmt::Display for InvalidData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid data")
    }
}