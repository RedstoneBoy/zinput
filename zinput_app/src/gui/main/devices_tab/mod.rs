use std::sync::Arc;

use paste::paste;
use zinput_engine::{
    device::{component::ComponentKind, components},
    eframe::{self, egui},
    util::Uuid,
    Engine, DeviceView,
};

use self::controller_view::ControllerView;

use super::Screen;

mod controller_view;

pub struct DevicesTab {
    engine: Arc<Engine>,

    selected: Option<Uuid>,

    component: Option<ComponentSelection>,
    viewer: Option<Box<dyn ComponentView>>,
}

impl DevicesTab {
    pub fn new(engine: Arc<Engine>) -> Self {
        DevicesTab {
            engine,

            selected: None,

            component: None,
            viewer: None,
        }
    }

    fn show_device_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("device_list").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let last_selected = self.selected;

                    for entry in self.engine.devices() {
                        if ui.selectable_value(
                            &mut self.selected,
                            Some(*entry.uuid()),
                            &entry.info().name,
                        ).clicked() {
                            if last_selected != Some(*entry.uuid()) {
                                self.component = None;
                                self.viewer = None;
                            }
                        }
                    }
                });
        });
    }

    fn show_main_panel(&mut self, ctx: &egui::Context) {
        let Some(selected) = self.selected
        else { return; };

        let Some(view) = self.engine.get_device(&selected)
        else { return; };

        if self.component.is_none() && view.info().controllers.len() > 0 {
            self.component = Some(Default::default());
            self.viewer = get_component_view(ComponentKind::Controller, 0, view);
            return;
        }

        egui::TopBottomPanel::top("component_select").show(ctx, |ui| {
            egui::ComboBox::from_label("Component")
                .selected_text(self.component.map_or(String::new(), |c| format!("{c}")))
                .show_ui(ui, |ui| {
                    self.add_components(ui, view);
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
}

impl Screen for DevicesTab {
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

fn get_component_view(kind: ComponentKind, index: usize, device: DeviceView) -> Option<Box<dyn ComponentView>> {
    match kind {
        ComponentKind::Controller => Some(Box::new(ControllerView::new(device, index))),
        _ => None,
    }
}

trait ComponentView {
    fn update(&mut self, ctx: &egui::Context);
}