use std::sync::Arc;

use crate::zinput::engine::Engine;

use super::Shared;
use super::vcontroller::VController;

pub(super) struct State {
    pub engine: Arc<Engine>,
    pub shared: Shared,

    pub vcons: Vec<VController>,
}

impl State {
    pub fn new(engine: Arc<Engine>, shared: Shared) -> Self {
        State {
            engine,
            shared,

            vcons: Vec::new(),
        }
    }
}