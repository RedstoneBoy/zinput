use bindlang::{to_struct, ty::{ToType, Type, IntWidth}, to_bitfield};

struct ButtonType;
impl ToType for ButtonType {
    fn to_type() -> Type {
        to_bitfield! {
            name = ControllerButtons;
            size = IntWidth::W64;
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

    match res {
        Ok(module) => println!("{}", module.display(&source)),
        Err(err) => println!("{}", err),
    }
}
