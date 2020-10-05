#![no_main]
#![no_std]

use adafruit_feather_nrf52840_express as _;

use rtic::app;

use nrf52840_hal::{clocks, gpio};

use nrf52840_pac as pac;

use bbqueue::{self, BBBuffer, ConstBBBuffer};

use nrf52_cryptocell::CryptoCellBackend;
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};
use nrf52_utils::timer::Timer;
use psila_data::{
    cluster_library::{AttributeDataType, ClusterLibraryStatus},
    device_profile::SimpleDescriptor,
    security::DEFAULT_LINK_KEY,
    ExtendedAddress, Key,
};
use psila_service::{self, ClusterLibraryHandler, PsilaService};

use bbqueue::consts::U1024 as TxBufferSize;
use bbqueue::consts::U1024 as RxBufferSize;

static RX_BUFFER: BBBuffer<RxBufferSize> = BBBuffer(ConstBBBuffer::new());
static TX_BUFFER: BBBuffer<TxBufferSize> = BBBuffer(ConstBBBuffer::new());

use nrf_smartled::pwm::Pwm;
use smart_leds::{gamma, RGB8};
use smart_leds_trait::SmartLedsWrite;

use palette::{Pixel, Srgb, Yxy};

use byteorder::{ByteOrder, LittleEndian};

const TIMER_SECOND: u32 = 1_000_000;

const MANUFACTURER_NAME: &'static str = "ERIK of Sweden";
const MODEL_IDENTIFIER: &'static str = "Lampan";

pub struct ClusterHandler {
    on_off: bool,
    neopixel: Pwm<pac::PWM0>,
    colour: Yxy,
}

impl ClusterHandler {
    pub fn new(pin: gpio::Pin<gpio::Output<gpio::PushPull>>, pwm: pac::PWM0) -> Self {
        let colour = Yxy::new(0.0, 0.0, 0.0);
        let neopixel = Pwm::new(pwm, pin);
        Self {
            on_off: false,
            neopixel,
            colour,
        }
    }

    fn update_led(&mut self) {
        let mut pixel = RGB8::default();
        if self.on_off {
            let raw: [u8; 3] = Srgb::from(self.colour).into_format().into_raw();
            pixel.r = raw[0];
            pixel.g = raw[1];
            pixel.b = raw[2];
        } else {
            pixel.r = 0;
            pixel.g = 0;
            pixel.b = 0;
        }
        let pixels = [pixel; 1];
        let _ = self.neopixel.write(gamma(pixels.iter().cloned()));
    }

    pub fn set_on_off(&mut self, enable: bool) {
        self.on_off = enable;
        self.update_led();
    }

    pub fn get_level(&self) -> u8 {
        (self.colour.luma * 254.0) as u8
    }

    pub fn set_level(&mut self, level: u8) {
        let level_float = (level as f32) / 254.0;
        self.colour.luma = level_float;
        self.update_led();
    }

    pub fn get_x(&self) -> u16 {
        (self.colour.x * 65536.0) as u16
    }

    pub fn set_x(&mut self, x: u16) {
        self.colour.x = (x as f32) / 65536.0;
        self.update_led();
    }

    pub fn get_y(&self) -> u16 {
        (self.colour.y * 65536.0) as u16
    }

    pub fn set_y(&mut self, y: u16) {
        self.colour.y = (y as f32) / 65536.0;
        self.update_led();
    }

