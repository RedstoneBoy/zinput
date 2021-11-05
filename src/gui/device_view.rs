use std::sync::Arc;

use eframe::{egui, epi};
use uuid::Uuid;

use crate::{
    api::component::{controller::Button, touch_pad::TouchPadShape},
    zinput::engine::Engine,
};

pub struct DeviceView {
    engine: Arc<Engine>,

    selected_controller: Option<Uuid>,
}

impl DeviceView {
    pub fn new(engine: Arc<Engine>) -> Self {
        DeviceView {
            engine,

            selected_controller: None,
        }
    }

    pub fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::Window::new("Device View").show(ctx, |ui| {
            egui::ComboBox::from_label("Devices")
                .selected_text(
                    self.selected_controller
                        .and_then(|id| self.engine.get_device_info(&id))
                        .map_or("".to_owned(), |dev| dev.name.clone()),
                )
                .show_ui(ui, |ui| {
                    for device_ref in self.engine.devices() {
                        ui.selectable_value(
                            &mut self.selected_controller,
                            Some(*device_ref.id()),
                            &device_ref.name,
                        );
                    }
                });
            
            let device = match self.selected_controller.and_then(|id| self.engine.get_device(&id)) {
                Some(device) => device,
                None => return,
            };

            let device_info = match self.selected_controller.and_then(|id| self.engine.get_device_info(&id)) {
                Some(device_info) => device_info,
                None => return,
            };

            if let Some(controller_data) = device.controllers.get(0)
            {
                ui.heading("Controller");
                egui::Grid::new("controller_buttons").show(ui, |ui| {
                    let mut col = 0;
                    for button in std::array::IntoIter::new(Button::BUTTONS) {
                        let mut label = egui::Label::new(format!("{}", button));
                        if button.is_pressed(controller_data.buttons) {
                            label = label.underline();
                        }
                        ui.add(label);
                        col += 1;
                        if col >= 4 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    for (x, y, name) in [
                        (
                            controller_data.left_stick_x,
                            controller_data.left_stick_y,
                            "Left Stick: ",
                        ),
                        (
                            controller_data.right_stick_x,
                            controller_data.right_stick_y,
                            "Right Stick:",
                        ),
                    ] {
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(format!(
                                    "{} {:+0.2}, {:+0.2}",
                                    name,
                                    (x as f32) / 127.5 - 1.0,
                                    (y as f32) / 127.5 - 1.0
                                ))
                                .monospace(),
                            );

                            let painter = egui::Painter::new(
                                ui.ctx().clone(),
                                ui.layer_id(),
                                egui::Rect {
                                    min: ui.available_rect_before_wrap().min,
                                    max: ui.available_rect_before_wrap().min
                                        + egui::vec2(30.0, 30.0),
                                },
                            );
                            Self::paint_joystick(&painter, x, y, true);
                            ui.expand_to_include_rect(painter.clip_rect());
                        });
                    }
                });

                for (trigger, name) in [
                    (controller_data.l1_analog, "L1"),
                    (controller_data.r1_analog, "R1"),
                    (controller_data.l2_analog, "L2"),
                    (controller_data.r2_analog, "R2"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(format!(
                                "{}: {:+0.2}",
                                name,
                                (trigger as f32) / 255.0
                            ))
                            .monospace(),
                        );

                        let painter = egui::Painter::new(
                            ui.ctx().clone(),
                            ui.layer_id(),
                            egui::Rect {
                                min: ui.available_rect_before_wrap().min,
                                max: ui.available_rect_before_wrap().min + egui::vec2(50.0, 20.0),
                            },
                        );
                        Self::paint_trigger(&painter, trigger);
                        ui.expand_to_include_rect(painter.clip_rect());
                    });
                }
            }

            if let Some(motion_data) = device.motions.get(0)
            {
                ui.separator();

                ui.heading("Motion");

                ui.add(
                    egui::Label::new(format!("Gyro Pitch: {:+0.2}", motion_data.gyro_pitch))
                        .monospace(),
                );
                ui.add(
                    egui::Label::new(format!("Gyro Roll:  {:+0.2}", motion_data.gyro_roll))
                        .monospace(),
                );
                ui.add(
                    egui::Label::new(format!("Gyro Yaw:   {:+0.2}", motion_data.gyro_yaw))
                        .monospace(),
                );

                ui.add(
                    egui::Label::new(format!("Accelerometer X: {:+0.2}", motion_data.accel_x))
                        .monospace(),
                );
                ui.add(
                    egui::Label::new(format!("Accelerometer Y: {:+0.2}", motion_data.accel_y))
                        .monospace(),
                );
                ui.add(
                    egui::Label::new(format!("Accelerometer Z: {:+0.2}", motion_data.accel_z))
                        .monospace(),
                );
            }

            let mut need_separator = true;
            
            for (i, touch_pad) in device
                .touch_pads
                .iter()
                .enumerate()
            {
                if need_separator {
                    ui.separator();
                    need_separator = false;
                }

                ui.heading(format!("Touch Pad #{}", i + 1));
                ui.horizontal(|ui| {
                    let mut label = egui::Label::new("Pressed");
                    if touch_pad.pressed {
                        label = label.underline();
                    }
                    ui.add(label);
                    let mut label = egui::Label::new("Touched");
                    if touch_pad.touched {
                        label = label.underline();
                    }
                    ui.add(label);

                    let painter = egui::Painter::new(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Rect {
                            min: ui.available_rect_before_wrap().min,
                            max: ui.available_rect_before_wrap().min + egui::vec2(60.0, 60.0),
                        },
                    );
                    Self::paint_joystick(
                        &painter,
                        (touch_pad.touch_x / 256) as u8,
                        (touch_pad.touch_y / 256) as u8,
                        device_info.touch_pads[i].shape == TouchPadShape::Circle,
                    );
                    ui.expand_to_include_rect(painter.clip_rect());
                });
            }

            need_separator = true;

            for (analog_comp_index, analog) in device
                .analogs
                .iter()
                .enumerate()
            {
                if need_separator {
                    ui.separator();
                    need_separator = false;
                }

                ui.heading(format!("Analogs #{}", analog_comp_index));

                for i in 0..analog.analogs.len() {
                    let value = analog.analogs[i];
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(format!(
                                "Analog {}: {:+0.2}",
                                i,
                                (value as f32) / 255.0
                            ))
                            .monospace(),
                        );

                        let painter = egui::Painter::new(
                            ui.ctx().clone(),
                            ui.layer_id(),
                            egui::Rect {
                                min: ui.available_rect_before_wrap().min,
                                max: ui.available_rect_before_wrap().min
                                    + egui::vec2(50.0, 20.0),
                            },
                        );
                        Self::paint_trigger(&painter, value);
                        ui.expand_to_include_rect(painter.clip_rect());
                    });
                }
            }

            need_separator = true;

            for (button_comp_index, buttons) in device
                .buttons
                .iter()
                .enumerate()
            {
                if need_separator {
                    ui.separator();
                    need_separator = false;
                }

                ui.heading(format!("Buttons #{}", button_comp_index));

                egui::Grid::new(format!("buttons{}", button_comp_index)).show(ui, |ui| {
                    let mut col = 0;
                    for i in 0..64 {
                        let mut label = egui::Label::new(format!("{}", i));
                        if (buttons.buttons >> i) & 1 == 1 {
                            label = label.underline();
                        }
                        ui.add(label);
                        col += 1;
                        if col >= 8 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                });
            }
        });
    }

    fn paint_joystick(painter: &egui::Painter, x: u8, y: u8, draw_circle: bool) {
        let x = (x as f32) / 255.0;
        let y = 1.0 - (y as f32) / 255.0;

        let clip_rect = painter.clip_rect();

        let offset = egui::vec2(2.0, 2.0);
        let scale = clip_rect.max - clip_rect.min - offset - offset;

        let center = clip_rect.min + offset + egui::vec2(0.5, 0.5) * scale;

        if draw_circle {
            painter.circle_stroke(
                center,
                f32::min(scale.x / 2.0, scale.y / 2.0),
                egui::Stroke::new(1.0, egui::Rgba::from_rgb(0.3, 0.3, 0.3)),
            );
        }

        painter.circle_filled(center, 1.0, egui::Rgba::from_rgb(1.0, 0.3, 0.1));

        painter.rect_stroke(
            clip_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Rgba::from_rgb(1.0, 1.0, 1.0)),
        );

        let point = clip_rect.min + offset + egui::vec2(x, y) * scale;

        painter.circle_filled(point, 2.0, egui::Rgba::from_rgb(0.1, 0.3, 1.0));
    }

    fn paint_trigger(painter: &egui::Painter, trigger: u8) {
        let trigger = (trigger as f32) / 255.0;

        let clip_rect = painter.clip_rect();

        let scale_x = clip_rect.max.x - clip_rect.min.x;

        let fill_rect = egui::Rect {
            min: clip_rect.min,
            max: egui::pos2(clip_rect.min.x + trigger * scale_x, clip_rect.max.y),
        };

        painter.rect_stroke(
            clip_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Rgba::from_rgb(1.0, 1.0, 1.0)),
        );

        painter.rect_filled(fill_rect, 0.0, egui::Rgba::from_rgb(0.3, 0.3, 0.3));
    }
}
