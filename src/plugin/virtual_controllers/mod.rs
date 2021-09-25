use std::sync::{Arc, atomic::AtomicBool};

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use uuid::Uuid;

use crate::{api::{Plugin, PluginKind, PluginStatus}, zinput::engine::Engine};

use self::{gui::Gui, state::State, vcontroller::VController};

mod gui;
mod state;
mod vcontroller;

const MAX_CONTROLLERS: usize = 16;

pub struct VirtualControllers {
    inner: Mutex<Inner>,
    shared: Shared,
}

impl VirtualControllers {
    pub fn new() -> Self {
        VirtualControllers {
            inner: Mutex::new(Inner::Uninit),
            shared: Shared::new(),
        }
    }
}

impl Plugin for VirtualControllers {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine, self.shared.clone());
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "virtual_controllers"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Custom(format!("Virtual Controllers"))
    }

    fn update_gui(&self, ctx: &eframe::egui::CtxRef, frame: &mut eframe::epi::Frame<'_>, ui: &mut eframe::egui::Ui) {
        self.inner.lock().update_gui(ctx, frame, ui);
    }

    fn on_component_update(&self, id: &Uuid) {
        // todo
    }
}

enum Inner {
    Uninit,
    Init {
        gui: Gui,
        state: State,
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>, shared: Shared) {
        match self {
            Self::Uninit => {
                *self = Self::Init {
                    gui: Gui::new(engine.clone()),
                    state: State::new(engine, shared),
                };
            }
            Self::Init { .. } => {
                self.stop();
                self.init(engine, shared);
            }
        }
    }

    fn stop(&mut self) {
        // todo
        *self = Inner::Uninit;
    }

    fn status(&self) -> PluginStatus {
        match self {
            Self::Uninit => PluginStatus::Stopped,
            Self::Init { .. } => PluginStatus::Running,
        }
    }

    fn update_gui(&mut self, ctx: &eframe::egui::CtxRef, frame: &mut eframe::epi::Frame<'_>, ui: &mut eframe::egui::Ui) {
        match self {
            Self::Init { gui, state } => {
                gui.update(state, ctx, frame, ui);
            }
            Self::Uninit => {}
        }
    }
}

#[derive(Clone)]
struct Shared {
    stop: Arc<AtomicBool>,

    send_vc: Sender<(VController, usize)>,
    recv_vc: Receiver<(VController, usize)>,
}

impl Shared {
    fn new() -> Self {
        let (send_vc, recv_vc) = crossbeam_channel::bounded(1);

        Shared {
            stop: Arc::new(AtomicBool::new(false)),

            send_vc,
            recv_vc,
        }
    }
}