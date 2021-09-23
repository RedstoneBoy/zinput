pub mod dsus;
#[cfg(target_os = "linux")]
pub mod uinput;
#[cfg(target_os = "windows")]
pub mod xinput;
