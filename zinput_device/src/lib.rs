use paste::paste;

pub mod component;

#[macro_export]
macro_rules! components {
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

macro_rules! device_info {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste! {
            #[derive(Clone)]
            pub struct DeviceInfo {
                pub name: String,
    
                $(pub [< $cname s >]: Vec<$ctype>,)*
            }
    
            impl DeviceInfo {
                pub fn new(name: String) -> Self {
                    DeviceInfo {
                        name,

                        $([< $cname s >]: Vec::new(),)*
                    }
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

components!(info device_info);
components!(data device);