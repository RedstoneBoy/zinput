use std::{
    io::Write,
    net::{ToSocketAddrs, UdpSocket},
};

use zinput_device::{components, Device};

use crate::{DeviceHeader, PacketHeader};

pub struct Sender {
    pub socket: UdpSocket,
}

impl Sender {
    pub fn new(socket: UdpSocket) -> Self {
        Sender { socket }
    }

    fn serialize<'a, 'b, 'c, 'd>(
        &'a self,
        devices: &'b [Device],
        names: &'c [[u8; 16]],
        buffer: &'d mut [u8],
    ) -> std::io::Result<&'d [u8]> {
        if names.len() != devices.len() {
            return Ok(&[]);
        }

        let mut cursor = std::io::Cursor::new(&mut *buffer);

        cursor.write(
            PacketHeader {
                devices: devices.len() as _,
            }
            .as_bytes(),
        )?;

        for (device, name) in devices.iter().zip(names.iter()) {
            cursor.write(
                DeviceHeader {
                    name: *name,
                    analogs: device.analogs.len() as _,
                    buttons: device.buttons.len() as _,
                    controllers: device.controllers.len() as _,
                    motions: device.motions.len() as _,
                    mouses: device.mouses.len() as _,
                    touch_pads: device.touch_pads.len() as _,
                }
                .as_bytes(),
            )?;
        }

        for device in devices {
            macro_rules! write_device {
                ($($cname:ident : $ctype:ty ),* $(,)?) => {
                    paste::paste! {
                        $(
                            cursor.write(unsafe {
                                core::slice::from_raw_parts(
                                    device.[< $cname s >].as_slice() as *const _ as _,
                                    device.[< $cname s >].len() * core::mem::size_of::<$ctype>(),
                                )
                            })?;
                        )*
                    }
                };
            }

            components!(data write_device);
        }

        let pos = cursor.position() as usize;

        Ok(&buffer[..pos])
    }

    pub fn send(
        &self,
        devices: &[Device],
        names: &[[u8; 16]],
        buffer: &mut [u8],
    ) -> std::io::Result<usize> {
        let buffer = self.serialize(devices, names, buffer)?;

        self.socket.send(buffer)
    }

    pub fn send_to<A>(
        &self,
        devices: &[Device],
        names: &[[u8; 16]],
        buffer: &mut [u8],
        addr: A,
    ) -> std::io::Result<usize>
    where
        A: ToSocketAddrs,
    {
        let buffer = self.serialize(devices, names, buffer)?;

        self.socket.send_to(buffer, addr)
    }
}
