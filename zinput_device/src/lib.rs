#![feature(once_cell)]

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
    };
    (kind $macro:ident) => {
        $macro! {
            controller: $crate::component::ComponentKind::Controller,
            motion:     $crate::component::ComponentKind::Motion,
            analog:     $crate::component::ComponentKind::Analogs,
            button:     $crate::component::ComponentKind::Buttons,
            touch_pad:  $crate::component::ComponentKind::TouchPad,
        }
    };
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

            impl<'a> DeviceMut<'a> {
                pub fn to_ffi(&mut self) -> DeviceMutFfi {
                    DeviceMutFfi {
                        $([< $cname s >]: FfiSlice {
                            ptr: self.[< $cname s >].as_mut_ptr() as _,
                            len: self.[< $cname s >].len(),
                        },)*
                    }
                }
            }

            #[repr(C)]
            pub struct DeviceMutFfi {
                $(pub [< $cname s >]: FfiSlice,)*
            }

            unsafe impl bindlang::ty::BLType for DeviceMutFfi {
                fn bl_type() -> bindlang::ty::Type {
                    use std::collections::HashMap;
                    use std::sync::LazyLock;
                    use bindlang::ty::{Field, Struct, Type, BLType};

                    static TYPE: LazyLock<Type> = LazyLock::new(|| {
                        let mut fields = HashMap::new();
                        let mut _i = 0;
                        $(
                            fields.insert(stringify!([< $cname s >]), Field {
                                ty: Type::Slice(<$ctype as BLType>::bl_type().into()),
                                byte_offset: _i,
                            });
                            _i += std::mem::size_of::<FfiSlice>() as i32;
                        )*
    
                        Type::Struct(Struct {
                            name: "device",
                            fields,
                            size: std::mem::size_of::<DeviceMutFfi>() as i32,
                        })
                    });

                    TYPE.clone()
                }
            }

            #[repr(C)]
            pub struct FfiSlice {
                pub ptr: *mut std::ffi::c_void,
                pub len: usize,
            }
        }
    }
}

components!(config device_config);
components!(info device_info);
components!(data device);
