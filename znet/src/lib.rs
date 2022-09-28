#![feature(generic_const_exprs)]
#![allow(incomplete_features)]
#![cfg_attr(not(any(feature = "sender", feature = "receiver")), no_std)]

//! The znet protocol is a simple UDP-based protocol designed to send controller data over a LAN connection. 
//! 
//! An `input sender` must send a packet to the `input receiver` every time it needs to send a controller update.
//! The packet must contain the state of every controller the sender wants the receiver to know about.
//! 
//! Each sender connected to a receiver should have a unique name.
//! 
//! The maximum number of devices in a packet is recommended to be 4.
//! 
//! Changing the number of devices between packets is akin to plugging / unplugging controllers.
//! 
//! When disconnected, a client should send some packets with zero devices to indicate that is has disconnected.
//! 
//! If a sender waits too long to send a packet, the receiver will assume that the sender (and all of its controllers) has disconnected.
//! Receivers are recommended to have a timeout of 10 seconds.
//! 
//! Receivers must be able to receive input from multiple clients.
//! The devices that the receiver actually uses must be decided by itself.

use zinput_device::component::{controller::Controller, motion::Motion};

#[repr(C)]
#[derive(Clone, Default)]
pub struct Packet {
    pub name: [u8; 16],
    pub num_devices: u8,
    pub devices: [Device; 4],
}

impl Packet {
    pub fn as_bytes(&self) -> &[u8] {
        let len = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as _, len) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as _, len) }
    }
}

#[repr(C)]
#[derive(Clone, Default)]
pub struct Device {
    pub controller: Controller,
    pub motion: Motion,
}

#[cfg(feature = "sender")]
mod sender;
#[cfg(feature = "sender")]
pub use sender::Sender;

#[cfg(feature = "receiver")]
mod receiver;
#[cfg(feature = "receiver")]
pub use receiver::Receiver;