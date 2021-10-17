pub mod gc_adaptor;
#[cfg(target_os = "windows")]
pub mod raw_input;
pub mod steam_controller;
pub mod swi_recv;
#[cfg(target_os = "windows")]
pub mod xinput;

#[macro_export]
macro_rules! device_bundle {
    ($name:ident , $($cname:ident : $ctype:ty $( [ $clen:expr ] )?),* $(,)?) => {
        use paste::paste;

        struct $name<'a> {
            api: &'a crate::zinput::engine::Engine,
            device_id: uuid::Uuid,
            $($cname: crate::device_bundle!(field $cname : $ctype $( [ $clen ] )?),)*
        }

        paste! {
            impl<'a> $name<'a> {
                fn new(
                    api: &'a crate::zinput::engine::Engine,
                    name: String,
                    $($cname: crate::device_bundle!(info $cname : $ctype $( [ $clen ] )? ),)*
                ) -> Self {
                    let mut device_info = crate::api::device::DeviceInfo::new(name);

                    $(let $cname = crate::device_bundle!(init(api, $cname, device_info) $cname : $ctype $( [ $clen ] )?);)*

                    let device_id = api.new_device(device_info);

                    $name {
                        api,
                        device_id,
                        $($cname,)*
                    }
                }

                fn update(&self) -> Result<(), crate::api::InvalidComponentIdError> {
                    $(crate::device_bundle!(update(self) $cname : $ctype $( [ $clen ] )?);)*
                    Ok(())
                }
            }

            impl<'a> Drop for $name<'a> {
                fn drop(&mut self) {
                    self.api.remove_device(&self.device_id);
                    $(crate::device_bundle!(drop(self) $cname : $ctype $( [ $clen ] )?);)*
                }
            }
        }
    };

    (field $cname:ident : $ctype:ty) => {
        (uuid::Uuid, $ctype)
    };

    (field $cname:ident : $ctype:ty [ $clen:expr ]) => {
        [(uuid::Uuid, $ctype); $clen]
    };

    (info $cname:ident : $ctype:ty) => {
        <$ctype as crate::api::component::ComponentData>::Info
    };

    (info $cname:ident : $ctype:ty [ $clen:expr ]) => {
        [<$ctype as crate::api::component::ComponentData>::Info; $clen]
    };

    (init ( $api:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty) => {{
        paste! {
            let id = $api.[< new_ $cname >]($info);
            $dinfo = $dinfo.[< with_ $cname >](id);
            (id, $ctype::default())
        }
    }};

    (init ( $api:expr, $info:expr, $dinfo:ident ) $cname:ident : $ctype:ty [ $clen:expr ]) => {{
        paste! {
            let mut out: [std::mem::MaybeUninit<(uuid::Uuid, $ctype)>; $clen] = std::mem::MaybeUninit::uninit_array::<$clen>();
            for i in 0..$clen {
                let id = $api.[< new_ $cname >]($info[i]);
                $dinfo = $dinfo.[< with_ $cname >](id);
                out[i].write((id, $ctype::default()));
            }
            out.map(|val| unsafe { val.assume_init() })
        }
    }};

    (update ( $this:expr ) $cname:ident : $ctype:ty) => {
        paste! {
            $this.api.[< update_ $cname >](&$this.$cname.0, &$this.$cname.1)?;
        }
    };

    (update ( $this:expr ) $cname:ident : $ctype:ty [ $clen:expr ]) => {
        paste! {
            for i in 0..$clen {
                $this.api.[< update_ $cname >](&$this.$cname[i].0, &$this.$cname[i].1);
            }
        }
    };

    (drop ( $this:expr ) $cname:ident : $ctype:ty) => {
        paste! {
            $this.api.[< remove_ $cname >](&$this.$cname.0);
        }
    };

    (drop ( $this:expr ) $cname:ident : $ctype:ty [ $clen:expr ]) => {
        paste! {
            for i in 0..$clen {
                $this.api.[< remove_ $cname >](&$this.$cname[i].0);
            }
        }
    };
}
