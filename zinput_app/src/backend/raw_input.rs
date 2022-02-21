use std::{
    collections::HashMap,
    ffi::{c_void, OsStr},
    mem::{self, MaybeUninit},
    os::windows::prelude::OsStrExt,
    ptr::{self, null_mut},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use anyhow::Result;
use parking_lot::Mutex;
use winapi::{
    shared::{hidpi, hidusage, minwindef, windef},
    um::{libloaderapi::GetModuleHandleW, winuser},
};
use zinput_device::component::{
    analogs::{Analogs, AnalogsInfo},
    buttons::{Buttons, ButtonsInfo},
};
use zinput_engine::{
    plugin::{Plugin, PluginKind, PluginStatus},
    Engine,
};

const T: &'static str = "backend:raw_input";

pub struct RawInput {
    state: Mutex<Option<Inner>>,
}

impl RawInput {
    pub fn new() -> Self {
        RawInput {
            state: Mutex::new(None),
        }
    }
}

impl Plugin for RawInput {
    fn init(&self, zinput_api: Arc<Engine>) {
        *self.state.lock() = Some(Inner::new(zinput_api));
    }

    fn stop(&self) {
        *self.state.lock() = None;
    }

    fn status(&self) -> PluginStatus {
        match &*self.state.lock() {
            Some(inner) => inner.status(),
            None => PluginStatus::Stopped,
        }
    }

    fn name(&self) -> &str {
        "raw_input"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Backend
    }
}

struct Inner {
    hwnd_opt: Arc<Mutex<Option<Hwnd>>>,
    status: Arc<Mutex<PluginStatus>>,

    handle: Option<JoinHandle<()>>,
}

impl Inner {
    fn new(api: Arc<Engine>) -> Self {
        let hwnd_opt = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(PluginStatus::Running));

        let handle = Some(std::thread::spawn(new_raw_input_thread(Thread {
            api: api,
            hwnd_opt: hwnd_opt.clone(),
            status: status.clone(),
        })));

        Inner {
            hwnd_opt,
            status,

            handle,
        }
    }

    fn status(&self) -> PluginStatus {
        self.status.lock().clone()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // wait until the raw input thread either creates a window or fails to create one
        while self.hwnd_opt.lock().is_none()
            && matches!(&*self.status.lock(), &PluginStatus::Running)
        {
            std::thread::sleep(Duration::from_secs(1));
        }

        // if the raw input thread created a window
        if let Some(hwnd) = &*self.hwnd_opt.lock() {
            unsafe {
                winuser::PostMessageW(hwnd.0, winuser::WM_DESTROY, 0, 0);
            }
        }

        if let Some(handle) = mem::replace(&mut self.handle, None) {
            match handle.join() {
                Ok(()) => {}
                Err(_) => {
                    log::error!(target: T, "error joining input thread!");
                }
            }
        }
    }
}

struct Hwnd(*mut windef::HWND__);
unsafe impl Send for Hwnd {}

struct Thread {
    api: Arc<Engine>,
    hwnd_opt: Arc<Mutex<Option<Hwnd>>>,
    status: Arc<Mutex<PluginStatus>>,
}

fn new_raw_input_thread(thread: Thread) -> impl FnOnce() {
    move || {
        let status = thread.status.clone();

        match raw_input_thread(thread) {
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

struct WindowClass(minwindef::ATOM);

impl Drop for WindowClass {
    fn drop(&mut self) {
        unsafe {
            let hinst = GetModuleHandleW(ptr::null_mut());
            winuser::UnregisterClassW(&self.0, hinst);
        }
    }
}

fn raw_input_thread(thread: Thread) -> Result<()> {
    let Thread { api, hwnd_opt, .. } = thread;

    let class_name = "ZInput RawInput Backend".os_str();
    let window_title = "zinput raw_input".os_str();

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
        let window_class = WindowClass(winuser::RegisterClassW(&window_class));

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

        *hwnd_opt.lock() = Some(Hwnd(hwnd));

        {
            let state = Box::leak(Box::new(State::new(api)));

            register_raw_input(hwnd)?;
            update_device_list(state);

            winuser::SetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA, state as *mut _ as _);
        }

        log::info!(target: T, "driver initialized");

        let mut msg: winuser::MSG = mem::zeroed();
        while winuser::GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
            winuser::TranslateMessage(&msg);
            winuser::DispatchMessageW(&msg);
        }

        let state = winuser::GetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA) as *mut State;

        winuser::SetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA, 0);

        let state = Box::from_raw(state);
        drop(state);
        drop(window_class);
    }

    Ok(())
}

