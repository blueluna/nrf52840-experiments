#![no_main]
#![no_std]

use nrf52840_dk as _;

use rtic::app;

use nrf52840_hal::{clocks, gpio, uarte};

use psila_nrf52::{
    pac::{self, radio::state::STATE_A},
    radio::{Radio, MAX_PACKET_LENGHT},
};

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        radio: Radio,
        uart: uarte::Uarte<pac::UARTE0>,
        #[init(11)]
        channel: u8,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
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

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.start_energy_detect(65536);

        init::LateResources {
            radio,
            uart: uarte0,
        }
    }

    #[task(binds = RADIO, resources = [channel, radio, uart],)]
    fn radio(cx: radio::Context) {
        let uarte = cx.resources.uart;
        let radio = cx.resources.radio;
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
                    defmt::info!("Failed to encode packet");
                }
            }
            let channel = cx.resources.channel.wrapping_add(1);
            let channel = if channel > 26 { 11 } else { channel };
            radio.set_channel(channel);
            *cx.resources.channel = channel;
            radio.start_energy_detect(65536);
        } else {
            match radio.state() {
                STATE_A::DISABLED => {
                    defmt::info!("DISABLED");
                }
                STATE_A::RXRU => {
                    defmt::info!("RXRU");
                }
                STATE_A::RXIDLE => {
                    defmt::info!("RX IDLE");
                }
                STATE_A::RX => {
                    defmt::info!("RX");
                }
                STATE_A::RXDISABLE => {
                    defmt::info!("RX DISABLE");
                }
                STATE_A::TXRU => {
                    defmt::info!("TXRU");
                }
                STATE_A::TXIDLE => {
                    defmt::info!("TX IDLE");
                }
                STATE_A::TX => {
                    defmt::info!("TX");
                }
                STATE_A::TXDISABLE => {
                    defmt::info!("TX DISABLE");
                }
            }
        }
    }
};
