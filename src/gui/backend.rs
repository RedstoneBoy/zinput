use std::sync::Arc;

use eframe::{egui, epi};

use crate::{api::Backend, zinput::engine::Engine};

pub struct BackendConfig {
    engine: Arc<Engine>,
    backends: Vec<Arc<dyn Backend>>,
    selected_backend: Option<usize>,
}

impl BackendConfig {
    pub fn new(engine: Arc<Engine>, backends: Vec<Arc<dyn Backend>>) -> Self {
        BackendConfig {
            engine,
            backends,
            selected_backend: None,
        }
    }

    pub fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::Window::new("Backend Config").show(ctx, |ui| {
            egui::ComboBox::from_label("Backends")
                .selected_text(match self.selected_backend {
                    Some(i) => self.backends[i].name(),
                    None => "",
                })
                .show_ui(ui, |ui| {
                    for (i, backend) in self.backends.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_backend, Some(i), backend.name());
                    }
                });
            if let Some(backend_index) = self.selected_backend {
                ui.label(format!("status: {}", self.backends[backend_index].status()));
                if ui.button("Restart").clicked() {
                    self.backends[backend_index].stop();
                    self.backends[backend_index].init(self.engine.clone());
                }

                self.backends[backend_index].update_gui(ctx, _frame, ui);
            }
        });
    }
}