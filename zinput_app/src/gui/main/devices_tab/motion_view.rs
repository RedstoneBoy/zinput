use zinput_engine::{eframe::egui, DeviceView};

use super::ComponentView;

pub struct MotionView {
    view: DeviceView,
    index: usize,

    angle: String,
}

impl MotionView {
    pub fn new(view: DeviceView, index: usize) -> Self {
        MotionView {
            view,
            index,

            angle: String::new(),
        }
    }
}

impl ComponentView for MotionView {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let device = self.view.device();
            let Some(motion) = device.motions.get(self.index)
            else { return; };

            ui.label(format!("Accel X: {}", motion.accel_x));
            ui.label(format!("Accel Y: {}", motion.accel_y));
            ui.label(format!("Accel Z: {}", motion.accel_z));

            ui.label(format!("Gyro Pitch: {}", motion.gyro_pitch));
            ui.label(format!("Gyro Roll: {}", motion.gyro_roll));
            ui.label(format!("Gyro Yaw: {}", motion.gyro_yaw));
        });
    }
}
