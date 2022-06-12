use std::sync::Arc;

use zinput_engine::{Engine, eframe::{self, egui}, plugin::{Plugin, PluginKind}};

use super::Screen;

pub struct OutputTab {
    engine: Arc<Engine>,
    frontends: Vec<Arc<dyn Plugin + Send + Sync>>,

    selected: usize,
}

impl OutputTab {
    pub fn new(engine: Arc<Engine>, plugins: &[Arc<dyn Plugin + Send + Sync>]) -> Self {
        let frontends = plugins.iter()
            .filter(|p| p.kind() == PluginKind::Frontend)
            .cloned()
            .collect();
        
        OutputTab {
            engine,
            frontends,

            selected: 0,
        }
    }

    fn show_output_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("output_list").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for i in 0..self.frontends.len() {
                        ui.selectable_value(
                            &mut self.selected,
                            i,
                            self.frontends[i].name(),
                        );
                    }
                });
        });
    }

    fn show_main_panel(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(plugin) = self.frontends.get(self.selected)
            else { return; };

            plugin.update_gui(ctx, frame, ui);
        });
    }
}

impl Screen for OutputTab {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show_output_list(ctx);
        self.show_main_panel(ctx, frame);
    }
}