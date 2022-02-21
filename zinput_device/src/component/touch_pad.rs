use super::ComponentData;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TouchPadShape {
    Circle,
    Rectangle,
}

#[derive(Clone)]
pub struct TouchPadInfo {
    pub shape: TouchPadShape,
    pub is_button: bool,
}

impl TouchPadInfo {
    pub fn new(shape: TouchPadShape, is_button: bool) -> Self {
        TouchPadInfo { shape, is_button }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct TouchPad {
    pub touch_x: u16,
    pub touch_y: u16,
    pub pressed: bool,
    pub touched: bool,
}

impl ComponentData for TouchPad {
    type Info = TouchPadInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }
}
