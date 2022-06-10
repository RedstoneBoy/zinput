use std::sync::Arc;

use zinput_engine::{
    eframe::{self, egui},
    plugin::Plugin,
    Engine,
};

mod device_cfg;
mod device_view;
mod motion_cmp;
mod plugins;

pub struct Gui {
    cfg: device_cfg::DeviceCfg,
    plugins: plugins::PluginConfig,
    cv: device_view::DeviceViewer,
    motion: motion_cmp::MotionCmp,

    first_update: bool,
}

impl Gui {
    pub fn new(engine: Arc<Engine>, plugins: Vec<Arc<dyn Plugin + Send + Sync>>) -> Self {
        Gui {
            cfg: device_cfg::DeviceCfg::new(engine.clone()),
            plugins: plugins::PluginConfig::new(engine.clone(), plugins),
            cv: device_view::DeviceViewer::new(engine.clone()),
            motion: motion_cmp::MotionCmp::new(engine),

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
        
        self.cfg.update(ctx, frame);
        self.plugins.update(ctx, frame);
        self.cv.update(ctx, frame);
        self.motion.update(ctx, frame);
        ctx.request_repaint();
    }
}
