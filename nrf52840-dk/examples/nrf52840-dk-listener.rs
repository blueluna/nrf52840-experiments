#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprintln, peripheral::ITM};

use rtfm::app;

use bbqueue::{self, BBBuffer, ConstBBBuffer};

use nrf52840_hal::{clocks, gpio, uarte};

use nrf52840_pac as pac;

use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

// Use a packet buffer that can hold 16 packages
pub(crate) use bbqueue::consts::U2048 as PacketBufferSize;

static PKT_BUFFER: BBBuffer<PacketBufferSize> = BBBuffer(ConstBBBuffer::new());

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        radio: Radio,
        itm: ITM,
        uart: uarte::Uarte<pac::UARTE0>,
        rx_producer: bbqueue::Producer<'static, PacketBufferSize>,
        rx_consumer: bbqueue::Consumer<'static, PacketBufferSize>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let port0 = gpio::p0::Parts::new(cx.device.P0);
        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
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

        init::LateResources {
            radio,
            itm: cx.core.ITM,
            uart: uarte0,
            rx_producer: q_producer,
            rx_consumer: q_consumer,
        }
    }

    #[task(binds = RADIO, resources = [radio, rx_producer],)]
    fn radio(cx: radio::Context) {
        let radio = cx.resources.radio;
        let queue = cx.resources.rx_producer;

        match queue.grant_exact(MAX_PACKET_LENGHT) {
            Ok(mut grant) => {
                if grant.buf().len() < MAX_PACKET_LENGHT {
                    grant.commit(0);
                } else {
                    let packet_len = radio.receive_slice(grant.buf());
                    grant.commit(packet_len);
                }
            }
            Err(_) => {
                // Drop package
                let mut buffer = [0u8; MAX_PACKET_LENGHT];
                radio.receive(&mut buffer);
            }
        }
    }

    #[idle(resources = [rx_consumer, uart, itm])]
    fn idle(cx: idle::Context) -> ! {
        let mut host_packet = [0u8; MAX_PACKET_LENGHT * 2];
        let queue = cx.resources.rx_consumer;
        let uarte = cx.resources.uart;
        let itm_port = &mut cx.resources.itm.stim[0];

        iprintln!(itm_port, "~ listening ~");

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
                        iprintln!(itm_port, "Failed to encode packet");
                    }
                }
                grant.release(packet_length);
            }
        }
    }
};
