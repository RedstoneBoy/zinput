use std::sync::Arc;

use eframe::{egui, epi};

use crate::{
    api::Plugin,
    zinput::engine::Engine,
};

mod plugins;
mod device_view;
mod motion_cmp;

pub struct Gui {
    plugins: plugins::PluginConfig,
    cv: device_view::DeviceView,
    motion: motion_cmp::MotionCmp,
}

impl Gui {
    pub fn new(
        engine: Arc<Engine>,
        plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
    ) -> Self {
        Gui {
            plugins: plugins::PluginConfig::new(engine.clone(), plugins),
            cv: device_view::DeviceView::new(engine.clone()),
            motion: motion_cmp::MotionCmp::new(engine),
        }
    }
}

impl epi::App for Gui {
    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.plugins.update(ctx, frame);
        self.cv.update(ctx, frame);
        self.motion.update(ctx, frame);
        ctx.request_repaint();
    }

    fn name(&self) -> &str {
        "zinput"
    }
}
