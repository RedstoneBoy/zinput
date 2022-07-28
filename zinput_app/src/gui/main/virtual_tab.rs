use std::{
    fmt::Write,
    sync::Arc,
};

use paste::paste;
use zinput_engine::{
    device::{component::ComponentKind, components, DeviceInfo},
    eframe::{self, egui},
    util::Uuid,
    DeviceView, Engine,
};

use crate::virt::{VirtualDevices, VDeviceHandle};

use super::Screen;

const NEW_DEVICE_NAME: &'static str = "Virtual Device";

struct VDeviceData {
    info: DeviceInfo,
    handle: Option<VDeviceHandle>,
    views: Vec<DeviceView>,
}

impl VDeviceData {
    fn new(name: String) -> Self {
        VDeviceData {
            info: DeviceInfo::new(name).with_id(Uuid::new_v4().to_string()).autoload_config(true),
            handle: None,
            views: Vec::new(),
        }
    }

    fn enabled(&self) -> bool {
        self.handle.is_some()
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
enum InnerTab {
    Info,
    Bindings,
}

pub struct VirtualTab {
    engine: Arc<Engine>,

    selected: Option<usize>,
    devices: VirtualDevices,
    data: Vec<VDeviceData>,

    itab: InnerTab,
}

impl Screen for VirtualTab {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.show_device_list(ctx);
        self.show_main_panel(ctx);
    }
}

impl VirtualTab {
    pub fn new(engine: Arc<Engine>) -> Self {
        VirtualTab {
            engine: engine.clone(),

            selected: None,
            devices: VirtualDevices::new(engine),
            data: Vec::new(),

            itab: InnerTab::Info,
        }
    }

    fn show_device_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("virtual_device_list").show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| {
                if ui.button("Create").clicked() {
                    self.create_device();
                }
            });

            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let last_selected = self.selected;

                    for (i, data) in self.data.iter().enumerate() {
                        if ui
                            .selectable_value(
                                &mut self.selected,
                                Some(i),
                                &data.info.name,
                            )
                            .clicked()
                        {
                            if last_selected != Some(i) {
                                // self.component = None;
                                // self.viewer = None;
                            }
                        }
                    }
                });
        });
    }

    fn show_main_panel(&mut self, ctx: &egui::Context) {
        let Some(devid) = self.selected
        else { return; };

        if devid >= self.data.len() {
            self.selected = None;
            return;
        }

        // if true, a device was removed, invalidating devid
        self.show_top_bar(ctx, devid);
        
        if self.selected.is_none() {
            return;
        }

        if self.data[devid].enabled() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    let label = egui::RichText::new("Device must be disabled to edit it.")
                        .size(36.0);
                    ui.label(label);
                })
            });

            return;
        }

        match self.itab {
            InnerTab::Info => {
                self.show_itab_info(devid);
            }
            InnerTab::Bindings => {
                self.show_itab_bindings(devid);
            }
        }
    }

    /// Returns true if a device was removed
    fn show_top_bar(&mut self, ctx: &egui::Context, devid: usize) {
        let mut removed = false;

        egui::TopBottomPanel::top("vdevice_top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.itab == InnerTab::Info, "Info").clicked() {
                    self.itab = InnerTab::Info;
                }
                if ui.selectable_label(self.itab == InnerTab::Bindings, "Bindings").clicked() {
                    self.itab = InnerTab::Bindings;
                }
    
                ui.separator();

                let toggle_text = match self.data[devid].enabled() {
                    true => "Disable",
                    false => "Enable",
                };

                if ui.button(toggle_text).clicked() {
                    self.toggle_device(devid);
                }
    
                if ui.button("Delete").clicked() {
                    self.remove_device(devid);
                }
            });
        });
    }

    fn show_itab_info(&mut self, devid: usize) {

    }

    fn show_itab_bindings(&mut self, devid: usize) {
        
    }

    fn create_device(&mut self) {
        let mut name = NEW_DEVICE_NAME.to_owned();
        let mut i = 2;
        while self.data.iter().any(|d| d.info.name == name) {
            name.clear();
            write!(name, "{} {}", NEW_DEVICE_NAME, i).expect("String write error");
            i += 1;
        }

        self.data.push(VDeviceData::new(name));
    }

    fn toggle_device(&mut self, devid: usize) {
        let data = &mut self.data[devid];

        match data.handle {
            Some(handle) => {
                self.devices.remove(handle);
                data.handle = None;
            }
            None => {
                let out = self.engine.new_device(data.info.clone())
                    .expect("virtual_tab: virtual device handle was not dropped");

                let views = data.views.clone();

                data.handle = self.devices.insert(out, views).into();
            }
        }
    }
    
    fn remove_device(&mut self, devid: usize) {
        let data = self.data.remove(devid);

        if let Some(handle) = data.handle {
            self.devices.remove(handle);
        }

        self.selected = None;
    }

    fn save_device_data(&mut self, devid: usize) {
        todo!();
    }
}

trait ComponentView {
    fn update(&mut self, ctx: &egui::Context);
}
