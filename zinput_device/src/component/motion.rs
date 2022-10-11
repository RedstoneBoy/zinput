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

impl Default for MotionInfo {
    fn default() -> Self {
        MotionInfo {
            has_gyro: true,
            has_accel: true,
        }
    }
}

pub type MotionConfig = ();

/// Gyro values are degrees per second
/// Acceleration is in G (1G = 9.8m/s^2)
#[repr(C, align(8))]
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

#[cfg(feature = "bindlang")]
unsafe impl bindlang::ty::BLType for Motion {
    fn bl_type() -> bindlang::ty::Type {
        use std::sync::LazyLock;
        
        static TYPE: LazyLock<bindlang::ty::Type> = LazyLock::new(|| {
            bindlang::to_struct! {
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
