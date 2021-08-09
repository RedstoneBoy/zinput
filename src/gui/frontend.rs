use std::sync::Arc;

use eframe::{egui, epi};

use crate::api::Frontend;

pub struct FrontendConfig {
    frontends: Vec<Arc<dyn Frontend>>,
    selected_frontend: Option<usize>,
}

impl FrontendConfig {
    pub fn new(frontends: Vec<Arc<dyn Frontend>>) -> Self {
        FrontendConfig {
            frontends,
            selected_frontend: None,
        }
    }

    pub fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::Window::new("Frontend Config").show(ctx, |ui| {
            egui::ComboBox::from_label("Frontends")
                .selected_text(match self.selected_frontend {
                    Some(i) => self.frontends[i].name(),
                    None => "",
                })
                .show_ui(ui, |ui| {
                    for (i, frontend) in self.frontends.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_frontend, Some(i), frontend.name());
                    }
                });
            if let Some(frontend_index) = self.selected_frontend {
                self.frontends[frontend_index].update_gui(ctx, _frame, ui);
            }
        });
    }
}
