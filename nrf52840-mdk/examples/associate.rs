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
    network_layer::NetworkState,
    radio::{Radio, MAX_PACKET_LENGHT},
    NetworkLayer,
    timer::Timer,
};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut TIMER: pac::TIMER1 = ();
    static mut LED_RED: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_BLUE: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut NETWORK: NetworkLayer = ();

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

        // Configure timer1 to generate a interrupt every second
        let mut timer1 = device.TIMER1;
        timer1.init();
        timer1.fire_at(1, 30000000);

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

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        TIMER = timer1;
        LED_RED = p0.p0_23.degrade().into_push_pull_output(gpio::Level::High);
        LED_BLUE = p0.p0_24.degrade().into_push_pull_output(gpio::Level::Low);
        RADIO = radio;
        UARTE = uarte0;
        NETWORK = NetworkLayer::new(extended_address);
    }

    #[interrupt(resources = [LED_BLUE, NETWORK, RADIO, TIMER],)]
    fn TIMER1() {
        let mut timer = resources.TIMER;
        let mut network = resources.NETWORK;
        let mut radio = resources.RADIO;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];

        (*resources.LED_BLUE).set_low();

        match network.state() {
            NetworkState::Orphan => {
                hprintln!("Search").unwrap();
            }
            NetworkState::Join => {
                hprintln!("Join network").unwrap();
            }
            NetworkState::QueryStatus => {
                hprintln!("Query status").unwrap();
            }
            NetworkState::Associated => {
                hprintln!("Associated").unwrap();
            }
        }

        let (size, fire_at) = network.build_packet(&mut packet);
        if size > 0 {
            let _used = radio.queue_transmission(&mut packet[..size]);
        }
        if fire_at > 0 {
            timer.fire_at(1, fire_at);
        }
    }

    #[interrupt(resources = [LED_BLUE, LED_RED, NETWORK, RADIO, TIMER, UARTE],)]
    fn RADIO() {
        let mut timer = resources.TIMER;
        let uarte = resources.UARTE;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut host_packet = [0u8; (MAX_PACKET_LENGHT as usize) * 2];
        let mut radio = resources.RADIO;
        let mut network = resources.NETWORK;
        
        (*resources.LED_BLUE).set_high();
        (*resources.LED_RED).set_high();
        
        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            let fire_at = network.radio_receive(&packet[1..(packet_len - 1)]);
            if fire_at > 0 {
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
            (*resources.LED_RED).set_low();
        }
    }
};
