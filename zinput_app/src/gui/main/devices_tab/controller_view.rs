use zinput_engine::{
    device::component::controller::{Controller, ControllerConfig},
    eframe::{
        egui,
        emath::{pos2, Rect},
    },
    DeviceView,
};

use crate::gui::util::view::{slider::Slider, stick::StickView};

use super::ComponentView;

pub struct ControllerView {
    view: DeviceView,
    index: usize,

    configure: bool,
    sample_stick: SampleStick,
}

impl ControllerView {
    pub fn new(view: DeviceView, index: usize) -> Self {
        ControllerView {
            view,
            index,

            configure: true,
            sample_stick: SampleStick::None,
        }
    }

    fn put_stick(
        ui: &mut egui::Ui,
        rect: Rect,
        x: f32,
        y: f32,
        deadzone: Option<&mut f32>,
        calibration: Option<&[f32; 32]>,
        calibrate_button: Option<String>,
    ) -> Option<egui::Response> {
        let mut response = None;

        let mut view = StickView::new(x, y);
        if let Some(deadzone) = &deadzone {
            view = view.deadzone(**deadzone);
        }

        if let Some(calibration) = calibration {
            view = view.polygon(calibration);
        }

        ui.put(rect, view);

        if let Some(text) = calibrate_button {
            response = Some(ui.put(
                Rect {
                    min: pos2(rect.min.x, rect.max.y + 2.0),
                    max: pos2(rect.max.x, rect.max.y + 20.0),
                },
                egui::Button::new(text),
            ));
        }

        let Some(deadzone) = deadzone
        else { return response; };

        ui.put(
            Rect {
                min: pos2(rect.min.x, rect.max.y + 24.0),
                max: pos2(rect.max.x, rect.max.y + 36.0),
            },
            egui::DragValue::new(deadzone)
                .prefix("Deadzone: ")
                .clamp_range(0.0..=1.0)
                .speed(1.0 / 256.0),
        );

        response
    }

    fn put_trigger(
        ui: &mut egui::Ui,
        label: impl Into<String>,
        rect: Rect,
        value: f32,
        right: bool,
        range: Option<&mut [f32; 2]>,
    ) {
        let mut slider = Slider::new(value).label(label).show_values(true);

        if let Some([min, max]) = range {
            slider = slider.min_value(min).max_value(max);
        }

        if right {
            slider = slider.right_to_left();
        }

        ui.put(rect, slider);
    }

    fn get_rects(ui: &mut egui::Ui) -> Rects {
        let max_rect = ui.available_rect_before_wrap();
        let stick_view_size = f32::min(150.0, max_rect.width() / 4.0);

        let lstick = Rect {
            min: pos2(
                max_rect.center().x - stick_view_size - 10.0,
                max_rect.top() + 5.0,
            ),
            max: pos2(
                max_rect.center().x - 10.0,
                max_rect.top() + 5.0 + stick_view_size,
            ),
        };

        let rstick = Rect {
            min: pos2(max_rect.center().x + 10.0, max_rect.top() + 5.0),
            max: pos2(
                max_rect.center().x + stick_view_size + 10.0,
                max_rect.top() + 5.0 + stick_view_size,
            ),
        };

        let lt_x1 = max_rect.left() + 5.0;
        let lt_x2 = lstick.left() - 5.0;
        let rt_x1 = rstick.right() + 5.0;
        let rt_x2 = max_rect.right() - 5.0;
        let t1_center_y = lstick.top() + 40.0;
        let t2_center_y = lstick.bottom() - 40.0;

        let l1 = Rect {
            min: pos2(lt_x1, t1_center_y - 30.0),
            max: pos2(lt_x2, t1_center_y + 30.0),
        };
        let r1 = Rect {
            min: pos2(rt_x1, t1_center_y - 30.0),
            max: pos2(rt_x2, t1_center_y + 30.0),
        };
        let l2 = Rect {
            min: pos2(lt_x1, t2_center_y - 30.0),
            max: pos2(lt_x2, t2_center_y + 30.0),
        };
        let r2 = Rect {
            min: pos2(rt_x1, t2_center_y - 30.0),
            max: pos2(rt_x2, t2_center_y + 30.0),
        };

        Rects {
            lstick,
            rstick,
            l1,
            r1,
            l2,
            r2,
        }
    }