    pub fn set_color(&mut self, x: u16, y: u16) {
        self.colour.x = (x as f32) / 65536.0;
        self.colour.y = (y as f32) / 65536.0;
        self.update_led();
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
                0x0102,
                0,
                &[0x0000, 0x0006, 0x0008, 0x0300],
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
            (0x0104, 0x0008, 0x0000) => {
                // current level
                value[0] = self.get_level();
                Ok((AttributeDataType::Unsigned8, 1))
            }
            (0x0104, 0x0300, 0x0003) => {
                // current x
                LittleEndian::write_u16(&mut value[0..=2], self.get_x());
                Ok((AttributeDataType::Unsigned16, 2))
            }
            (0x0104, 0x0300, 0x0004) => {
                // current y
                LittleEndian::write_u16(&mut value[0..=2], self.get_y());
                Ok((AttributeDataType::Unsigned16, 2))
            }
            (0x0104, 0x0300, 0x0008) => {
                // color mode
                value[0] = 0x01; // Current X, Current Y
                Ok((AttributeDataType::Enumeration8, 1))
            }
            (0x0104, 0x0300, 0x400a) => {
                // color capabilities
                let capabilities = 0b_0000_0000_0000_1000; // XY
                LittleEndian::write_u16(&mut value[0..=2], capabilities);
                Ok((AttributeDataType::Bitmap16, 2))
            }
            (_, _, _) => {
                defmt::info!(
                    "Read attribute: {:u16} {:u16} {:u16}",
                    profile,
                    cluster,
                    attribute
                );
                Err(ClusterLibraryStatus::UnsupportedAttribute)
            }
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
        arguments: &[u8],
    ) -> Result<(), ClusterLibraryStatus> {
        match (profile, cluster, command) {
            (0x0104, 0x0006, 0x00) => {
                // set off
                self.set_on_off(false);
                Ok(())
            }
            (0x0104, 0x0006, 0x01) => {
                // set on
                self.set_on_off(true);
                Ok(())
            }
            (0x0104, 0x0006, 0x02) => {
                // toggle
                self.set_on_off(!self.on_off);
                Ok(())
            }
            (0x0104, 0x0008, 0x00) => {
                // move to level
                if arguments.len() >= 3 {
                    let level = arguments[0];
                    let transition_time = LittleEndian::read_u16(&arguments[1..=2]);
                    defmt::info!("Move to level: {:u8} {:u16}", level, transition_time);
                    self.set_level(level);
                } else {
                    defmt::warn!("Move to level ?");
                }
                Ok(())
            }
            (0x0104, 0x0008, 0x01) => {
                // move
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move: {:u8} {:u8}", mode, rate);
                Ok(())
            }
            (0x0104, 0x0008, 0x02) => {
                // step
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!("Step: {:u8} {:u8} {:u16}", mode, step, transition_time);
                Ok(())
            }
            (0x0104, 0x0008, 0x03) | (0x0104, 0x0008, 0x07) => {
                // stop
                defmt::info!("Stop");
                Ok(())
            }
            (0x0104, 0x0008, 0x04) => {
                // move to level, on / off
                let level = arguments[0];
                let _transition_time = LittleEndian::read_u16(&arguments[1..=2]);
                self.set_on_off(level > 0);
                self.set_level(level);
                Ok(())
            }
            (0x0104, 0x0008, 0x05) => {
                // move, on / off
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move (on/off): {:u8} {:u8}", mode, rate);
                Ok(())
            }
            (0x0104, 0x0008, 0x06) => {
                // step, on / off
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!(
                    "Step (on/off): {:u8} {:u8} {:u16}",
                    mode,
                    step,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x00) => {
                // move to hue
                let hue = arguments[0];
                let direction = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!(
                    "Move to hue: {:u8} {:u8} {:u16}",
                    hue,
                    direction,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x01) => {
                // move hue
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move hue: {:u8} {:u8}", mode, rate);
                Ok(())
            }
            (0x0104, 0x0300, 0x02) => {
                // step hue
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!("Step hue: {:u8} {:u8} {:u16}", mode, step, transition_time);
                Ok(())
            }
            (0x0104, 0x0300, 0x03) => {
                // move to saturation
                let saturation = arguments[0];
                let transition_time = LittleEndian::read_u16(&arguments[1..3]);
                defmt::info!(
                    "Move to saturation: {:u8} {:u16}",
                    saturation,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x04) => {
                // move saturation
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move saturation: {:u8} {:u8}", mode, rate);
                Ok(())
            }
            (0x0104, 0x0300, 0x05) => {
                // step saturation
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!(
                    "Step saturation: {:u8} {:u8} {:u16}",
                    mode,
                    step,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x06) => {
                // move to hue and saturation
                let hue = arguments[0];
                let saturation = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!(
                    "Move to  hue and saturation: {:u8} {:u8} {:u16}",
                    hue,
                    saturation,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x07) => {
                // move to color
                if arguments.len() >= 6 {
                    let x = LittleEndian::read_u16(&arguments[0..2]);
                    let y = LittleEndian::read_u16(&arguments[2..4]);
                    let transition_time = LittleEndian::read_u16(&arguments[4..6]);
                    defmt::info!("Move to color: {:u16} {:u16} {:u16}", x, y, transition_time);
                    self.set_color(x, y);
                } else {
                    defmt::warn!("Move to color ?");
                }
                Ok(())
            }
            (0x0104, 0x0300, 0x08) => {
                // move color
                let rate_x = LittleEndian::read_u16(&arguments[0..2]);
                let rate_y = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!("Move color: {:u16} {:u16}", rate_x, rate_y);
                Ok(())
            }
            (0x0104, 0x0300, 0x09) => {
                // step color
                let step_x = LittleEndian::read_u16(&arguments[0..2]);
                let step_y = LittleEndian::read_u16(&arguments[2..4]);
                let transition_time = LittleEndian::read_u16(&arguments[4..6]);
                defmt::info!(
                    "Step color: {:u16} {:u16} {:u16}",
                    step_x,
                    step_y,
                    transition_time
                );
                Ok(())
            }
            (0x0104, 0x0300, 0x47) => {
                // stop move step
                defmt::info!("Stop move step");
                Ok(())
            }
            (_, _, _) => {
                defmt::info!("Operation {:u16} {:u16} {:u8}", profile, cluster, command);
                Err(ClusterLibraryStatus::UnsupportedClusterCommand)
            }
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
        let led = port0
            .p0_16
            .into_push_pull_output(gpio::Level::Low)
            .degrade();

        let handler = ClusterHandler::new(led, cx.device.PWM0);

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
        timer1.fire_at(1, TIMER_SECOND * 10);
        timer1.fire_at(2, TIMER_SECOND);

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

            let fire_at = match service.timeout() {
                Ok(time) => time,
                Err(_) => {
                    defmt::warn!("service timeout failed");
                    0
                }
            };
            if fire_at > 0 {
                timer.fire_at(1, fire_at);
            }
        }
        if timer.is_compare_event(2) {
            timer.ack_compare_event(2);
            let _ = service.update(timer.now());
            timer.fire_plus(2, TIMER_SECOND);
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
            Err(nrf52_radio_802154::radio::Error::CcaBusy) => {
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
            let fire_at = match service.receive(&grant[1..packet_length - 1], timestamp) {
                Ok(fire_at) => fire_at,
                Err(_e) => {
                    defmt::warn!("service receive failed");
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
