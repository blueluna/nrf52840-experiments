#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use bbqueue::{self, bbq, BBQueue};

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;

use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut LED_1: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_2: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut RX_PRODUCER: bbqueue::Producer = ();
    static mut RX_CONSUMER: bbqueue::Consumer = ();

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

        let bb_queue = bbq![MAX_PACKET_LENGHT * 16].unwrap();
        let (q_producer, q_consumer) = bb_queue.split();

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(11);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        LED_1 = p0.p0_24.degrade().into_push_pull_output(gpio::Level::Low);
        LED_2 = p0.p0_23.degrade().into_push_pull_output(gpio::Level::High);
        RADIO = radio;
        UARTE = uarte0;
        RX_PRODUCER = q_producer;
        RX_CONSUMER = q_consumer;
    }

    #[interrupt(resources = [LED_1, LED_2, RADIO, RX_PRODUCER],)]
    fn RADIO() {
        let radio = resources.RADIO;
        let queue = resources.RX_PRODUCER;

        (*resources.LED_1).set_high();
        (*resources.LED_2).set_high();

        match queue.grant(MAX_PACKET_LENGHT) {
            Ok(mut grant) => {
                let packet_len = radio.receive_slice(grant.buf());
                if packet_len > 0 {
                    queue.commit(packet_len, grant);
                    (*resources.LED_2).set_low();
                } else {
                    queue.commit(0, grant);
                }
            }
            Err(_) => {
                hprintln!("Failed to queue packet").unwrap();
            }
        }
    }

    #[idle(resources = [RX_CONSUMER, UARTE])]
    fn idle() -> ! {
        let mut host_packet = [0u8; MAX_PACKET_LENGHT * 2];
        let queue = resources.RX_CONSUMER;
        let uarte = resources.UARTE;

        loop {
            if let Ok(grant) = queue.read() {
                let packet_length = grant[0] as usize;
                match esercom::com_encode(
                    esercom::MessageType::RadioReceive,
                    &grant[1..packet_length],
                    &mut host_packet,
                ) {
                    Ok(written) => {
                        uarte.write(&host_packet[..written]).unwrap();
                    }
                    Err(_) => {
                        hprintln!("Failed to encode packet").unwrap();
                    }
                }
                queue.release(packet_length, grant);
            }
        }
    }
};
