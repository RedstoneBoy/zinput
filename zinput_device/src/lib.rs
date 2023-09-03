#![feature(lazy_cell)]
#![cfg_attr(
    not(any(feature = "serde", feature = "device")),
    no_std
)]

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
            mouse:      $crate::component::mouse::MouseConfig,
        }
    };
    (data $macro:ident) => {
        $macro! {
            controller: $crate::component::controller::Controller,
            motion:     $crate::component::motion::Motion,
            analog:     $crate::component::analogs::Analogs,
            button:     $crate::component::buttons::Buttons,
            touch_pad:  $crate::component::touch_pad::TouchPad,
            mouse:      $crate::component::mouse::Mouse,
        }
    };
    (info $macro:ident) => {
        $macro! {
            controller: $crate::component::controller::ControllerInfo,
            motion:     $crate::component::motion::MotionInfo,
            analog:     $crate::component::analogs::AnalogsInfo,
            button:     $crate::component::buttons::ButtonsInfo,
            touch_pad:  $crate::component::touch_pad::TouchPadInfo,
            mouse:      $crate::component::mouse::MouseInfo,
        }
    };
    (kind $macro:ident) => {
        $macro! {
            controller: $crate::component::ComponentKind::Controller,
            motion:     $crate::component::ComponentKind::Motion,
            analog:     $crate::component::ComponentKind::Analogs,
            button:     $crate::component::ComponentKind::Buttons,
            touch_pad:  $crate::component::ComponentKind::TouchPad,
            mouse:      $crate::component::ComponentKind::Mouse,
        }
    };
}

#[allow(unused_macros)]
macro_rules! device_config {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste::paste! {
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
            #[derive(Clone, Default)]
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

#[allow(unused_macros)]
macro_rules! device_info {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste::paste! {
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

                pub fn create_device(&self) -> Device {
                    Device {
                        $([< $cname s >]: vec![Default::default(); self.[< $cname s >].len()],)*
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

#[allow(unused_macros)]
macro_rules! device {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste::paste! {
            #[derive(Clone, Default)]
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

            #[repr(C)]
            pub struct DeviceMut<'a> {
                $(pub [< $cname s >]: &'a mut [$ctype],)*
            }

            #[repr(C)]
            pub struct DeviceRef<'a> {
                $(pub [< $cname s >]: &'a [$ctype],)*
            }
        }
    }
}

#[cfg(feature = "device")]
components!(config device_config);
#[cfg(feature = "device")]
components!(info device_info);
#[cfg(feature = "device")]
components!(data device);
