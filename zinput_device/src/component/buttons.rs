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

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Buttons {
    pub buttons: u64,
}

impl Default for Buttons {
    fn default() -> Self {
        Buttons { buttons: 0 }
    }
}

impl ComponentData for Buttons {
    type Info = ButtonsInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }
}
