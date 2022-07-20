use std::collections::HashMap;

use bindlang::ty::{ToType, Type, Struct};
use serde::{Deserialize, Serialize};

use super::ComponentData;

#[derive(Clone, PartialEq, Eq)]
pub struct AnalogsInfo {
    pub analogs: u8,
}

impl Default for AnalogsInfo {
    fn default() -> Self {
        AnalogsInfo { analogs: 0 }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AnalogsConfig {
    pub ranges: [[u8; 2]; 8],
}

impl Default for AnalogsConfig {
    fn default() -> Self {
        AnalogsConfig {
            ranges: [[0, 255]; 8],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Analogs {
    pub analogs: [u8; 8],
}

impl ToType for Analogs {
    fn to_type() -> Type {
        // TODO
        Type::Struct(Struct {
            name: "Analogs",
            fields: HashMap::new(),
            size: std::mem::size_of::<Analogs>(),
        })
    }
}

impl Default for Analogs {
    fn default() -> Self {
        Analogs { analogs: [0; 8] }
    }
}

impl ComponentData for Analogs {
    type Info = AnalogsInfo;
    type Config = AnalogsConfig;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, config: &AnalogsConfig) {
        for i in 0..8 {
            let min = config.ranges[i][0] as f32;
            let max = config.ranges[i][1] as f32;
            let range = max - min;
            self.analogs[i] =
                (((f32::clamp(self.analogs[i] as f32, min, max) - min) / range) * 255.0) as u8;
        }
    }
}
