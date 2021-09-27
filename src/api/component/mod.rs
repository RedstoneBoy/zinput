use dsu_protocol::types::Buttons;

use self::{analogs::Analogs, controller::Controller, motion::Motion};

pub mod analogs;
pub mod buttons;
pub mod controller;
pub mod motion;
pub mod schema;
pub mod touch_pad;

pub const COMPONENT_KINDS: [ComponentKind; 5] = [
    ComponentKind::Analogs,
    ComponentKind::Buttons,
    ComponentKind::Controller,
    ComponentKind::Motion,
    ComponentKind::TouchPad,
];

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Analogs,
    Buttons,
    Controller,
    Motion,
    TouchPad,
}

impl std::fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentKind::Analogs => write!(f, "Analogs"),
            ComponentKind::Buttons => write!(f, "Buttons"),
            ComponentKind::Controller => write!(f, "Controller"),
            ComponentKind::Motion => write!(f, "Motion"),
            ComponentKind::TouchPad => write!(f, "Touch Pad"),
        }
    }
}

pub trait ComponentData: Default {
    const KIND: ComponentKind;

    type Info;

    fn update(&mut self, from: &Self);
}

pub struct Component<D: ComponentData> {
    pub info: D::Info,
    pub data: D,
}

impl<D: ComponentData> Component<D> {
    pub fn new(info: D::Info) -> Self {
        Component {
            info,
            data: D::default(),
        }
    }
}
