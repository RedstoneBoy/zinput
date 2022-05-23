#[derive(Copy, Clone, Debug, Default)]
pub struct Stick<T> {
    pub x: T,
    pub y: T,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Acceleration<T> {
    pub x: T,
    pub y: T,
    pub z: T,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Gyroscope<T> {
    pub pitch: T,
    pub roll: T,
    pub yaw: T,
}
