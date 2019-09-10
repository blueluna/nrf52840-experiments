#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;
use nrf52840_pac::radio::state::STATER;

use esercom;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut CHANNEL: u8 = 11;

    #[init]
    fn init() {
        let pins = device.P0.split();
        // Configure to use external clocks, and start them
        let _clocks = device
            .CLOCK
            .constrain()
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let uarte0 = device.UARTE0.constrain(
            uarte::Pins {
                txd: pins
                    .p0_06
                    .into_push_pull_output(gpio::Level::High)
                    .degrade(),
                rxd: pins.p0_08.into_floating_input().degrade(),
                cts: Some(pins.p0_07.into_floating_input().degrade()),
                rts: Some(
                    pins.p0_05
                        .into_push_pull_output(gpio::Level::High)
                        .degrade(),
                ),
            },
            uarte::Parity::EXCLUDED,
            uarte::Baudrate::BAUD115200,
        );

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(11);
        radio.start_energy_detect(65536);

        RADIO = radio;
        UARTE = uarte0;
    }

    #[interrupt(resources = [CHANNEL, RADIO, UARTE],)]
    fn RADIO() {
        let uarte = resources.UARTE;
        let radio = resources.RADIO;
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];

        let energy_level = radio.report_energy_detect();
        if let Some(energy_level) = energy_level {
            let mut packet = [0u8; 2];
            packet[0] = radio.get_channel();
            packet[1] = energy_level;
            match esercom::com_encode(
                esercom::MessageType::EnergyDetect,
                &packet,
                &mut host_packet,
            ) {
                Ok(written) => {
                    uarte.write(&host_packet[..written]).unwrap();
                }
                Err(_) => {
                    hprintln!("Failed to encode packet").unwrap();
                }
            }
            let channel = resources.CHANNEL.wrapping_add(1);
            let channel = if channel > 26 { 11 } else { channel };
            radio.set_channel(channel);
            *resources.CHANNEL = channel;
            radio.start_energy_detect(65536);
        } else {
            match radio.state() {
                STATER::DISABLED => {
                    hprintln!("DISABLED").unwrap();
                }
                STATER::RXRU => {
                    hprintln!("RXRU").unwrap();
                }
                STATER::RXIDLE => {
                    hprintln!("RX IDLE").unwrap();
                }
                STATER::RX => {
                    hprintln!("RX").unwrap();
                }
                STATER::RXDISABLE => {
                    hprintln!("RX DISABLE").unwrap();
                }
                STATER::TXRU => {
                    hprintln!("TXRU").unwrap();
                }
                STATER::TXIDLE => {
                    hprintln!("TX IDLE").unwrap();
                }
                STATER::TX => {
                    hprintln!("TX").unwrap();
                }
                STATER::TXDISABLE => {
                    hprintln!("TX DISABLE").unwrap();
                }
                _ => {}
            }
        }
    }
};
