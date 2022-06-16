use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{Context, Error, Result};
use parking_lot::Mutex;
use sdl2::{controller::{Button as SdlButton, Axis, GameController}, event::{Event, EventType}, joystick::{Joystick, HatState}};
use zinput_engine::{
    device::component::{
        analogs::{Analogs, AnalogsInfo},
        buttons::{Buttons, ButtonsInfo},
        controller::{Button, Controller, ControllerInfo},
    },
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

use crate::util::thread::ThreadHandle;

const T: &'static str = "backend:sdl2";

const MAPPINGS: &[u8] = include_bytes!("sdl2_mappings.txt");

pub struct Sdl2 {
    inner: Mutex<Inner>,
}

impl Sdl2 {
    pub fn new() -> Self {
        Sdl2 {
            inner: Mutex::new(Inner::Uninit),
        }
    }
}

impl Plugin for Sdl2 {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine);
    }

    fn stop(&self) {
        self.inner.lock().stop();
    }

    fn status(&self) -> PluginStatus {
        self.inner.lock().status()
    }

    fn name(&self) -> &str {
        "sdl2"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

enum Inner {
    Uninit,
    Init {
        status: Arc<Mutex<PluginStatus>>,

        handle: ThreadHandle<()>,
    },
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));

        let thread = Thread {
            engine,
            status: status.clone(),
            stop: stop.clone(),
        };

        *self = Inner::Init {
            status,
            handle: ThreadHandle::spawn(stop, create_sdl2_thread(thread)),
        };
    }

    fn stop(&mut self) {
        match std::mem::replace(self, Inner::Uninit) {
            Inner::Uninit => {}
            Inner::Init { status, handle, .. } => match handle.stop() {
                Ok(()) => {
                    *status.lock() = PluginStatus::Stopped;
                }
                Err(_) => {
                    log::error!(target: T, "sdl2 thread panicked");
                    *status.lock() = PluginStatus::Error("sdl2 thread panicked".into());
                }
            },
        }
    }

    fn status(&self) -> PluginStatus {
        let Inner::Init { status, .. } = self
        else { return PluginStatus::Stopped };

        status.lock().clone()
    }
}

struct Thread {
    engine: Arc<Engine>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
}

fn create_sdl2_thread(thread: Thread) -> impl FnOnce() {
    move || {
        let status = thread.status.clone();

        match sdl2_thread(thread) {
            Ok(()) => log::info!(target: T, "sdl2 thread closed"),
            Err(err) => {
                *status.lock() = PluginStatus::Error(format!("sdl2 thread crashed: {err:#}"));

                log::error!(target: T, "sdl2 thread crashed: {err:?}")
            }
        }
    }
}

