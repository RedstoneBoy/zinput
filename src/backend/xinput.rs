use std::{
    convert::TryInto,
    net::{SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use eframe::{egui, epi};
use parking_lot::Mutex;
use rusty_xinput::{XInputHandle, XInputState, XInputUsageError};
use uuid::Uuid;

use crate::api::{PluginKind, component::{
    controller::{Button, Controller, ControllerInfo},
    motion::{Motion, MotionInfo},
}};
use crate::api::device::DeviceInfo;
use crate::api::{Plugin, PluginStatus};
use crate::zinput::engine::Engine;

const T: &'static str = "backend:xinput";

pub struct XInput {
    inner: Mutex<Inner>,
}

impl XInput {
    pub fn new() -> Self {
        XInput {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for XInput {
    fn init(&self, zinput_api: Arc<Engine>) {
        self.inner.lock().init(zinput_api)
    }

    fn stop(&self) {
        self.inner.lock().stop()
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "xinput"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }

    fn update_gui(&self, _ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>, ui: &mut egui::Ui) {
        
    }
}

struct Inner {
    handle: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<PluginStatus>>,
}

impl Inner {
    fn new() -> Self {
        Inner {
            handle: None,
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(PluginStatus::Running)),
        }
    }

    fn init(&mut self, api: Arc<Engine>) {
        *self.status.lock() = PluginStatus::Running;
        self.stop = Arc::new(AtomicBool::new(false));
        self.handle = Some(std::thread::spawn(new_xinput_thread(Thread {
            status: self.status.clone(),
            stop: self.stop.clone(),
            api,
        })));
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = std::mem::replace(&mut self.handle, None) {
            match handle.join() {
                Ok(()) => (),
                Err(_) => log::info!(target: T, "driver panicked"),
            }
        }
        *self.status.lock() = PluginStatus::Stopped;
    }

    fn status(&self) -> PluginStatus {
        self.status.lock().clone()
    }
}

impl Drop for XInput {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Thread {
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
}

fn new_xinput_thread(thread: Thread) -> impl FnOnce() {
    move || {
        log::info!(target: T, "driver initialized");

        let status = thread.status.clone();

        match xinput_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "driver stopped");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(err) => {
                log::error!(target: T, "driver crashed: {:#}", err);
                *status.lock() = PluginStatus::Error(format!("driver crashed: {:#}", err));
            }
        }
    }
}

fn xinput_thread(thread: Thread) -> Result<()> {
    let Thread {
        status,
        stop,
        api,
    } = thread;

    let xinput = XInputHandle::load_default()
        .map_err(|err| anyhow::anyhow!("failed to load xinput: {:?}", err))?;

    let mut controllers = Controllers::new(api, xinput);

    let frame_timer = crossbeam_channel::tick(Duration::from_millis(15));
    let mut new_controller_timer = 0.0f32;

    while !stop.load(Ordering::Acquire) {
        crossbeam_channel::select! {
            recv(frame_timer) -> _ => {
                new_controller_timer += 15.0 / 1000.0;
                if new_controller_timer >= 1.0 {
                    new_controller_timer = 0.0;

                    controllers.poll_disconnected();
                }

                controllers.update()?;
            }
        }
    }

    Ok(())
}

struct Controllers {
    api: Arc<Engine>,
    controllers: [Option<XController>; 4],
    xinput: XInputHandle,
}

impl Controllers {
    fn new(api: Arc<Engine>, xinput: XInputHandle) -> Self {
        Controllers {
            api,
            controllers: [None, None, None, None],
            xinput,
        }
    }

    fn update(&mut self) -> Result<()> {
        for i in 0..self.controllers.len() {
            if let Some(ctrl) = &mut self.controllers[i] {
                match self.xinput.get_state(i as u32) {
                    Ok(state) => ctrl.update(&*self.api, &state)?,
                    Err(XInputUsageError::DeviceNotConnected) => self.disconnect(i),
                    Err(err) => {
                        log::error!(target: T, "controller polling error: {:?}", err);
                    }
                }
            }
        }

        Ok(())
    }

    fn poll_disconnected(&mut self) {
        for i in 0..self.controllers.len() {
            if self.controllers[i].is_none() {
                match self.xinput.get_state(i as u32) {
                    Ok(_) => self.connect(i),
                    // polling a non-connected controller causes a long delay
                    // so we only poll at most one non-connected controller.
                    Err(XInputUsageError::DeviceNotConnected) => break,
                    Err(err) => {
                        log::error!(target: T, "new controller state polling error: {:?}", err);
                        break;
                    }
                }
            }
        }
    }

    fn connect(&mut self, index: usize) {
        if self.controllers[index].is_none() {
            self.controllers[index] = Some(XController::new(&*self.api, index));
        }
    }

    fn disconnect(&mut self, index: usize) {
        if let Some(ctrl) = &self.controllers[index] {
            self.api.remove_controller(&ctrl.controller_id);
            self.api.remove_device(&ctrl.device_id);
        }
        self.controllers[index] = None;
    }
}

impl Drop for Controllers {
    fn drop(&mut self) {
        for i in 0..self.controllers.len() {
            self.disconnect(i);
        }
    }
}

#[derive(Clone)]
struct XController {
    device_id: Uuid,
    controller_id: Uuid,
    controller: Controller,
}

impl XController {
    fn new(api: &(Engine), id: usize) -> Self {
        let controller_id = api.new_controller(xinput_controller_info());
        let device_id = api.new_device(DeviceInfo::new(format!("XInput Controller {}", id + 1))
            .with_controller(controller_id));
        
        XController {
            device_id,
            controller_id,
            controller: Controller::default(),
        }
    }

    fn update(&mut self, api: &(Engine), state: &XInputState) -> Result<()> {
        macro_rules! translate {
            ($state:expr, $($from:ident => $to:expr),* $(,)?) => {{
                let mut buttons = 0;
                $(if $state.$from() { $to.set_pressed(&mut buttons); })*
                buttons
            }};
        }

        self.controller.buttons = translate!(state,
            north_button       => Button::Y,
            south_button       => Button::A,
            east_button        => Button::B,
            west_button        => Button::X,
            arrow_up           => Button::Up,
            arrow_down         => Button::Down,
            arrow_left         => Button::Left,
            arrow_right        => Button::Right,
            start_button       => Button::Start,
            select_button      => Button::Select,
            left_shoulder      => Button::L1,
            right_shoulder     => Button::R1,
            left_trigger_bool  => Button::L2,
            right_trigger_bool => Button::R2,
            left_thumb_button  => Button::LStick,
            right_thumb_button => Button::RStick,
        );

        self.controller.l2_analog = state.left_trigger();
        self.controller.r2_analog = state.right_trigger();

        let (lpad_x, lpad_y) = state.left_stick_raw();
        let (rpad_x, rpad_y) = state.right_stick_raw();

        self.controller.left_stick_x = ((lpad_x / 256) + 128) as u8;
        self.controller.left_stick_y = ((lpad_y / 256) + 128) as u8;
        self.controller.right_stick_x = ((rpad_x / 256) + 128) as u8;
        self.controller.right_stick_y = ((rpad_y / 256) + 128) as u8;
        
        api.update_controller(&self.controller_id, &self.controller)?;

        Ok(())
    }
}

fn xinput_controller_info() -> ControllerInfo {
    let mut info = ControllerInfo::default()
        .with_lstick()
        .with_rstick()
        .with_l2_analog()
        .with_r2_analog();

    macro_rules! for_buttons {
        ($($button:expr),* $(,)?) => {
            $($button.set_pressed(&mut info.buttons);)*
        }
    }
    for_buttons!(
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::Start,
        Button::Select,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::LStick,
        Button::RStick,
    );

    info
}
