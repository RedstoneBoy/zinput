use zinput_engine::{eframe::egui, DeviceView};

use crate::gui::util::view::stick::StickView;

use super::ComponentView;

pub struct TouchPadView {
    view: DeviceView,
    index: usize,
}

impl TouchPadView {
    pub fn new(view: DeviceView, index: usize) -> Self {
        TouchPadView {
            view,
            index,
        }
    }
}

impl ComponentView for TouchPadView {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let device = self.view.device();
            let Some(touch_pad) = device.touch_pads.get(self.index)
            else { return; };

            let to_stick = |p| ((p as f32) - (u16::MAX as f32 / 2.0)) / (u16::MAX as f32 / 2.0);

            let view = StickView::new(to_stick(touch_pad.touch_x), to_stick(touch_pad.touch_y))
                .draw_pos(touch_pad.touched)
                .draw_center_dot(false)
                .draw_square(true)
                .pos_radius(if touch_pad.pressed { 4.0 } else { 2.0 })
                .min_size(100.0);

            ui.add(view);
        });
    }
}
