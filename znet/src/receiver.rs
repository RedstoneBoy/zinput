use std::net::UdpSocket;

use rustc_hash::FxHashMap;
use zinput_device::components;

use crate::{PacketHeader, DeviceHeader};

// TODO: Deserialization is currently unsound as there are no overflow checks

pub struct Receiver<'a> {
    pub socket: UdpSocket,
    buffer: &'a mut [u8],

    data: FxHashMap<[u8; 16], NetDeviceInfo>,
}

impl<'a> Receiver<'a> {
    /// # Safety
    /// `buffer` must be aligned to 8 bytes
    pub unsafe fn new(socket: UdpSocket, buffer: &'a mut [u8]) -> Self {
        Receiver {
            socket,
            buffer,

            data: FxHashMap::default(),
        }
    }
    
    pub fn recv(&mut self) -> std::io::Result<()> {
        self.data.clear();

        let (buffer_len, _) = self.socket.recv_from(&mut self.buffer)?;

        let mut buffer = &self.buffer[..buffer_len];

        let packet_header_len = core::mem::size_of::<PacketHeader>();
        if buffer.len() < packet_header_len {
            return Ok(());
        }

        let header: &PacketHeader = unsafe { &*(buffer as *const _ as *const PacketHeader) };
        buffer = &buffer[packet_header_len..];
        
        let device_headers_len = header.devices as usize * core::mem::size_of::<DeviceHeader>();

        if buffer.len() < device_headers_len {
            return Ok(());
        }

        let device_headers: &[DeviceHeader] = unsafe { core::slice::from_raw_parts(buffer as *const _ as _, header.devices as usize) };

        let mut buffer_pos = packet_header_len + device_headers_len;

        for &header in device_headers {
            if buffer.len() + packet_header_len < buffer_pos + header.data_len() {
                break;
            }

            let ndev = NetDeviceInfo::new(header, buffer_pos);
            buffer_pos += header.data_len();

            self.data.insert(header.name, ndev);
        }

        Ok(())
    }

    pub fn device_names(&self) -> impl Iterator<Item = &[u8; 16]> {
        self.data.keys()
    }

    pub fn device(&self, name: &[u8; 16]) -> Option<NetDevice> {
        let info = self.data.get(name)?.clone();

        Some(NetDevice {
            buffer: &self.buffer,
            info,
        })
    }
}

macro_rules! net_device {
    ($($cname:ident : $ctype:ty),* $(,)?) => {
        paste::paste! {
            #[derive(Clone)]
            struct NetDeviceInfo {
                header: DeviceHeader,
                
                $([< $cname _offset >]: usize,)*
            }

            #[allow(unused_assignments)]
            impl NetDeviceInfo {
                fn new(header: DeviceHeader, device_offset: usize) -> Self {
                    let mut offset = device_offset;

                    $(
                        let [< $cname _offset >] = offset;
                        offset += header.[< $cname s >] as usize * core::mem::size_of::<$ctype>();
                    )*

                    NetDeviceInfo {
                        header,

                        $([< $cname _offset >],)*
                    }
                }
            }

            pub struct NetDevice<'a> {
                buffer: &'a [u8],
                info: NetDeviceInfo,
            }

            impl<'a> NetDevice<'a> {
                $(pub fn [< $cname s >](&self) -> &[$ctype] {
                    unsafe {
                        core::slice::from_raw_parts(&self.buffer[self.info.[< $cname _offset >] as usize..] as *const [u8] as _, self.info.header.[< $cname s >] as usize)
                    }
                })*
            }
        }
    };
}

components!(data net_device);