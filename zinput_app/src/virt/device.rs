use bindlang::backend_cranelift::Program;
use hidapi::DeviceInfo;
use paste::paste;
use zinput_engine::{DeviceView, DeviceHandle, device::{Device, components, DeviceMutFfi}};

fn info_to_device(info: &DeviceInfo) -> Device {
    macro_rules! info_to_device {
        ($($cname:ident : $ctype:ty),* $(,)?) => {
            paste! {
                Device {
                    $([< $cname s >]: vec![Default::default(); info.[< $cname s>].len()],)*
                }
            }
        }
    }

    components!(data info_to_device)
}

struct Input {
    view: DeviceView,
    device: Device,
}

impl Input {
    fn new(view: DeviceView) -> Self {
        let info = view.info();
        let device = info_to_device(info);

        Input { view, device }
    }
}

pub struct VDevice {
    name: String,
    
    inputs: Vec<Input>,
    inputs_ffi: Vec<DeviceMutFfi>,
    inputs_ffi_mut: Vec<&mut DeviceMutFfi>,
    // output: Device,
    output_handle: DeviceHandle,
    

    program: Program<DeviceMutFfi>,
}

impl VDevice {
    pub(super) fn new(name: String, inputs: Vec<DeviceView>, output: DeviceHandle, program: Program<DeviceMutFfi>) -> Self {
        let inputs = inputs.into_iter().map(Input::new).collect();

        VDevice {
            name,

            inputs,
            inputs_ffi: Vec::new(),
            inputs_ffi_mut: Vec::new(),
            // output: info_to_device(output.info()),
            output_handle: output,
            
            
            program,
        }
    }

    pub(super) fn update(&mut self, input_index: usize) {
        for input in &mut self.inputs {
            self.inputs_ffi.
        }

        let output = 
        self.program.call(output, inputs, input)
        let view = &self.inputs[view_index];
        self.updater.update(view, view_index, &self.out);

        self.inputs_ffi.clear();
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}