use super::ComponentData;

pub struct MotionInfo {
    pub has_gyro: bool,
    pub has_accel: bool,
}

impl Default for MotionInfo {
    fn default() -> Self {
        MotionInfo {
            has_gyro: true,
            has_accel: true,
        }
    }
}

#[derive(Clone, Default)]
pub struct Motion {
    pub gyro_pitch: f32,
    pub gyro_roll: f32,
    pub gyro_yaw: f32,
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
}

impl ComponentData for Motion {
    type Info = MotionInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }
}