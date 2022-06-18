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

impl Default for TouchPadInfo {
    fn default() -> Self {
        TouchPadInfo {
            shape: TouchPadShape::Circle,
            is_button: true,
        }
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

impl ComponentData for TouchPad {
    type Config = TouchPadConfig;
    type Info = TouchPadInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _: &Self::Config) {}
}
