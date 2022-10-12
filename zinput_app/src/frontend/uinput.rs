use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use input_linux::{
    AbsoluteAxis, AbsoluteInfo, AbsoluteInfoSetup, EventKind as ILEventKind, Key, UInputHandle,
};
use parking_lot::Mutex;
use zinput_engine::{device::component::controller::{Button, Controller}, DeviceView};
use zinput_engine::{
    eframe::{self, egui},
    plugin::{Plugin, PluginKind, PluginStatus},
    util::Uuid,
    Engine,
};

const T: &'static str = "frontend:uinput";

pub struct UInput {
    inner: Mutex<Inner>,
}

impl UInput {
    pub fn new() -> Self {
        UInput {
            inner: Mutex::new(Inner::Uninit),
        }
    }
}

impl Plugin for UInput {
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
        "uinput"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Frontend
    }

    fn update_gui(&self, ctx: &egui::Context, frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }
}

enum Inner {
    Uninit,
    Init {
        engine: Arc<Engine>,
        status: Arc<Mutex<PluginStatus>>,
        stop: Arc<AtomicBool>,
        handle: JoinHandle<()>,

        device_send: Sender<Vec<Uuid>>,
        selected: Vec<Uuid>,
    },
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));

        let (device_send, device_recv) = crossbeam_channel::unbounded();

        let handle = std::thread::spawn(new_uinput_thread(Thread {
            engine: engine.clone(),
            device_recv,
            status: status.clone(),
            stop: stop.clone(),
        }));

        *self = Inner::Init {
            engine,
            status,
            stop,
            handle,

            device_send,
            selected: Vec::new(),
        };
    }

    fn stop(&mut self) {
        match std::mem::replace(self, Inner::Uninit) {
            Inner::Uninit => {}
            Inner::Init {
                handle,
                status,
                stop,
                ..
            } => {
                stop.store(true, Ordering::Release);

                match handle.join() {
                    Ok(()) => {}
                    Err(_) => log::info!(target: T, "driver panicked"),
                }

                *status.lock() = PluginStatus::Stopped;
            }
        }
    }

    fn status(&self) -> PluginStatus {
        match self {
            Inner::Uninit => PluginStatus::Stopped,
            Inner::Init { status, .. } => status.lock().clone(),
        }
    }

    fn update_gui(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        let Inner::Init {
            engine,
            device_send,
            selected,
            ..
        } = self
        else { return };

        #[derive(PartialEq, Eq)]
        enum Action {
            Remove(usize),
            Change(usize, Uuid),
            Add(Uuid),
        }

        let mut action = None;

        // Devices

        for i in 0..selected.len() {
            if action.is_some() {
                break;
            }
            egui::ComboBox::from_label(format!("UInput Controller {}", i + 1))
                .selected_text(match engine.get_device(&selected[i]) {
                    Some(view) => view.info().name.clone(),
                    None => {
                        action = Some(Action::Remove(i));
                        break;
                    }
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut action, Some(Action::Remove(i)), "[None]");
                    for entry in engine.devices() {
                        ui.selectable_value(
                            &mut action,
                            Some(Action::Change(i, *entry.uuid())),
                            &entry.info().name,
                        );
                    }
                });
        }

        if selected.len() < 4 && action.is_none() {
            egui::ComboBox::from_label(format!(
                "ViGEm XBox Controller {}",
                selected.len() + 1
            ))
            .selected_text("[None]")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut action, None, "[None]");
                for entry in engine.devices() {
                    ui.selectable_value(
                        &mut action,
                        Some(Action::Add(*entry.uuid())),
                        &entry.info().name,
                    );
                }
            });
        }

        if let Some(action) = action {
            match action {
                Action::Remove(i) => {
                    selected.remove(i);
                }
                Action::Change(i, id) => selected[i] = id,
                Action::Add(id) => selected.push(id),
            }

            device_send.send(selected.clone()).unwrap();
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_recv: Receiver<Vec<Uuid>>,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
}

