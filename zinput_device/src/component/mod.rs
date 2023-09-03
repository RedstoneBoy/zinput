pub mod analogs;
pub mod buttons;
pub mod controller;
pub mod motion;
pub mod mouse;
pub mod touch_pad;

#[cfg(feature = "serde")]
pub trait ComponentConfig: Default + serde::Deserialize<'static> + serde::Serialize {}

#[cfg(not(feature = "serde"))]
pub trait ComponentConfig: Default {}

#[cfg(feature = "serde")]
impl<T> ComponentConfig for T where T: Default + serde::Deserialize<'static> + serde::Serialize {}

#[cfg(not(feature = "serde"))]
impl<T> ComponentConfig for T where T: Default {}

pub trait ComponentData: Default {
    type Config: ComponentConfig;
    type Info;

    fn update(&mut self, from: &Self);
    fn configure(&mut self, config: &Self::Config);
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Analogs = 0,
    Buttons = 1,
    Controller = 2,
    Motion = 3,
    Mouse = 4,
    TouchPad = 5,
}

impl core::fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ComponentKind::Analogs => write!(f, "Analogs"),
            ComponentKind::Buttons => write!(f, "Buttons"),
            ComponentKind::Controller => write!(f, "Controller"),
            ComponentKind::Motion => write!(f, "Motion"),
            ComponentKind::Mouse => write!(f, "Mouse"),
            ComponentKind::TouchPad => write!(f, "Touch Pad"),
        }
    }
}
