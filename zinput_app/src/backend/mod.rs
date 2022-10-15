pub mod joycon;
#[cfg(target_os = "windows")]
pub mod raw_input;
pub mod usb_devices;
#[cfg(target_os = "windows")]
pub mod xinput;
pub mod znet_recv;

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

    ($name:ident ( $($engine_type:tt)+ ), $($cname:ident : $ctype:ty $( [ $clen:expr ] )?),* $(,)?) => {
        use paste::paste;

        struct $name<'a> {
            _lifetime: std::marker::PhantomData<&'a ()>,
            handle: zinput_engine::DeviceHandle,
            $($cname: crate::device_bundle!(field $cname : $ctype $( [ $clen ] )?),)*
        }

        paste! {
            impl<'a> $name<'a> {
                fn new(
                    engine: $($engine_type<'a>)+,
                    name: String,
                    id: Option<String>,
                    autoload_config: bool,
                    $($cname: crate::device_bundle!(info $cname : $ctype $( [ $clen ] )? ),)*
                ) -> std::result::Result<Self, zinput_engine::DeviceAlreadyExists> {
                    let mut device_info = zinput_engine::device::DeviceInfo::new(name);
                    device_info.id = id;
                    device_info.autoload_config = autoload_config;

                    $(let $cname = crate::device_bundle!(init(engine, $cname, device_info) $cname : $ctype $( [ $clen ] )?);)*

                    let handle = engine.new_device(device_info)?;

                    Ok($name {
                        _lifetime: std::marker::PhantomData,
                        handle,
                        $($cname,)*
                    })
                }

                fn update(&self) {
                    use zinput_engine::device::component::ComponentData;

                    self.handle.update(|dev| {
                        $(crate::device_bundle!(update(self, dev) $cname : $ctype $( [ $clen ] )?);)*
                    });
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

    (init ( $engine:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty) => {
        crate::device_bundle!(init($engine, $info, $dinfo) $cname : $ctype [ 1 ])
    };

    (init ( $engine:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty [ $clen:expr ]) => {{
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