fn new_uinput_thread(thread: Thread) -> impl FnOnce() {
    || {
        let status = thread.status.clone();
        match uinput_thread(thread) {
            Ok(()) => {
                log::info!(target: T, "uinput thread closed");
                *status.lock() = PluginStatus::Stopped;
            }
            Err(e) => {
                log::error!(target: T, "uinput thread crashed: {}", e);
                *status.lock() = PluginStatus::Error(format!("uinput thread crashed: {}", e));
            }
        }
    }
}

fn uinput_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_recv,
        stop,
        ..
    } = thread;

    let uinput = init_uinput()?;

    let (update_send, update_recv) = crossbeam_channel::bounded(10);

    let mut joysticks = Vec::<Joystick>::new();

    loop {
        crossbeam_channel::select! {
            recv(device_recv) -> device_recv => {
                let Ok(ids) = device_recv
                else { return Ok(()); }; // Sender dropped which means plugin is uninitialized

                if ids.len() < joysticks.len() {
                    joysticks.truncate(ids.len());
                } else if ids.len() > joysticks.len() {
                    for i in joysticks.len()..ids.len() {
                        let uinput_device = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(&uinput)
                            .context("failed to open uinput device")?;

                        let uinput_device = UInputHandle::new(uinput_device);

                        let Some(mut view) = engine.get_device(&ids[i])
                        else { anyhow::bail!("tried to get device with invalid uuid"); };
                        view.register_channel(update_send.clone());

                        joysticks.push(Joystick::new(view, uinput_device)?);
                    }
                }
            },
            recv(update_recv) -> uid => {
                let Ok(uid) = uid
                else { continue; };

                for joystick in &joysticks {
                    if joystick.view.uuid() != &uid { continue; };

                    joystick.update()?;
                }
            }
            default(Duration::from_secs(1)) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
            }
        }
    }

    Ok(())
}

struct Joystick {
    view: DeviceView,

    uinput_device: UInputHandle<File>,
}

