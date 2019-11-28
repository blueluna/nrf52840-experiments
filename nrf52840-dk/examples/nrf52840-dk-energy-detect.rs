#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprintln, peripheral::ITM};

use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;
use nrf52840_pac::radio::state::STATER;

use esercom;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        radio: Radio,
        itm: ITM,
        uart: uarte::Uarte<pac::UARTE0>,
        #[init(11)]
        channel: u8,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let pins = cx.device.P0.split();
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

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.start_energy_detect(65536);

        init::LateResources {
            radio,
            itm: cx.core.ITM,
            uart: uarte0,
        }
    }

    #[task(binds = RADIO, resources = [channel, radio, uart, itm],)]
    fn radio(cx: radio::Context) {
        let uarte = cx.resources.uart;
        let radio = cx.resources.radio;
        let itm_port = &mut cx.resources.itm.stim[0];
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
                    iprintln!(itm_port, "Failed to encode packet");
                }
            }
            let channel = cx.resources.channel.wrapping_add(1);
            let channel = if channel > 26 { 11 } else { channel };
            radio.set_channel(channel);
            *cx.resources.channel = channel;
            radio.start_energy_detect(65536);
        } else {
            match radio.state() {
                STATER::DISABLED => {
                    iprintln!(itm_port, "DISABLED");
                }
                STATER::RXRU => {
                    iprintln!(itm_port, "RXRU");
                }
                STATER::RXIDLE => {
                    iprintln!(itm_port, "RX IDLE");
                }
                STATER::RX => {
                    iprintln!(itm_port, "RX");
                }
                STATER::RXDISABLE => {
                    iprintln!(itm_port, "RX DISABLE");
                }
                STATER::TXRU => {
                    iprintln!(itm_port, "TXRU");
                }
                STATER::TXIDLE => {
                    iprintln!(itm_port, "TX IDLE");
                }
                STATER::TX => {
                    iprintln!(itm_port, "TX");
                }
                STATER::TXDISABLE => {
                    iprintln!(itm_port, "TX DISABLE");
                }
                _ => {}
            }
        }
    }
};
