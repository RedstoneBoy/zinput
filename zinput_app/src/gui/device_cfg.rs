use std::sync::Arc;

use zinput_engine::{
    eframe::{self, egui},
    DeviceView, Engine,
};

pub struct DeviceCfg {
    engine: Arc<Engine>,

    selected_controller: Option<DeviceView>,
}

impl DeviceCfg {
    pub fn new(engine: Arc<Engine>) -> Self {
        DeviceCfg {
            engine,

            selected_controller: None,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Device Config").show(ctx, |ui| {
            egui::ComboBox::from_label("Devices")
                .selected_text(
                    self.selected_controller
                        .as_ref()
                        .map_or("[None]".to_owned(), |view| view.info().name.clone()),
                )
                .show_ui(ui, |ui| {
                    let mut index = None;
                    ui.selectable_value(&mut index, None, "[None]");
                    for entry in self.engine.devices() {
                        ui.selectable_value(&mut index, Some(*entry.uuid()), &entry.info().name);
                    }
                    self.selected_controller = index.and_then(|i| self.engine.get_device(&i));
                });

            let (device, device_raw, cfg) = match &self.selected_controller {
                Some(view) => (view.device(), view.device_raw(), view.config()),
                None => return,
            };

            // TODO: Analogs

            if let (Some(controller), Some(controller_raw), Some(cfg)) = (
                device.controllers.get(0),
                device_raw.controllers.get(0),
                cfg.controllers.get(0),
            ) {
                
            }
        });
    }
}
