use std::sync::Arc;

use parking_lot::Mutex;
use zinput_engine::{plugin::{Plugin, PluginStatus, PluginKind}, Engine, eframe::{egui, epi}, event::{EventKind, Event}};

mod device_builder;
mod vdevice;

use self::device_builder::DeviceBuilder;
use self::vdevice::VDevice;

pub struct VConPlugin {
    state: Mutex<State>,
}

impl Plugin for VConPlugin {
    fn init(&self, engine: Arc<Engine>) {
        *self.state.lock() = State::init(engine);
    }

    fn stop(&self) {
        self.state.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.state.lock().status()
    }

    fn name(&self) -> &str {
        "vcon"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Custom("vcon".to_owned())
    }

    fn events(&self) -> &[EventKind] {
        &[EventKind::DeviceAdded, EventKind::DeviceRemoved, EventKind::DeviceUpdate]
    }

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, _ui: &mut egui::Ui) {
        
    }

    fn on_event(&self, _event: &Event) {}
}

enum State {
    Uninit,
    Init {
        engine: Arc<Engine>,

        vdevs: Vec<VDevice>,
        vdev_builder: Option<DeviceBuilder>,
    }
}

impl State {
    fn init(engine: Arc<Engine>) -> Self {
        State::Init {
            engine,

            vdevs: Vec::new(),
            vdev_builder: None,
        }
    }

    fn stop(&mut self) {
        *self = State::Uninit;
    }

    fn status(&self) -> PluginStatus {
        match self {
            State::Uninit => PluginStatus::Stopped,
            _ => PluginStatus::Running,
        }
    }


}