use bindlang::{ty::{BLType, Type, BitNames}, util::Width};

use super::ComponentData;

#[derive(Clone, PartialEq, Eq)]
pub struct ButtonsInfo {
    pub buttons: u64,
}

impl Default for ButtonsInfo {
    fn default() -> Self {
        ButtonsInfo { buttons: 0 }
    }
}

pub type ButtonsConfig = ();

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Buttons {
    pub buttons: u64,
}

unsafe impl BLType for Buttons {
    fn bl_type() -> Type {
        Type::Bitfield("Buttons", Width::W64, BitNames::default())
    }
}

impl Default for Buttons {
    fn default() -> Self {
        Buttons { buttons: 0 }
    }
}

impl ComponentData for Buttons {
    type Config = ButtonsConfig;
    type Info = ButtonsInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _: &Self::Config) {}
}
