use std::sync::Arc;

use crossbeam_channel::{select, Receiver};
use uuid::Uuid;

use crate::api::Frontend;

pub struct Thread {
    pub update_channel: Receiver<Uuid>,
    pub frontends: Vec<Arc<dyn Frontend + Send + Sync>>,
}

pub fn new_event_thread(thread: Thread) -> impl FnOnce() {
    move || event_thread(thread)
}

fn event_thread(
    Thread {
        update_channel,
        frontends,
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
                    }
                    Err(_) => {}
                }
            }
        }
    }
}
