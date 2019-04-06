#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use esercom;
use ieee802154::mac::ExtendedAddress;
use nrf52_radio_802154::{
    mac::service::State as MacState,
    radio::{Radio, MAX_PACKET_LENGHT},
    timer::Timer,
    Service,
};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut TIMER: pac::TIMER1 = ();
    static mut LED_1: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_2: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut SERVICE: Service = ();

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

        // MAC (EUI-48) address to EUI-64
        // Add FF FE in the middle
        //
        //    01 23 45 67 89 AB
        //  /  /  /       \  \  \
        // 01 23 45 FF FE 67 89 AB
        let devaddr_lo = device.FICR.deviceaddr[0].read().bits();
        let devaddr_hi = device.FICR.deviceaddr[1].read().bits() as u16;
        let extended_address = (devaddr_hi as u64) << 48
            | ((devaddr_lo & 0xff000000) as u64) << 40
            | ((devaddr_lo & 0x00ffffff) as u64)
            | 0x000000fffe000000u64;
        let extended_address = ExtendedAddress(extended_address);

        let mut timer1 = device.TIMER1;
        timer1.init();
        timer1.fire_at(1, 30000000);

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

        TIMER = timer1;
        LED_1 = pins
            .p0_13
            .degrade()
            .into_push_pull_output(gpio::Level::High);
        LED_2 = pins.p0_14.degrade().into_push_pull_output(gpio::Level::Low);
        RADIO = radio;
        UARTE = uarte0;
        SERVICE = Service::new(extended_address);
    }

    #[interrupt(resources = [LED_1, SERVICE, RADIO, TIMER],)]
    fn TIMER1() {
        let mut timer = resources.TIMER;
        let mut service = resources.SERVICE;
        let mut radio = resources.RADIO;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];

        hprintln!("time").unwrap();

        (*resources.LED_1).set_low();

        timer.ack_compare_event(1);

        match service.state() {
            MacState::Orphan => {
                hprintln!("Orphan, beacon query").unwrap();
            }
            MacState::ActiveScan => {
                hprintln!("Orphan, no PAN found").unwrap();
            }
            MacState::Join => {
                hprintln!("Associate with PAN").unwrap();
            }
            MacState::QueryStatus => {
                hprintln!("Query association status").unwrap();
            }
            MacState::Associated => {
                hprintln!("Associated").unwrap();
            }
        }

        let (size, fire_at) = service.build_packet(&mut packet);
        if size > 0 {
            hprintln!("send").unwrap();
            let _used = radio.queue_transmission(&mut packet[..size]);
        }
        if fire_at > 0 {
            timer.fire_at(1, fire_at);
        }
    }

    #[interrupt(resources = [LED_1, LED_2, SERVICE, RADIO, TIMER, UARTE],)]
    fn RADIO() {
        let mut timer = resources.TIMER;
        let uarte = resources.UARTE;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let mut radio = resources.RADIO;
        let mut service = resources.SERVICE;

        (*resources.LED_1).set_high();
        (*resources.LED_2).set_high();

        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            let fire_at = service.radio_receive(&packet[1..(packet_len - 1)]);
            if fire_at > 0 {
                hprintln!("fire").unwrap();
                timer.fire_at(1, fire_at);
            }
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
