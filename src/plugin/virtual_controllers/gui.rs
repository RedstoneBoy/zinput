use std::sync::Arc;

use eframe::{egui, epi};

use crate::zinput::engine::Engine;

use super::state::State;
use super::vcontroller::{mapping::RawMapping, VInput};
use super::MAX_CONTROLLERS;

const ROW_WIDTH: usize = 4;

pub(super) struct Gui {
    engine: Arc<Engine>,

    editing: bool,
    edit: EditContext,
}

impl Gui {
    pub fn new(engine: Arc<Engine>) -> Self {
        Gui {
            engine,

            editing: false,
            edit: EditContext::empty(),
        }
    }

    pub fn update(
        &mut self,
        state: &mut State,
        ctx: &egui::CtxRef,
        frame: &mut epi::Frame,
        ui: &mut egui::Ui,
    ) {
        self.update_main_window(state, ctx, frame, ui);
    }

    fn update_main_window(
        &mut self,
        state: &mut State,
        ctx: &egui::CtxRef,
        frame: &mut epi::Frame,
        ui: &mut egui::Ui,
    ) {
        if self.editing {
            self.update_edit_window(state, ctx, frame, ui);
            return;
        }

        egui::Grid::new("vcons_grid")
            .min_col_width(125.0)
            .min_row_height(50.0)
            .show(ui, |ui| {
                let mut x = 0;
                for i in 0..MAX_CONTROLLERS {
                    if x >= ROW_WIDTH {
                        x = 0;
                        ui.end_row();
                    }

                    x += 1;

                    ui.vertical_centered(|ui| {
                        ui.label(format!("Virtual Controller {}", i + 1));

                        if i < state.vcons.len() {
                            if ui.button(state.vcons[i].name()).clicked() {
                                // todo
                            }
                        } else if i == state.vcons.len() && state.vcons.len() < MAX_CONTROLLERS {
                            if ui.button("Add Controller").clicked() {
                                self.edit = EditContext::add_controller();
                                self.editing = true;
                            }
                        } else if i > state.vcons.len() {
                            ui.label("");
                        }
                    });
                }
            });
    }

    fn update_edit_window(
        &mut self,
        state: &mut State,
        ctx: &egui::CtxRef,
        frame: &mut epi::Frame,
        ui: &mut egui::Ui,
    ) {
        // Profile

        // Input Devices
        self.update_edit_window_input_devices(state, ctx, frame, ui);

        // Components

        // Save / Close buttons
        ui.separator();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(), |ui| {
                if ui.button("Close").clicked() {
                    self.editing = false;
                }

                ui.add(egui::Button::new("Save").enabled(self.edit.can_save()));
            });
        });
    }

    fn update_edit_window_input_devices(
        &mut self,
        state: &mut State,
        ctx: &egui::CtxRef,
        frame: &mut epi::Frame,
        ui: &mut egui::Ui,
    ) {
        ui.heading("Input Devices");
        egui::Grid::new("vcinputgrid").show(ui, |ui| {
            let mut x = 0;
            let mut to_remove = None;
            for i in 0..self.edit.input.devices.len() {
                let cur_device = self.edit.input.devices[i];
                egui::ComboBox::from_label(format!("Device {}", i + 1))
                    .selected_text(
                        self
                            .engine
                            .get_device(&cur_device)
                            .map_or("".to_owned(), |dev| dev.name.clone()),
                    )
                    .show_ui(ui, |ui| {
                        for device in self.engine.devices() {
                            ui.selectable_value(
                                &mut self.edit.input.devices[0],
                                *device.key(),
                                self
                                    .engine
                                    .get_device(device.key())
                                    .map_or("".to_owned(), |dev| dev.name.clone()),
                            );
                        }
                        let mut remove = false;
                        ui.selectable_value(
                            &mut remove,
                            true,
                            "[Remove]"
                        );
                        if remove {
                            to_remove = Some(i);
                        }
                    });
                    
                x += 1;
                if x >= 4 {
                    ui.end_row();
                    x = 0;
                }
            }

            if let Some(to_remove) = to_remove {
                self.edit.input.devices.remove(to_remove);
            }

            let mut selected = None;

            egui::ComboBox::from_label("New Device")
                .show_ui(ui, |ui| {
                    for device in self.engine.devices() {
                        ui.selectable_value(
                            &mut selected,
                            Some(*device.key()),
                            self
                                .engine
                                .get_device(device.key())
                                .map_or("".to_owned(), |dev| dev.name.clone()),
                        );
                    }
                });
            
            if let Some(new_device) = selected {
                self.edit.input.devices.push(new_device);
            }
        });
    }
}

struct EditContext {
    mode: EditMode,
    profile: VCProfile,

    input: VInput,
    mapping: RawMapping,
}

impl EditContext {
    fn empty() -> Self {
        EditContext {
            mode: EditMode::Empty,
            profile: VCProfile::new(),

            input: VInput::new(),
            mapping: RawMapping::default(),
        }
    }

    fn add_controller() -> Self {
        EditContext {
            mode: EditMode::Add,
            profile: VCProfile::new(),

            input: VInput::new(),
            mapping: RawMapping::default(),
        }
    }

    fn can_save(&self) -> bool {
        matches!(&self.mode, EditMode::Add)
    }
}

enum EditMode {
    Empty,
    Add,
}

struct VCProfile {}

impl VCProfile {
    fn new() -> Self {
        VCProfile {}
    }
}
