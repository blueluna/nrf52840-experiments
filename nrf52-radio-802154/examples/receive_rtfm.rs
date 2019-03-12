#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_dk_bsp::hal::{gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();

    #[init]
    fn init() {
        let pins = device.P0.split();
        // Configure low frequency clock source
        device
            .CLOCK
            .lfclksrc
            .write(|w| w.src().xtal().external().disabled().bypass().disabled());
        // Start high frequency clock
        device.CLOCK.events_hfclkstarted.reset();
        device
            .CLOCK
            .tasks_hfclkstart
            .write(|w| w.tasks_hfclkstart().set_bit());
        while device
            .CLOCK
            .events_hfclkstarted
            .read()
            .events_hfclkstarted()
            .bit_is_clear()
        {}
        // Start low frequency clock
        device.CLOCK.events_lfclkstarted.reset();
        device
            .CLOCK
            .tasks_lfclkstart
            .write(|w| w.tasks_lfclkstart().set_bit());
        while device
            .CLOCK
            .events_lfclkstarted
            .read()
            .events_lfclkstarted()
            .bit_is_clear()
        {}

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
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        RADIO = radio;
        UARTE = uarte0;
    }

    #[idle]
    fn idle() -> ! {
        loop {}
    }

    #[interrupt(resources = [RADIO, UARTE],)]
    fn RADIO() {
        let uarte = resources.UARTE;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let radio = resources.RADIO;
        if radio.is_phyend_event() {
            let packet_len = radio.receive(&mut packet);
            radio.receive_prepare();
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
            }
        } else if radio.is_disabled_event() {
            radio.clear_disabled();
            radio.receive_prepare();
        }
    }
};
