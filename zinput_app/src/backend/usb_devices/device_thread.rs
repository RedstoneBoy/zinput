use std::{
    ops::ControlFlow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use rusb::{Device, DeviceHandle, GlobalContext};
use zinput_engine::Engine;

use super::{util::UsbExt, ThreadData};

const T: &'static str = "backend:usb_devices";

pub trait DeviceDriver {
    const NAME: &'static str;

    fn new(engine: &Arc<Engine>, id: u64) -> Result<Self>
    where Self: Sized;

    fn open_device(
        &mut self,
        device: &Device<GlobalContext>,
    ) -> Result<DeviceHandle<GlobalContext>> {
        let mut handle = device.open().context("failed to open device")?;

        match handle.set_auto_detach_kernel_driver(true) {
            Ok(()) => {}
            Err(rusb::Error::NotSupported) => {}
            Err(err) => {
                Err(err).context("failed to auto-detach kernel drivers")?;
            }
        }

        let iface = device.find_interface(|_| true)?;

        handle
            .claim_interface(iface)
            .context("failed to claim interface")?;

        Ok(handle)
    }

    fn initialize(&mut self, _handle: &mut DeviceHandle<GlobalContext>) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, handle: &mut DeviceHandle<GlobalContext>) -> Result<ControlFlow<()>>;

    fn uninitialize(&mut self, _handle: &mut DeviceHandle<GlobalContext>) -> Result<()> {
        Ok(())
    }
}

pub struct DeviceThread<D: DeviceDriver> {
    device: Device<GlobalContext>,
    driver: D,
    stop: Arc<AtomicBool>,
}

impl<D> DeviceThread<D>
where
    D: DeviceDriver,
{
    pub(super) fn new(data: ThreadData) -> Box<dyn FnOnce() + Send> {
        Box::new(move || {
            let ThreadData {
                device_id,
                device,
                stop,
                engine,
            } = data;

            log::info!(
                target: T,
                "device for '{}' found, id: {}",
                D::NAME,
                device_id
            );

            let driver = match D::new(&engine, device_id) {
                Ok(driver) => driver,
                Err(err) => {
                    log::warn!(target: T, "failed to create device driver for '{}' (id {}): {:?}", D::NAME, device_id, err);
                    return;
                }
            };
            let thread = Self {
                device,
                driver,
                stop,
            };

            match thread.run() {
                Ok(()) => {
                    log::info!(
                        target: T,
                        "device thread for '{}' (id {}) closed",
                        D::NAME,
                        device_id
                    );
                }
                Err(err) => {
                    log::warn!(
                        target: T,
                        "device thread for '{}' (id {}) crashed: {:#}",
                        D::NAME,
                        device_id,
                        err
                    );
                }
            }
        })
    }

    fn run(mut self) -> Result<()> {
        let mut handle = self.driver.open_device(&self.device)?;

        self.driver.initialize(&mut handle)?;

        while !self.stop.load(Ordering::Acquire) {
            match self.driver.update(&mut handle)? {
                ControlFlow::Break(()) => break,
                ControlFlow::Continue(()) => {}
            }
        }

        self.driver.uninitialize(&mut handle)?;

        Ok(())
    }
}