struct State {
    api: Arc<Engine>,
    joysticks: HashMap<usize, Joystick>,

    device_list: Vec<winuser::RAWINPUTDEVICELIST>,
}

impl State {
    fn new(api: Arc<Engine>) -> Self {
        State {
            api,
            joysticks: HashMap::new(),

            device_list: Vec::new(),
        }
    }
}

crate::device_bundle!(DeviceBundle(owned), analog: Analogs, button: Buttons,);

struct Joystick {
    button_caps: Vec<hidpi::HIDP_BUTTON_CAPS>,
    value_caps: Vec<hidpi::HIDP_VALUE_CAPS>,
    preparsed: Vec<u8>,
    buttons: Vec<hidusage::USAGE>,

    bundle: DeviceBundle<'static>,
}

fn register_raw_input(hwnd: *mut windef::HWND__) -> Result<()> {
    let device = winuser::RAWINPUTDEVICE {
        usUsagePage: 1,
        usUsage: 4,
        dwFlags: winuser::RIDEV_DEVNOTIFY | winuser::RIDEV_INPUTSINK,
        hwndTarget: hwnd as _,
    };

    if unsafe {
        winuser::RegisterRawInputDevices(
            &device,
            1,
            mem::size_of::<winuser::RAWINPUTDEVICE>() as u32,
        )
    } == 0
    {
        anyhow::bail!("failed to register raw input devices");
    }

    Ok(())
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
        winuser::WM_INPUT => {
            let mut dw_size = 0;

            winuser::GetRawInputData(
                lparam as usize as _,
                winuser::RID_INPUT,
                null_mut(),
                &mut dw_size,
                mem::size_of::<winuser::RAWINPUTHEADER>() as u32,
            );

            let mut raw_input: winuser::RAWINPUT = mem::zeroed();

            if winuser::GetRawInputData(
                lparam as usize as _,
                winuser::RID_INPUT,
                &mut raw_input as *mut _ as _,
                &mut dw_size,
                mem::size_of::<winuser::RAWINPUTHEADER>() as u32,
            ) != dw_size
            {
                log::error!(target: T, "incorrect size returned for raw input!");
            }

            if raw_input.header.dwType == winuser::RIM_TYPEHID {
                match update_device(
                    state,
                    raw_input.header.hDevice as usize,
                    &raw_input.data.hid(),
                ) {
                    Ok(()) => {}
                    Err(err) => {
                        log::warn!(target: T, "failed to update device: {}", err);
                    }
                }
            }

            0
        }
        winuser::WM_INPUT_DEVICE_CHANGE => {
            update_device_list(state);
            0
        }
        _ => winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn update_device_list(state: &mut State) {
    const DEVICE_LIST_SIZE: u32 = mem::size_of::<winuser::RAWINPUTDEVICELIST>() as u32;

    let State {
        api,
        device_list,

        joysticks,
    } = state;

    let mut num_devices = 0;
    if unsafe {
        winuser::GetRawInputDeviceList(ptr::null_mut(), &mut num_devices, DEVICE_LIST_SIZE)
    } == u32::MAX
    {
        log::warn!(target: T, "failed to get number of devices connected");
        return;
    }

    device_list.clear();
    device_list.reserve_exact(num_devices as usize);

    let list_result = unsafe {
        winuser::GetRawInputDeviceList(device_list.as_mut_ptr(), &mut num_devices, DEVICE_LIST_SIZE)
    };
    if list_result == u32::MAX {
        log::warn!(target: T, "failed to get device list");
        return;
    }

    unsafe {
        device_list.set_len(list_result as usize);
    }
    device_list.retain(|dev| dev.dwType == winuser::RIM_TYPEHID);

    let mut all_devices = Vec::new();

    for dev_node in &*device_list {
        let handle = dev_node.hDevice;

        all_devices.push(handle as usize);

        if joysticks.contains_key(&(handle as usize)) {
            continue;
        }

        match is_joystick(handle) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(err) => {
                log::warn!(target: T, "{}", err);
                continue;
            }
        }

        let joystick = match get_joystick_info(api.clone(), handle) {
            Ok(info) => info,
            Err(err) => {
                log::warn!(target: T, "failed to get joystick info: {}", err);
                continue;
            }
        };

        joysticks.insert(handle as usize, joystick);
    }

    joysticks.retain(|k, _| all_devices.contains(k));
}