fn sdl2_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        stop,
        ..
    } = thread;

    let sdl2 = sdl2::init()
        .map_err(Error::msg)
        .context("failed to initialize sdl2")?;

    let sys_controller = sdl2
        .game_controller()
        .map_err(Error::msg)
        .context("failed to initialize game controller subsystem")?;
    
    let mut mappings = MAPPINGS;
    sys_controller.load_mappings_from_read(&mut mappings)
        .map_err(Error::msg)
        .context("failed to load controller mappings")?;

    let sys_joystick = sdl2
        .joystick()
        .map_err(Error::msg)
        .context("failed to initialize joystick subsystem")?;

    let mut events = sdl2
        .event_pump()
        .map_err(Error::msg)
        .context("failed to initialize event pump")?;

    for event in [
        EventType::JoyAxisMotion,
        EventType::JoyButtonDown,
        EventType::JoyButtonUp,
        EventType::JoyDeviceAdded,
        EventType::JoyDeviceRemoved,
        EventType::JoyHatMotion,
        EventType::ControllerAxisMotion,
        EventType::ControllerButtonDown,
        EventType::ControllerButtonUp,
        EventType::ControllerDeviceAdded,
        EventType::ControllerDeviceRemoved,
    ] {
        events.enable_event(event);
    }

    let mut gamepad_handles = HashMap::<u32, GameController>::new();
    let mut joystick_handles = HashMap::<u32, Joystick>::new();

    let mut gamepads = HashMap::<u32, SdlGamepad>::new();
    let mut joysticks = HashMap::<u32, SdlJoystickBundle>::new();

    while !stop.load(Ordering::Acquire) {
        for event in events.poll_iter() {
            match event {
                Event::JoyAxisMotion {
                    which,
                    axis_idx,
                    value,
                    ..
                } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    let Some(joystick) = joysticks.get_mut(&which)
                    else { continue; };
    
                    joystick.set_axis(axis_idx, value);
                    
                    joystick.update();
                }
                Event::JoyButtonDown {
                    which, button_idx, ..
                } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    let Some(joystick) = joysticks.get_mut(&which)
                    else { continue; };
    
                    joystick.set_button(button_idx, true);
                    
                    joystick.update();
                }
                Event::JoyButtonUp {
                    which, button_idx, ..
                } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    let Some(joystick) = joysticks.get_mut(&which)
                    else { continue; };
    
                    joystick.set_button(button_idx, false);
                    
                    joystick.update();
                }
                Event::JoyDeviceAdded { which, .. } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    let joystick = match sys_joystick.open(which) {
                        Ok(joystick) => joystick,
                        Err(err) => {
                            log::warn!(target: T, "failed to open joystick: {err:?}");
                            continue;
                        }
                    };
    
                    let joystick_bundle = SdlJoystickBundle::new(which, &joystick, &engine)?;
    
                    joysticks.insert(which, joystick_bundle);
                    joystick_handles.insert(which, joystick);
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    joysticks.remove(&which);
                    joystick_handles.remove(&which);
                }
                Event::JoyHatMotion {
                    which,
                    hat_idx,
                    state,
                    ..
                } => {
                    if sys_controller.is_game_controller(which) {
                        continue;
                    }
    
                    let Some(joystick) = joysticks.get_mut(&which)
                    else { continue; };
    
                    joystick.set_hat(hat_idx, state);
                    
                    joystick.update();
                }
    
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => {
                    let Some(gamepad) = gamepads.get_mut(&which)
                    else { continue; };
    
                    let ctrl = &mut gamepad.controller[0];
    
                    let value = (value as f32 + -(i16::MIN as f32)) / (u16::MAX as f32);
                    let value = (value * 255.0) as u8;
    
                    let axis = match axis {
                        Axis::LeftX => &mut ctrl.left_stick_x,
                        Axis::LeftY => &mut ctrl.left_stick_y,
                        Axis::RightX => &mut ctrl.right_stick_x,
                        Axis::RightY => &mut ctrl.right_stick_y,
                        Axis::TriggerLeft => &mut ctrl.l2_analog,
                        Axis::TriggerRight => &mut ctrl.r2_analog,
                    };
    
                    *axis = value;
    
                    gamepad.update();
                }
                Event::ControllerButtonDown { which, button, .. } => {
                    let Some(gamepad) = gamepads.get_mut(&which)
                    else { continue; };
    
                    let Some(button) = convert_button(button)
                    else { continue; };
    
                    button.set_pressed(&mut gamepad.controller[0].buttons);
    
                    gamepad.update();
                }
                Event::ControllerButtonUp { which, button, .. } => {
                    let Some(gamepad) = gamepads.get_mut(&which)
                    else { continue; };
    
                    let Some(button) = convert_button(button)
                    else { continue; };
    
                    button.set_clear(&mut gamepad.controller[0].buttons);
    
                    gamepad.update();
                }
                Event::ControllerDeviceAdded { which, .. } => {
                    gamepads.insert(
                        which,
                        SdlGamepad::new(
                            &engine,
                            sys_joystick
                                .name_for_index(which)
                                .map_err(Error::msg)
                                .context("failed to get joystick name")?,
                            Some(format!("sdl2/{which}")),
                            false,
                            [sdl2_gamepad_info()],
                        )?,
                    );
    
                    let game_controller = sys_controller.open(which)
                        .map_err(Error::msg)
                        .context("failed to open game controller")?;
                    
                    gamepad_handles.insert(which, game_controller);
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    gamepads.remove(&which);
                    gamepad_handles.remove(&which);
                }
    
                event => anyhow::bail!("unknown event: {event:?}"),
            }
        }
    }

    Ok(())
}

crate::device_bundle!(SdlGamepad, controller: Controller);
crate::device_bundle!(SdlJoystick, analog: Analogs, button: Buttons);

