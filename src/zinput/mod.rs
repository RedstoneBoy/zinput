use std::{sync::Arc, thread::JoinHandle};

use crossbeam_channel::Receiver;

use crate::{api::{Event, Plugin}, gui::Gui};

pub mod engine;
mod events;

use self::engine::Engine;

pub struct ZInput {
    plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
    engine: Arc<Engine>,

    event_channel: Receiver<Event>,
    event_thread_handler: Option<JoinHandle<()>>,
}

impl ZInput {
    pub fn new() -> Self {
        let (event_sender, event_channel) = crossbeam_channel::bounded(32);

        ZInput {
            plugins: Vec::new(),
            engine: Arc::new(Engine::new(event_sender)),

            event_channel,
            event_thread_handler: None,
        }
    }

    pub fn add_plugin(&mut self, plugin: Arc<dyn Plugin + Send + Sync>) {
        self.plugins.push(plugin);
    }

    pub fn run(&mut self) {
        for plugin in &self.plugins {
            plugin.init(self.engine.clone());
        }

        let event_thread_handler = std::thread::spawn(events::new_event_thread(events::Thread {
            event_channel: self.event_channel.clone(),
            plugins: self.plugins.clone(),
        }));
        self.event_thread_handler = Some(event_thread_handler);

        let app = Gui::new(self.engine.clone(), self.plugins.clone());
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
