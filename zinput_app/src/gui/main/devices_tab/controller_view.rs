use zinput_engine::{
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
}

impl ControllerView {
    pub fn new(view: DeviceView, index: usize) -> Self {
        ControllerView {
            view,
            index,

            configure: true,
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
            let device = self.view.device_raw();
            let Some(controller) = device.controllers.get(self.index)
            else { return; };

            let mut cfg_write = self.view.config_mut();
            let Some(cfg) = cfg_write.get().controllers.get_mut(self.index)
            else { return; };

            let lx = (controller.left_stick_x as f32 - 127.5) / 127.5;
            let ly = (controller.left_stick_y as f32 - 127.5) / 127.5;
            let rx = (controller.right_stick_x as f32 - 127.5) / 127.5;
            let ry = (controller.right_stick_y as f32 - 127.5) / 127.5;
            let l1 = controller.l1_analog as f32 / 255.0;
            let r1 = controller.r1_analog as f32 / 255.0;
            let l2 = controller.l2_analog as f32 / 255.0;
            let r2 = controller.r2_analog as f32 / 255.0;

            let l_dz = cfg.left_stick.deadzone as f32 / 255.0;
            let r_dz = cfg.right_stick.deadzone as f32 / 255.0;

            let max_rect = ui.available_rect_before_wrap();
            let stick_view_size = max_rect.width() / 4.0;

            ui.put(
                Rect {
                    min: pos2(
                        max_rect.center().x - stick_view_size - 10.0,
                        max_rect.top() + 5.0,
                    ),
                    max: pos2(
                        max_rect.center().x - 10.0,
                        max_rect.top() + 5.0 + stick_view_size,
                    ),
                },
                {
                    let mut view = StickView::new(lx, ly).deadzone(l_dz);
                    if let Some(samples) = &cfg.left_stick.samples {
                        view = view.polygon(samples);
                    }
                    view
                },
            );

            ui.put(
                Rect {
                    min: pos2(max_rect.center().x + 10.0, max_rect.top() + 5.0),
                    max: pos2(
                        max_rect.center().x + stick_view_size + 10.0,
                        max_rect.top() + 5.0 + stick_view_size,
                    ),
                },
                {
                    let mut view = StickView::new(rx, ry).deadzone(r_dz);
                    if let Some(samples) = &cfg.right_stick.samples {
                        view = view.polygon(samples);
                    }
                    view
                },
            );

            let [mut l1_min, mut l1_max] = cfg.l1_range.map(|v| v as f32 / 255.0);
            let [mut l2_min, mut l2_max] = cfg.l2_range.map(|v| v as f32 / 255.0);
            let [mut r1_min, mut r1_max] = cfg.r1_range.map(|v| v as f32 / 255.0);
            let [mut r2_min, mut r2_max] = cfg.r2_range.map(|v| v as f32 / 255.0);

            ui.put(
                Rect {
                    min: pos2(max_rect.left() + 5.0, max_rect.top() + 5.0),
                    max: pos2(max_rect.left() + 5.0 + 100.0, max_rect.top() + 5.0 + 60.0),
                },
                Slider::new(l1)
                    .label("L1")
                    .show_values(true)
                    .min_value(&mut l1_min)
                    .max_value(&mut l1_max),
            );

            ui.put(
                Rect {
                    min: pos2(max_rect.left() + 5.0, max_rect.top() + 65.0 + 15.0),
                    max: pos2(
                        max_rect.left() + 5.0 + 100.0,
                        max_rect.top() + 65.0 + 15.0 + 60.0,
                    ),
                },
                Slider::new(l2)
                    .label("L2")
                    .show_values(true)
                    .min_value(&mut l2_min)
                    .max_value(&mut l2_max),
            );

            ui.put(
                Rect {
                    min: pos2(max_rect.right() - 5.0 - 100.0, max_rect.top() + 5.0),
                    max: pos2(max_rect.right() - 5.0, max_rect.top() + 5.0 + 60.0),
                },
                Slider::new(r1)
                    .label("R1")
                    .show_values(true)
                    .min_value(&mut r1_min)
                    .max_value(&mut r1_max)
                    .right_to_left(),
            );

            ui.put(
                Rect {
                    min: pos2(max_rect.right() - 5.0 - 100.0, max_rect.top() + 65.0 + 15.0),
                    max: pos2(max_rect.right() - 5.0, max_rect.top() + 65.0 + 15.0 + 60.0),
                },
                Slider::new(r2)
                    .label("R2")
                    .show_values(true)
                    .min_value(&mut r2_min)
                    .max_value(&mut r2_max)
                    .right_to_left(),
            );

            cfg.l1_range = [l1_min, l1_max].map(|v| (v * 255.0) as u8);
            cfg.l2_range = [l2_min, l2_max].map(|v| (v * 255.0) as u8);
            cfg.r1_range = [r1_min, r1_max].map(|v| (v * 255.0) as u8);
            cfg.r2_range = [r2_min, r2_max].map(|v| (v * 255.0) as u8);
        });
    }
}
