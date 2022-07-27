use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

use crossbeam_channel::{Sender, TrySendError};
use index_map::IndexMap;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use paste::paste;
use uuid::Uuid;
use zinput_device::{Device, DeviceConfig, DeviceConfigMut, DeviceInfo, DeviceMut};

pub struct DeviceHandle {
    internal: Arc<InternalDevice>,
}

impl DeviceHandle {
    pub(super) fn new(internal: Arc<InternalDevice>) -> Option<Self> {
        internal
            .handle
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| DeviceHandle { internal })
    }

    pub fn update<F>(&self, mut updater: F)
    where
        F: for<'a> FnMut(DeviceMut<'a>),
    {
        {
            let mut device_raw = self.internal.device_raw.write();
            updater(device_raw.as_mut());
        }

        {
            let mut device = self.internal.device.write();
            updater(device.as_mut());
            self.internal.config.read().configure(device.as_mut());
        }

        self.internal.channels.lock().retain(|_, channel| {
            match channel.try_send(self.internal.uuid) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }

    pub fn view(&self) -> DeviceView {
        DeviceView::new(self.internal.clone())
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.internal.info
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        assert!(
            self.internal.handle.load(Ordering::Acquire) == true,
            "DeviceHandle was already dropped"
        );

        self.internal.handle.store(false, Ordering::Release);
    }
}

pub struct DeviceView {
    internal: Arc<InternalDevice>,

    channel: Option<usize>,
}

impl DeviceView {
    pub(super) fn new(internal: Arc<InternalDevice>) -> Self {
        internal.views.fetch_add(1, Ordering::AcqRel);
        DeviceView {
            internal,
            channel: None,
        }
    }

    pub fn config(&self) -> ConfigRead {
        ConfigRead {
            lock: self.internal.config.read(),
        }
    }

    pub fn config_mut(&self) -> ConfigWrite {
        ConfigWrite {
            lock: self.internal.config.write(),
        }
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.internal.info
    }

    pub fn device(&self) -> DeviceRead {
        DeviceRead {
            lock: self.internal.device.read(),
        }
    }

    pub fn device_raw(&self) -> DeviceRead {
        DeviceRead {
            lock: self.internal.device_raw.read(),
        }
    }

    pub fn uuid(&self) -> &Uuid {
        &self.internal.uuid
    }

    pub fn register_channel(&mut self, channel: Sender<Uuid>) {
        if let Some(channel) = self.channel.take() {
            self.internal.channels.lock().remove(channel);
        }

        let channel = self.internal.channels.lock().insert(channel);
        self.channel = Some(channel);
    }

    pub fn saved_configs(&self) -> anyhow::Result<Vec<String>> {
        saved_configs()
    }

    pub fn load_config(&self, name: &str) -> anyhow::Result<()> {
        self.internal.load_config(name)
    }

    pub fn save_config(&self, name: &str) -> anyhow::Result<()> {
        self.internal.save_config(name)
    }

    pub fn delete_config(&self, name: &str) -> anyhow::Result<()> {
        self.internal.delete_config(name)
    }

    pub fn reset_config(&self) {
        self.internal.reset_config()
    }
}

impl Clone for DeviceView {
    fn clone(&self) -> Self {
        DeviceView::new(self.internal.clone())
    }
}

impl Drop for DeviceView {
    fn drop(&mut self) {
        assert!(
            self.internal.views.load(Ordering::Acquire) > 0,
            "number of DeviceViews was incorrect"
        );

        if let Some(channel) = self.channel.take() {
            self.internal.channels.lock().remove(channel);
        }

        self.internal.views.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct DeviceRead<'a> {
    lock: RwLockReadGuard<'a, Device>,
}

impl<'a> Deref for DeviceRead<'a> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.lock.deref()
    }
}

pub struct ConfigRead<'a> {
    lock: RwLockReadGuard<'a, DeviceConfig>,
}

impl<'a> Deref for ConfigRead<'a> {
    type Target = DeviceConfig;

    fn deref(&self) -> &DeviceConfig {
        self.lock.deref()
    }
}

pub struct ConfigWrite<'a> {
    lock: RwLockWriteGuard<'a, DeviceConfig>,
}

impl<'a> ConfigWrite<'a> {
    pub fn get(&mut self) -> DeviceConfigMut {
        self.lock.as_mut()
    }
}

pub(super) struct InternalDevice {
    pub(super) uuid: Uuid,

    pub(super) handle: AtomicBool,
    views: AtomicUsize,

    config: RwLock<DeviceConfig>,
    info: DeviceInfo,
    device: RwLock<Device>,
    device_raw: RwLock<Device>,

