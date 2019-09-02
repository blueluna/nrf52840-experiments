#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;

use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};
use nrf52_radio_802154::timer::Timer;

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut LED_1: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_2: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut TIMER: pac::TIMER1 = ();

    #[init]
    fn init() {
        let p0 = device.P0.split();
        // Configure to use external clocks, and start them
        let _clocks = device
            .CLOCK
            .constrain()
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let uarte0 = device.UARTE0.constrain(
            uarte::Pins {
                txd: p0.p0_20.into_push_pull_output(gpio::Level::High).degrade(),
                rxd: p0.p0_19.into_floating_input().degrade(),
                cts: None,
                rts: None,
            },
            uarte::Parity::EXCLUDED,
            uarte::Baudrate::BAUD115200,
        );

        let mut timer1 = device.TIMER1;
        timer1.init();
        timer1.fire_at(1, 10_000_000);

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(11);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        LED_1 = p0.p0_24.degrade().into_push_pull_output(gpio::Level::Low);
        LED_2 = p0.p0_23.degrade().into_push_pull_output(gpio::Level::High);
        RADIO = radio;
        UARTE = uarte0;
        TIMER = timer1;
    }

    #[interrupt(resources = [LED_1, LED_2, RADIO, TIMER, UARTE],)]
    fn RADIO() {
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let mut radio = resources.RADIO;
        let mut timer = resources.TIMER;
        let mut uarte = resources.UARTE;

        (*resources.LED_1).set_high();
        (*resources.LED_2).set_high();

        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            match esercom::com_encode(
                esercom::MessageType::RadioReceive,
                &packet[1..packet_len],
                &mut host_packet,
            ) {
                Ok(written) => {
                    uarte.write(&host_packet[..written]).unwrap();
                }
                Err(_) => {
                    hprintln!("Failed to encode packet").unwrap();
                }
            }
            (*resources.LED_2).set_low();
        }
        let state = radio.state();
        let events = radio.events();
        packet[0] = state.bits();
        packet[1] = events as u8;
        packet[2] = (events >> 8) as u8;
        packet[3] = (events >> 16) as u8;
        packet[4] = (events >> 24) as u8;
        match esercom::com_encode(
            esercom::MessageType::RadioState,
            &packet[..5],
            &mut host_packet,
        ) {
            Ok(written) => {
                uarte.write(&host_packet[..written]).unwrap();
            }
            Err(_) => {
                hprintln!("Failed to encode radio state packet").unwrap();
            }
        }
        timer.fire_at(1, 10_000_000);
    }

    #[interrupt(resources = [RADIO, TIMER, UARTE],)]
    fn TIMER1() {
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let mut radio = resources.RADIO;
        let mut timer = resources.TIMER;
        let mut uarte = resources.UARTE;

        timer.ack_compare_event(1);

        let state = radio.state();
        let events = radio.events();
        packet[0] = state.bits();
        packet[1] = events as u8;
        packet[2] = (events >> 8) as u8;
        packet[3] = (events >> 16) as u8;
        packet[4] = (events >> 24) as u8;
        match esercom::com_encode(
            esercom::MessageType::RadioState,
            &packet[..5],
            &mut host_packet,
        ) {
            Ok(written) => {
                uarte.write(&host_packet[..written]).unwrap();
            }
            Err(_) => {
                hprintln!("Failed to encode radio state packet").unwrap();
            }
        }

        timer.fire_at(1, 10_000_000);
    }
};
