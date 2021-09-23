use std::{collections::HashSet, fs::{File, OpenOptions}, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use input_linux::{AbsoluteAxis, AbsoluteInfo, AbsoluteInfoSetup, EventKind, Key, UInputHandle};
use parking_lot::Mutex;
use uuid::Uuid;

use crate::{
    api::{
        component::controller::{Button, Controller},
        component::motion::Motion,
        Frontend,
    },
    zinput::engine::Engine,
};

const T: &'static str = "frontend:uinput";

pub struct UInput {
    inner: Mutex<Inner>,
    signals: Arc<Signals>,
}

impl UInput {
    pub fn new() -> Self {
        UInput {
            inner: Mutex::new(Inner::new()),
            signals: Arc::new(Signals::new()),
        }
    }
}

impl Frontend for UInput {
    fn init(&self, engine: Arc<Engine>) {
        self.inner.lock().init(engine, self.signals.clone());
    }

    fn name(&self) -> &str {
        "uinput"
    }

    fn update_gui(
        &self,
        ctx: &eframe::egui::CtxRef,
        frame: &mut eframe::epi::Frame<'_>,
        ui: &mut eframe::egui::Ui,
    ) {
        self.inner.lock().update_gui(ctx, frame, ui)
    }

    fn on_component_update(&self, id: &Uuid) {
        if self.signals.listen_update.lock().contains(id) && !self.signals.update.0.is_full() {
            // unwrap: the channel cannot become disconnected as it is Arc-owned by Self
            self.signals.update.0.send(*id).unwrap();
        }
    }
}

struct Inner {
    device: Sender<(usize, Option<Uuid>)>,
    device_recv: Option<Receiver<(usize, Option<Uuid>)>>,
    engine: Option<Arc<Engine>>,

    selected_devices: [Option<Uuid>; 4],
}

impl Inner {
    fn new() -> Self {
        let (device, device_recv) = crossbeam_channel::unbounded();
        Inner {
            device,
            device_recv: Some(device_recv),
            engine: None,

            selected_devices: [None; 4],
        }
    }
}

impl Inner {
    fn init(&mut self, engine: Arc<Engine>, signals: Arc<Signals>) {
        self.engine = Some(engine.clone());
        std::thread::spawn(new_uinput_thread(Thread {
            engine,
            device_change: std::mem::replace(&mut self.device_recv, None).unwrap(),
            signals,
        }));
    }

    fn update_gui(
        &mut self,
        _ctx: &eframe::egui::CtxRef,
        _frame: &mut eframe::epi::Frame<'_>,
        ui: &mut eframe::egui::Ui,
    ) {
        if let Some(engine) = self.engine.clone() {
            for i in 0..self.selected_devices.len() {
                egui::ComboBox::from_label(format!("UInput Controller {}", i + 1))
                    .selected_text(
                        self.selected_devices[i]
                            .and_then(|id| engine.get_device(&id))
                            .map_or("[None]".to_owned(), |dev| dev.name.clone()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(&mut self.selected_devices[i], None, "[None]")
                            .clicked()
                        {
                            self.device.send((i, None)).unwrap();
                        }
                        for device_ref in engine.devices() {
                            if ui
                                .selectable_value(
                                    &mut self.selected_devices[i],
                                    Some(*device_ref.key()),
                                    &device_ref.name,
                                )
                                .clicked()
                            {
                                self.device.send((i, Some(*device_ref.key()))).unwrap();
                            }
                        }
                    });
            }
        }
    }
}

struct Signals {
    listen_update: Mutex<HashSet<Uuid>>,
    update: (Sender<Uuid>, Receiver<Uuid>),
}

impl Signals {
    fn new() -> Self {
        Signals {
            listen_update: Mutex::new(HashSet::new()),
            update: crossbeam_channel::bounded(4),
        }
    }
}

struct Thread {
    engine: Arc<Engine>,
    device_change: Receiver<(usize, Option<Uuid>)>,
    signals: Arc<Signals>,
}

fn new_uinput_thread(thread: Thread) -> impl FnOnce() {
    || match uinput_thread(thread) {
        Ok(()) => log::info!(target: T, "uinput thread closed"),
        Err(e) => log::error!(target: T, "uinput thread crashed: {}", e),
    }
}

fn uinput_thread(thread: Thread) -> Result<()> {
    let Thread {
        engine,
        device_change,
        signals,
    } = thread;

    let uinput = init_uinput()?;

    let mut joysticks = Vec::<Joystick>::new();

    loop {
        crossbeam_channel::select! {
            recv(device_change) -> device_change => {
                match device_change {
                    Ok((idx, Some(device_id))) => {
                        if let Some(joystick) = joysticks.get(idx) {
                            let mut signals = signals.listen_update.lock();
                            signals.remove(&joystick.controller_id);
                            if let Some(motion_id) = &joystick.motion_id {
                                signals.remove(motion_id);
                            }
                        }

                        if idx > joysticks.len() {
                            log::error!(target: T, "tried to add controller to index {} but there are only {} joysticks", idx, joysticks.len());
                            continue;
                        }

                        let name = match engine.get_device(&device_id) {
                            Some(device) => device.name.clone(),
                            None => {
                                log::error!(target: T, "tried to add non-existent controller");
                                continue;
                            }
                        };

                        let controller_id = match engine.get_device(&device_id)
                            .and_then(|device| device.controller)
                        {
                            Some(id) => id,
                            None => {
                                log::error!(target: T, "tried to add controller without controller component");
                                continue;
                            }
                        };
                        signals.listen_update.lock().insert(controller_id);

                        let motion_id = engine.get_device(&device_id)
                            .and_then(|device| device.motion)
                            .map(|id| {
                                signals.listen_update.lock().insert(id);
                                id
                            });
                        
                        let uinput_device = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(&uinput)
                            .context("failed to open uinput device")?;
                        
                        let uinput_device = UInputHandle::new(uinput_device);

                        let joystick = Joystick::new(&name, controller_id, motion_id, uinput_device)?;
                        
                        joysticks.insert(idx, joystick);
                    }
                    Ok((idx, None)) => {
                        if let Some(joystick) = joysticks.get(idx) {
                            let mut signals = signals.listen_update.lock();
                            signals.remove(&joystick.controller_id);
                            if let Some(motion_id) = &joystick.motion_id {
                                signals.remove(motion_id);
                            }

                            joysticks.remove(idx);
                        } else {
                            log::error!(target: T, "tried to remove controller out of bounds at index {} when len is {}", idx, joysticks.len());
                        }
                    }
                    Err(_) => {
                        // todo
                    }
                }
            },
            recv(signals.update.1) -> uid => {
                let uid = match uid {
                    Ok(uid) => uid,
                    Err(_) => {
                        // todo
                        continue;
                    }
                };

                for joystick in &joysticks {
                    if joystick.controller_id == uid {
                        let controller = match engine.get_controller(&uid) {
                            Some(controller) => controller,
                            None => continue,
                        };

                        joystick.update_controller(&controller.data)?;
                    } else {
                        if let Some(motion_id) = &joystick.motion_id {
                            if motion_id == &uid {
                                let motion = match engine.get_motion(&uid) {
                                    Some(motion) => motion,
                                    None => continue,
                                };
        
                                joystick.update_motion(&motion.data)?;
                            }
                        }
                    }
                }
            }
        }
    }
}

struct Joystick {
    controller_id: Uuid,
    motion_id: Option<Uuid>,

    uinput_device: UInputHandle<File>,
}

impl Joystick {
    fn new(name: &str, controller_id: Uuid, motion_id: Option<Uuid>, uinput_device: UInputHandle<File>) -> Result<Self> {
        macro_rules! keybits {
            ($device:expr, $($key:expr),* $(,)?) => {
                $($device.set_keybit($key)?;)*
            }
        }

        let ud = uinput_device;

        ud.set_evbit(EventKind::Key)?;
        keybits!(ud,
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

        ud.set_evbit(EventKind::Absolute)?;
        ud.set_absbit(AbsoluteAxis::X)?;
        ud.set_absbit(AbsoluteAxis::Y)?;
        ud.set_absbit(AbsoluteAxis::RX)?;
        ud.set_absbit(AbsoluteAxis::RY)?;
        ud.set_absbit(AbsoluteAxis::Z)?;
        ud.set_absbit(AbsoluteAxis::RZ)?;

        const DEFAULT_INFO: AbsoluteInfo = AbsoluteInfo { value: 0, minimum: 0, maximum: 255, fuzz: 0, flat: 0, resolution: 0 };

        ud.create(
            &input_linux::InputId::default(),
            name.as_bytes(),
            0,
            &[
                AbsoluteInfoSetup { axis: AbsoluteAxis::X, info: DEFAULT_INFO },
                AbsoluteInfoSetup { axis: AbsoluteAxis::Y, info: DEFAULT_INFO },
                AbsoluteInfoSetup { axis: AbsoluteAxis::RX, info: DEFAULT_INFO },
                AbsoluteInfoSetup { axis: AbsoluteAxis::RY, info: DEFAULT_INFO },
                AbsoluteInfoSetup { axis: AbsoluteAxis::Z, info: DEFAULT_INFO },
                AbsoluteInfoSetup { axis: AbsoluteAxis::RZ, info: DEFAULT_INFO },
            ],
        ).context("failed to create uinput device")?;
        
        Ok(Joystick {
            controller_id,
            motion_id,

            uinput_device: ud,
        })
    }

    fn update_controller(&self, data: &Controller) -> Result<()> {
        use input_linux::sys as ils;
        
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
                data.left_stick_x  => ils::ABS_X,
                data.left_stick_y  => ils::ABS_Y,
                data.right_stick_x => ils::ABS_RX,
                data.right_stick_y => ils::ABS_RY,
                data.l2_analog     => ils::ABS_Z,
                data.r2_analog     => ils::ABS_RZ,
            }
        };

        let mut written = 0;
        while written < events.len() {
            written += self.uinput_device.write(&events[written..])?;
        }
    
        Ok(())
    }

    fn update_motion(&self, data: &Motion) -> Result<()> {
        // TODO: update
    
        Ok(())
    }
}

impl Drop for Joystick {
    fn drop(&mut self) {
        match self.uinput_device.dev_destroy() {
            Ok(()) => {},
            Err(err) => log::warn!(target: T, "failed to destroy uinput device: {}", err),
        }
    }
}

fn init_uinput() -> Result<PathBuf> {
    let mut udev = udev::Enumerator::new()?;
    udev.match_subsystem("misc")?;
    udev.match_sysname("uinput")?;
    let mut devices = udev.scan_devices()?;
    let uinput_device = devices.next().ok_or(anyhow::anyhow!("uinput system not found"))?;
    let uinput_devnode = uinput_device.devnode().ok_or(anyhow::anyhow!("uinput system does not have devnode"))?;

    Ok(uinput_devnode.to_owned())
}