fn is_joystick(handle: *mut c_void) -> Result<bool> {
    let mut dev_info_size = mem::size_of::<winuser::RID_DEVICE_INFO>();
    let mut dev_info: winuser::RID_DEVICE_INFO = unsafe { mem::zeroed() };

    let result = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            winuser::RIDI_DEVICEINFO,
            &mut dev_info as *mut _ as _,
            &mut dev_info_size as *mut _ as _,
        )
    };
    if result == u32::MAX {
        anyhow::bail!("failed to get raw input device info");
    }

    if dev_info.dwType != winuser::RIM_TYPEHID {
        return Ok(false);
    }

    unsafe {
        if dev_info.u.hid().usUsagePage != 0x01 || dev_info.u.hid().usUsage != 0x04 {
            return Ok(false);
        }
    }

    Ok(true)
}

fn get_joystick_info(api: Arc<Engine>, handle: *mut c_void) -> Result<Joystick> {
    let mut preparsed_len = 0;
    if unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            winuser::RIDI_PREPARSEDDATA,
            ptr::null_mut(),
            &mut preparsed_len,
        )
    } != 0
    {
        anyhow::bail!("failed to get preparsed data length");
    }
    let mut preparsed = Vec::<u8>::with_capacity(preparsed_len as usize);
    let preparsed_result = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            winuser::RIDI_PREPARSEDDATA,
            preparsed.as_mut_ptr() as _,
            &mut preparsed_len,
        )
    };
    if preparsed_result == 0 || preparsed_result == u32::MAX {
        anyhow::bail!("failed to get preparsed data");
    }
    unsafe {
        preparsed.set_len(preparsed_result as usize);
    }

    let mut caps = MaybeUninit::zeroed();
    if unsafe { hidpi::HidP_GetCaps(preparsed.as_mut_ptr() as _, caps.as_mut_ptr()) }
        != hidpi::HIDP_STATUS_SUCCESS
    {
        anyhow::bail!("failed to get device capabilities");
    }
    let caps = unsafe { caps.assume_init() };

    let mut button_caps: Vec<hidpi::HIDP_BUTTON_CAPS> =
        Vec::with_capacity(caps.NumberInputButtonCaps as usize);
    let mut button_caps_len = caps.NumberInputButtonCaps;
    if unsafe {
        hidpi::HidP_GetButtonCaps(
            hidpi::HidP_Input,
            button_caps.as_mut_ptr() as _,
            &mut button_caps_len,
            preparsed.as_mut_ptr() as _,
        )
    } != hidpi::HIDP_STATUS_SUCCESS
    {
        anyhow::bail!("failed to get device button capabilities");
    }
    unsafe {
        button_caps.set_len(button_caps_len as usize);
    }

    let mut total_buttons = 0;
    for button_cap in &button_caps {
        total_buttons +=
            unsafe { button_cap.u.Range().UsageMax - button_cap.u.Range().UsageMin + 1 };
    }
    if total_buttons > 64 {
        log::warn!(
            target: T,
            "joystick {} has more than 64 buttons",
            handle as usize
        );
    }

    let mut value_caps = Vec::with_capacity(caps.NumberInputValueCaps as usize);
    let mut value_caps_len = caps.NumberInputValueCaps;
    if unsafe {
        hidpi::HidP_GetValueCaps(
            hidpi::HidP_Input,
            value_caps.as_mut_ptr() as _,
            &mut value_caps_len,
            preparsed.as_mut_ptr() as _,
        )
    } != hidpi::HIDP_STATUS_SUCCESS
    {
        anyhow::bail!("failed to get device value capabilities");
    }
    unsafe {
        value_caps.set_len(value_caps_len as usize);
    }

    if value_caps_len > 8 {
        log::warn!(
            target: T,
            "joystick {} has more than 8 analogs",
            handle as usize
        );
    }

    // todo: meta info
    let bundle = DeviceBundle::new(
        api.clone(),
        format!("Raw Input Device {}", handle as u64),
        [AnalogsInfo::default()],
        [ButtonsInfo::default()],
    );

    Ok(Joystick {
        button_caps,
        value_caps,
        preparsed,
        buttons: Vec::new(),

        bundle,
    })
}

