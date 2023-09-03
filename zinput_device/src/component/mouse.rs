use serde::{Serialize, Deserialize};

use super::ComponentData;


#[derive(Clone, Default, PartialEq, Eq)]
pub struct MouseInfo {}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct MouseConfig {
    pub sensitivity: f32,
}

#[derive(Copy, Clone, Default)]
pub struct Mouse {
    
}

impl ComponentData for Mouse {
    type Config = MouseConfig;
    type Info = MouseInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _config: &Self::Config) {
        todo!()
    }
}

#[cfg(feature = "bindlang")]
unsafe impl bindlang::ty::BLType for Mouse {
    fn bl_type() -> bindlang::ty::Type {
        use std::sync::LazyLock;

        static TYPE: LazyLock<bindlang::ty::Type> = LazyLock::new(|| {
            bindlang::to_struct! {
                name = Mouse;
            }
        });

        TYPE.clone()
    }
}