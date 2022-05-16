#![feature(maybe_uninit_uninit_array)]
#![feature(generic_associated_types)]

use std::sync::Arc;

mod backend;
mod frontend;
mod gui;
mod zinput;

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    let mut zinput = zinput::ZInput::new();
    zinput.add_plugin(Arc::new(backend::gc_adaptor::GcAdaptor::new()));
    zinput.add_plugin(Arc::new(backend::joycon::Joycon::new()));
    zinput.add_plugin(Arc::new(backend::pa_switch::PASwitch::new()));
    zinput.add_plugin(Arc::new(backend::steam_controller::SteamController::new()));
    zinput.add_plugin(Arc::new(backend::swi_recv::Swi::new()));

    zinput.add_plugin(Arc::new(frontend::dsus::Dsus::new()));
    zinput.add_plugin(Arc::new(frontend::swi_send::Swi::new()));

    #[cfg(target_os = "windows")]
    {
        zinput.add_plugin(Arc::new(backend::raw_input::RawInput::new()));
        zinput.add_plugin(Arc::new(backend::xinput::XInput::new()));
        zinput.add_plugin(Arc::new(frontend::xinput::XInput::new()));
    }

    #[cfg(target_os = "linux")]
    {
        zinput.add_plugin(Arc::new(frontend::uinput::UInput::new()));
    }

    zinput.run();
}
