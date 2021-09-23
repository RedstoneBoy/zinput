use std::sync::Arc;

use eframe::{egui, epi};
use uuid::Uuid;

use crate::zinput::engine::Engine;

pub mod component;
pub mod device;

use self::component::{analogs::{Analogs, AnalogsInfo}, buttons::{Buttons, ButtonsInfo}, controller::{Controller, ControllerInfo}, motion::{Motion, MotionInfo}, touch_pad::{TouchPad, TouchPadInfo}};

use self::device::DeviceInfo;

pub trait Backend {
    fn init(&self, zinput_api: Arc<Engine>);
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
