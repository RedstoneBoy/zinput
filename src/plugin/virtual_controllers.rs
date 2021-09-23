use crate::api::{Plugin, PluginKind, PluginStatus};

pub struct VirtualControllers {}

impl VirtualControllers {
    pub fn new() -> Self {
        VirtualControllers {}
    }
}

impl Plugin for VirtualControllers {
    fn init(&self, zinput_api: std::sync::Arc<crate::zinput::engine::Engine>) {
        todo!()
    }

    fn stop(&self) {
        todo!()
    }

    fn status(&self) -> PluginStatus {
        todo!()
    }

    fn name(&self) -> &str {
        "virtual_controllers"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Custom(format!("Virtual Controllers"))
    }
}
