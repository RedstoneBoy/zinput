use std::net::{UdpSocket, ToSocketAddrs};

use crate::Packet;

pub struct Sender {
    pub socket: UdpSocket,
}

impl Sender {
    pub fn new(socket: UdpSocket) -> Self {
        Sender { socket }
    }

    pub fn send(&self, packet: &Packet) -> std::io::Result<usize>
    {
        self.socket.send(packet.as_bytes())
    }

    pub fn send_to<A>(&self, packet: &Packet, addr: A) -> std::io::Result<usize>
    where
        A: ToSocketAddrs,
    {
        self.socket.send_to(packet.as_bytes(), addr)
    }
}