use std::{collections::HashSet, sync::Arc};

use zinput_engine::{
    eframe::{egui, epi},
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

pub struct PluginConfig {
    engine: Arc<Engine>,
    plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
    show_plugins: HashSet<usize>,

    status_text: String,
}

impl PluginConfig {
    pub fn new(engine: Arc<Engine>, plugins: Vec<Arc<dyn Plugin + Send + Sync>>) -> Self {
        PluginConfig {
            engine,
            plugins,
            show_plugins: HashSet::new(),

            status_text: String::new(),
        }
    }

    pub fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        egui::Window::new("Plugins").show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.plugin_row(ui, |kind| kind == &PluginKind::Backend, "Backend");
                ui.separator();
                self.plugin_row(ui, |kind| kind == &PluginKind::Frontend, "Frontend");
                ui.separator();
                self.plugin_row(ui, |kind| matches!(kind, PluginKind::Custom(_)), "Other");
            });

            ui.separator();

            ui.label(&self.status_text);
        });

        for show in self.show_plugins.iter().cloned().collect::<Vec<_>>() {
            let plugin = &self.plugins[show];

            let mut open = true;

            egui::Window::new(format!("[plugin] {}", plugin.name()))
                .open(&mut open)
                .show(ctx, |ui| {
                    self.plugins[show].update_gui(ctx, frame, ui);
                });

            if !open {
                self.show_plugins.remove(&show);
            }
        }
    }

    fn plugin_row(&mut self, ui: &mut egui::Ui, filter: impl Fn(&PluginKind) -> bool, name: &str) {
        ui.vertical(|ui| {
            ui.heading(name);

            for i in 0..self.plugins.len() {
                if !filter(&self.plugins[i].kind()) {
                    continue;
                }

                ui.horizontal(|ui| {
                    let plugin = &self.plugins[i];

                    let plugin_button =
                        egui::Button::new(plugin.name()).text_color(match plugin.status() {
                            PluginStatus::Running => egui::Color32::GREEN,
                            PluginStatus::Stopped => egui::Color32::WHITE,
                            PluginStatus::Error(_) => egui::Color32::RED,
                        });

                    let plugin_button = ui.add(plugin_button);

                    if plugin_button.hovered() {
                        self.status_text.clear();
                        self.status_text = format!("{}: {}", plugin.name(), plugin.status());
                    }

                    if plugin_button.clicked() {
                        if self.show_plugins.contains(&i) {
                            self.show_plugins.remove(&i);
                        } else {
                            self.show_plugins.insert(i);
                        }
                    }

                    let is_running = plugin.status() == PluginStatus::Running;

                    let start_text = if is_running { "stop" } else { "start" };
                    if ui.button(start_text).clicked() {
                        if is_running {
                            plugin.stop();
                        } else {
                            plugin.init(self.engine.clone());
                        }
                    }
                });
            }
        });
    }
}
