use zinput_engine::{
    eframe::{
        egui,
    },
    DeviceView,
};

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

            let [ax, ay, az] = [motion.accel_x, motion.accel_y, motion.accel_z];

            self.angle.clear();

            let accel_mag = f32::sqrt(ax.powi(2) + ay.powi(2) + az.powi(2));

            log::info!("{accel_mag}");

            if accel_mag >= 0.95 && accel_mag <= 1.05 {
                self.angle.push_str("facing ");
            } else {
                self.angle.push_str("moving ");
            }

            if ax.abs() >= ay.abs() && ax.abs() >= ay.abs() {
                if ax < 0.0 {
                    self.angle.push_str("left");
                } else {
                    self.angle.push_str("right");
                }
            } else if ay.abs() >= ax.abs() && ay.abs() >= az.abs() {
                if ay < 0.0 {
                    self.angle.push_str("up");
                } else {
                    self.angle.push_str("down");
                }
            } else if az.abs() >= ax.abs() && az.abs() >= ay.abs() {
                if az < 0.0 {
                    self.angle.push_str("towards screen");
                } else {
                    self.angle.push_str("towards user");
                }
            }

            ui.label(&self.angle);
        });
    }
}