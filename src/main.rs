use std::sync::Arc;

mod api;
mod backend;
mod frontend;
mod gui;
mod zinput;

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    let mut zinput = zinput::ZInput::new();
    zinput.add_backend(Arc::new(backend::gc_adaptor::GcAdaptor::new()));
    zinput.add_backend(Arc::new(backend::steam_controller::SteamController::new()));
    zinput.add_backend(Arc::new(backend::swi::Swi::new()));
    zinput.add_backend(Arc::new(backend::xinput::XInput::new()));
    zinput.add_frontend(Arc::new(frontend::dsus::Dsus::new()));
    zinput.add_frontend(Arc::new(frontend::xinput::XInput::new()));
    zinput.run();
}
