use std::sync::Arc;

use eframe::{egui, epi};

use crate::{
    api::{Backend, Frontend},
    zinput::engine::Engine,
};

mod backend;
mod device_view;
mod frontend;
mod motion_cmp;

pub struct Gui {
    backends: backend::BackendConfig,
    frontends: frontend::FrontendConfig,
    cv: device_view::DeviceView,
    motion: motion_cmp::MotionCmp,
}

impl Gui {
    pub fn new(
        engine: Arc<Engine>,
        backends: Vec<Arc<dyn Backend + Send + Sync>>,
        frontends: Vec<Arc<dyn Frontend + Send + Sync>>,
    ) -> Self {
        Gui {
            backends: backend::BackendConfig::new(engine.clone(), backends),
            frontends: frontend::FrontendConfig::new(frontends),
            cv: device_view::DeviceView::new(engine.clone()),
            motion: motion_cmp::MotionCmp::new(engine),
        }
    }
}

impl epi::App for Gui {
    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.backends.update(ctx, frame);
        self.frontends.update(ctx, frame);
        self.cv.update(ctx, frame);
        self.motion.update(ctx, frame);
        ctx.request_repaint();
    }

    fn name(&self) -> &str {
        "zinput"
    }
}
