pub mod joycon;
#[cfg(target_os = "windows")]
pub mod raw_input;
pub mod steam_controller;
pub mod swi_recv;
pub mod usb_devices;
#[cfg(target_os = "windows")]
pub mod xinput;

#[macro_export]
macro_rules! device_bundle {
    ($name:ident, $($cname:ident : $ctype:ty $( [ $clen:expr ] )?),* $(,)?) => {
        type EngineRef<'a> = &'a zinput_engine::Engine;

        crate::device_bundle!($name(EngineRef), $($cname : $ctype $( [ $clen ] )?),*);
    };

    ($name:ident (owned), $($cname:ident : $ctype:ty $( [ $clen:expr ] )?),* $(,)?) => {
        type EngineArc<'a> = Arc<zinput_engine::Engine>;

        crate::device_bundle!($name(EngineArc), $($cname : $ctype $( [ $clen ] )?),*);
    };

    ($name:ident ( $($api_type:tt)+ ), $($cname:ident : $ctype:ty $( [ $clen:expr ] )?),* $(,)?) => {
        use paste::paste;

        struct $name<'a> {
            _lifetime: std::marker::PhantomData<&'a ()>,
            api: $($api_type<'a>)+,
            device_id: zinput_engine::util::Uuid,
            $($cname: crate::device_bundle!(field $cname : $ctype $( [ $clen ] )?),)*
        }

        paste! {
            impl<'a> $name<'a> {
                fn new(
                    api: $($api_type<'a>)+,
                    name: String,
                    $($cname: crate::device_bundle!(info $cname : $ctype $( [ $clen ] )? ),)*
                ) -> Self {
                    let mut device_info = zinput_engine::device::DeviceInfo::new(name);

                    $(let $cname = crate::device_bundle!(init(api, $cname, device_info) $cname : $ctype $( [ $clen ] )?);)*

                    let device_id = api.new_device(device_info);

                    $name {
                        _lifetime: std::marker::PhantomData,
                        api,
                        device_id,
                        $($cname,)*
                    }
                }

                fn update(&self) -> Result<(), zinput_engine::ComponentUpdateError> {
                    use zinput_engine::device::component::ComponentData;

                    self.api.update(&self.device_id, |dev| {
                        $(crate::device_bundle!(update(self, dev) $cname : $ctype $( [ $clen ] )?);)*
                    })?;

                    Ok(())
                }
            }

            impl<'a> Drop for $name<'a> {
                fn drop(&mut self) {
                    self.api.remove_device(&self.device_id);
                }
            }
        }
    };

    (field $cname:ident : $ctype:ty) => {
        crate::device_bundle!(field $cname : $ctype [ 1 ])
    };

    (field $cname:ident : $ctype:ty [ $clen:expr ]) => {
        [$ctype; $clen]
    };

    (info $cname:ident : $ctype:ty) => {
        crate::device_bundle!(info $cname : $ctype [ 1 ])
    };

    (info $cname:ident : $ctype:ty [ $clen:expr ]) => {
        [<$ctype as zinput_engine::device::component::ComponentData>::Info; $clen]
    };

    (init ( $api:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty) => {
        crate::device_bundle!(init($api, $info, $dinfo) $cname : $ctype [ 1 ])
    };

    (init ( $api:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty [ $clen:expr ]) => {{
        paste! {
            $dinfo.[< $cname s >] = $info.into();
            [(); $clen].map(|_| $ctype::default())
        }
    }};

    (update ( $this:expr, $dev:ident ) $cname:ident : $ctype:ty) => {
        crate::device_bundle!(update($this, $dev) $cname : $ctype [ 1 ])
    };

    (update ( $this:expr, $dev:ident ) $cname:ident : $ctype:ty [ $clen:expr ]) => {
        paste! {
            for i in 0..$clen {
                $dev.[< $cname s >][i].update(&$this.$cname[i]);
            }
        }
    };
}
