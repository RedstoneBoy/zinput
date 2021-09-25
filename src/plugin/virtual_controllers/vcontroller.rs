use paste::paste;
use uuid::Uuid;

use crate::api::component::{ComponentData, analogs::Analogs, buttons::Buttons, controller::Controller, motion::Motion, touch_pad::TouchPad};
use crate::api::device::{Components, DeviceInfo};
use crate::zinput::engine::Engine;

macro_rules! vctrl {
    (
        single { $($sfname:ident : $sftype:ty),* $(,)? }
        multiple { $($mfname:ident : $mftype:ty),* $(,)? }
    ) => {
        #[derive(Default)]
        pub struct VInput {
            pub $($sfname: Vec<<$sftype as ComponentData>::Info>,)*
            pub $($mfname: Vec<<$mftype as ComponentData>::Info>,)*
        }

        #[derive(Default)]
        pub struct VOutput {
            pub $($sfname: Option<ComponentBundle<$sftype>>,)*
            pub $($mfname: Vec<ComponentBundle<$mftype>>,)*
        }

        #[derive(Default)]
        pub struct VOutputBuilder {
            pub $($sfname: Option<<$sftype as ComponentData>::Info>,)*
            pub $($mfname: Vec<<$mftype as ComponentData>::Info>,)*
        }

        #[derive(Default)]
        pub struct VController {
            pub input: VInput,
            pub output: VOutput,
            device_id: Uuid,

            name: String,
        }

        impl VController {
            pub fn new(engine: &Engine, input: VInput, name: String, output: VOutputBuilder) -> Self {
                paste! {
                    $(let $sfname: Option<Uuid> = output
                        .$sfname
                        .map(|info| engine.[< new_ $sfname >](info));)*
                    $(let $mfname: Vec<Uuid> = output
                        .$mfname
                        .into_iter()
                        .map(|info| engine.[< new_ $mfname >](info))
                        .collect();)*
                    
                    let output = {
                        $(let $sfname = $sfname
                            .map(|ref id| ComponentBundle { id: *id, data: Default::default() });)*
                        $(let $mfname = $mfname
                            .iter()
                            .map(|id| ComponentBundle { id: *id, data: Default::default() })
                            .collect();)*

                        VOutput {
                            $($sfname,)*
                            $($mfname,)*
                        }
                    };

                    let device_info = DeviceInfo {
                        name: name.clone(),
                        components: Components {
                            $($sfname,)*
                            $($mfname,)*
                        },
                    };

                    let device_id = engine.new_device(device_info);

                    VController {
                        input,
                        output,
                        device_id,
                        
                        name,
                    }
                }
                
            }

            pub fn device_id(&self) -> &Uuid {
                &self.device_id
            }

            pub fn name(&self) -> &str {
                &self.name
            }
        }
    };
}

crate::schema_macro!(vctrl);

pub struct ComponentBundle<C: ComponentData> {
    id: Uuid,
    pub data: C,
}

impl<C: ComponentData> ComponentBundle<C> {
    pub fn id(&self) -> &Uuid {
        &self.id
    }
}