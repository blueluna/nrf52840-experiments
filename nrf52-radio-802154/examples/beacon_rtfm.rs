#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_dk_bsp::hal::{gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;
use ieee802154;
use nrf52_radio_802154::{Radio, MAX_PACKET_LENGHT};

pub fn build_beacon_request(sequence: u8, mut data: &mut [u8]) -> usize {
    let mut payload = [0u8; 1];
    let command = ieee802154::mac_command::Command::BeaconRequest;
    let size = command.encode(&mut payload);
    let frame = ieee802154::mac::Frame {
        header: ieee802154::mac::Header {
            seq: sequence,
            frame_type: ieee802154::mac::FrameType::MacCommand,
            security: ieee802154::mac::Security::None,
            frame_pending: false,
            ack_request: false,
            pan_id_compress: false,
            version: ieee802154::mac::FrameVersion::Ieee802154_2003,
            destination: ieee802154::mac::Address::broadcast(&ieee802154::mac::AddressMode::Short),
            source: ieee802154::mac::Address::None,
        },
        payload: &payload[..size],
        footer: [0u8; 2],
    };
    frame.encode(&mut data, ieee802154::mac::WriteFooter::No)
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

    #[idle]
    fn idle() -> ! {
        loop {}
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
                (*resources.LED_2).set_low();
            }
        } else if radio.is_disabled_event() {
            hprintln!("Disabled").unwrap();
            radio.clear_disabled();
            radio.receive_prepare();
        } else if radio.is_ccabusy_event() {
            hprintln!("CCA busy").unwrap();
            radio.clear_ccabusy();
            radio.receive_prepare();
        }
    }
};