    fn draw_view(
        ui: &mut egui::Ui,
        rects: Rects,
        controller: &Controller,
        mut sample: &mut SampleStick,
        mut concfg: Option<&mut ControllerConfig>,
    ) {
        let lx = (controller.left_stick_x as f32 - 127.5) / 127.5;
        let ly = (controller.left_stick_y as f32 - 127.5) / 127.5;
        let rx = (controller.right_stick_x as f32 - 127.5) / 127.5;
        let ry = (controller.right_stick_y as f32 - 127.5) / 127.5;
        let l1 = controller.l1_analog as f32 / 255.0;
        let r1 = controller.r1_analog as f32 / 255.0;
        let l2 = controller.l2_analog as f32 / 255.0;
        let r2 = controller.r2_analog as f32 / 255.0;

        match &mut sample {
            SampleStick::Left(sampler) => {
                sampler.add(controller.left_stick_x, controller.left_stick_y);
            }
            SampleStick::Right(sampler) => {
                sampler.add(controller.right_stick_x, controller.right_stick_y);
            }
            SampleStick::None => {}
        }

        let mut cfg = concfg.as_ref().map(|cfg| Configurate::from_cfg(cfg));

        // Sticks

        #[derive(Copy, Clone)]
        enum CalType {
            None,
            Start,
            Stop,
        }

        // Left Stick

        let cal_type = if concfg.is_some() {
            match &sample {
                SampleStick::Left(_) => CalType::Stop,
                SampleStick::Right(_) => CalType::None,
                SampleStick::None => CalType::Start,
            }
        } else {
            CalType::None
        };

        let response = Self::put_stick(
            ui,
            rects.lstick,
            lx,
            ly,
            cfg.as_mut().map(|cfg| &mut cfg.left_stick_deadzone),
            match sample {
                SampleStick::Left(Sampler { samples }) => Some(samples),
                _ => concfg.as_ref().and_then(|cfg| cfg.left_stick.samples.as_ref()),
            },
            match cal_type {
                CalType::Start => Some("Calibrate".into()),
                CalType::Stop => Some("Finish Calibration".into()),
                CalType::None => None,
            },
        );

        if let Some(response) = response {
            if response.clicked() {
                match cal_type {
                    CalType::Start => { *sample = SampleStick::Left(Sampler::new()); }
                    CalType::Stop => {
                        let SampleStick::Left(sampler) = std::mem::replace(sample, SampleStick::None)
                        else { unreachable!() };
                        
                        if let Some(concfg) = &mut concfg {
                            concfg.left_stick.samples = Some(sampler.samples);
                        }
                    }
                    CalType::None => {},
                }
            }
        }

        // Right Stick

        let cal_type = if concfg.is_some() {
            match &sample {
                SampleStick::Left(_) => CalType::None,
                SampleStick::Right(_) => CalType::Stop,
                SampleStick::None => CalType::Start,
            }
        } else {
            CalType::None
        };

        let response = Self::put_stick(
            ui,
            rects.rstick,
            rx,
            ry,
            cfg.as_mut().map(|cfg| &mut cfg.right_stick_deadzone),
            match sample {
                SampleStick::Right(Sampler { samples }) => Some(samples),
                _ => concfg.as_ref().and_then(|cfg| cfg.right_stick.samples.as_ref()),
            },
            match cal_type {
                CalType::Start => Some("Calibrate".into()),
                CalType::Stop => Some("Finish Calibration".into()),
                CalType::None => None,
            },
        );

        if let Some(response) = response {
            if response.clicked() {
                match cal_type {
                    CalType::Start => { *sample = SampleStick::Right(Sampler::new()); }
                    CalType::Stop => {
                        let SampleStick::Right(sampler) = std::mem::replace(sample, SampleStick::None)
                        else { unreachable!() };
                        
                        if let Some(concfg) = &mut concfg {
                            concfg.right_stick.samples = Some(sampler.samples);
                        }
                    }
                    CalType::None => {},
                }
            }
        }

        // L1

        Self::put_trigger(
            ui,
            "L1",
            rects.l1,
            l1,
            false,
            cfg.as_mut().map(|cfg| &mut cfg.trigger_ranges[0]),
        );

        // R1

        Self::put_trigger(
            ui,
            "R1",
            rects.r1,
            r1,
            true,
            cfg.as_mut().map(|cfg| &mut cfg.trigger_ranges[1]),
        );

        // L2

        Self::put_trigger(
            ui,
            "L2",
            rects.l2,
            l2,
            false,
            cfg.as_mut().map(|cfg| &mut cfg.trigger_ranges[2]),
        );

        // R2

        Self::put_trigger(
            ui,
            "R2",
            rects.r2,
            r2,
            true,
            cfg.as_mut().map(|cfg| &mut cfg.trigger_ranges[3]),
        );

        if let (Some(cfg), Some(concfg)) = (cfg, concfg) {
            cfg.apply(concfg);
        }
    }
}

