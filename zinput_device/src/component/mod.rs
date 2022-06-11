use serde::{Deserialize, Serialize};

pub mod analogs;
pub mod buttons;
pub mod controller;
pub mod motion;
pub mod touch_pad;

pub trait ComponentConfig: Default + Deserialize<'static> + Serialize {}

impl<T> ComponentConfig for T where T: Default + Deserialize<'static> + Serialize {}

pub trait ComponentData: Default {
    type Config: ComponentConfig;
    type Info;

    fn update(&mut self, from: &Self);
    fn configure(&mut self, config: &Self::Config);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Analogs,
    Buttons,
    Controller,
    Motion,
    TouchPad,
}