    channels: Mutex<IndexMap<Sender<Uuid>>>,
}

macro_rules! internal_device_components {
    ($($field_name:ident : $ctype:ty),* $(,)?) => {
        paste! {
            impl InternalDevice {
                pub(super) fn new(info: DeviceInfo, uuid: Uuid) -> Arc<Self> {
                    let device = Device {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };
                    let device = RwLock::new(device);

                    let device_raw = Device {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };
                    let device_raw = RwLock::new(device_raw);

                    let mut config = DeviceConfig {
                        $([< $field_name s >]: vec![Default::default(); info.[< $field_name s >].len()]),*
                    };

                    if info.autoload_config {
                        if let Some(id) = &info.id {
                            match load_config(id) {
                                // TODO: Config validation
                                Ok(loaded) => { config = loaded; },
                                Err(err) => {
                                    log::warn!("failed to load config for device '{id}': {err:?}");
                                }
                            }
                        }
                    }

                    let config = RwLock::new(config);

                    Arc::new(InternalDevice {
                        uuid,

                        handle: AtomicBool::new(false),
                        views: AtomicUsize::new(0),

                        config,
                        info,
                        device,
                        device_raw,

                        channels: Mutex::default(),
                    })
                }

                pub(super) fn info(&self) -> &DeviceInfo {
                    &self.info
                }

                pub(super) fn should_remove(&self) -> bool {
                    (self.handle.load(Ordering::Acquire) == false)
                        && (self.views.load(Ordering::Acquire) == 0)
                }

                fn load_config(&self, name: &str) -> anyhow::Result<()> {
                    let cfg = load_config(name)?;
                    // TODO: Config validation
                    *self.config.write() = cfg;

                    Ok(())
                }

                fn save_config(&self, name: &str) -> anyhow::Result<()> {
                    let cfg = self.config.read();

                    save_config(name, &cfg)
                }

                fn delete_config(&self, name: &str) -> anyhow::Result<()> {
                    delete_config(name)
                }

                pub fn reset_config(&self) {
                    *self.config.write() = DeviceConfig {
                        $([< $field_name s >]: vec![Default::default(); self.info.[< $field_name s >].len()]),*
                    };
                }
            }
        }
    };
}

internal_device_components!(
    controller: Controller,
    motion: Motion,
    analog: Analogs,
    button: Buttons,
    touch_pad: TouchPad,
);

fn load_config(name: &str) -> anyhow::Result<DeviceConfig> {
    use anyhow::Context;
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(format!("config/{name}.json"))
        .with_context(|| format!("failed to open file '{}'", format!("config/{name}.json")))?;

    let mut string =
        String::with_capacity(file.metadata().map(|meta| meta.len() as usize).unwrap_or(0));
    file.read_to_string(&mut string)
        .with_context(|| format!("failed to read file '{}'", format!("config/{name}.json")))?;

    let config: DeviceConfig = serde_json::from_str(&string).with_context(|| {
        format!(
            "failed to deserialize file '{}'",
            format!("config/{name}.json")
        )
    })?;

    Ok(config)
}

fn save_config(name: &str, config: &DeviceConfig) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::fs::OpenOptions;
    use std::io::Write;

    let _ = std::fs::create_dir("config");

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("config/{name}.json"))
        .with_context(|| format!("failed to open file '{}'", format!("config/{name}.json")))?;

    let string = serde_json::to_string(config).with_context(|| {
        format!(
            "failed to serialize file '{}'",
            format!("config/{name}.json")
        )
    })?;

    file.write_all(string.as_bytes())
        .with_context(|| format!("failed to write file '{}'", format!("config/{name}.json")))?;

    Ok(())
}

fn delete_config(name: &str) -> anyhow::Result<()> {
    let _ = std::fs::create_dir("config");

    let _ = std::fs::remove_file(format!("config/{name}.json"));

    Ok(())
}

fn saved_configs() -> anyhow::Result<Vec<String>> {
    use anyhow::Context;

    let _ = std::fs::create_dir("config");

    let mut configs = Vec::new();

    for entry in std::fs::read_dir("config").context("failed to read config directory")? {
        let Ok(entry) = entry
        else { continue; };

        let path = entry.path();

        let Some("json") = path.extension().and_then(|e| e.to_str())
        else { continue; };

        let file_name = entry.file_name();

        let Some(file_name) = file_name.to_str()
        else { continue; };

        configs.push(file_name[..(file_name.len() - 5)].to_owned());
    }

    Ok(configs)
}
