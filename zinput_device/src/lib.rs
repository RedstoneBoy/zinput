pub mod component;

use component::{
    analogs::{Analogs, AnalogsInfo},
    buttons::{Buttons, ButtonsInfo},
    controller::{Controller, ControllerInfo},
    motion::{Motion, MotionInfo},
    touch_pad::{TouchPad, TouchPadInfo},
};

#[derive(Clone)]
pub struct DeviceInfo {
    pub name: String,

    pub controllers: Vec<ControllerInfo>,
    pub motions: Vec<MotionInfo>,
    pub analogs: Vec<AnalogsInfo>,
    pub buttons: Vec<ButtonsInfo>,
    pub touch_pads: Vec<TouchPadInfo>,
}

impl DeviceInfo {
    pub fn new(name: String) -> Self {
        DeviceInfo {
            name,

            controllers: Vec::new(),
            motions: Vec::new(),
            analogs: Vec::new(),
            buttons: Vec::new(),
            touch_pads: Vec::new(),
        }
    }

    pub fn add_controller(&mut self, info: ControllerInfo) -> usize {
        self.controllers.push(info);
        self.controllers.len() - 1
    }

    pub fn add_motion(&mut self, info: MotionInfo) -> usize {
        self.motions.push(info);
        self.motions.len() - 1
    }

    pub fn add_analog(&mut self, info: AnalogsInfo) -> usize {
        self.analogs.push(info);
        self.analogs.len() - 1
    }

    pub fn add_button(&mut self, info: ButtonsInfo) -> usize {
        self.buttons.push(info);
        self.buttons.len() - 1
    }

    pub fn add_touch_pad(&mut self, info: TouchPadInfo) -> usize {
        self.touch_pads.push(info);
        self.touch_pads.len() - 1
    }
}

pub struct Device {
    pub controllers: Vec<Controller>,
    pub motions: Vec<Motion>,
    pub analogs: Vec<Analogs>,
    pub buttons: Vec<Buttons>,
    pub touch_pads: Vec<TouchPad>,
}

impl Device {
    pub fn as_mut(&mut self) -> DeviceMut {
        DeviceMut {
            controllers: &mut self.controllers,
            motions: &mut self.motions,
            analogs: &mut self.analogs,
            buttons: &mut self.buttons,
            touch_pads: &mut self.touch_pads,
        }
    }
}

pub struct DeviceMut<'a> {
    pub controllers: &'a mut [Controller],
    pub motions: &'a mut [Motion],
    pub analogs: &'a mut [Analogs],
    pub buttons: &'a mut [Buttons],
    pub touch_pads: &'a mut [TouchPad],
}
