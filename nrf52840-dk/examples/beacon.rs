#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_dk_bsp::hal::{gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;
use ieee802154::mac;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

pub fn build_beacon_request(sequence: u8, mut data: &mut [u8]) -> usize {
    let frame = mac::Frame {
        header: mac::Header {
            seq: sequence,
            frame_type: mac::FrameType::MacCommand,
            security: mac::Security::None,
            frame_pending: false,
            ack_request: false,
            pan_id_compress: false,
            version: mac::FrameVersion::Ieee802154_2003,
            destination: mac::Address::broadcast(&mac::AddressMode::Short),
            source: mac::Address::None,
        },
        content: mac::FrameContent::Command(mac::command::Command::BeaconRequest),
        payload: &[0u8; 0],
        footer: [0u8; 2],
    };
    frame.encode(&mut data, mac::WriteFooter::No)
}

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut BEACON_TIMER: pac::TIMER1 = ();
    static mut LED_1: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_2: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut SEQUENCE: u8 = 0u8;
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();

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

        // Configure timer1 to generate a interrupt every second
        let timer1 = device.TIMER1;
        timer1.mode.write(|w| w.mode().timer());
        timer1.bitmode.write(|w| w.bitmode()._32bit());
        timer1.prescaler.write(|w| unsafe { w.prescaler().bits(4) });
        timer1.cc[0].write(|w| unsafe { w.bits(30000000) });
        timer1.shorts.write(|w| w.compare0_stop().enabled());
        timer1.intenset.write(|w| w.compare0().set_bit());
        timer1.tasks_start.write(|w| w.tasks_start().set_bit());

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

        BEACON_TIMER = timer1;
        LED_1 = pins
            .p0_13
            .degrade()
            .into_push_pull_output(gpio::Level::High);
        LED_2 = pins.p0_14.degrade().into_push_pull_output(gpio::Level::Low);
        RADIO = radio;
        UARTE = uarte0;
    }

    #[interrupt(resources = [BEACON_TIMER, RADIO, SEQUENCE, LED_1],)]
    fn TIMER1() {
        let timer = resources.BEACON_TIMER;
        // Clear event and restart
        timer.events_compare[0].write(|w| w.events_compare().clear_bit());
        timer.tasks_clear.write(|w| w.tasks_clear().set_bit());
        timer.tasks_start.write(|w| w.tasks_start().set_bit());
        (*resources.LED_1).set_low();
        let mut radio = resources.RADIO;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let size = build_beacon_request(*resources.SEQUENCE, &mut packet);
        let used = radio.queue_transmission(&mut packet[..size]);
        if used != size {
            hprintln!("Failed to send beacon").unwrap();
        }
        *resources.SEQUENCE = resources.SEQUENCE.wrapping_add(1);
    }

    #[interrupt(resources = [RADIO, UARTE, LED_1, LED_2],)]
    fn RADIO() {
        let uarte = resources.UARTE;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let mut radio = resources.RADIO;
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
    }
};
