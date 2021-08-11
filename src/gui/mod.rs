use std::sync::Arc;

use eframe::{egui, epi};

use crate::{
    api::{Backend, Frontend},
    zinput::engine::Engine,
};

mod backend;
mod device_view;
mod frontend;

pub struct Gui {
    backends: backend::BackendConfig,
    frontends: frontend::FrontendConfig,
    cv: device_view::DeviceView,
}

impl Gui {
    pub fn new(
        engine: Arc<Engine>,
        backends: Vec<Arc<dyn Backend>>,
        frontends: Vec<Arc<dyn Frontend + Send + Sync>>,
    ) -> Self {
        Gui {
            backends: backend::BackendConfig::new(engine.clone(), backends),
            frontends: frontend::FrontendConfig::new(frontends),
            cv: device_view::DeviceView::new(engine),
        }
    }
}

impl epi::App for Gui {
    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.backends.update(ctx, frame);
        self.frontends.update(ctx, frame);
        self.cv.update(ctx, frame);
        ctx.request_repaint();
    }

    fn name(&self) -> &str {
        "zinput"
    }
}
