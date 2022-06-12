use zinput_engine::{eframe::{egui, emath::{Rect, vec2, pos2}}, DeviceView};

use crate::gui::util::view::stick::StickView;

use super::ComponentView;

pub struct ControllerView {
    view: DeviceView,
    index: usize,
}

impl ControllerView {
    pub fn new(view: DeviceView, index: usize) -> Self {
        ControllerView {
            view,
            index,
        }
    }
}

impl ComponentView for ControllerView {
    fn update(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let device = self.view.device();
        let Some(controller) = device.controllers.get(0)
        else { return; };

        let lx = (controller.left_stick_x as f32 - 127.5) / 127.5;
        let ly = (controller.left_stick_y as f32 - 127.5) / 127.5;
        let rx = (controller.right_stick_x as f32 - 127.5) / 127.5;
        let ry = (controller.right_stick_y as f32 - 127.5) / 127.5;

        let stick_view_size = ui.max_rect().width() / 4.0;

        ui.put(
            Rect {
                min: pos2(ui.max_rect().center().x - stick_view_size - 10.0, ui.max_rect().top()),
                max: pos2(ui.max_rect().center().x - 10.0, ui.max_rect().top() + stick_view_size),
            },
            StickView::new(lx, ly)
        );

        ui.put(
            Rect {
                min: pos2(ui.max_rect().center().x + 10.0, ui.max_rect().top()),
                max: pos2(ui.max_rect().center().x + stick_view_size + 10.0, ui.max_rect().top() + stick_view_size),
            },
            StickView::new(rx, ry)
        );
    }
}