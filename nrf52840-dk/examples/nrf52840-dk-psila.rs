#![no_main]
#![no_std]

use nrf52840_dk as _;

use rtic::app;

use nrf52840_hal::{clocks, gpio};

use nrf52840_pac as pac;

use bbqueue::{self, BBBuffer, ConstBBBuffer};

use embedded_hal::digital::v2::OutputPin;

use nrf52_cryptocell::CryptoCellBackend;
use psila_data::{
    cluster_library::{AttributeDataType, ClusterLibraryStatus},
    device_profile::SimpleDescriptor,
    security::DEFAULT_LINK_KEY,
    ExtendedAddress, Key,
};
use psila_nrf52::{
    radio::{Radio, MAX_PACKET_LENGHT},
    timer::Timer,
};
use psila_service::{self, ClusterLibraryHandler, PsilaService};

use bbqueue::consts::U1024 as TxBufferSize;
use bbqueue::consts::U1024 as RxBufferSize;

static RX_BUFFER: BBBuffer<RxBufferSize> = BBBuffer(ConstBBBuffer::new());
static TX_BUFFER: BBBuffer<TxBufferSize> = BBBuffer(ConstBBBuffer::new());

const TIMER_SECOND: u32 = 1_000_000;

const MANUFACTURER_NAME: &'static str = "ERIK of Sweden";
const MODEL_IDENTIFIER: &'static str = "Lampan";

pub struct ClusterHandler {
    on_off: bool,
    led: gpio::Pin<gpio::Output<gpio::PushPull>>,
}

impl ClusterHandler {
    pub fn new(mut led: gpio::Pin<gpio::Output<gpio::PushPull>>) -> Self {
        let _ = led.set_high();
        Self { on_off: false, led }
    }

    pub fn set_on_off(&mut self, enable: bool) {
        self.on_off = enable;
        if self.on_off {
            let _ = self.led.set_low();
        } else {
            let _ = self.led.set_high();
        }
    }
}

