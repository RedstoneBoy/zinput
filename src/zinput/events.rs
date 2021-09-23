use std::sync::Arc;

use crossbeam_channel::{select, Receiver};
use uuid::Uuid;

use crate::api::Plugin;

pub struct Thread {
    pub update_channel: Receiver<Uuid>,
    pub frontends: Vec<Arc<dyn Plugin + Send + Sync>>,
    pub backends: Vec<Arc<dyn Plugin + Send + Sync>>,
}

pub fn new_event_thread(thread: Thread) -> impl FnOnce() {
    move || event_thread(thread)
}

fn event_thread(
    Thread {
        update_channel,
        frontends,
        backends,
    }: Thread,
) {
    loop {
        select! {
            recv(update_channel) -> uuid => {
                match uuid {
                    Ok(uuid) => {
                        for frontend in &frontends {
                            frontend.on_component_update(&uuid);
                        }

                        for backend in &backends {
                            backend.on_component_update(&uuid);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}
