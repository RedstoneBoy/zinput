use bindlang::backend_cranelift::Program;
use paste::paste;
use zinput_engine::{
    device::{components, Device, DeviceInfo, DeviceMutFfi},
    DeviceHandle, DeviceView,
};

struct Input {
    view: DeviceView,
    device: Device,
    ffi: DeviceMutFfi,
}

impl Input {
    fn new(view: DeviceView) -> Self {
        let mut device = view.info().create_device();

        Input {
            view,
            ffi: device.as_mut().to_ffi(),
            device,
        }
    }
}

pub struct VDevice {
    name: String,

    inputs: Vec<Input>,
    inputs_ffi: (*mut *mut DeviceMutFfi, usize, usize),
    output_handle: DeviceHandle,

    program: Option<Program<DeviceMutFfi>>,
}

unsafe impl Send for VDevice {}

impl VDevice {
    pub fn new(name: String, inputs: Vec<DeviceView>, output: DeviceHandle) -> Self {
        let inputs = inputs.into_iter().map(Input::new).collect();

        VDevice {
            name,

            inputs,
            inputs_ffi: Vec::new().into_raw_parts(),
            output_handle: output,

            program: None,
        }
    }

    pub fn set_program(&mut self, program: Option<Program<DeviceMutFfi>>) {
        self.program = program;
    }

    pub fn update(&mut self, input_index: usize) {
        let Some(program) = &mut self.program
        else { return; };

        let mut inputs_ffi = unsafe {
            Vec::from_raw_parts(
                self.inputs_ffi.0 as *mut &mut DeviceMutFfi,
                self.inputs_ffi.1,
                self.inputs_ffi.2,
            )
        };

        for input in &mut self.inputs {
            input.ffi = input.device.as_mut().to_ffi();
            inputs_ffi.push(&mut input.ffi);
        }

        self.output_handle.update(|mut output| {
            let mut output = output.to_ffi();

            program.call(&mut output, &mut inputs_ffi, input_index);
        });

        let parts = inputs_ffi.into_raw_parts();
        self.inputs_ffi = (parts.0 as _, parts.1, parts.2);
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
