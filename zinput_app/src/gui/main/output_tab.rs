use std::sync::Arc;

use zinput_engine::{Engine, eframe::{self, egui}, plugin::{Plugin, PluginKind, PluginStatus}};

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
                        let plugin = &self.frontends[i];

                        let text = egui::RichText::new(plugin.name()).color(match plugin.status() {
                            PluginStatus::Running => egui::Color32::GREEN,
                            PluginStatus::Stopped => egui::Color32::WHITE,
                            PluginStatus::Error(_) => egui::Color32::RED,
                        });

                        ui.selectable_value(
                            &mut self.selected,
                            i,
                            text,
                        );
                    }
                });
        });
    }

    fn show_main_panel(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("output_status").show(ctx, |ui| {
            let Some(plugin) = self.frontends.get(self.selected)
            else { return; };

            ui.horizontal_centered(|ui| {
                ui.label(format!("status: {}", plugin.status()));

                ui.with_layout(egui::Layout::right_to_left(), |ui| {
                    let is_running = plugin.status() == PluginStatus::Running;

                    let button_text = if is_running { "stop" } else { "start" };

                    if ui.button(button_text).clicked() {
                        if is_running {
                            plugin.stop();
                        } else {
                            plugin.init(self.engine.clone());
                        }
                    }
    
                    ui.separator();
                });
            });
        });

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