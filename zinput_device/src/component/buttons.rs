use super::ComponentData;

#[derive(Clone, PartialEq, Eq)]
pub struct ButtonsInfo {
    pub buttons: u64,
}

impl Default for ButtonsInfo {
    fn default() -> Self {
        ButtonsInfo { buttons: 0 }
    }
}

pub type ButtonsConfig = ();

#[repr(C, align(8))]
#[derive(Copy, Clone)]
pub struct Buttons {
    pub buttons: u64,
}

#[cfg(feature = "bindlang")]
unsafe impl bindlang::ty::BLType for Buttons {
    fn bl_type() -> bindlang::ty::Type {
        bindlang::ty::Type::Bitfield(
            "Buttons",
            bindlang::util::Width::W64,
            bindlang::ty::BitNames::default(),
        )
    }
}

impl Default for Buttons {
    fn default() -> Self {
        Buttons { buttons: 0 }
    }
}

impl ComponentData for Buttons {
    type Config = ButtonsConfig;
    type Info = ButtonsInfo;

    fn update(&mut self, from: &Self) {
        self.clone_from(from);
    }

    fn configure(&mut self, _: &Self::Config) {}
}
