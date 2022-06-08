use std::sync::Arc;

use zinput_engine::{eframe::{egui, epi}, util::Uuid, Engine, DeviceView};

pub struct MotionCmp {
    engine: Arc<Engine>,

    dev1: Option<DeviceView>,
    dev2: Option<DeviceView>,

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
                        .map_or("".to_owned(), |view| view.info().name.clone()),
                )
                .show_ui(ui, |ui| {
                    let devices = self.engine.devices();
                    let mut index = None;
                    for (i, info) in devices.iter().enumerate() {
                        ui.selectable_value(
                            &mut index,
                            Some(i),
                            &info.name,
                        );
                    }
                    self.dev1 = index.and_then(|i| devices.get(i));
                });
            egui::ComboBox::from_label("Device 2")
                .selected_text(
                    self.dev2
                        .map_or("".to_owned(), |view| view.info().name.clone()),
                )
                .show_ui(ui, |ui| {
                    let devices = self.engine.devices();
                    let mut index = None;
                    for (i, info) in devices.iter().enumerate() {
                        ui.selectable_value(
                            &mut index,
                            Some(i),
                            &info.name,
                        );
                    }
                    self.dev2 = index.and_then(|i| devices.get(i));
                });

            let (motion1, motion2) = match (&self.dev1, &self.dev2) {
                (Some(view1), Some(view2)) => {
                    let m1 = view1.device().motions.get(0).cloned();
                    let m2 = view2.device().motions.get(0).cloned();

                    match (m1, m2) {
                        (Some(m1), Some(m2)) => (m1, m2),
                        _ => return,
                    }
                }
                _ => return,
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