impl ClusterLibraryHandler for ClusterHandler {
    fn active_endpoints(&self) -> &[u8] {
        &[0x01]
    }
    fn get_simple_desciptor(&self, endpoint: u8) -> Option<SimpleDescriptor> {
        match endpoint {
            0x01 => Some(SimpleDescriptor::new(
                0x01,
                0x0104,
                0x0100,
                0,
                &[0x0000, 0x0006],
                &[],
            )),
            _ => None,
        }
    }
    fn read_attribute(
        &self,
        profile: u16,
        cluster: u16,
        _endpoint: u8,
        attribute: u16,
        value: &mut [u8],
    ) -> Result<(AttributeDataType, usize), ClusterLibraryStatus> {
        defmt::info!(
            "Read attribute: {=u16} {=u16} {=u16}",
            profile,
            cluster,
            attribute
        );
        match (profile, cluster, attribute) {
            (0x0104, 0x0000, 0x0000) => {
                value[0] = 0x02;
                Ok((AttributeDataType::Unsigned8, 1))
            }
            (0x0104, 0x0000, 0x0004) => {
                value[0] = MANUFACTURER_NAME.len() as u8;
                let end = MANUFACTURER_NAME.len() + 1;
                value[1..end].copy_from_slice(MANUFACTURER_NAME.as_bytes());
                Ok((AttributeDataType::CharacterString, end))
            }
            (0x0104, 0x0000, 0x0005) => {
                value[0] = MODEL_IDENTIFIER.len() as u8;
                let end = MODEL_IDENTIFIER.len() + 1;
                value[1..end].copy_from_slice(MODEL_IDENTIFIER.as_bytes());
                Ok((AttributeDataType::CharacterString, end))
            }
            (0x0104, 0x0000, 0x0007) => {
                value[0] = 0x01;
                Ok((AttributeDataType::Enumeration8, 1))
            }
            (0x0104, 0x0006, 0x0000) => {
                value[0] = if self.on_off { 0x01 } else { 0x00 };
                Ok((AttributeDataType::Boolean, 1))
            }
            (_, _, _) => Err(ClusterLibraryStatus::UnsupportedAttribute),
        }
    }
    fn write_attribute(
        &mut self,
        profile: u16,
        cluster: u16,
        _endpoint: u8,
        attribute: u16,
        data_type: AttributeDataType,
        value: &[u8],
    ) -> Result<(), ClusterLibraryStatus> {
        match (profile, cluster, attribute, data_type) {
            (0x0104, 0x0000, 0x0000, _) | (0x0104, 0x0000, 0x0007, _) => {
                Err(ClusterLibraryStatus::ReadOnly)
            }
            (0x0104, 0x0006, 0x0000, AttributeDataType::Boolean) => {
                self.set_on_off(value[0] == 0x01);
                Ok(())
            }
            (0x0104, 0x0006, 0x0000, _) => Err(ClusterLibraryStatus::InvalidValue),
            (_, _, _, _) => Err(ClusterLibraryStatus::UnsupportedAttribute),
        }
    }
    fn run(
        &mut self,
        profile: u16,
        cluster: u16,
        _endpoint: u8,
        command: u8,
        _arguments: &[u8],
    ) -> Result<(), ClusterLibraryStatus> {
        match (profile, cluster, command) {
            (0x0104, 0x0006, 0x00) => {
                self.set_on_off(false);
                Ok(())
            }
            (0x0104, 0x0006, 0x01) => {
                self.set_on_off(true);
                Ok(())
            }
            (0x0104, 0x0006, 0x02) => {
                self.set_on_off(!self.on_off);
                Ok(())
            }
            (_, _, _) => Err(ClusterLibraryStatus::UnsupportedClusterCommand),
        }
    }
}

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        timer: pac::TIMER1,
        radio: Radio,
        service: PsilaService<'static, TxBufferSize, CryptoCellBackend, ClusterHandler>,
        rx_producer: bbqueue::Producer<'static, RxBufferSize>,
        rx_consumer: bbqueue::Consumer<'static, RxBufferSize>,
        tx_consumer: bbqueue::Consumer<'static, TxBufferSize>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let mut timer0 = cx.device.TIMER0;
        timer0.init();

        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let port0 = gpio::p0::Parts::new(cx.device.P0);
        let led_1 = port0
            .p0_13
            .into_push_pull_output(gpio::Level::Low)
            .degrade();

        let handler = ClusterHandler::new(led_1);

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
        timer1.fire_in(1, TIMER_SECOND);

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
            service: PsilaService::new(
                cryptocell,
                tx_producer,
                extended_address,
                default_link_key,
                handler,
            ),
            rx_producer,
            rx_consumer,
            tx_consumer,
        }
    }

    #[task(binds = TIMER1, resources = [service, timer], spawn = [radio_tx])]
    fn timer(cx: timer::Context) {
        let timer = cx.resources.timer;
        let service = cx.resources.service;

        if timer.is_compare_event(1) {
            timer.ack_compare_event(1);
            let _ = service.update(timer.now());
            timer.fire_in(1, TIMER_SECOND);
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
                        }
                        Err(e) => match e {
                            psila_service::Error::MalformedPacket => {
                                defmt::warn!(
                                    "service handle acknowledge failed, malformed package"
                                );
                            }
                            psila_service::Error::NotEnoughSpace => {
                                defmt::warn!("service handle acknowledge failed, queue full");
                            }
                            _ => {
                                defmt::warn!("service handle acknowledge failed");
                            }
                        },
                    }
                }
            }
            Err(psila_nrf52::radio::Error::CcaBusy) => {
                defmt::warn!("CCA Busy");
            }
        }
        let _ = cx.spawn.radio_tx();
    }

    #[task(resources = [rx_consumer, service, timer], spawn = [radio_tx])]
    fn radio_rx(cx: radio_rx::Context) {
        let queue = cx.resources.rx_consumer;
        let service = cx.resources.service;
        let timer = cx.resources.timer;

        if let Ok(grant) = queue.read() {
            let timestamp = timer.now();
            let packet_length = grant[0] as usize;
            if let Err(_) = service.receive(timestamp, &grant[1..packet_length - 1]) {
                defmt::warn!("service receive failed");
            }
            grant.release(packet_length);
            let _ = cx.spawn.radio_tx();
        }
    }

    #[task(resources = [radio, tx_consumer], spawn = [radio_rx])]
    fn radio_tx(cx: radio_tx::Context) {
        let queue = cx.resources.tx_consumer;
        let radio = cx.resources.radio;

        if !radio.is_tx_busy() {
            if let Ok(grant) = queue.read() {
                let packet_length = grant[0] as usize;
                let data = &grant[1..=packet_length];
                let _ = radio.queue_transmission(data);
                grant.release(packet_length + 1);
            }
            let _ = cx.spawn.radio_rx();
        }
    }

    extern "C" {
        fn QDEC();
    }
};
