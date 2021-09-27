use super::{ComponentData, ComponentKind};

pub struct AnalogsInfo {
    pub analogs: u8,
}

impl Default for AnalogsInfo {
    fn default() -> Self {
        AnalogsInfo { analogs: 0 }
    }
}

#[derive(Copy, Clone)]
pub struct Analogs {
    pub analogs: [u8; 8],
}

impl Default for Analogs {
    fn default() -> Self {
        Analogs { analogs: [0; 8] }
    }
}

impl ComponentData for Analogs {
    const KIND: ComponentKind = ComponentKind::Analogs;
    type Info = AnalogsInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }
}
