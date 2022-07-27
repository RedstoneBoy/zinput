use std::sync::Arc;

use paste::paste;
use zinput_engine::{
    device::{component::ComponentKind, components},
    eframe::{self, egui},
    util::Uuid,
    DeviceView, Engine,
};

use crate::virt::VirtualDevices;

use super::Screen;

pub struct VirtualTab {
    engine: Arc<Engine>,

    selected: Option<usize>,
    devices: VirtualDevices,
}

impl VirtualTab {
    pub fn new(engine: Arc<Engine>) -> Self {
        VirtualTab {
            engine: engine.clone(),

            selected: None,
            devices: VirtualDevices::new(engine),
        }
    }

    fn show_device_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("virtual_device_list").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let last_selected = self.selected;

                    // for entry in self.engine.devices() {
                    //     if ui
                    //         .selectable_value(
                    //             &mut self.selected,
                    //             Some(*entry.uuid()),
                    //             &entry.info().name,
                    //         )
                    //         .clicked()
                    //     {
                    //         if last_selected != Some(*entry.uuid()) {
                    //             self.component = None;
                    //             self.viewer = None;
                    //         }
                    //     }
                    // }
                });
        });
    }

    fn show_main_panel(&mut self, ctx: &egui::Context) {
        let Some(selected) = self.selected
        else { return; };

        Self::show_save_window(ctx, &mut self.config_save, &view);

        if self.component.is_none() && view.info().controllers.len() > 0 {
            self.component = Some(Default::default());
            self.viewer = get_component_view(ComponentKind::Controller, 0, view);
            return;
        }

        egui::TopBottomPanel::top("component_select").show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Component")
                    .selected_text(self.component.map_or(String::new(), |c| format!("{c}")))
                    .show_ui(ui, |ui| {
                        self.add_components(ui, view.clone());
                    });

                ui.separator();

                ui.label("Configs");

                Self::config_button(ui, "Load", &view, &mut self.configs, |ui, configs| {
                    for config in configs {
                        let text = egui::WidgetText::from(config);
                        let galley = text.into_galley(
                            ui,
                            Some(false),
                            f32::INFINITY,
                            egui::TextStyle::Button,
                        );
                        let new_width = galley.size().x + ui.spacing().item_spacing.x * 2.0;
                        if new_width > ui.min_size().x {
                            ui.set_min_width(new_width);
                        }

                        if ui.selectable_label(false, config).clicked() {
                            match view.load_config(config) {
                                Ok(()) => {}
                                Err(err) => {
                                    log::error!("failed to load config: {err:?}");
                                }
                            }
                        }
                    }
                });

                Self::config_button(ui, "Save", &view, &mut self.configs, |ui, configs| {
                    ui.set_min_width(50.0);

                    if ui.selectable_label(false, "New...").clicked() {
                        self.config_save = Some(String::new());
                    }

                    ui.separator();

                    for config in configs {
                        let text = egui::WidgetText::from(config);
                        let galley = text.into_galley(
                            ui,
                            Some(false),
                            f32::INFINITY,
                            egui::TextStyle::Button,
                        );
                        let new_width = galley.size().x + ui.spacing().item_spacing.x * 2.0;
                        if new_width > ui.min_size().x {
                            ui.set_min_width(new_width);
                        }

                        if ui.selectable_label(false, config).clicked() {
                            match view.save_config(config) {
                                Ok(()) => {}
                                Err(err) => {
                                    log::error!("failed to save config: {err:?}");
                                }
                            }
                        }
                    }
                });

                Self::config_button(ui, "Delete", &view, &mut self.configs, |ui, configs| {
                    for config in configs {
                        let text = egui::WidgetText::from(config);
                        let galley = text.into_galley(
                            ui,
                            Some(false),
                            f32::INFINITY,
                            egui::TextStyle::Button,
                        );
                        let new_width = galley.size().x + ui.spacing().item_spacing.x * 2.0;
                        if new_width > ui.min_size().x {
                            ui.set_min_width(new_width);
                        }

                        if ui.selectable_label(false, config).clicked() {
                            match view.delete_config(config) {
                                Ok(()) => {}
                                Err(err) => {
                                    log::error!("failed to delete config: {err:?}");
                                }
                            }
                        }
                    }
                });

                if ui.button("Reset").clicked() {
                    view.reset_config();
                }
            });
        });

        let Some(viewer) = &mut self.viewer
        else { return; };

        viewer.update(ctx);
    }

    fn add_components(&mut self, ui: &mut egui::Ui, view: DeviceView) {
        macro_rules! add_comps {
            ($($cname:ident : $ckind:expr),* $(,)?) => {
                let last_component = self.component;
                paste! {
                    $(
                        for i in 0..view.info().[< $cname s >].len() {
                            let selection = ComponentSelection {
                                kind: $ckind,
                                index: i,
                            };
                            let text = format!("{selection}");
                            if ui.selectable_value(
                                &mut self.component,
                                Some(selection),
                                text,
                            ).clicked() {
                                if last_component != Some(selection) {
                                    self.viewer = get_component_view($ckind, i, view);
                                    return;
                                }
                            }
                        }
                    )*
                }
            };
        }

        components!(kind add_comps);
    }

    fn show_save_window(ctx: &egui::Context, save_file: &mut Option<String>, view: &DeviceView) {
        if save_file.is_none() {
            return;
        }

        egui::Window::new("Save")
            .resizable(false)
            .collapsible(false)
            .auto_sized()
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(save_file.as_mut().unwrap());

                    if ui.button("Save").clicked() {
                        match view.save_config(save_file.as_ref().unwrap()) {
                            Ok(()) => {}
                            Err(err) => {
                                log::error!("failed to save config file: {err:?}");
                            }
                        }

                        save_file.as_mut().unwrap().clear();

                        *save_file = None;
                    }

                    if ui.button("Cancel").clicked() {
                        *save_file = None;
                    }
                });
            });
    }

    fn config_button(
        ui: &mut egui::Ui,
        name: impl Into<String>,
        view: &DeviceView,
        configs: &mut Vec<String>,
        add_contents: impl FnOnce(&mut egui::Ui, &[String]),
    ) {
        let name = name.into();

        let response = ui.button(&name);
        let popup_id = ui.make_persistent_id(format!("devices/configs/{name}"));
        if response.clicked() {
            ui.memory().toggle_popup(popup_id);
            if ui.memory().is_popup_open(popup_id) {
                match view.saved_configs() {
                    Ok(cfgs) => {
                        *configs = cfgs;
                    }
                    Err(err) => {
                        log::warn!("failed to get config list: {err:?}");
                    }
                }
            }
        }

        egui::popup_below_widget(ui, popup_id, &response, |ui| {
            add_contents(ui, configs);
        });
    }
}

impl Screen for VirtualTab {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.show_device_list(ctx);
        self.show_main_panel(ctx);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct ComponentSelection {
    kind: ComponentKind,
    index: usize,
}

impl Default for ComponentSelection {
    fn default() -> Self {
        ComponentSelection {
            kind: ComponentKind::Controller,
            index: 0,
        }
    }
}

impl std::fmt::Display for ComponentSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if self.index > 0 {
            write!(f, " {}", self.index + 1)?;
        }

        Ok(())
    }
}

fn get_component_view(
    kind: ComponentKind,
    index: usize,
    device: DeviceView,
) -> Option<Box<dyn ComponentView>> {
    match kind {
        ComponentKind::Controller => Some(Box::new(ControllerView::new(device, index))),
        ComponentKind::Motion => Some(Box::new(MotionView::new(device, index))),
        _ => None,
    }
}

trait ComponentView {
    fn update(&mut self, ctx: &egui::Context);
}
