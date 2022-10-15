use std::{fmt::Write, sync::Arc};

use paste::paste;
use zinput_engine::{
    device::{component::ComponentKind, components, DeviceInfo},
    eframe::{self, egui},
    util::Uuid,
    DeviceView, Engine,
};

use crate::virt::{VDeviceHandle, VirtualDevices};

use super::Screen;

mod controller;

const NEW_DEVICE_NAME: &'static str = "Virtual Device";

struct VDeviceData {
    info: DeviceInfo,
    handle: Option<VDeviceHandle>,
    views: Vec<DeviceView>,

    code: String,
}

impl VDeviceData {
    fn new(name: String) -> Self {
        let mut info = DeviceInfo::new(name)
            .with_id(Uuid::new_v4().to_string())
            .autoload_config(true);
        info.add_controller(Default::default());

        VDeviceData {
            info,
            handle: None,
            views: Vec::new(),

            code: String::new(),
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

    info_selected: ComponentSelection,
    info_editor: Option<Box<dyn ComponentEditor>>,

    bindings_input_selected: usize,
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

            info_selected: Default::default(),
            info_editor: None,

            bindings_input_selected: 0,
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
                            .selectable_value(&mut self.selected, Some(i), &data.info.name)
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
                    let label =
                        egui::RichText::new("Device must be disabled to edit it.").size(36.0);
                    ui.label(label);
                })
            });

            return;
        }

        match self.itab {
            InnerTab::Info => {
                self.show_itab_info(ctx, devid);
            }
            InnerTab::Bindings => {
                self.show_itab_bindings(ctx, devid);
            }
        }
    }

    /// Returns true if a device was removed
    fn show_top_bar(&mut self, ctx: &egui::Context, devid: usize) {
        egui::TopBottomPanel::top("vdevice_top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.itab == InnerTab::Info, "Info")
                    .clicked()
                {
                    self.itab = InnerTab::Info;
                }
                if ui
                    .selectable_label(self.itab == InnerTab::Bindings, "Bindings")
                    .clicked()
                {
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

    fn show_itab_info(&mut self, ctx: &egui::Context, devid: usize) {
        egui::TopBottomPanel::top("vdevice_info_top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                if ui
                    .text_edit_singleline(&mut self.data[devid].info.name)
                    .changed()
                {
                    // TODO: set save flag
                }

                ui.separator();

                ui.label("Components");
                let add_resp = ui.button("Add");
                let popup_id = ui.make_persistent_id("vcomp_add_popup");

                if add_resp.clicked() {
                    ui.memory().toggle_popup(popup_id);
                }

                egui::popup_below_widget(ui, popup_id, &add_resp, |ui| {
                    let info = &mut self.data[devid].info;
                    macro_rules! comp_add {
                        ($($cname:ident : $ckind:expr),* $(,)?) => {
                            paste! {
                                $(
                                    {
                                        let text = egui::WidgetText::from(format!("{}", $ckind));
                                        let galley = text.into_galley(
                                            ui,
                                            Some(false),
                                            f32::INFINITY,
                                            egui::TextStyle::Button,
                                        );
                                        let new_width = galley.size().x + ui.spacing().item_spacing.x * 2.0;
                                        if new_width > ui.min_size().x {
                                            ui.set_min_width(new_width);
                                        }

                                        if ui.selectable_label(false, format!("{}", $ckind)).clicked() {
                                            info.[< $cname s >].push(Default::default());
                                        }
                                    }
                                )*
                            }
                        }
                    }

                    components!(kind comp_add);
                });

                if ui.button("Remove").clicked() {
                    self.info_selected.remove(&mut self.data[devid].info);
                }
            });
        });

        // components
        egui::SidePanel::left("vdevice_component_list").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let last_selected = self.info_selected;

                    let info = &mut self.data[devid].info;
                    macro_rules! list_comp {
                        ($($cname:ident : $ckind:expr),* $(,)?) => {
                            paste! {
                                $(
                                    for i in 0..info.[< $cname s >].len() {
                                        let selection = ComponentSelection { kind: $ckind, index: i };
                                        ui.selectable_value(
                                            &mut self.info_selected,
                                            selection,
                                            format!("{}", selection),
                                        );
                                    }
                                )*
                            }
                        };
                    }

                    components!(kind list_comp);

                    if last_selected != self.info_selected || self.info_editor.is_none() {
                        self.info_editor = Some(self.info_selected.get_editor());
                    }
                });
        });

        let Some(editor) = &mut self.info_editor
        else { return; };

        if editor.update(&mut self.data[devid].info, ctx) {
            // TODO: set save flag
        }
    }

    fn show_itab_bindings(&mut self, ctx: &egui::Context, devid: usize) {
        egui::SidePanel::left("vdev_inputs_list").show(ctx, |ui| {
            ui.vertical_centered_justified(|ui| {
                let add_resp = ui.button("Add");
                let popup_id = ui.make_persistent_id("vdev_input_new");

                if add_resp.clicked() {
                    ui.memory().toggle_popup(popup_id);
                }

                egui::popup_below_widget(ui, popup_id, &add_resp, |ui| {
                    let mut select = None;

                    for entry in self.engine.devices() {
                        ui.selectable_value(
                            &mut select,
                            Some(*entry.uuid()),
                            entry.info().name.clone(),
                        );
                    }

                    if let Some(uuid) = select {
                        let Some(view) = self.engine.get_device(&uuid)
                        else { return; };
                        self.data[devid].views.push(view);
                    }
                });

                if ui.button("Remove").clicked() {
                    if self.bindings_input_selected < self.data[devid].views.len() {
                        self.data[devid].views.remove(self.bindings_input_selected);
                    }
                }
            });

            ui.separator();

            for (i, view) in self.data[devid].views.iter().enumerate() {
                ui.selectable_value(
                    &mut self.bindings_input_selected,
                    i,
                    view.info().name.clone(),
                );
            }
        });

        egui::TopBottomPanel::top("vdev_bindings_topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_label(false, "Code");

                ui.separator();

                ui.button("Save");
                ui.button("Load");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.data[devid].code)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(32)
                    .show(ui);
            });
        });
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
                let out = self
                    .engine
                    .new_device(data.info.clone())
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct ComponentSelection {
    kind: ComponentKind,
    index: usize,
}

impl ComponentSelection {
    fn get_editor(&self) -> Box<dyn ComponentEditor> {
        match self.kind {
            ComponentKind::Controller => Box::new(controller::Editor::new(self.index)),
            _ => {
                struct EmptyEditor;

                impl ComponentEditor for EmptyEditor {
                    fn update(&mut self, _: &mut DeviceInfo, _: &egui::Context) -> bool {
                        false
                    }
                }

                Box::new(EmptyEditor)
            }
        }
    }

    fn remove(&self, info: &mut DeviceInfo) {
        macro_rules! remove_kind {
            ($($cname:ident : $ckind:expr),* $(,)?) => {
                paste! {
                    $(
                        if self.kind == $ckind {
                            if self.index < info.[< $cname s >].len() {
                                info.[< $cname s >].remove(self.index);
                            }
                        }
                    )*
                }
            };
        }

        components!(kind remove_kind);
    }
}

impl Default for ComponentSelection {
    fn default() -> Self {
        ComponentSelection {
            kind: ComponentKind::Controller,
            index: 0,
        }
    }
}

impl std::fmt::Display for ComponentSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if self.index > 0 {
            write!(f, " {}", self.index + 1)?;
        }

        Ok(())
    }
}

trait ComponentEditor {
    /// Returns true if info was updated
    fn update(&mut self, info: &mut DeviceInfo, ctx: &egui::Context) -> bool;
}
