use std::{sync::Arc, thread::JoinHandle};

use crossbeam_channel::Receiver;
use uuid::Uuid;

use crate::{
    api::Plugin,
    gui::Gui,
};

pub mod engine;
mod events;

use self::engine::Engine;

pub struct ZInput {
    backends: Vec<Arc<dyn Plugin + Send + Sync>>,
    frontends: Vec<Arc<dyn Plugin + Send + Sync>>,
    engine: Arc<Engine>,

    update_receiver: Receiver<Uuid>,
    event_thread_handler: Option<JoinHandle<()>>,
}

impl ZInput {
    pub fn new() -> Self {
        let (update_sender, update_receiver) = crossbeam_channel::bounded(32);

        ZInput {
            backends: Vec::new(),
            frontends: Vec::new(),
            engine: Arc::new(Engine::new(update_sender)),

            update_receiver,
            event_thread_handler: None,
        }
    }

    pub fn add_backend(&mut self, backend: Arc<dyn Plugin + Send + Sync>) {
        self.backends.push(backend);
    }

    pub fn add_frontend(&mut self, frontend: Arc<dyn Plugin + Send + Sync>) {
        self.frontends.push(frontend);
    }

    pub fn run(&mut self) {
        for backend in &self.backends {
            backend.init(self.engine.clone());
        }

        for frontend in &self.frontends {
            frontend.init(self.engine.clone());
        }

        let event_thread_handler = std::thread::spawn(events::new_event_thread(events::Thread {
            update_channel: self.update_receiver.clone(),
            frontends: self.frontends.clone(),
            backends: self.backends.clone(),
        }));
        self.event_thread_handler = Some(event_thread_handler);

        let app = Gui::new(
            self.engine.clone(),
            self.backends.clone(),
            self.frontends.clone(),
        );
        let options = eframe::NativeOptions::default();

        // TODO: make sure program stops cleanly

        eframe::run_native(Box::new(app), options);

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
