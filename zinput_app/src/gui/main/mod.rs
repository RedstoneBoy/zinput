use std::{collections::HashMap, sync::Arc};

use zinput_engine::{
    eframe::{self, egui},
    plugin::Plugin,
    Engine,
};

mod devices_tab;
mod drivers_tab;
mod output_tab;
mod virtual_tab;

pub struct MainUi {
    tab: Tab,

    screens: HashMap<Tab, Box<dyn Screen>>,
}

impl MainUi {
    pub fn new(engine: Arc<Engine>, plugins: Vec<Arc<dyn Plugin + Send + Sync>>) -> Self {
        MainUi {
            tab: Tab::Devices,

            screens: {
                let mut map = HashMap::new();
                map.insert(
                    Tab::Devices,
                    Box::new(devices_tab::DevicesTab::new(engine.clone())) as _,
                );
                map.insert(
                    Tab::Drivers,
                    Box::new(drivers_tab::DriversTab::new(engine.clone(), &plugins)) as _,
                );
                map.insert(
                    Tab::Output,
                    Box::new(output_tab::OutputTab::new(engine.clone(), &plugins)) as _,
                );
                map.insert(
                    Tab::VirtualDevices,
                    Box::new(virtual_tab::VirtualTab::new(engine)) as _,
                );
                map
            },
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for tab in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, tab.name());
                }
            });
        });

        let Some(screen) = self.screens.get_mut(&self.tab)
        else { return; };

        screen.update(ctx, frame);
    }
}

trait Screen {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame);
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum Tab {
    Drivers,
    Devices,
    VirtualDevices,
    Output,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Drivers, Tab::Devices, Tab::VirtualDevices, Tab::Output];

    fn name(&self) -> &str {
        match self {
            Tab::Drivers => "Drivers",
            Tab::Devices => "Devices",
            Tab::VirtualDevices => "Virtual Devices",
            Tab::Output => "Output",
        }
    }
}
