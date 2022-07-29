use zinput_engine::{
    device::{
        component::controller::{Analog, Button},
        DeviceInfo,
    },
    eframe::egui,
};

use super::ComponentEditor;

pub struct Editor {
    index: usize,
}

impl Editor {
    pub fn new(index: usize) -> Self {
        Editor { index }
    }
}

impl ComponentEditor for Editor {
    fn update(&mut self, info: &mut DeviceInfo, ctx: &egui::Context) -> bool {
        let Some(controller) = info.controllers.get_mut(self.index)
        else { return false; };

        let mut changed = false;

        egui::SidePanel::left("vcomp_controller").show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| {
                ui.label("Available Buttons");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for button in Button::BUTTONS {
                        let mut has = controller.has_button(button);
                        if ui
                            .selectable_value(&mut has, true, format!("{button}"))
                            .clicked()
                        {
                            controller.set_button(button, !controller.has_button(button));
                            changed = true;
                        }
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| {
                ui.label("Available Analogs");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for analog in Analog::ANALOGS {
                        let mut has = controller.has_analog(analog);
                        if ui
                            .selectable_value(&mut has, true, format!("{analog}"))
                            .clicked()
                        {
                            controller.set_analog(analog, !controller.has_analog(analog));
                            changed = true;
                        }
                    }
                });
            });
        });

        changed
    }
}
