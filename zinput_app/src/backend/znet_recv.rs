use std::{
    collections::HashMap,
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use zinput_engine::{
    device::{components, DeviceInfo},
    eframe::{self, egui},
    plugin::{Plugin, PluginKind, PluginStatus},
    DeviceHandle, Engine,
};
use znet::Receiver;

const T: &'static str = "backend:znet_recv";

const DEFAULT_PORT: &'static str = "26810";
const DEVICE_TIMEOUT: Duration = Duration::from_secs(5);

const BUFFER_SIZE: usize = 4096;

const TIMEOUT_KIND: std::io::ErrorKind = {
    #[cfg(target_os = "windows")]
    {
        std::io::ErrorKind::TimedOut
    }
    #[cfg(target_os = "linux")]
    {
        std::io::ErrorKind::WouldBlock
    }
};

pub struct ZNet {
    inner: Mutex<Inner>,
}

impl ZNet {
    pub fn new() -> Self {
        ZNet {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Plugin for ZNet {
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
        "znet_recv"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }

    fn update_gui(&self, _ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        let mut inner = self.inner.lock();
        ui.label(format!("Port: {}", inner.gui().old_port));

        if matches!(&*inner, Inner::Uninit { .. }) {
            ui.text_edit_singleline(&mut inner.gui().port);
        }
    }
}

#[derive(Clone)]
struct Gui {
    old_port: String,
    port: String,
}

impl Gui {
    fn new() -> Self {
        Gui {
            old_port: DEFAULT_PORT.to_owned(),
            port: DEFAULT_PORT.to_owned(),
        }
    }
}

enum Inner {
    Uninit {
        gui: Gui,
    },
    Init {
        handle: JoinHandle<()>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<PluginStatus>>,
        gui: Gui,
    },
}

impl Inner {
    fn new() -> Self {
        Inner::Uninit { gui: Gui::new() }
    }

    fn gui(&mut self) -> &mut Gui {
        match self {
            Inner::Uninit { gui } => gui,
            Inner::Init { gui, .. } => gui,
        }
    }

    fn init(&mut self, api: Arc<Engine>) {
        if matches!(self, Inner::Init { .. }) {
            self.stop();
        }
        let gui = self.gui().clone();

        let status = Arc::new(Mutex::new(PluginStatus::Running));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = std::thread::spawn(znet_thread(
            gui.port.clone(),
            status.clone(),
            stop.clone(),
            api,
        ));

        *self = Inner::Init {
            handle,
            stop,
            status,
            gui,
        };
    }

    fn stop(&mut self) {
        let gui = self.gui().clone();

        match std::mem::replace(self, Inner::Uninit { gui }) {
            Inner::Uninit { .. } => {}
            Inner::Init {
                handle,
                stop,
                status,
                ..
            } => {
                stop.store(true, Ordering::SeqCst);

                match handle.join() {
                    Ok(()) => (),
                    Err(_) => log::info!(target: T, "driver panicked"),
                }

                *status.lock() = PluginStatus::Stopped;
            }
        }
    }

    fn status(&self) -> PluginStatus {
        match self {
            Inner::Uninit { .. } => PluginStatus::Stopped,
            Inner::Init { status, .. } => status.lock().clone(),
        }
    }
}

impl Drop for ZNet {
    fn drop(&mut self) {
        self.stop();
    }
}

fn znet_thread(
    port: String,
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    api: Arc<Engine>,
) -> impl FnOnce() {
    move || {
        log::info!(target: T, "driver initialized");

        match znet(port, stop, api) {
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

fn znet(port: String, stop: Arc<AtomicBool>, engine: Arc<Engine>) -> Result<()> {
    #[repr(C, align(8))]
    struct AlignedBuffer([u8; BUFFER_SIZE]);

    let mut buffer = Box::new(AlignedBuffer([0; BUFFER_SIZE]));
    let mut conn = unsafe { ZNetConn::new(engine, port, &mut buffer.0)? };

    conn.receiver
        .socket
        .set_read_timeout(Some(Duration::from_secs(1)))?;

    while !stop.load(Ordering::Acquire) {
        match conn.receive_data() {
            Ok(()) => (),
            Err(err) if err.kind() == TIMEOUT_KIND => {
                continue;
            }
            Err(err) => {
                return Err(err).context("failed to receive znet data");
            }
        }

        conn.update()?;
    }

    Ok(())
}

struct ZNetConn<'a> {
    engine: Arc<Engine>,
    receiver: Receiver<'a>,
    devices: HashMap<[u8; 16], DeviceBundle>,
}

impl<'a> ZNetConn<'a> {
    /// # Safety
    /// `buffer` must be aligned to 8 bytes
    unsafe fn new(engine: Arc<Engine>, port: String, buffer: &'a mut [u8]) -> Result<ZNetConn<'a>> {
        let port: u16 = port.parse().context("port is not a valid number")?;
        let socket =
            UdpSocket::bind(format!("0.0.0.0:{}", port)).context("failed to bind address")?;

        Ok(ZNetConn {
            engine,
            receiver: Receiver::new(socket, buffer),
            devices: HashMap::new(),
        })
    }

    fn receive_data(&mut self) -> std::io::Result<()> {
        // Clear inactive devices
        let now = Instant::now();

        self.devices
            .retain(|_, bundle| now - bundle.last_update < DEVICE_TIMEOUT);

        // Receive data
        self.receiver.recv()?;

        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        for name in self.receiver.device_names() {
            let Some(data) = self.receiver.device(name)
            else { continue };

            // Remove device if number of components changed
            if let Some(bundle) = self.devices.get_mut(name) {
                macro_rules! verify_device {
                    ($($cname:ident : $ctype:ty),* $(,)?) => {{
                        paste::paste! {
                            let info = bundle.handle.info();

                            true
                            $(&& info.[< $cname s >].len() == data.[< $cname s >]().len())*
                        }
                    }}
                }

                if !components!(data verify_device) {
                    self.devices.remove(name);
                }
            }

            let bundle = match self.devices.get_mut(name) {
                Some(bundle) => bundle,
                None => {
                    macro_rules! create_device {
                        ($($cname:ident : $citype:ty),* $(,)?) => {{
                            paste::paste! {
                                let mut name_end = 0;
                                for i in 0..16 {
                                    if name[i] != 0 {
                                        name_end = i + 1;
                                    }
                                }

                                DeviceInfo {
                                    name: match std::str::from_utf8(&name[..name_end]) {
                                        Ok(name) => format!("znet: {name}"),
                                        Err(_) => {
                                            let int = u128::from_le_bytes(*name);
                                            format!("znet: {int:X}")
                                        }
                                    },
                                    id: Some({
                                        let int = u128::from_le_bytes(*name);
                                        let mut id = format!("{int:X}");

                                        $(
                                            id.push_str(&format!(",{}", data.[< $cname s >]().len()));
                                        )*

                                        id
                                    }),
                                    autoload_config: false,
                                    $([< $cname s >]: vec![Default::default(); data.[< $cname s >]().len()],)*
                                }
                            }
                        }}
                    }

                    let handle = self.engine.new_device(components!(info create_device))?;

                    self.devices.insert(
                        *name,
                        DeviceBundle {
                            last_update: Instant::now(),
                            handle,
                        },
                    );
                    self.devices.get_mut(name).unwrap()
                }
            };

            bundle.last_update = Instant::now();

            macro_rules! update_device {
                ($($cname:ident : $ctype:ty),* $(,)?) => {
                    paste::paste! {
                        bundle.handle.update(|device| {
                            $(
                                for (data_out, data_in) in device.[< $cname s >]
                                    .iter_mut()
                                    .zip(data.[< $cname s >]().iter())
                                {
                                    data_out.clone_from(data_in);
                                }
                            )*
                        });
                    }
                }
            }

            components!(data update_device);
        }

        Ok(())
    }
}

struct DeviceBundle {
    last_update: Instant,
    handle: DeviceHandle,
}
