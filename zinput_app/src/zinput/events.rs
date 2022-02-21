use std::sync::Arc;

use crossbeam_channel::{select, Receiver};
use zinput_engine::{event::{Event, EventKind}, plugin::Plugin};

const T: &'static str = "events";

pub struct Thread {
    pub event_channel: Receiver<Event>,
    pub plugins: Vec<Arc<dyn Plugin + Send + Sync>>,
}

pub fn new_event_thread(thread: Thread) -> impl FnOnce() {
    move || event_thread(thread)
}

fn event_thread(
    Thread {
        event_channel,
        plugins,
    }: Thread,
) {
    const VEC: Vec<usize> = Vec::new();
    // an array of a vector of an index
    // event_plugin_indices[x][y] = z
    // where x = event index
    //       y = internal plugin index
    //       z = plugin index in plugins vector
    let mut event_plugin_indices: [Vec<usize>; Event::NUM_INDICES] = [VEC; Event::NUM_INDICES];
    for (index, plugin) in plugins.iter().enumerate() {
        for ekind in plugin.events() {
            event_plugin_indices[ekind.to_index()].push(index);
        }
    }

    loop {
        select! {
            recv(event_channel) -> event => {
                match event {
                    Ok(event) => {
                        for &plugin_id in &event_plugin_indices[event.to_index()] {
                            plugins[plugin_id].on_event(&event);
                        }
                    }
                    Err(err) => {
                        log::error!(target: T, "event channel received error: {}", err);
                        todo!()
                    }
                }
            }
        }
    }
}

trait EventIndex {
    const NUM_INDICES: usize;

    fn to_index(&self) -> usize;
}

impl EventIndex for Event {
    const NUM_INDICES: usize = EventKind::NUM_INDICES;

    fn to_index(&self) -> usize {
        self.kind().to_index()
    }
}

impl EventIndex for EventKind {
    const NUM_INDICES: usize = 3;

    fn to_index(&self) -> usize {
        match self {
            EventKind::DeviceUpdate => 0,
            EventKind::DeviceAdded => 1,
            EventKind::DeviceRemoved => 2,
        }
    }
}
