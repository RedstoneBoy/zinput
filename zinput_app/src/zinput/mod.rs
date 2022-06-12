use std::sync::Arc;

use zinput_engine::{eframe, plugin::Plugin, Engine};

use crate::gui::Gui;

pub struct ZInput {
    plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
    engine: Arc<Engine>,
}

impl ZInput {
    pub fn new() -> Self {
        ZInput {
            plugins: Vec::new(),
            engine: Arc::new(Engine::new()),
        }
    }

    pub fn add_plugin(&mut self, plugin: Arc<dyn Plugin + Send + Sync>, init: bool) {
        if init {
            plugin.init(self.engine.clone());
        }

        self.plugins.push(plugin);
    }

    pub fn run(&mut self) {
        let app = Gui::new(self.engine.clone(), self.plugins.clone());
        let options = eframe::NativeOptions::default();

        // TODO: make sure program stops cleanly

        eframe::run_native("zinput", options, Box::new(|_| Box::new(app)));

        /*
        for frontend in &self.frontends {
            frontend.stop();
        }

        for backend in &self.backends {
            backend.stop();
        }

        for handle in std::mem::replace(&mut self.event_thread_handler, None) {
            match handle.join() {
                Ok(()) => {},
                Err(_) => log::error!("event handler thread crashed"),
            }
        }*/
    }
}
