#[macro_export]
macro_rules! schema_macro {
    ($macro_name:ident) => {
        $macro_name! {
            single {
                controller: crate::api::component::controller::Controller,
                motion: crate::api::component::motion::Motion,
            }
            multiple {
                analogs: crate::api::component::analogs::Analogs,
                buttons: crate::api::component::buttons::Buttons,
                touch_pad: crate::api::component::touch_pad::TouchPad,
            }
        }
    };

    (unified $macro_name:ident) => {
        $macro_name! {
            controller: crate::api::component::controller::Controller,
            motion: crate::api::component::motion::Motion,
            analogs: crate::api::component::analogs::Analogs,
            buttons: crate::api::component::buttons::Buttons,
            touch_pad: crate::api::component::touch_pad::TouchPad,
        }
    };
}