impl Joystick {
    fn new(view: DeviceView, uinput_device: UInputHandle<File>) -> Result<Self> {
        macro_rules! keybits {
            ($device:expr, $($key:expr),* $(,)?) => {
                $($device.set_keybit($key)?;)*
            }
        }

        let ud = uinput_device;

        ud.set_evbit(ILEventKind::Key)?;
        keybits!(
            ud,
            Key::ButtonNorth,
            Key::ButtonSouth,
            Key::ButtonEast,
            Key::ButtonWest,
            Key::ButtonDpadDown,
            Key::ButtonDpadLeft,
            Key::ButtonDpadRight,
            Key::ButtonDpadUp,
            Key::ButtonStart,
            Key::ButtonSelect,
            Key::ButtonTL,
            Key::ButtonTR,
            Key::ButtonTL2,
            Key::ButtonTR2,
            Key::ButtonThumbl,
            Key::ButtonThumbr,
            Key::ButtonMode,
        );

        ud.set_evbit(ILEventKind::Absolute)?;
        ud.set_absbit(AbsoluteAxis::X)?;
        ud.set_absbit(AbsoluteAxis::Y)?;
        ud.set_absbit(AbsoluteAxis::RX)?;
        ud.set_absbit(AbsoluteAxis::RY)?;
        ud.set_absbit(AbsoluteAxis::Z)?;
        ud.set_absbit(AbsoluteAxis::RZ)?;

        const DEFAULT_INFO: AbsoluteInfo = AbsoluteInfo {
            value: 0,
            minimum: 0,
            maximum: 255,
            fuzz: 0,
            flat: 0,
            resolution: 0,
        };

        ud.create(
            &input_linux::InputId::default(),
            view.info().name.as_bytes(),
            0,
            &[
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::X,
                    info: DEFAULT_INFO,
                },
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::Y,
                    info: DEFAULT_INFO,
                },
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::RX,
                    info: DEFAULT_INFO,
                },
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::RY,
                    info: DEFAULT_INFO,
                },
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::Z,
                    info: DEFAULT_INFO,
                },
                AbsoluteInfoSetup {
                    axis: AbsoluteAxis::RZ,
                    info: DEFAULT_INFO,
                },
            ],
        )
        .context("failed to create uinput device")?;

        Ok(Joystick {
            view,
            uinput_device: ud,
        })
    }

    fn update(&self) -> Result<()> {
        use input_linux::sys as ils;

        let device = self.view.device();
        let Some(data) = device.controllers.get(0)
        else { return Ok(()); };

        macro_rules! make_events {
            (
                buttons { $($btnfrom:expr => $btnto:expr),* $(,)? }
                analogs { $($afrom:expr => $ato:expr),* $(,)? }
            ) => {
                [
                    $(ils::input_event {
                        time: ils::timeval { tv_sec: 0, tv_usec: 0 },
                        type_: ils::EV_KEY as _,
                        code: $btnto as _,
                        value: $btnfrom.is_pressed(data.buttons) as _,
                    },)*
                    $(ils::input_event {
                        time: ils::timeval { tv_sec: 0, tv_usec: 0 },
                        type_: ils::EV_ABS as _,
                        code: $ato as _,
                        value: $afrom as _,
                    },)*
                    ils::input_event {
                        time: ils::timeval { tv_sec: 0, tv_usec: 0 },
                        type_: ils::EV_SYN as _,
                        code: ils::SYN_REPORT as _,
                        value: 0,
                    }
                ]
            };
        }

        // todo

        let events = make_events! {
            buttons {
                Button::A      => ils::BTN_SOUTH,
                Button::B      => ils::BTN_EAST,
                Button::X      => ils::BTN_WEST,
                Button::Y      => ils::BTN_NORTH,
                Button::Up     => ils::BTN_DPAD_UP,
                Button::Down   => ils::BTN_DPAD_DOWN,
                Button::Left   => ils::BTN_DPAD_LEFT,
                Button::Right  => ils::BTN_DPAD_RIGHT,
                Button::Start  => ils::BTN_START,
                Button::Select => ils::BTN_SELECT,
                Button::L1     => ils::BTN_TL,
                Button::R1     => ils::BTN_TR,
                Button::L2     => ils::BTN_TL2,
                Button::R2     => ils::BTN_TR2,
                Button::LStick => ils::BTN_THUMBL,
                Button::RStick => ils::BTN_THUMBR,
                Button::Home   => ils::BTN_MODE,
            }
            analogs {
                data.left_stick_x          => ils::ABS_X,
                (255 - data.left_stick_y)  => ils::ABS_Y,
                data.right_stick_x         => ils::ABS_RX,
                (255 - data.right_stick_y) => ils::ABS_RY,
                data.l2_analog             => ils::ABS_Z,
                data.r2_analog             => ils::ABS_RZ,
            }
        };

        let mut written = 0;
        while written < events.len() {
            written += self.uinput_device.write(&events[written..])?;
        }

        Ok(())
    }
}

impl Drop for Joystick {
    fn drop(&mut self) {
        match self.uinput_device.dev_destroy() {
            Ok(()) => {}
            Err(err) => log::warn!(target: T, "failed to destroy uinput device: {}", err),
        }
    }
}

fn init_uinput() -> Result<PathBuf> {
    // let mut udev = udev::Enumerator::new()?;
    // udev.match_subsystem("misc")?;
    // udev.match_sysname("uinput")?;
    // let mut devices = udev.scan_devices()?;
    // let uinput_device = devices
        // .next()
        // .ok_or(anyhow::anyhow!("uinput system not found"))?;
    // let uinput_devnode = uinput_device
        // .devnode()
        // .ok_or(anyhow::anyhow!("uinput system does not have devnode"))?;
// 
    // Ok(uinput_devnode.to_owned())
    Ok("/dev/uinput".into())
}
