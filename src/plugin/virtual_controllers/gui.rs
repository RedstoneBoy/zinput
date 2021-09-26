use std::sync::Arc;

use eframe::{egui, epi};

use crate::zinput::engine::Engine;

use super::MAX_CONTROLLERS;
use super::state::State;

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
    
    pub fn update(&mut self, state: &mut State, ctx: &egui::CtxRef, frame: &mut epi::Frame, ui: &mut egui::Ui) {
        self.update_main_window(state, ctx, frame, ui);
    }

    fn update_main_window(&mut self, state: &mut State, ctx: &egui::CtxRef, frame: &mut epi::Frame, ui: &mut egui::Ui) {
        if self.editing {
            self.update_edit_window(state, ctx, frame, ui);
            return;
        }

        egui::Grid::new("vcons_grid")
            .min_col_width(150.0)
            .show(ui, |ui| {
            let mut x = 0;
            for i in 0..=state.vcons.len() {
                if x >= ROW_WIDTH {
                    ui.end_row();
                }

                x += 1;

                ui.vertical(|ui| {
                    ui.label(format!("Virtual Controller {}", i + 1));

                    if i < state.vcons.len() {
                        if ui.button(state.vcons[i].name()).clicked() {
                            // todo
                        }
                    } else if state.vcons.len() < MAX_CONTROLLERS {
                        if ui.button("Add Controller").clicked() {
                            self.edit = EditContext::add_controller();
                            self.editing = true;
                        }
                    }
                });
            }
        });
    }

    fn update_edit_window(&mut self, state: &mut State, ctx: &egui::CtxRef, frame: &mut epi::Frame, ui: &mut egui::Ui) {
        // Profile

        // Inputs
        ui.heading("Input Components");
        
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
}

struct EditContext {
    mode: EditMode,
    profile: VCProfile,
}

impl EditContext {
    fn empty() -> Self {
        EditContext {
            mode: EditMode::Empty,
            profile: VCProfile::new(),
        }
    }

    fn add_controller() -> Self {
        EditContext {
            mode: EditMode::Add,
            profile: VCProfile::new(),
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

struct VCProfile {

}

impl VCProfile {
    fn new() -> Self {
        VCProfile {  }
    }
}