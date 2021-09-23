use std::{collections::HashSet, fs::File, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use input_linux::{AbsoluteAxis, EventKind, Key, UInputHandle};
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
                        
                        let uinput_device = File::open(&uinput)
                            .context("failed to open uinput device")?;
                        
                        let uinput_device = UInputHandle::new(uinput_device);

                        let joystick = Joystick::new(&name, device_id, controller_id, motion_id, uinput_device)?;
                        
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
    device_id: Uuid,
    controller_id: Uuid,
    motion_id: Option<Uuid>,

    uinput_device: UInputHandle<File>,
}

impl Joystick {
    fn new(name: &str, device_id: Uuid, controller_id: Uuid, motion_id: Option<Uuid>, uinput_device: UInputHandle<File>) -> Result<Self> {
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
            Key::ButtonConfig,
        );

        ud.set_evbit(EventKind::Absolute)?;
        ud.set_absbit(AbsoluteAxis::X)?;
        ud.set_absbit(AbsoluteAxis::Y)?;
        ud.set_absbit(AbsoluteAxis::RX)?;
        ud.set_absbit(AbsoluteAxis::RY)?;
        ud.set_absbit(AbsoluteAxis::Z)?;
        ud.set_absbit(AbsoluteAxis::RZ)?;

        ud.create(
            &input_linux::InputId::default(),
            name.as_bytes(),
            0,
            &[],
        ).context("failed to create uinput device")?;
        
        todo!()
    }
    
    fn update_controller(&self, data: &Controller) -> Result<()> {
        // TODO: update
    
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