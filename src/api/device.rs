use paste::paste;
use uuid::Uuid;

use super::component::ComponentData;

macro_rules! components {
    (
        single { $($sfname:ident : $sftype:ty),* $(,)? }
        multiple { $($mfname:ident : $mftype:ty),* $(,)? }
    ) => {
        #[derive(Default)]
        pub struct Components {
            $(pub $sfname: Option<Uuid>,)*
            $(pub $mfname: Vec<Uuid>,)*
        }

        impl Components {
            paste! {
                $(
                    pub fn [< set_ $sfname >](mut self, $sfname: Uuid) -> Self {
                        self.$sfname = Some($sfname);
                        self
                    }
                )*

                $(
                    pub fn [< add_ $mfname >](mut self, $mfname: Uuid) -> Self {
                        self.$mfname.push($mfname);
                        self
                    }
                )*
            }
        }
    };
}

crate::schema_macro!(components);

pub struct DeviceInfo {
    pub name: String,
    pub components: Components,
}

impl DeviceInfo {
    pub fn new(name: String, components: Components) -> Self {
        DeviceInfo {
            name,
            components,
        }
    }
}
