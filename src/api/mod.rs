use std::sync::Arc;

use eframe::{egui, epi};
use uuid::Uuid;

use crate::zinput::engine::Engine;

use self::device::DeviceInfo;

pub mod component;
pub mod device;
pub mod widget;

pub trait Plugin {
    fn init(&self, zinput_api: Arc<Engine>);
    fn stop(&self);

    fn status(&self) -> PluginStatus;

    fn name(&self) -> &str;
    fn kind(&self) -> PluginKind;
    fn events(&self) -> &[EventKind] { &[] }

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, _ui: &mut egui::Ui) {}

    fn on_event(&self, _event: &Event) {}
}

#[derive(Debug)]
pub enum ComponentUpdateError {
    InvalidDeviceId,
    InvalidIndex,
}

impl std::error::Error for ComponentUpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for ComponentUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentUpdateError::InvalidDeviceId => write!(f, "invalid device id"),
            ComponentUpdateError::InvalidIndex => write!(f, "invalid index"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PluginStatus {
    Running,
    Stopped,
    Error(String),
}

impl std::fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginStatus::Running => write!(f, "running"),
            PluginStatus::Stopped => write!(f, "stopped"),
            PluginStatus::Error(err) => write!(f, "error: {}", err),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PluginKind {
    Backend,
    Frontend,
    Custom(String),
}

impl std::fmt::Display for PluginKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginKind::Backend => write!(f, "backend"),
            PluginKind::Frontend => write!(f, "frontend"),
            PluginKind::Custom(kind) => write!(f, "custom: {}", kind),
        }
    }
}

#[derive(Clone)]
pub enum Event {
    DeviceUpdate(Uuid),
    DeviceAdded(Uuid, DeviceInfo),
    DeviceRemoved(Uuid),
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::DeviceUpdate(_) => EventKind::DeviceUpdate,
            Event::DeviceAdded(_, _) => EventKind::DeviceAdded,
            Event::DeviceRemoved(_) => EventKind::DeviceRemoved,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EventKind {
    DeviceUpdate,
    DeviceAdded,
    DeviceRemoved,
}