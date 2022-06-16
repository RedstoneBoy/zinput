use std::sync::Arc;

use zinput_engine::{
    eframe::{self, egui},
    plugin::Plugin,
    Engine,
};

mod main;
mod util;

pub struct Gui {
    main_ui: main::MainUi,

    first_update: bool,
}

impl Gui {
    pub fn new(engine: Arc<Engine>, plugins: Vec<Arc<dyn Plugin + Send + Sync>>) -> Self {
        Gui {
            main_ui: main::MainUi::new(engine, plugins),

            first_update: true,
        }
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.first_update {
            self.first_update = false;
            ctx.set_visuals(egui::Visuals::dark());
        }

        self.main_ui.update(ctx, frame);

        ctx.request_repaint();
    }
}
