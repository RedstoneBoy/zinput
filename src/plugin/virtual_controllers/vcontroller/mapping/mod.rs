mod controller;

use crate::api::component::ComponentKind;

pub use self::controller::ControllerMapping;

#[derive(Default)]
pub struct RawMapping {
    pub mappings: Vec<ComponentMapping>,
}

impl RawMapping {

}

pub enum ComponentMapping {
    Controller(ControllerMapping),
}

impl ComponentMapping {
    pub fn kind(&self) -> ComponentKind {
        use ComponentMapping::*;
        
        match self {
            Controller(_) => ComponentKind::Controller,
        }
    }
}