#![no_main]
#![no_std]

use rtic::app;

use nrf52840_dk as _;

#[app(device = nrf52840_pac, peripherals = true)]
mod app {
    use bbqueue::{self, BBBuffer};

    use nrf52840_hal::{clocks, gpio, uarte};

    use nrf52840_pac as pac;

    use psila_nrf52::radio::{Radio, MAX_PACKET_LENGHT};

    // Use a packet buffer that can hold 16 packages
    const PACKET_BUFFER_SIZE: usize = 2048;

    static PKT_BUFFER: BBBuffer<PACKET_BUFFER_SIZE> = BBBuffer::new();

    #[local]
    struct LocalResources {
        radio: Radio,
        uart: uarte::Uarte<pac::UARTE0>,
        rx_producer: bbqueue::Producer<'static, PACKET_BUFFER_SIZE>,
        rx_consumer: bbqueue::Consumer<'static, PACKET_BUFFER_SIZE>,
    }

    #[shared]
    struct SharedResources {}

    #[init]
    fn init(cx: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
        let port0 = gpio::p0::Parts::new(cx.device.P0);
        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let uarte0 = uarte::Uarte::new(
            cx.device.UARTE0,
            uarte::Pins {
                txd: port0
                    .p0_06
                    .into_push_pull_output(gpio::Level::High)
                    .degrade(),
                rxd: port0.p0_08.into_floating_input().degrade(),
                cts: Some(port0.p0_07.into_floating_input().degrade()),
                rts: Some(
                    port0
                        .p0_05
                        .into_push_pull_output(gpio::Level::High)
                        .degrade(),
                ),
            },
            uarte::Parity::EXCLUDED,
            uarte::Baudrate::BAUD115200,
        );

        let (q_producer, q_consumer) = PKT_BUFFER.try_split().unwrap();

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        (
            SharedResources {},
            LocalResources {
                radio,
                uart: uarte0,
                rx_producer: q_producer,
                rx_consumer: q_consumer,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = RADIO, local = [radio, rx_producer])]
    fn radio(cx: radio::Context) {
        let radio = cx.local.radio;
        let queue = cx.local.rx_producer;

        match queue.grant_exact(MAX_PACKET_LENGHT) {
            Ok(mut grant) => {
                if grant.buf().len() < MAX_PACKET_LENGHT {
                    grant.commit(0);
                } else {
                    if let Ok(packet_len) = radio.receive_slice(grant.buf()) {
                        grant.commit(packet_len);
                    } else {
                        grant.commit(0);
                    }
                }
            }
            Err(_) => {
                // Drop package
                let mut buffer = [0u8; MAX_PACKET_LENGHT];
                let _ = radio.receive(&mut buffer);
            }
        }
    }

    #[idle(local = [rx_consumer, uart])]
    fn idle(cx: idle::Context) -> ! {
        let mut host_packet = [0u8; MAX_PACKET_LENGHT * 2];
        let queue = cx.local.rx_consumer;
        let uarte = cx.local.uart;

        defmt::info!("~ listening ~");

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
                        defmt::info!("Failed to encode packet");
                    }
                }
                grant.release(packet_length);
            }
        }
    }
}