impl ComponentView for ControllerView {
    fn update(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("devices/controller/top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.configure, true, "Configure");
                ui.selectable_value(&mut self.configure, false, "View");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_min_width(550.0);

            let rects = Self::get_rects(ui);

            if self.configure {
                let device = self.view.device_raw();
                let Some(controller) = device.controllers.get(self.index)
                else { return; };

                let mut cfg_write = self.view.config_mut();
                let Some(cfg) = cfg_write.get().controllers.get_mut(self.index)
                else { return; };

                Self::draw_view(
                    ui,
                    rects,
                    controller,
                    &mut self.sample_stick,
                    Some(cfg),
                );
            } else {
                let device = self.view.device();
                let Some(controller) = device.controllers.get(self.index)
                else { return; };

                Self::draw_view(ui, rects, controller, &mut SampleStick::None, None);
            }
        });
    }
}

struct Rects {
    lstick: Rect,
    rstick: Rect,
    l1: Rect,
    r1: Rect,
    l2: Rect,
    r2: Rect,
}

struct Configurate {
    left_stick_deadzone: f32,
    right_stick_deadzone: f32,

    trigger_ranges: [[f32; 2]; 4],
}

impl Configurate {
    fn from_cfg(cfg: &ControllerConfig) -> Self {
        fn u8_to_f32(val: u8) -> f32 {
            val as f32 / 255.0
        }

        Configurate {
            left_stick_deadzone: u8_to_f32(cfg.left_stick.deadzone),
            right_stick_deadzone: u8_to_f32(cfg.right_stick.deadzone),

            trigger_ranges: [cfg.l1_range, cfg.r1_range, cfg.l2_range, cfg.r2_range]
                .map(|arr| arr.map(u8_to_f32)),
        }
    }

    fn apply(&self, cfg: &mut ControllerConfig) {
        fn f32_to_u8(val: f32) -> u8 {
            (val * 255.0) as u8
        }

        cfg.left_stick.deadzone = f32_to_u8(self.left_stick_deadzone);
        cfg.right_stick.deadzone = f32_to_u8(self.right_stick_deadzone);
        cfg.l1_range = self.trigger_ranges[0].map(f32_to_u8);
        cfg.r1_range = self.trigger_ranges[1].map(f32_to_u8);
        cfg.l2_range = self.trigger_ranges[2].map(f32_to_u8);
        cfg.r2_range = self.trigger_ranges[3].map(f32_to_u8);
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
        fn index_to_angle(index: usize) -> f32 {
            (index as f32) * (std::f32::consts::PI * 2.0 / 32.0)
        }

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
