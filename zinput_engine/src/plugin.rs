use std::sync::Arc;

use eframe::egui;

use crate::{
    event::{Event, EventKind},
    Engine,
};

pub trait Plugin {
    fn init(&self, engine: Arc<Engine>);
    fn stop(&self);

    fn status(&self) -> PluginStatus;

    fn name(&self) -> &str;
    fn kind(&self) -> PluginKind;
    fn events(&self) -> &[EventKind] {
        &[]
    }

    fn update_gui(&self, _ctx: &egui::Context, _frame: &mut eframe::Frame, _ui: &mut egui::Ui) {}

    fn on_event(&self, _event: &Event) {}
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
