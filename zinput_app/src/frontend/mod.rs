pub mod dsus;
pub mod swi_send;
#[cfg(target_os = "linux")]
pub mod uinput;
#[cfg(target_os = "windows")]
pub mod xinput;
