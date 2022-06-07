use anyhow::{Context, Result};

#[cfg(target_os = "windows")]
mod ctx {
    use std::{ffi::OsStr, os::windows::prelude::OsStrExt, ptr, thread::JoinHandle};

    use anyhow::{Context, Result};
    use crossbeam_channel::Sender;
    use winapi::{
        shared::{minwindef, windef},
        um::{
            dbt::{DBT_DEVTYP_DEVICEINTERFACE, DEV_BROADCAST_DEVICEINTERFACE_W},
            libloaderapi::GetModuleHandleW,
            winuser::{self, DEVICE_NOTIFY_WINDOW_HANDLE},
        },
    };

    const T: &'static str = "usb_devices:hotplug";

    pub struct HotPlugInner {
        handle: Option<JoinHandle<()>>,
        hwnd: Hwnd,
    }

    impl HotPlugInner {
        pub fn register<F>(callback: F) -> Result<Self>
        where
            F: FnMut() + Send + 'static,
        {
            let callback: Box<dyn FnMut() + Send + 'static> = Box::new(callback);

            let (send, recv) = crossbeam_channel::unbounded();

            let handle = std::thread::spawn(hotplug_thread(callback, send));

            let hwnd = recv.recv().context("hotplug thread dropped sender")??;

            Ok(HotPlugInner {
                handle: Some(handle),
                hwnd,
            })
        }
    }

    impl Drop for HotPlugInner {
        fn drop(&mut self) {
            // safety: PostMessageW can be called from other threads
            unsafe {
                winuser::PostMessageW(self.hwnd.0, winuser::WM_DESTROY, 0, 0);
            }

            let handle = self
                .handle
                .take()
                .expect("hotplug thread handle disappeared!?");
            match handle.join() {
                Ok(()) => {}
                Err(_) => {
                    log::error!(target: T, "hotplug thread panicked");
                }
            }
        }
    }

    struct Hwnd(*mut windef::HWND__);
    // safety: on threads other than the window thread, hwnd must only be used with thread-safe functions
    unsafe impl Send for Hwnd {}

    struct WindowClass(minwindef::ATOM);

    impl Drop for WindowClass {
        fn drop(&mut self) {
            unsafe {
                let hinst = GetModuleHandleW(ptr::null_mut());
                winuser::UnregisterClassW(&self.0, hinst);
            }
        }
    }

    trait ToOsStr {
        fn os_str(&self) -> Vec<u16>;
    }

    impl ToOsStr for str {
        fn os_str(&self) -> Vec<u16> {
            OsStr::new(self).encode_wide().chain(Some(0)).collect()
        }
    }

    fn hotplug_thread(
        callback: Box<dyn FnMut() + Send + 'static>,
        send: Sender<Result<Hwnd>>,
    ) -> impl FnOnce() {
        move || {
            let send2 = send.clone();
            match run_hotplug_thread(callback, send) {
                Ok(()) => {}
                Err(e) => {
                    log::error!(target: T, "hotplug thread crashed: {}", e);
                    // ignore error if "main" thread already received a result
                    let _ = send2.send(Err(e));
                }
            }
        }
    }

    struct State {
        callback: Box<dyn FnMut() + Send + 'static>,
    }

    fn run_hotplug_thread(
        callback: Box<dyn FnMut() + Send + 'static>,
        send: Sender<Result<Hwnd>>,
    ) -> Result<()> {
        let class_name = "ZInput usb_devices Hotplug".os_str();
        let window_title = "zinput_usb_devices_hotplug".os_str();

        unsafe {
            let hinst = GetModuleHandleW(ptr::null_mut());
            let window_class = winuser::WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_proc),
                hInstance: hinst,
                lpszClassName: class_name.as_ptr(),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null_mut(),
            };
            let _window_class = WindowClass(winuser::RegisterClassW(&window_class));

            let hwnd = winuser::CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                0,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                ptr::null_mut(),
                ptr::null_mut(),
                hinst,
                ptr::null_mut(),
            );

            if hwnd.is_null() {
                anyhow::bail!("failed to create window");
            }

            register_window_events(hwnd)?;

            send.send(Ok(Hwnd(hwnd)))
                .expect("usb hotplug hwnd receiver was dropped");

            {
                let state = Box::into_raw(Box::new(State { callback }));

                winuser::SetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA, state as _);
            }

            let mut msg: winuser::MSG = std::mem::zeroed();
            while winuser::GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
                winuser::TranslateMessage(&msg);
                winuser::DispatchMessageW(&msg);
            }

            let state = winuser::GetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA) as *mut State;
            winuser::SetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA, 0);
            let _ = Box::from_raw(state);
        }

        Ok(())
    }

    fn register_window_events(hwnd: *mut windef::HWND__) -> Result<()> {
        let guid_pnp = winapi::shared::guiddef::GUID {
            Data1: 0x25dbce51,
            Data2: 0x6c8f,
            Data3: 0x4a72,
            Data4: [0x8a, 0x6d, 0xb5, 0x4c, 0x2b, 0x4f, 0xc8, 0x35],
        };

        unsafe {
            let mut filter: DEV_BROADCAST_DEVICEINTERFACE_W = std::mem::zeroed();

            filter.dbcc_size = std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32;
            filter.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
            filter.dbcc_classguid = guid_pnp;

            let notification = winuser::RegisterDeviceNotificationA(
                hwnd as _,
                &mut filter as *mut _ as _,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );

            if notification.is_null() {
                anyhow::bail!("failed to register device notification");
            }

            Ok(())
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: windef::HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        let state = winuser::GetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA) as usize as *mut State;
        let state = &mut *state;

        match msg {
            winuser::WM_DESTROY => {
                winuser::PostQuitMessage(0);
                0
            }
            winuser::WM_DEVICECHANGE => {
                (state.callback)();
                0
            }
            _ => winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod ctx {
    use anyhow::{Context, Result};
    use rusb::{GlobalContext, Hotplug, HotplugBuilder, Registration};

    pub struct HotPlugInner {
        reg: Registration<GlobalContext>,
    }

    impl HotPlugInner {
        pub fn register<F>(callback: F) -> Result<Self>
        where
            F: FnMut() + Send + 'static,
        {
            let reg = HotplugBuilder::new()
                .register(GlobalContext {}, Box::new(HotPlugImpl { callback }))
                .context("failed to register rusb hotplug callback")?;

            Ok(HotPlugInner { reg })
        }
    }

    struct HotPlugImpl<F> {
        callback: F,
    }

    impl<F> Hotplug<GlobalContext> for HotPlugImpl<F>
    where
        F: FnMut() + Send,
    {
        fn device_arrived(&mut self, device: rusb::Device<GlobalContext>) {
            (self.callback)()
        }

        fn device_left(&mut self, device: rusb::Device<GlobalContext>) {}
    }
}

use ctx::HotPlugInner;

pub struct HotPlug {
    _inner: HotPlugInner,
}

impl HotPlug {
    pub fn register<F>(callback: F) -> Result<Self>
    where
        F: FnMut() + Send + 'static,
    {
        Ok(HotPlug {
            _inner: HotPlugInner::register(callback).context("failed to register hotplug event")?,
        })
    }
}
