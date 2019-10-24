#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprintln, peripheral::ITM};

use rtfm::app;

use nrf52840_hal::{clocks, prelude::*};

use nrf52840_pac as pac;

use bbqueue::{self, bbq, BBQueue};

use log;

use nrf52_utils::logger;
use nrf52_cryptocell::CryptoCellBackend;
use nrf52_radio_802154::{
    radio::{Radio, MAX_PACKET_LENGHT},
    timer::Timer,
};
use psila_data::{ExtendedAddress, Key, security::DEFAULT_LINK_KEY};
use psila_service::{self, PsilaService};

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut TIMER: pac::TIMER1 = ();
    static mut RADIO: Radio = ();
    static mut SERVICE: PsilaService<CryptoCellBackend> = ();
    static mut ITM: ITM = ();
    static mut RX_PRODUCER: bbqueue::Producer = ();
    static mut RX_CONSUMER: bbqueue::Consumer = ();
    static mut TX_CONSUMER: bbqueue::Consumer = ();
    static mut LOG_CONSUMER: bbqueue::Consumer = ();

    #[init]
    fn init() {
        let itm_port = &mut core.ITM.stim[0];
        let log_consumer = logger::init();

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

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(11);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        let rx_queue = bbq![MAX_PACKET_LENGHT * 32].unwrap();
        let (rx_producer, rx_consumer) = rx_queue.split();

        let tx_queue = bbq![MAX_PACKET_LENGHT * 8].unwrap();
        let (tx_producer, tx_consumer) = tx_queue.split();

        let cryptocell = CryptoCellBackend::new(device.CRYPTOCELL);
        let default_link_key = Key::from(DEFAULT_LINK_KEY);

        TIMER = timer1;
        RADIO = radio;
        SERVICE = PsilaService::new(
            cryptocell,
            tx_producer,
            extended_address,
            default_link_key,
            );
        ITM = core.ITM;
        RX_PRODUCER = rx_producer;
        RX_CONSUMER = rx_consumer;
        TX_CONSUMER = tx_consumer;
        LOG_CONSUMER = log_consumer;
    }

    #[interrupt(resources = [SERVICE, TIMER], spawn = [radio_tx])]
    fn TIMER1() {
        let mut timer = resources.TIMER;
        let mut service = resources.SERVICE;

        log::info!("TIMER");

        timer.ack_compare_event(1);

        let fire_at = match service.timeout() {
            Ok(time) => time,
            Err(_) => {
                log::warn!("service timeout failed");
                0
            }
        };
        if fire_at > 0 {
            timer.fire_at(1, fire_at);
        }
        let _ = spawn.radio_tx();
    }

    #[interrupt(resources = [RADIO, SERVICE, RX_PRODUCER], spawn = [radio_tx])]
    fn RADIO() {
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut radio = resources.RADIO;
        let mut service = resources.SERVICE;
        let queue = resources.RX_PRODUCER;

        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            match service.handle_acknowledge(&packet[1..packet_len-1]) {
                Ok(to_me) => {
                    if to_me {
                        if let Ok(mut grant) = queue.grant(packet_len) {
                            grant.copy_from_slice(&packet[..packet_len]);
                            queue.commit(packet_len, grant);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("service handle acknowledge failed, {:?}", e);
                }
            }
            let _ = spawn.radio_tx();
        }
    }

    #[task(priority=1, resources = [RX_CONSUMER, SERVICE, TIMER])]
    fn radio_rx() {
        let queue = resources.RX_CONSUMER;
        let mut service = resources.SERVICE;
        let mut timer = resources.TIMER;

        if let Ok(grant) = queue.read() {
            let packet_length = grant[0] as usize;
            let fire_at = match service.receive(&grant[1..packet_length - 1]) {
                Ok(fire_at) => fire_at,
                Err(e) => {
                    log::warn!("service receive failed, {:?}", e);
                    0
                }
            };
            if fire_at > 0 {
                timer.fire_at(1, fire_at);
            }
            queue.release(packet_length, grant);
        }
    }

    #[task(resources = [RADIO, TX_CONSUMER], spawn = [radio_rx])]
    fn radio_tx() {
        let queue = resources.TX_CONSUMER;
        let mut radio = resources.RADIO;

        if let Ok(grant) = queue.read() {
            let packet_length = grant[0] as usize;
            log::info!("Send {} octets", packet_length);
            let _ = radio.queue_transmission(&grant[1..=packet_length]);
            queue.release(packet_length + 1, grant);
        }
        let _ = spawn.radio_rx();
    }

    #[idle(resources = [LOG_CONSUMER, ITM])]
    fn idle() -> ! {
        let itm_port = &mut resources.ITM.stim[0];
        loop {
            while let Ok(grant) = resources.LOG_CONSUMER.read() {
                for chunk in grant.buf().chunks(256) {
                    cortex_m::itm::write_all(itm_port, chunk);
                }
                resources.LOG_CONSUMER.release(grant.buf().len(), grant);
            }
        }
    }

    extern "C" {
        fn PDM();
        fn QDEC();
    }
};
