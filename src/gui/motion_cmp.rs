use std::sync::Arc;

use eframe::{egui, epi};
use uuid::Uuid;

use crate::zinput::engine::Engine;

pub struct MotionCmp {
    engine: Arc<Engine>,

    dev1: Option<Uuid>,
    dev2: Option<Uuid>,

    average: Vec<(f32, f32, f32)>,
    index: usize,
}

impl MotionCmp {
    pub fn new(engine: Arc<Engine>) -> Self {
        MotionCmp {
            engine,

            dev1: None,
            dev2: None,

            average: Vec::new(),
            index: 0,
        }
    }

    pub fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::Window::new("Motion Compare").show(ctx, |ui| {
            egui::ComboBox::from_label("Device 1")
                .selected_text(
                    self.dev1
                        .and_then(|id| self.engine.get_device(&id))
                        .map_or("".to_owned(), |dev| dev.name.clone()),
                )
                .show_ui(ui, |ui| {
                    for device_ref in self.engine.devices() {
                        ui.selectable_value(
                            &mut self.dev1,
                            Some(*device_ref.key()),
                            &device_ref.name,
                        );
                    }
                });
            egui::ComboBox::from_label("Device 2")
                .selected_text(
                    self.dev2
                        .and_then(|id| self.engine.get_device(&id))
                        .map_or("".to_owned(), |dev| dev.name.clone()),
                )
                .show_ui(ui, |ui| {
                    for device_ref in self.engine.devices() {
                        ui.selectable_value(
                            &mut self.dev2,
                            Some(*device_ref.key()),
                            &device_ref.name,
                        );
                    }
                });

            let (motion1, motion2) = if let (Some(dev1), Some(dev2)) = (
                self.dev1.and_then(|id| self.engine.get_device(&id)),
                self.dev2.and_then(|id| self.engine.get_device(&id)),
            ) {
                if let (Some(motion1), Some(motion2)) = (
                    dev1.components.motion.and_then(|id| self.engine.get_motion(&id)),
                    dev2.components.motion.and_then(|id| self.engine.get_motion(&id)),
                ) {
                    let motion1 = motion1.data.clone();
                    let motion2 = motion2.data.clone();

                    (motion1, motion2)
                } else {
                    (Default::default(), Default::default())
                }
            } else {
                (Default::default(), Default::default())
            };

            if self.average.len() >= 30 {
                self.average[self.index] = (
                    motion1.gyro_pitch / motion2.gyro_pitch,
                    motion1.gyro_yaw / motion2.gyro_yaw,
                    motion1.gyro_roll / motion2.gyro_roll,
                );
                self.index += 1;
                if self.index >= 30 {
                    self.index = 0;
                }
            } else {
                self.average.push((
                    motion1.gyro_pitch / motion2.gyro_pitch,
                    motion1.gyro_yaw / motion2.gyro_yaw,
                    motion1.gyro_roll / motion2.gyro_roll,
                ));
            }

            ui.add(
                egui::Label::new(format!(
                    "Gyro Pitch: {:+0.02}",
                    self.average.iter().map(|(v, _, _)| v).sum::<f32>() / 30.0
                ))
                .monospace(),
            );
            ui.add(
                egui::Label::new(format!(
                    "Gyro Yaw: {:+0.02}",
                    self.average.iter().map(|(_, v, _)| v).sum::<f32>() / 30.0
                ))
                .monospace(),
            );
            ui.add(
                egui::Label::new(format!(
                    "Gyro Roll: {:+0.02}",
                    self.average.iter().map(|(_, _, v)| v).sum::<f32>() / 30.0
                ))
                .monospace(),
            );
        });
    }
}
