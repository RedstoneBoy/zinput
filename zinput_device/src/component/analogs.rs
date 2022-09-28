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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
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

#[cfg(feature = "bindlang")]
unsafe impl bindlang::ty::BLType for Analogs {
    fn bl_type() -> bindlang::ty::Type {
        bindlang::to_struct!(
            name = Analogs;
            0: a0: u8;
            1: a1: u8;
            2: a2: u8;
            3: a3: u8;
            4: a4: u8;
            5: a5: u8;
            6: a6: u8;
            7: a7: u8;
        )
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
