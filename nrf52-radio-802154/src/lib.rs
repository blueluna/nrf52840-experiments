//! 802.15.4 Radio using the nRF52840 radio module

#![no_std]

pub mod mac;
pub mod radio;

pub use mac::service::Service;
