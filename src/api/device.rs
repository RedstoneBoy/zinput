use uuid::Uuid;

pub struct DeviceInfo {
    pub name: String,
    pub controller: Option<Uuid>,
    pub motion: Option<Uuid>,
}

impl DeviceInfo {
    pub fn new(name: String) -> Self {
        DeviceInfo {
            name,
            controller: None,
            motion: None,
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
}
