use std::sync::Arc;

use crossbeam_channel::{select, Receiver};
use uuid::Uuid;

use crate::api::Plugin;

pub struct Thread {
    pub update_channel: Receiver<Uuid>,
    pub plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
}

pub fn new_event_thread(thread: Thread) -> impl FnOnce() {
    move || event_thread(thread)
}

fn event_thread(
    Thread {
        update_channel,
        plugins,
    }: Thread,
) {
    loop {
        select! {
            recv(update_channel) -> uuid => {
                match uuid {
                    Ok(uuid) => {
                        for plugin in &plugins {
                            plugin.on_component_update(&uuid);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}
