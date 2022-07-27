use std::{sync::LazyLock, collections::HashMap};

use bindlang::{ty::{BLType, Type}, to_struct};

use super::ComponentData;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TouchPadShape {
    Circle,
    Rectangle,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TouchPadInfo {
    pub shape: TouchPadShape,
    pub is_button: bool,
}

impl TouchPadInfo {
    pub fn new(shape: TouchPadShape, is_button: bool) -> Self {
        TouchPadInfo { shape, is_button }
    }
}

pub type TouchPadConfig = ();

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct TouchPad {
    pub touch_x: u16,
    pub touch_y: u16,
    pub pressed: bool,
    pub touched: bool,
}

unsafe impl BLType for TouchPad {
    fn bl_type() -> Type {
        static TYPE: LazyLock<Type> = LazyLock::new(|| {
            to_struct! {
                name = TouchPad;
                0:  touch_x: u16;
                2:  touch_y: u16;
                4:  pressed: bool;
                5:  touched: bool;
            }
        });
        
        TYPE.clone()
    }
}

impl ComponentData for TouchPad {
    type Config = TouchPadConfig;
    type Info = TouchPadInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _: &Self::Config) {}
}
