use std::sync::Arc;

use zinput_engine::{
    device::component::controller::Controller,
    eframe::{self, egui},
    DeviceView, Engine,
};

pub struct DeviceCfg {
    engine: Arc<Engine>,

    selected_controller: Option<DeviceView>,

    sample: SampleStick,
}

impl DeviceCfg {
    pub fn new(engine: Arc<Engine>) -> Self {
        DeviceCfg {
            engine,

            selected_controller: None,

            sample: SampleStick::None,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut changed = false;
        let mut left_stick_dz = 0.0;
        let mut right_stick_dz = 0.0;
        let mut trigger_mins = [0.0; 4];
        let mut trigger_maxs = [0.0; 4];
        let mut finish_calibration = false;

        egui::Window::new("Device Config").show(ctx, |ui| {
            egui::ComboBox::from_label("Devices")
                .selected_text(
                    self.selected_controller
                        .as_ref()
                        .map_or("[None]".to_owned(), |view| view.info().name.clone()),
                )
                .show_ui(ui, |ui| {
                    let mut index = None;
                    ui.selectable_value(&mut index, None, "[None]");
                    for entry in self.engine.devices() {
                        ui.selectable_value(&mut index, Some(*entry.uuid()), &entry.info().name);
                    }
                    self.selected_controller = index.and_then(|i| self.engine.get_device(&i));
                });

            let (device, device_raw, cfg) = match &self.selected_controller {
                Some(view) => (view.device(), view.device_raw(), view.config()),
                None => return,
            };

            // TODO: Analogs

            if let (Some(controller), Some(controller_raw), Some(cfg)) = (
                device.controllers.get(0),
                device_raw.controllers.get(0),
                cfg.controllers.get(0),
            ) {
                ui.horizontal(|ui| {
                    let painter_uncfg = egui::Painter::new(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Rect {
                            min: ui.available_rect_before_wrap().min,
                            max: ui.available_rect_before_wrap().min + egui::vec2(300.0, 200.0),
                        },
                    );
                    Self::draw_controller(
                        &painter_uncfg,
                        "Unconfigured",
                        controller_raw,
                        &self.sample,
                        Some((cfg.left_stick.deadzone, cfg.right_stick.deadzone)),
                    );
                    ui.expand_to_include_rect(painter_uncfg.clip_rect());

                    let painter_cfg = egui::Painter::new(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Rect {
                            min: egui::pos2(
                                painter_uncfg.clip_rect().right() + 40.0,
                                painter_uncfg.clip_rect().top(),
                            ),
                            max: egui::pos2(
                                painter_uncfg.clip_rect().right() + 40.0,
                                painter_uncfg.clip_rect().top(),
                            ) + egui::vec2(300.0, 200.0),
                        },
                    );
                    Self::draw_controller(&painter_cfg, "Configured", controller, &SampleStick::None, None);
                    ui.expand_to_include_rect(painter_cfg.clip_rect());

                    egui::Painter::new(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Rect {
                            min: egui::pos2(
                                painter_uncfg.clip_rect().right(),
                                painter_uncfg.clip_rect().top(),
                            ),
                            max: egui::pos2(
                                painter_uncfg.clip_rect().right() + 40.0,
                                painter_uncfg.clip_rect().bottom(),
                            ),
                        },
                    )
                    .rect_filled(
                        egui::Rect {
                            min: egui::pos2(
                                painter_uncfg.clip_rect().right() + 20.0 - 1.0,
                                painter_uncfg.clip_rect().top(),
                            ),
                            max: egui::pos2(
                                painter_uncfg.clip_rect().right() + 20.0 + 1.0,
                                painter_uncfg.clip_rect().bottom(),
                            ),
                        },
                        4.0,
                        egui::Color32::GRAY,
                    );
                });

                ui.separator();
                ui.label(egui::RichText::new("Configure").size(24.0));

                left_stick_dz = cfg.left_stick.deadzone as f32 / 255.0;
                right_stick_dz = cfg.right_stick.deadzone as f32 / 255.0;
                trigger_mins[0] = cfg.l1_range[0] as f32 / 255.0;
                trigger_mins[1] = cfg.r1_range[0] as f32 / 255.0;
                trigger_mins[2] = cfg.l2_range[0] as f32 / 255.0;
                trigger_mins[3] = cfg.r2_range[0] as f32 / 255.0;
                trigger_maxs[0] = cfg.l1_range[1] as f32 / 255.0;
                trigger_maxs[1] = cfg.r1_range[1] as f32 / 255.0;
                trigger_maxs[2] = cfg.l2_range[1] as f32 / 255.0;
                trigger_maxs[3] = cfg.r2_range[1] as f32 / 255.0;

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if ui
                            .add(
                                egui::Slider::new(&mut left_stick_dz, 0.0..=1.0)
                                    .text("Left Stick Deadzone"),
                            )
                            .changed()
                        {
                            changed = true;
                        }

                        if ui
                            .add(
                                egui::Slider::new(&mut right_stick_dz, 0.0..=1.0)
                                    .text("Right Stick Deadzone"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    
                        if matches!(&self.sample, SampleStick::None) {
                            if ui.button("Calibrate Left Stick").clicked() {
                                self.sample = SampleStick::Left(Sampler::new());
                            }

                            if ui.button("Calibrate Right Stick").clicked() {
                                self.sample = SampleStick::Right(Sampler::new());
                            }
                        } else {
                            if ui.button("Finish Calibration").clicked() {
                                finish_calibration = true;
                            }
                        }
                    });

                    ui.vertical(|ui| {
                        for i in 0..4 {
                            let text = match i {
                                0 => "L1",
                                1 => "R1",
                                2 => "L2",
                                3 => "R2",
                                _ => unreachable!(),
                            };

                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Slider::new(&mut trigger_mins[i], 0.0..=1.0)
                                            .text(format!("{} Min", text)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }

                                if ui
                                    .add(
                                        egui::Slider::new(&mut trigger_maxs[i], 0.0..=1.0)
                                            .text(format!("{} Max", text)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                        }
                    });
                });

                match &mut self.sample {
                    SampleStick::None => {}
                    SampleStick::Left(sampler) => {
                        sampler.add(controller_raw.left_stick_x, controller_raw.left_stick_y);
                    }
                    SampleStick::Right(sampler) => {
                        sampler.add(controller_raw.right_stick_x, controller_raw.right_stick_y);
                    }
                }
            }
        });

        if changed {
            let mut cfg = match &self.selected_controller {
                Some(view) => view.config_mut(),
                None => return,
            };

            cfg.get().controllers[0].left_stick.deadzone = (left_stick_dz * 255.0) as u8;
            cfg.get().controllers[0].right_stick.deadzone = (right_stick_dz * 255.0) as u8;

            for i in 0..4 {
                if trigger_mins[i] > trigger_maxs[i] {
                    trigger_mins[i] = trigger_maxs[i];
                }
            }

            cfg.get().controllers[0].l1_range[0] = (trigger_mins[0] * 255.0) as u8;
            cfg.get().controllers[0].r1_range[0] = (trigger_mins[1] * 255.0) as u8;
            cfg.get().controllers[0].l2_range[0] = (trigger_mins[2] * 255.0) as u8;
            cfg.get().controllers[0].r2_range[0] = (trigger_mins[3] * 255.0) as u8;
            cfg.get().controllers[0].l1_range[1] = (trigger_maxs[0] * 255.0) as u8;
            cfg.get().controllers[0].r1_range[1] = (trigger_maxs[1] * 255.0) as u8;
            cfg.get().controllers[0].l2_range[1] = (trigger_maxs[2] * 255.0) as u8;
            cfg.get().controllers[0].r2_range[1] = (trigger_maxs[3] * 255.0) as u8;
        }

        if finish_calibration {
            let mut cfg = match &self.selected_controller {
                Some(view) => view.config_mut(),
                None => return,
            };

            match std::mem::replace(&mut self.sample, SampleStick::None) {
                SampleStick::None => {}
                SampleStick::Left(Sampler { samples }) => {
                    cfg.get().controllers[0].left_stick.samples = Some(samples);
                }
                SampleStick::Right(Sampler { samples }) => {
                    cfg.get().controllers[0].right_stick.samples = Some(samples);
                }
            }
        }
    }

    fn draw_controller(
        painter: &egui::Painter,
        name: &str,
        controller: &Controller,
        sample: &SampleStick,
        deadzones: Option<(u8, u8)>,
    ) {
        let font_id = egui::FontId {
            size: 24.0,
            family: egui::FontFamily::Proportional,
        };

        let clip_rect = painter.clip_rect();

        painter.text(
            egui::pos2(clip_rect.center().x, clip_rect.top() + 2.0),
            egui::Align2::CENTER_TOP,
            name,
            font_id,
            painter.ctx().style().visuals.text_color(),
        );

        Self::draw_stick(
            painter,
            "Left",
            controller.left_stick_x,
            controller.left_stick_y,
            egui::pos2(
                clip_rect.left() + 50.0 + 2.0,
                clip_rect.top() + 75.0 + 2.0 + 20.0,
            ),
            match sample {
                SampleStick::Left(s) => Some(s),
                _ => None,
            },
            deadzones.map(|(l, _)| l),
        );

        Self::draw_stick(
            painter,
            "Right",
            controller.right_stick_x,
            controller.right_stick_y,
            egui::pos2(
                clip_rect.right() - 50.0 - 2.0,
                clip_rect.top() + 75.0 + 2.0 + 20.0,
            ),
            match sample {
                SampleStick::Right(s) => Some(s),
                _ => None,
            },
            deadzones.map(|(_, r)| r),
        );

        Self::draw_trigger(
            painter,
            "L1",
            controller.l1_analog,
            egui::pos2(clip_rect.left() + 2.0, clip_rect.bottom() - 30.0),
            true,
        );

        Self::draw_trigger(
            painter,
            "L2",
            controller.l2_analog,
            egui::pos2(clip_rect.left() + 2.0, clip_rect.bottom() - 10.0),
            true,
        );

        Self::draw_trigger(
            painter,
            "R1",
            controller.r1_analog,
            egui::pos2(clip_rect.right() - 2.0, clip_rect.bottom() - 30.0),
            false,
        );

        Self::draw_trigger(
            painter,
            "R2",
            controller.r2_analog,
            egui::pos2(clip_rect.right() - 2.0, clip_rect.bottom() - 10.0),
            false,
        );
    }

    fn draw_stick(
        painter: &egui::Painter,
        name: &str,
        x: u8,
        y: u8,
        pos: egui::Pos2,
        sampler: Option<&Sampler>,
        deadzone: Option<u8>,
    ) {
        let font_id = egui::FontId {
            size: 12.0,
            family: egui::FontFamily::Proportional,
        };
        let mono_font_id = egui::FontId {
            size: 12.0,
            family: egui::FontFamily::Monospace,
        };

        let x = (x as f32 / 127.5) - 1.0;
        let y = (y as f32 / 127.5) - 1.0;

        let deadzone = match deadzone {
            Some(v) => v as f32 / 255.0,
            None => 0.0
        };

        let radius = 50.0;

        painter.circle_stroke(
            pos,
            f32::min(radius, radius),
            egui::Stroke::new(1.0, egui::Rgba::from_rgb(0.3, 0.3, 0.3)),
        );

        painter.circle_filled(pos, deadzone * radius, egui::Rgba::from_rgb(1.0, 0.3, 0.1));
        painter.circle_filled(pos, 1.0, egui::Rgba::from_rgb(1.0, 1.0, 1.0));

        let point = pos + egui::vec2(x, -y) * radius;

        painter.circle_filled(point, 2.0, egui::Rgba::from_rgb(0.1, 0.3, 1.0));

        if let Some(Sampler { samples }) = sampler {
            for i in 0..32 {
                let angle = index_to_angle(i);
                let scalar = samples[i];

                let x = scalar * angle.cos();
                let y = scalar * angle.sin();

                painter.circle_filled(pos + egui::vec2(x, -y) * radius, 1.0, egui::Color32::WHITE);
            }
        }

        let x_text_rect = painter.text(
            egui::pos2(pos.x - radius, pos.y - radius - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{:+.2}", x),
            mono_font_id.clone(),
            painter.ctx().style().visuals.text_color(),
        );

        painter.text(
            egui::pos2(pos.x + radius, pos.y - radius - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("{:+.2}", y),
            mono_font_id.clone(),
            painter.ctx().style().visuals.text_color(),
        );

        painter.text(
            egui::pos2(pos.x, x_text_rect.top() + 2.0),
            egui::Align2::CENTER_BOTTOM,
            name,
            font_id,
            painter.ctx().style().visuals.text_color(),
        );
    }

    fn draw_trigger(painter: &egui::Painter, name: &str, trigger: u8, pos: egui::Pos2, left: bool) {
        let font_id = egui::FontId {
            size: 12.0,
            family: egui::FontFamily::Proportional,
        };
        let mono_font_id = egui::FontId {
            size: 12.0,
            family: egui::FontFamily::Monospace,
        };

        let trigger = trigger as f32 / 255.0;

        let text_rect = painter.text(
            egui::pos2(pos.x, pos.y),
            if left {
                egui::Align2::LEFT_CENTER
            } else {
                egui::Align2::RIGHT_CENTER
            },
            name,
            font_id,
            painter.ctx().style().visuals.text_color(),
        );

        let outline_rect = egui::Rect {
            min: egui::pos2(
                if left {
                    text_rect.right() + 4.0
                } else {
                    text_rect.left() - 4.0 - 80.0
                },
                pos.y - 5.0,
            ),
            max: egui::pos2(
                if left {
                    text_rect.right() + 4.0 + 80.0
                } else {
                    text_rect.left() - 4.0
                },
                pos.y + 5.0,
            ),
        };

        let fill_rect = egui::Rect {
            min: egui::pos2(
                if left {
                    outline_rect.min.x
                } else {
                    outline_rect.max.x - 80.0 * trigger
                },
                outline_rect.min.y,
            ),
            max: egui::pos2(
                if left {
                    outline_rect.min.x + 80.0 * trigger
                } else {
                    outline_rect.max.x
                },
                outline_rect.max.y,
            ),
        };

        painter.rect_stroke(
            outline_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Rgba::from_rgb(1.0, 1.0, 1.0)),
        );

        painter.rect_filled(fill_rect, 0.0, egui::Rgba::from_rgb(0.3, 0.3, 0.3));

        painter.text(
            egui::pos2(
                if left {
                    outline_rect.right() + 4.0
                } else {
                    outline_rect.left() - 4.0
                },
                pos.y,
            ),
            if left {
                egui::Align2::LEFT_CENTER
            } else {
                egui::Align2::RIGHT_CENTER
            },
            format!("{:.2}", trigger),
            mono_font_id,
            painter.ctx().style().visuals.text_color(),
        );
    }
}

enum SampleStick {
    None,
    Left(Sampler),
    Right(Sampler),
}

struct Sampler {
    samples: [f32; 32],
}

impl Sampler {
    fn new() -> Self {
        Sampler { samples: [0.0; 32] }
    }

    fn add(&mut self, x: u8, y: u8) {
        let x = (x as f32 - 127.5) / 127.5;
        let y = (y as f32 - 127.5) / 127.5;
        let scalar = f32::sqrt(x.powi(2) + y.powi(2));
        let mut angle = f32::atan2(y, x);
        if angle < 0.0 {
            angle = 2.0 * std::f32::consts::PI + angle;
        }

        let (mut i1, mut i2) = (0, 0);
        let mut influence = 0.0;

        for i in 0..32 {
            let min_angle = index_to_angle(i);
            let max_angle = index_to_angle(i + 1);
            if min_angle <= angle && angle < max_angle {
                i1 = i;
                i2 = (i + 1) % 32;
                influence = (angle - min_angle) / (max_angle - min_angle);
                break;
            }
        }

        if influence <= 0.5 {
            self.samples[i1] = f32::max(self.samples[i1], scalar);
        }
        if influence >= 0.5 {
            self.samples[i2] = f32::max(self.samples[i2], scalar);
        }
    }
}

fn index_to_angle(index: usize) -> f32 {
    (index as f32) * (std::f32::consts::PI * 2.0 / 32.0)
}
