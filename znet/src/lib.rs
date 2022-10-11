mod receiver;
pub use receiver::Receiver;

mod sender;
pub use sender::Sender;
use zinput_device::components;

#[repr(C, align(8))]
#[derive(Copy, Clone)]
struct PacketHeader {
    devices: u8,
}

impl PacketHeader {
    fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as _, core::mem::size_of::<Self>()) }
    }
}

macro_rules! device_header {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste::paste! {
            #[repr(C, align(8))]
            #[derive(Copy, Clone)]
            struct DeviceHeader {
                name: [u8; 16],
                
                $([< $cname s >]: u8,)*
            }

            impl DeviceHeader {
                fn data_len(&self) -> usize {
                    let mut len = 0;

                    $(len += self.[< $cname s >] as usize * core::mem::size_of::<$ctype>();)*

                    len
                }
            }
        }
    };
}

components!(data device_header);

impl DeviceHeader {
    fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as _, core::mem::size_of::<Self>()) }
    }
}