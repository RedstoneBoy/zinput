use std::{sync::LazyLock, collections::HashMap};

use bindlang::{ty::{Type, BLType}, to_struct};

use super::ComponentData;

#[derive(Clone, PartialEq, Eq)]
pub struct MotionInfo {
    pub has_gyro: bool,
    pub has_accel: bool,
}

impl MotionInfo {
    pub fn new(has_gyro: bool, has_accel: bool) -> Self {
        MotionInfo {
            has_gyro,
            has_accel,
        }
    }
}

pub type MotionConfig = ();

/// Gyro values are degrees per second
/// Acceleration is in g (9.8m/s^2)
#[repr(C)]
#[derive(Clone, Default)]
pub struct Motion {
    /// Negative = Pitch forward
    pub gyro_pitch: f32,
    /// Negative = Clockwise
    pub gyro_roll: f32,
    /// Negative = Clockwise
    pub gyro_yaw: f32,
    /// -1.0 = Controller is placed left grip down
    /// 1.0  = Controller is placed right grip down
    pub accel_x: f32,
    /// -1.0 = Controller is placed face up
    /// 1.0  = Controller is placed face down
    pub accel_y: f32,
    /// -1.0 = Controller is placed triggers down
    /// 1.0  = Controller is placed grips down
    pub accel_z: f32,
}

impl BLType for Motion {
    fn bl_type() -> Type {
        static TYPE: LazyLock<Type> = LazyLock::new(|| {
            to_struct! {
                name = Motion;
                0:   pitch:   f32;
                4:   roll:    f32;
                8:   yaw:     f32;
                12:  accel_x: f32;
                16:  accel_y: f32;
                20:  accel_z: f32;
            }
        });
        
        TYPE.clone()
    }
}

impl ComponentData for Motion {
    type Config = MotionConfig;
    type Info = MotionInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _: &Self::Config) {}
}
