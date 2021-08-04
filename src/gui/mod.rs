use std::sync::Arc;

use eframe::{egui, epi};

use crate::{api::Backend, zinput::engine::Engine};

mod backend;
mod device_view;

pub struct Gui {
    backends: backend::BackendConfig,
    cv: device_view::DeviceView,
}

impl Gui {
    pub fn new(engine: Arc<Engine>, backends: Vec<Arc<dyn Backend>>) -> Self {
        Gui {
            backends: backend::BackendConfig::new(engine.clone(), backends),
            cv: device_view::DeviceView::new(engine),
        }
    }
}

impl epi::App for Gui {
    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.backends.update(ctx, frame);
        self.cv.update(ctx, frame);
        ctx.request_repaint();
    }

    fn name(&self) -> &str {
        "zinput"
    }
}