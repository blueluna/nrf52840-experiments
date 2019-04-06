//! 802.15.4 Radio using the nRF52840 radio module

#![no_std]

pub mod radio;
pub mod timer;
pub mod mac;

pub use mac::service::Service;
