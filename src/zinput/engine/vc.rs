use dashmap::DashMap;
use uuid::Uuid;

use super::Engine;

pub struct Virtual {
    devices: DashMap<Uuid, VirtualDevice>,
}

impl Virtual {
    pub fn update(&self, engine: &Engine, component_id: &Uuid) {}
}

struct VirtualDevice {
    controller: Option<Uuid>,
    motion: Option<Uuid>,
    // mappings: Mappings,
}
