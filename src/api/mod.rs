use std::sync::Arc;

use eframe::{egui, epi};
use uuid::Uuid;

use crate::zinput::engine::Engine;

pub mod component;
pub mod device;

use self::component::{analogs::{Analogs, AnalogsInfo}, buttons::{Buttons, ButtonsInfo}, controller::{Controller, ControllerInfo}, motion::{Motion, MotionInfo}, touch_pad::{TouchPad, TouchPadInfo}};

use self::device::DeviceInfo;

pub trait Backend {
    fn init(&self, zinput_api: Arc<dyn ZInputApi + Send + Sync>);
    fn stop(&self);

    fn status(&self) -> BackendStatus;

    fn name(&self) -> &str;

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, _ui: &mut egui::Ui) {}
}

pub trait Frontend {
    fn init(&self, engine: Arc<Engine>);

    fn name(&self) -> &str;

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, _ui: &mut egui::Ui) {}

    fn on_component_update(&self, _id: &Uuid) {}
}

pub trait ZInputApi {
    fn new_controller(&self, info: ControllerInfo) -> Uuid;
    fn new_motion(&self, info: MotionInfo) -> Uuid;

    fn new_analog(&self, info: AnalogsInfo) -> Uuid;
    fn new_button(&self, info: ButtonsInfo) -> Uuid;
    fn new_touch_pad(&self, info: TouchPadInfo) -> Uuid;

    fn new_device(&self, info: DeviceInfo) -> Uuid;

    fn update_controller(
        &self,
        id: &Uuid,
        data: &Controller,
    ) -> Result<(), InvalidComponentIdError>;
    fn update_motion(&self, id: &Uuid, data: &Motion) -> Result<(), InvalidComponentIdError>;

    fn update_analog(&self, id: &Uuid, data: &Analogs) -> Result<(), InvalidComponentIdError>;
    fn update_button(&self, id: &Uuid, data: &Buttons) -> Result<(), InvalidComponentIdError>;
    fn update_touch_pad(&self, id: &Uuid, data: &TouchPad) -> Result<(), InvalidComponentIdError>;

    fn remove_controller(&self, id: &Uuid);
    fn remove_motion(&self, id: &Uuid);

    fn remove_analog(&self, id: &Uuid);
    fn remove_button(&self, id: &Uuid);
    fn remove_touch_pad(&self, id: &Uuid);

    fn remove_device(&self, id: &Uuid);
}

#[derive(Debug)]
pub struct InvalidComponentIdError;

impl std::error::Error for InvalidComponentIdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for InvalidComponentIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid component id")
    }
}

#[derive(Clone, Debug)]
pub enum BackendStatus {
    Running,
    Stopped,
    Error(String),
}

impl std::fmt::Display for BackendStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendStatus::Running => write!(f, "running"),
            BackendStatus::Stopped => write!(f, "stopped"),
            BackendStatus::Error(err) => write!(f, "error: {}", err),
        }
    }
}
