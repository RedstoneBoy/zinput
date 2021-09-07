use uuid::Uuid;

pub struct DeviceInfo {
    pub name: String,
    pub controller: Option<Uuid>,
    pub motion: Option<Uuid>,
    pub touch_pads: Vec<Uuid>,
}

impl DeviceInfo {
    pub fn new(name: String) -> Self {
        DeviceInfo {
            name,
            controller: None,
            motion: None,
            touch_pads: Vec::new(),
        }
    }

    pub fn with_controller(mut self, controller: Uuid) -> Self {
        self.controller = Some(controller);
        self
    }

    pub fn with_motion(mut self, motion: Uuid) -> Self {
        self.motion = Some(motion);
        self
    }

    pub fn with_touch_pad(mut self, touch_pad: Uuid) -> Self {
        self.touch_pads.push(touch_pad);
        self
    }
}
