use std::net::UdpSocket;

use rustc_hash::FxHashMap;

use crate::Packet;

pub struct Receiver {
    pub socket: UdpSocket,
    info: FxHashMap<[u8; 16], SenderInfo>,
}

impl Receiver {
    pub fn new(socket: UdpSocket) -> Self {
        Receiver {
            socket,
            info: FxHashMap::default(),
        }
    }

    pub fn recv(&mut self) -> std::io::Result<Option<Packet>> {
        let mut packet = [0u8; std::mem::size_of::<Packet>()];

        let (len, _) = self.socket.recv_from(&mut packet)?;

        if len != packet.len() {
            return Ok(None);
        }

        let packet: Packet = unsafe { std::mem::transmute(packet) };

        match self.info.get_mut(&packet.name) {
            Some(info) => {
                info.num_devices = packet.num_devices;
            }
            None => {
                self.info.insert(packet.name, SenderInfo { num_devices: packet.num_devices });
            }
        }

        Ok(Some(packet))
    }

    pub fn info(&self) -> &FxHashMap<[u8; 16], SenderInfo> {
        &self.info
    }
}

pub struct SenderInfo {
    pub num_devices: u8,
}