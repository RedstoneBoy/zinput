use std::sync::Arc;

use crate::{
    api::{Backend, Frontend},
    gui::Gui,
};

pub mod engine;

use self::engine::Engine;

pub struct ZInput {
    backends: Vec<Arc<dyn Backend>>,
    frontends: Vec<Arc<dyn Frontend>>,
    engine: Arc<Engine>,
}

impl ZInput {
    pub fn new() -> Self {
        ZInput {
            backends: Vec::new(),
            frontends: Vec::new(),
            engine: Arc::new(Engine::new()),
        }
    }

    pub fn add_backend(&mut self, backend: Arc<dyn Backend>) {
        self.backends.push(backend);
    }

    pub fn add_frontend(&mut self, frontend: Arc<dyn Frontend>) {
        self.frontends.push(frontend);
    }

    pub fn run(&mut self) {
        for backend in &self.backends {
            backend.init(self.engine.clone());
        }

        for frontend in &self.frontends {
            frontend.init(self.engine.clone());
        }

        let app = Gui::new(
            self.engine.clone(),
            self.backends.clone(),
            self.frontends.clone(),
        );
        let options = eframe::NativeOptions::default();

        // TODO: make sure program stops cleanly

        eframe::run_native(Box::new(app), options);
    }
}
