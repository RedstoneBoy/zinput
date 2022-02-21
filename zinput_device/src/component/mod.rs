pub mod analogs;
pub mod buttons;
pub mod controller;
pub mod motion;
pub mod touch_pad;

pub trait ComponentData: Default {
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
