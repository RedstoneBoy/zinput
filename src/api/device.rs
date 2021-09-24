use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub controller: Option<Uuid>,
    pub motion: Option<Uuid>,

    pub analogs: Vec<Uuid>,
    pub buttons: Vec<Uuid>,
    pub touch_pads: Vec<Uuid>,
}

impl DeviceInfo {
    pub fn new(name: String) -> Self {
        DeviceInfo {
            name,
            controller: None,
            motion: None,

            analogs: Vec::new(),
            buttons: Vec::new(),
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

    pub fn with_analogs(mut self, analogs: Uuid) -> Self {
        self.analogs.push(analogs);
        self
    }

    pub fn with_buttons(mut self, buttons: Uuid) -> Self {
        self.buttons.push(buttons);
        self
    }

    pub fn with_touch_pad(mut self, touch_pad: Uuid) -> Self {
        self.touch_pads.push(touch_pad);
        self
    }
}
