#![feature(let_else)]
#![feature(generic_associated_types)]
#![feature(maybe_uninit_uninit_array)]

use std::sync::Arc;

mod backend;
mod frontend;
mod gui;
mod zinput;

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    let mut zinput = zinput::ZInput::new();

    #[cfg(target_os = "windows")]
    {
        zinput.add_plugin(Arc::new(backend::raw_input::RawInput::new()), true);
        zinput.add_plugin(Arc::new(backend::xinput::XInput::new()), true);
        zinput.add_plugin(Arc::new(frontend::vigem::Vigem::new()), false);
    }

    #[cfg(target_os = "linux")]
    {
        zinput.add_plugin(Arc::new(frontend::uinput::UInput::new()), false);
    }

    zinput.add_plugin(Arc::new(backend::joycon::Joycon::new()), true);
    zinput.add_plugin(Arc::new(backend::swi_recv::Swi::new()), false);
    zinput.add_plugin(Arc::new(backend::usb_devices::UsbDevices::new()), true);

    zinput.add_plugin(Arc::new(frontend::dsus::Dsus::new()), false);
    zinput.add_plugin(Arc::new(frontend::swi_send::Swi::new()), false);    

    zinput.run();
}