fn update_device(state: &mut State, device_id: usize, data: &winuser::RAWHID) -> Result<()> {
    let joystick = match state.joysticks.get_mut(&device_id) {
        Some(joystick) => joystick,
        None => anyhow::bail!("unknown joystick id {}", device_id),
    };

    joystick.bundle.button[0].buttons = 0;

    let mut bitset_offset = 0;

    for button_caps in &joystick.button_caps {
        let num_buttons =
            unsafe { button_caps.u.Range().UsageMax - button_caps.u.Range().UsageMin + 1 };
        let mut num_pressed = num_buttons as u32;
        joystick.buttons.clear();
        joystick.buttons.reserve_exact(num_buttons as usize);
        if unsafe {
            hidpi::HidP_GetUsages(
                hidpi::HidP_Input,
                button_caps.UsagePage,
                0,
                joystick.buttons.as_mut_ptr(),
                &mut num_pressed,
                joystick.preparsed.as_mut_ptr() as _,
                data.bRawData.as_ptr() as _,
                data.dwSizeHid,
            )
        } != hidpi::HIDP_STATUS_SUCCESS
        {
            anyhow::bail!("failed to get usages");
        }

        unsafe {
            joystick.buttons.set_len(num_pressed as usize);
        }

        for &usage in &joystick.buttons {
            let bit_index =
                unsafe { (usage - button_caps.u.Range().UsageMin) as usize + bitset_offset };
            if bit_index < 64 {
                joystick.bundle.button[0].buttons |= 1 << bit_index as u64;
            }
        }

        bitset_offset += num_buttons as usize;
    }

    let mut value_index = 0;
    for value_caps in &joystick.value_caps {
        let mut value = 0;
        let value_result = unsafe {
            hidpi::HidP_GetUsageValue(
                hidpi::HidP_Input,
                value_caps.UsagePage,
                0,
                value_caps.u.Range().UsageMin,
                &mut value,
                joystick.preparsed.as_mut_ptr() as _,
                data.bRawData.as_ptr() as _,
                data.dwSizeHid,
            )
        };

        if value_result == hidpi::HIDP_STATUS_INCOMPATIBLE_REPORT_ID {
            continue;
        }

        if value_result != hidpi::HIDP_STATUS_SUCCESS {
            anyhow::bail!("failed to get usage values");
        }

        if value_index >= joystick.bundle.analog[0].analogs.len() {
            break;
        }

        let value = if value as i32 >= value_caps.LogicalMax {
            1.0
        } else if value as i32 <= value_caps.LogicalMin {
            0.0
        } else {
            ((value as i32 as f32) - (value_caps.LogicalMin as f32))
                / (value_caps.LogicalMax as f32 - value_caps.LogicalMin as f32)
        };

        joystick.bundle.analog[0].analogs[value_index] = (value * 255.0) as u8;

        value_index += 1;
    }

    joystick.bundle.update()?;

    Ok(())
}

trait ToOsStr {
    fn os_str(&self) -> Vec<u16>;
}

impl ToOsStr for str {
    fn os_str(&self) -> Vec<u16> {
        OsStr::new(self).encode_wide().chain(Some(0)).collect()
    }
}
