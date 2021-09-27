use paste::paste;
use uuid::Uuid;

use super::component::{ComponentData, ComponentKind};

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

            pub fn contains(&self, kind: ComponentKind) -> bool {
                self.get_single(kind).is_some() || self.get_multiple(kind).len() > 0
            }

            pub fn get_single(&self, kind: ComponentKind) -> Option<&Uuid> {
                match kind {
                    $(<$sftype>::KIND => self.$sfname.as_ref(),)*
                    _ => None,
                }
            }

            pub fn get_multiple(&self, kind: ComponentKind) -> &[Uuid] {
                match kind {
                    $(<$mftype>::KIND => &*self.$mfname,)*
                    _ => &[],
                }
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
