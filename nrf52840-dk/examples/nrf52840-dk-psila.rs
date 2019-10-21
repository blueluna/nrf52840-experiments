#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprint, iprintln, peripheral::ITM};

use rtfm::app;

use nrf52840_hal::{clocks, gpio, prelude::*, uarte};

use nrf52840_pac as pac;

use bbqueue::{self, bbq, BBQueue};

use esercom;
use nrf52_cryptocell::CryptoCellBackend;
use nrf52_radio_802154::{
    radio::{Radio, MAX_PACKET_LENGHT},
    timer::Timer,
};
use psila_data::ExtendedAddress;
use psila_service::PsilaService;

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut TIMER: pac::TIMER1 = ();
    static mut RADIO: Radio = ();
    static mut UARTE: uarte::Uarte<pac::UARTE0> = ();
    static mut SERVICE: PsilaService<CryptoCellBackend> = ();
    static mut ITM: ITM = ();
    static mut RX_PRODUCER: bbqueue::Producer = ();
    static mut RX_CONSUMER: bbqueue::Consumer = ();
    static mut TX_CONSUMER: bbqueue::Consumer = ();

    #[init]
    fn init() {
        let itm_port = &mut core.ITM.stim[0];
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
        let extended_address = ExtendedAddress::new(extended_address);

        iprintln!(itm_port, "Address {:016x}", u64::from(extended_address));

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

        let rx_queue = bbq![MAX_PACKET_LENGHT * 32].unwrap();
        let (rx_producer, rx_consumer) = rx_queue.split();

        let tx_queue = bbq![MAX_PACKET_LENGHT * 8].unwrap();
        let (tx_producer, tx_consumer) = tx_queue.split();

        let cryptocell = CryptoCellBackend::new(device.CRYPTOCELL);

        TIMER = timer1;
        RADIO = radio;
        UARTE = uarte0;
        SERVICE = PsilaService::new(cryptocell, tx_producer, extended_address);
        ITM = core.ITM;
        RX_PRODUCER = rx_producer;
        RX_CONSUMER = rx_consumer;
        TX_CONSUMER = tx_consumer;
    }

    #[interrupt(resources = [SERVICE, RADIO, TIMER, ITM], spawn = [radio_tx])]
    fn TIMER1() {
        let mut timer = resources.TIMER;
        let mut service = resources.SERVICE;
        let itm_port = &mut resources.ITM.stim[0];

        iprintln!(itm_port, "TIMER1");

        timer.ack_compare_event(1);

        let fire_at = match service.timeout() {
            Ok(time) => time,
            Err(_) => {
                iprintln!(itm_port, "service timeout failed");
                0
            }
        };
        if fire_at > 0 {
            timer.fire_at(1, fire_at);
        }
        let _ = spawn.radio_tx();
    }

    #[interrupt(resources = [SERVICE, RADIO, TIMER, RX_PRODUCER, ITM], spawn = [radio_rx, radio_tx])]
    fn RADIO() {
        let mut timer = resources.TIMER;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut radio = resources.RADIO;
        let mut service = resources.SERVICE;
        let queue = resources.RX_PRODUCER;
        let itm_port = &mut resources.ITM.stim[0];

        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            let fire_at = match service.receive(&packet[1..packet_len]) {
                Ok(fire_at) => fire_at,
                Err(e) => {
                    iprintln!(itm_port, "service receive failed, {:?}", e);
                    0
                }
            };
            if fire_at > 0 {
                timer.fire_at(1, fire_at);
            }
            if let Ok(mut grant) = queue.grant(packet_len) {
                grant.copy_from_slice(&packet[..packet_len]);
                queue.commit(packet_len, grant);
                let _ = spawn.radio_rx();
            }
            let _ = spawn.radio_tx();
        }
    }

    #[task(resources = [RX_CONSUMER, UARTE])]
    fn radio_rx() {
        let mut host_packet = [0u8; MAX_PACKET_LENGHT * 2];
        let queue = resources.RX_CONSUMER;
        let uarte = resources.UARTE;

        if let Ok(grant) = queue.read() {
            let packet_length = grant[0] as usize;
            if let Ok(written) = esercom::com_encode(
                esercom::MessageType::RadioReceive,
                &grant[1..packet_length],
                &mut host_packet,
            ) {
                uarte.write(&host_packet[..written]).unwrap();
            }
            queue.release(packet_length, grant);
        }
    }

    #[task(resources = [RADIO, TX_CONSUMER, ITM])]
    fn radio_tx() {
        let queue = resources.TX_CONSUMER;
        let mut radio = resources.RADIO;
        let itm_port = &mut resources.ITM.stim[0];

        if let Ok(grant) = queue.read() {
            let packet_length = grant[0] as usize;
            iprintln!(itm_port, "TX Pop {} octets", packet_length);
            for b in grant[1..=packet_length].iter() {
                iprint!(itm_port, "{:02x} ", b);
            }
            iprintln!(itm_port);
            let _ = radio.queue_transmission(&grant[1..=packet_length]);
            queue.release(packet_length + 1, grant);
        }
    }

    extern "C" {
        fn UARTE1();
    }
};
