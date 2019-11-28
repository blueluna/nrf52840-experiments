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

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        radio: Radio,
        uart: uarte::Uarte<pac::UARTE0>,
        rx_producer: bbqueue::Producer,
        rx_consumer: bbqueue::Consumer,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let p0 = cx.device.P0.split();
        // Configure to use external clocks, and start them
        let _clocks = cx
            .device
            .CLOCK
            .constrain()
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let uarte0 = cx.device.UARTE0.constrain(
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

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        hprintln!("Initialise late resources").unwrap();

        init::LateResources {
            radio,
            uart: uarte0,
            rx_producer: q_producer,
            rx_consumer: q_consumer,
        }
    }

    #[task(binds = RADIO, resources = [radio, rx_producer])]
    fn radio(cx: radio::Context) {
        let radio = cx.resources.radio;
        let queue = cx.resources.rx_producer;

        match queue.grant(MAX_PACKET_LENGHT) {
            Ok(mut grant) => {
                let packet_len = radio.receive_slice(grant.buf());
                if packet_len > 0 {
                    queue.commit(packet_len, grant);
                } else {
                    queue.commit(0, grant);
                }
            }
            Err(_) => {
                // Drop package
                let mut buffer = [0u8; MAX_PACKET_LENGHT];
                radio.receive(&mut buffer);
                hprintln!("Failed to queue packet").unwrap();
            }
        }
    }

    #[idle(resources = [rx_consumer, uart])]
    fn idle(cx: idle::Context) -> ! {
        let mut host_packet = [0u8; MAX_PACKET_LENGHT * 2];
        let queue = cx.resources.rx_consumer;
        let uart = cx.resources.uart;

        loop {
            if let Ok(grant) = queue.read() {
                let packet_length = grant[0] as usize;
                match esercom::com_encode(
                    esercom::MessageType::RadioReceive,
                    &grant[1..packet_length],
                    &mut host_packet,
                ) {
                    Ok(written) => {
                        uart.write(&host_packet[..written]).unwrap();
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
