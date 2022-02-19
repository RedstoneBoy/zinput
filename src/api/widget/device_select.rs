use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;


pub struct DeviceSelect {
    devices: Arc<RwLock<Vec<Uuid>>>,
}