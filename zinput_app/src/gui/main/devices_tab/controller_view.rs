use zinput_engine::{eframe::egui, DeviceView};

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

    }
}