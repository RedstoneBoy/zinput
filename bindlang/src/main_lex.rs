#![feature(maybe_uninit_uninit_array)]

use std::{mem::MaybeUninit, time::{Duration, Instant}};

use bindlang::{to_struct, ty::{ToType, Type}, to_bitfield, util::Width};

struct ButtonType;
impl ToType for ButtonType {
    fn to_type() -> Type {
        to_bitfield! {
            name = ControllerButtons;
            size = Width::W64;
            a = 0;
            b = 1;
            x = 2;
            y = 3;
            up = 4;
            down = 5;
            left = 6;
            right = 7;
            start = 8;
            select = 9;
            l1 = 10;
            r1 = 11;
            l2 = 12;
            r2 = 13;
            l3 = 14;
            r3 = 15;
            l4 = 16;
            r4 = 17;
            lstick = 18;
            rstick = 19;
            home = 20;
            capture = 21;
        }
    }
}

#[repr(C)]
#[derive(Default, Debug)]
struct Device {
    buttons: u64,
    lx: f32,
    ly: f32,
    rx: f32,
    ry: f32,
    l: u8,
    r: u8,
    pitch: f64,
    roll: f64,
    yaw: f64,
}

fn main() {
    let source = std::fs::read_to_string("example.bind").unwrap();
    let res = bindlang::compile(&source, to_struct!(
        name = Device;
        0: buttons: ButtonType;
        8: lx: f32;
        12: ly: f32;
        16: lx: f32;
        20: ry: f32;
        24: l: u8;
        25: r: u8;
        32: pitch: f64;
        40: roll: f64;
        48: yaw: f64;
    ));

    let module = match res {
        Ok(ir) => ir,
        Err(err) => {
            println!("{}", err);
            return;
        }
    };

    let mut out = Device::default();
    let mut input1 = Device::default();

    let mut vm = bindlang::backend_vm::Vm::new();

    let mut times = MaybeUninit::<Duration>::uninit_array::<60>();

    for i in 0..60 {
        let start = Instant::now();
        vm.run(&module, 0, &mut out as *mut _ as _, &[&mut input1 as *mut _ as _]);
        let end = Instant::now();
        times[i].write(end - start);
    }

    let times = unsafe { times.map(|t| t.assume_init()) };

    let mut avg = 0.0;
    for time in times {
        avg += time.as_secs_f64();
    }
    avg = avg / 60.0;

    println!("{avg}s\n{}ms\n{}micros", avg * 10.0f64.powi(3), avg * 10.0f64.powi(6));
}
