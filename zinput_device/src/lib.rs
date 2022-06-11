use paste::paste;

pub mod component;

#[macro_export]
macro_rules! components {
    (config $macro:ident) => {
        $macro! {
            controller: $crate::component::controller::ControllerConfig,
            motion:     $crate::component::motion::MotionConfig,
            analog:     $crate::component::analogs::AnalogsConfig,
            button:     $crate::component::buttons::ButtonsConfig,
            touch_pad:  $crate::component::touch_pad::TouchPadConfig,
        }
    };
    (data $macro:ident) => {
        $macro! {
            controller: $crate::component::controller::Controller,
            motion:     $crate::component::motion::Motion,
            analog:     $crate::component::analogs::Analogs,
            button:     $crate::component::buttons::Buttons,
            touch_pad:  $crate::component::touch_pad::TouchPad,
        }
    };
    (info $macro:ident) => {
        $macro! {
            controller: $crate::component::controller::ControllerInfo,
            motion:     $crate::component::motion::MotionInfo,
            analog:     $crate::component::analogs::AnalogsInfo,
            button:     $crate::component::buttons::ButtonsInfo,
            touch_pad:  $crate::component::touch_pad::TouchPadInfo,
        }
    }
}

macro_rules! device_config {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste! {
            use serde::{Deserialize, Serialize};
            
            #[derive(Clone, Deserialize, Serialize)]
            pub struct DeviceConfig {
                $(pub [< $cname s >]: Vec<$ctype>,)*
            }
    
            impl DeviceConfig {
                pub fn configure(&self, device: DeviceMut) {
                    use component::ComponentData;
                    $(
                        for i in 0..device.[< $cname s >].len() {
                            device.[< $cname s >][i].configure(&self.[< $cname s >][i]);
                        }
                    )*
                }

                pub fn as_mut(&mut self) -> DeviceConfigMut {
                    DeviceConfigMut {
                        $([< $cname s >]: &mut self.[< $cname s >],)*
                    }
                }
            }

            pub struct DeviceConfigMut<'a> {    
                $(pub [< $cname s >]: &'a mut [$ctype],)*
            }
        }
    }
}

macro_rules! device_info {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste! {
            #[derive(Clone, PartialEq, Eq)]
            pub struct DeviceInfo {
                pub name: String,
                pub id: Option<String>,
                /// If this device has an id, the device config will be loaded without user interaction
                pub autoload_config: bool,
    
                $(pub [< $cname s >]: Vec<$ctype>,)*
            }
    
            impl DeviceInfo {
                pub fn new(name: String) -> Self {
                    DeviceInfo {
                        name,
                        id: None,
                        autoload_config: false,

                        $([< $cname s >]: Vec::new(),)*
                    }
                }

                pub fn with_id(mut self, id: String) -> Self {
                    self.id = Some(id);
                    self
                }

                pub fn autoload_config(mut self, autoload_config: bool) -> Self {
                    self.autoload_config = autoload_config;
                    self
                }

                $(
                    pub fn [< add_ $cname >](&mut self, info: $ctype) -> usize {
                        self.[< $cname s >].push(info);
                        self.[< $cname s >].len() - 1
                    }
                )*
            }
        }
    }
}

macro_rules! device {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste! {
            pub struct Device {
                $(pub [< $cname s >]: Vec<$ctype>,)*
            }

            impl Device {
                pub fn as_mut(&mut self) -> DeviceMut {
                    DeviceMut {
                        $([< $cname s >]: &mut self.[< $cname s >],)*
                    }
                }
            }
    
            pub struct DeviceMut<'a> {
                $(pub [< $cname s >]: &'a mut [$ctype],)*
            }
        }
    }
}

components!(config device_config);
components!(info device_info);
components!(data device);