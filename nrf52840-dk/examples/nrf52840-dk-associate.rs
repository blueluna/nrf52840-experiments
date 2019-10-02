#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprintln, peripheral::ITM};

use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use bbqueue::{self, bbq, BBQueue};

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
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut SERVICE: Service = ();
    static mut ITM: ITM = ();
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

        // MAC (EUI-48) address to EUI-64
        // Add FF FE in the middle
        //
        //    01 23 45 67 89 AB
        //  /  /  /       \  \  \
        // 01 23 45 FF FE 67 89 AB
        let devaddr_lo = device.FICR.deviceaddr[0].read().bits();
        let devaddr_hi = device.FICR.deviceaddr[1].read().bits() as u16;
        let extended_address = u64::from(devaddr_hi) << 48
            | u64::from(devaddr_lo & 0xff00_0000) << 40
            | u64::from(devaddr_lo & 0x00ff_ffff)
            | 0x0000_00ff_fe00_0000u64;
        let extended_address = ExtendedAddress(extended_address);

        let mut timer1 = device.TIMER1;
        timer1.init();
        timer1.fire_at(1, 30_000_000);

        let uarte0 = device.UARTE0.constrain(
            uarte::Pins {
                txd: p0.p0_06.into_push_pull_output(gpio::Level::High).degrade(),
                rxd: p0.p0_08.into_floating_input().degrade(),
                cts: Some(p0.p0_07.into_floating_input().degrade()),
                rts: Some(p0.p0_05.into_push_pull_output(gpio::Level::High).degrade()),
            },
            uarte::Parity::EXCLUDED,
            uarte::Baudrate::BAUD115200,
        );

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(11);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        let bb_queue = bbq![MAX_PACKET_LENGHT * 32].unwrap();
        let (rx_producer, rx_consumer) = bb_queue.split();

        TIMER = timer1;
        RADIO = radio;
        UARTE = uarte0;
        SERVICE = Service::new(extended_address);
        ITM = core.ITM;
        RX_PRODUCER = rx_producer;
        RX_CONSUMER = rx_consumer;
    }

    #[interrupt(resources = [SERVICE, RADIO, TIMER, ITM],)]
    fn TIMER1() {
        let mut timer = resources.TIMER;
        let mut service = resources.SERVICE;
        let mut radio = resources.RADIO;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let itm_port = &mut resources.ITM.stim[0];

        iprintln!(itm_port, "TIMER1");

        timer.ack_compare_event(1);

        match service.state() {
            MacState::Orphan => {
                iprintln!(itm_port, "Orphan, beacon query");
            }
            MacState::ActiveScan => {
                iprintln!(itm_port, "Orphan, no PAN found");
            }
            MacState::Join => {
                iprintln!(itm_port, "Associate with PAN");
            }
            MacState::QueryStatus => {
                iprintln!(itm_port, "Query association status");
            }
            MacState::Associated => {
                iprintln!(itm_port, "Associated");
            }
        }

        let (size, fire_at) = service.build_packet(&mut packet);
        if size > 0 {
            iprintln!(itm_port, "SEND");
            let _used = radio.queue_transmission(&packet[..size]);
        }
        if fire_at > 0 {
            timer.fire_at(1, fire_at);
        }
    }

    #[interrupt(resources = [SERVICE, RADIO, TIMER, RX_PRODUCER],)]
    fn RADIO() {
        let mut timer = resources.TIMER;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut radio = resources.RADIO;
        let mut service = resources.SERVICE;
        let queue = resources.RX_PRODUCER;

        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            let fire_at = service.radio_receive(&packet[1..(packet_len - 1)]);
            if fire_at > 0 {
                timer.fire_at(1, fire_at);
            }
            if let Ok(mut grant) = queue.grant(packet_len) {
                grant.copy_from_slice(&packet[..packet_len]);
                queue.commit(packet_len, grant);
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
                if let Ok(written) = esercom::com_encode(esercom::MessageType::RadioReceive, &grant[1..packet_length], &mut host_packet) {
                    uarte.write(&host_packet[..written]).unwrap();
                }
                queue.release(packet_length, grant);
            }
        }
    }
};
