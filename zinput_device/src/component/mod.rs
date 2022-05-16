pub mod analogs;
pub mod buttons;
pub mod controller;
pub mod motion;
pub mod touch_pad;

pub trait ComponentData: Default {
    type Info;

    fn update(&mut self, from: &Self);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Analogs,
    Buttons,
    Controller,
    Motion,
    TouchPad,
}