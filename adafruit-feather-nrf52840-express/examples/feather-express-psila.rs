#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_itm;

use cortex_m::peripheral::ITM;

use rtic::app;

use nrf52840_hal::clocks;

use nrf52840_pac as pac;

use bbqueue::{self, BBBuffer, ConstBBBuffer};

use nrf52_cryptocell::CryptoCellBackend;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};
use nrf52_utils::{logger, timer::Timer};
use psila_data::{security::DEFAULT_LINK_KEY, ExtendedAddress, Key};
use psila_service::{self, PsilaService};

use bbqueue::consts::U4096 as TxBufferSize;
use bbqueue::consts::U4096 as RxBufferSize;

static RX_BUFFER: BBBuffer<RxBufferSize> = BBBuffer(ConstBBBuffer::new());
static TX_BUFFER: BBBuffer<TxBufferSize> = BBBuffer(ConstBBBuffer::new());

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        timer: pac::TIMER1,
        radio: Radio,
        service: PsilaService<'static, TxBufferSize, CryptoCellBackend>,
        itm: ITM,
        rx_producer: bbqueue::Producer<'static, RxBufferSize>,
        rx_consumer: bbqueue::Consumer<'static, RxBufferSize>,
        tx_consumer: bbqueue::Consumer<'static, TxBufferSize>,
        log_consumer: bbqueue::Consumer<'static, logger::LogBufferSize>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let mut timer0 = cx.device.TIMER0;
        timer0.init();
        let log_consumer = logger::init(timer0);

        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        // MAC (EUI-48) address to EUI-64
        // Add FF FE in the middle
        //
        //    01 23 45 67 89 AB
        //  /  /  /       \  \  \
        // 01 23 45 FF FE 67 89 AB
        let devaddr_lo = cx.device.FICR.deviceaddr[0].read().bits();
        let devaddr_hi = cx.device.FICR.deviceaddr[1].read().bits() as u16;
        let extended_address = u64::from(devaddr_hi) << 48
            | u64::from(devaddr_lo & 0xff00_0000) << 40
            | u64::from(devaddr_lo & 0x00ff_ffff)
            | 0x0000_00ff_fe00_0000u64;
        let extended_address = ExtendedAddress::new(extended_address);

        let mut timer1 = cx.device.TIMER1;
        timer1.init();
        timer1.fire_at(1, 30_000_000);

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        let (rx_producer, rx_consumer) = RX_BUFFER.try_split().unwrap();
        let (tx_producer, tx_consumer) = TX_BUFFER.try_split().unwrap();

        let cryptocell = CryptoCellBackend::new(cx.device.CRYPTOCELL);
        let default_link_key = Key::from(DEFAULT_LINK_KEY);

        init::LateResources {
            timer: timer1,
            radio,
            service: PsilaService::new(cryptocell, tx_producer, extended_address, default_link_key),
            itm: cx.core.ITM,
            rx_producer,
            rx_consumer,
            tx_consumer,
            log_consumer,
        }
    }

    #[task(binds = TIMER1, resources = [service, timer], spawn = [radio_tx])]
    fn timer(cx: timer::Context) {
        let timer = cx.resources.timer;
        let service = cx.resources.service;

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
            log::info!("Fire at {}", fire_at);
            timer.fire_at(1, fire_at);
        }
        let _ = cx.spawn.radio_tx();
    }

    #[task(binds = RADIO, resources = [radio, service, rx_producer], spawn = [radio_tx])]
    fn radio(cx: radio::Context) {
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let radio = cx.resources.radio;
        let service = cx.resources.service;
        let queue = cx.resources.rx_producer;

        match radio.receive(&mut packet) {
            Ok(packet_len) => {
                if packet_len > 0 {
                    match service.handle_acknowledge(&packet[1..packet_len - 1]) {
                        Ok(to_me) => {
                            if to_me {
                                if let Ok(mut grant) = queue.grant_exact(packet_len) {
                                    grant.copy_from_slice(&packet[..packet_len]);
                                    grant.commit(packet_len);
                                }
                            }
                            let _ = cx.spawn.radio_tx();
                        }
                        Err(e) => {
                            log::warn!("service handle acknowledge failed, {:?}", e);
                        }
                    }
                }
            }
            Err(nrf52_radio_802154::radio::Error::CcaBusy) => {
                log::warn!("CCA Busy");
            }
        }
    }

    #[task(priority=1, resources = [rx_consumer, service, timer], spawn = [radio_tx])]
    fn radio_rx(cx: radio_rx::Context) {
        let queue = cx.resources.rx_consumer;
        let service = cx.resources.service;
        let timer = cx.resources.timer;

        if let Ok(grant) = queue.read() {
            let timestamp = timer.now();
            let packet_length = grant[0] as usize;
            let fire_at = match service.receive(&grant[1..packet_length - 1], timestamp) {
                Ok(fire_at) => fire_at,
                Err(e) => {
                    log::warn!("service receive failed, {:?}", e);
                    0
                }
            };
            if fire_at > 0 {
                timer.fire_at(1, fire_at);
            }
            grant.release(packet_length);
            let _ = cx.spawn.radio_tx();
        }
    }

    #[task(resources = [radio, tx_consumer], spawn = [radio_rx])]
    fn radio_tx(cx: radio_tx::Context) {
        let queue = cx.resources.tx_consumer;
        let radio = cx.resources.radio;

        if let Ok(grant) = queue.read() {
            let packet_length = grant[0] as usize;
            let _ = radio.queue_transmission(&grant[1..=packet_length]);
            grant.release(packet_length + 1);
            log::info!("Send {} bytes", packet_length);
        }
        let _ = cx.spawn.radio_rx();
    }

    #[idle(resources = [log_consumer, itm])]
    fn idle(cx: idle::Context) -> ! {
        let itm_port = &mut cx.resources.itm.stim[0];
        loop {
            while let Ok(grant) = cx.resources.log_consumer.read() {
                let length = grant.buf().len();
                for chunk in grant.buf().chunks(256) {
                    cortex_m::itm::write_all(itm_port, chunk);
                }
                grant.release(length);
            }
        }
    }

    extern "C" {
        fn PDM();
        fn QDEC();
    }
};
