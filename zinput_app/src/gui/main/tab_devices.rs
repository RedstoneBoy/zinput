use zinput_engine::eframe::{self, egui};

use super::Screen;

pub struct DevicesTab {
    selected: Option<u8>,
}

impl DevicesTab {
    pub fn new() -> Self {
        DevicesTab { selected: None }
    }
}

impl Screen for DevicesTab {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::SidePanel::left("device_list").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for i in 0..=255 {
                    ui.selectable_value(&mut self.selected, Some(i), format!("Device {i}"));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(selected) = self.selected
            else { return; };
        });
    }
}