struct SdlJoystickBundle<'a> {
    joystick: SdlJoystick<'a>,
    num_real_buttons: u8,
}

impl<'a> SdlJoystickBundle<'a> {
    fn new(id: u32, joystick: &Joystick, engine: &'a Engine) -> Result<Self> {
        if joystick.num_axes() > 8 {
            log::warn!(target: T, "TODO: joystick has more than 8 axes");
        }

        if joystick.num_buttons() + joystick.num_hats() * 4 > 64 {
            log::warn!(target: T, "TODO: joystick has more than 64 buttons");
        }

        let num_axis = u8::min(8, joystick.num_axes() as u8);
        let num_real_buttons = joystick.num_buttons();
        let num_buttons =
            u32::min(64, num_real_buttons + joystick.num_hats() * 4) as u64;
        let mut buttons_info = ButtonsInfo::default();
        for i in 0..num_buttons {
            buttons_info.buttons |= 1 << i;
        }

        let joystick = SdlJoystick::new(
            &engine,
            joystick.name(),
            Some(format!("sdl2/{id}")),
            false,
            [AnalogsInfo { analogs: num_axis }],
            [buttons_info],
        )?;

        Ok(SdlJoystickBundle {
            joystick,
            num_real_buttons: num_real_buttons as u8,
        })
    }

    fn update(&self) {
        self.joystick.update();
    }

    fn set_button(&mut self, button: u8, to: bool) {
        if button >= 64 { return; }

        if to {
            self.joystick.button[0].buttons |= 1 << button;
        } else {
            self.joystick.button[0].buttons &= !(1 << button);
        }
    }

    fn set_axis(&mut self, axis: u8, value: i16) {
        if axis >= 8 { return; }

        let value = (value as f32 + -(i16::MIN as f32)) / (u16::MAX as f32);
        let value = (value * 255.0) as u8;
        self.joystick.analog[0].analogs[axis as usize] = value;
    }

    fn set_hat(&mut self, hat: u8, state: HatState) {
        let up = matches!(state, HatState::Up | HatState::LeftUp | HatState::RightUp);
        let down = matches!(state, HatState::Down | HatState::LeftDown | HatState::RightDown);
        let left = matches!(state, HatState::Left | HatState::LeftDown | HatState::LeftUp);
        let right = matches!(state, HatState::Right | HatState::RightDown | HatState::RightUp);

        let hat_idx = self.num_real_buttons + hat;
        if hat_idx > 60 { return; }

        self.set_button(hat_idx, up);
        self.set_button(hat_idx + 1, down);
        self.set_button(hat_idx + 2, left);
        self.set_button(hat_idx + 3, right);
    }
}

fn convert_button(button: SdlButton) -> Option<Button> {
    Some(match button {
        SdlButton::A => Button::A,
        SdlButton::B => Button::B,
        SdlButton::X => Button::X,
        SdlButton::Y => Button::Y,
        SdlButton::Back => Button::Select,
        SdlButton::Guide => Button::Home,
        SdlButton::Start => Button::Start,
        SdlButton::LeftStick => Button::LStick,
        SdlButton::RightStick => Button::RStick,
        SdlButton::LeftShoulder => Button::L1,
        SdlButton::RightShoulder => Button::R1,
        SdlButton::DPadUp => Button::Up,
        SdlButton::DPadDown => Button::Down,
        SdlButton::DPadLeft => Button::Left,
        SdlButton::DPadRight => Button::Right,
        _ => return None,
    })
}

fn sdl2_gamepad_info() -> ControllerInfo {
    macro_rules! buttons {
        ($($button:expr),* $(,)?) => {{
            let mut buttons = 0;
            $($button.set_pressed(&mut buttons);)*
            buttons
        }};
    }

    ControllerInfo {
        buttons: buttons!(
            Button::A,
            Button::B,
            Button::X,
            Button::Y,
            Button::Start,
            Button::Select,
            Button::Home,
            Button::LStick,
            Button::RStick,
            Button::L1,
            Button::R1,
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
        ),
        analogs: 0,
    }
    .with_lstick()
    .with_rstick()
    .with_l2_analog()
    .with_r2_analog()
}
