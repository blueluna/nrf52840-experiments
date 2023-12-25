#![no_main]
#![no_std]

use adafruit_feather_nrf52840_express as _;

use rtic::app;

use nrf52840_hal::gpio;
use nrf52840_pac as pac;

use psila_data::{
    cluster_library::{AttributeDataType, ClusterLibraryStatus},
    device_profile::SimpleDescriptor,
};
use psila_service::ClusterLibraryHandler;

use nrf_smartled::pwm::Pwm;
use smart_leds::{gamma, RGB8};
use smart_leds_trait::SmartLedsWrite;

use palette::{Pixel, Srgb, Yxy};

use byteorder::{ByteOrder, LittleEndian};

// Manufacturer name for this example
const MANUFACTURER_NAME: &'static str = "ERIK of Sweden";
// Model identifier for this example
const MODEL_IDENTIFIER: &'static str = "Lampan";

/// Home automation profile
const PROFILE_HOME_AUTOMATION: u16 = 0x0104;
/// Colour dimmable light device
const DEVICE_COLOR_DIMMABLE_LIGHT: u16 = 0x0102;

/// Basic cluster
const CLUSTER_BASIC: u16 = 0x0000;
/// Basic cluster attribute, library version
const BASIC_ATTR_LIBRARY_VERSION: u16 = 0x0000;
/// Basic cluster attribute, manufacturer name
const BASIC_ATTR_MANUFACTURER_NAME: u16 = 0x0004;
/// Basic cluster attribute, model identifier
const BASIC_ATTR_MODEL_IDENTIFIER: u16 = 0x0005;
/// Basic cluster attribute, power source
const BASIC_ATTR_POWER_SOURCE: u16 = 0x0007;

/// On/off cluster
const CLUSTER_ON_OFF: u16 = 0x0006;
/// On/off cluster attribute, on/off state
const ON_OFF_ATTR_ON_OFF_STATE: u16 = 0x0000;
/// On/off cluster command, off
const ON_OFF_CMD_OFF: u8 = 0x00;
/// On/off cluster command, on
const ON_OFF_CMD_ON: u8 = 0x01;
/// On/off cluster command, toggle
const ON_OFF_CMD_TOGGLE: u8 = 0x02;

/// Level control cluster
const CLUSTER_LEVEL_CONTROL: u16 = 0x0008;
/// Level control cluster attribute, current level
const LEVEL_CONTROL_ATTR_CURRENT_LEVEL: u16 = 0x0000;
/// Level control cluster command, move to level
const LEVEL_CONTROL_CMD_MOVE_TO_LEVEL: u8 = 0x00;
/// Level control cluster command, move
const LEVEL_CONTROL_CMD_MOVE: u8 = 0x01;
/// Level control cluster command, step
const LEVEL_CONTROL_CMD_STEP: u8 = 0x02;
/// Level control cluster command, stop
const LEVEL_CONTROL_CMD_STOP: u8 = 0x03;
/// Level control cluster command, move to level with on/off
const LEVEL_CONTROL_CMD_MOVE_TO_LEVEL_ON_OFF: u8 = 0x04;
/// Level control cluster command, move with on/off
const LEVEL_CONTROL_CMD_MOVE_ON_OFF: u8 = 0x05;
/// Level control cluster command, step with on/off
const LEVEL_CONTROL_CMD_STEP_ON_OFF: u8 = 0x06;
/// Level control cluster command, stop with on/off
const LEVEL_CONTROL_CMD_STOP_ON_OFF: u8 = 0x07;

/// Colour control cluster
const CLUSTER_COLOR_CONTROL: u16 = 0x0300;
/// Colour control cluster attribute, Current X
const COLOR_CONTROL_ATTR_CURRENT_X: u16 = 0x0003;
/// Colour control cluster attribute, Current Y
const COLOR_CONTROL_ATTR_CURRENT_Y: u16 = 0x0004;
/// Colour control cluster attribute, Colour mode
const COLOR_CONTROL_ATTR_COLOR_MODE: u16 = 0x0008;
/// Colour control cluster attribute, Colour capabilities
const COLOR_CONTROL_ATTR_COLOR_CAPABILITIES: u16 = 0x400a;
/// Colour control cluster command, Move to hue
const COLOR_CONTROL_CMD_MOVE_TO_HUE: u8 = 0x00;
/// Colour control cluster command, Move hue
const COLOR_CONTROL_CMD_MOVE_HUE: u8 = 0x01;
/// Colour control cluster command, Step hue
const COLOR_CONTROL_CMD_STEP_HUE: u8 = 0x02;
/// Colour control cluster command, Move to saturation
const COLOR_CONTROL_CMD_MOVE_TO_SATURATION: u8 = 0x03;
/// Colour control cluster command, Move saturation
const COLOR_CONTROL_CMD_MOVE_SATURATION: u8 = 0x04;
/// Colour control cluster command, Step saturation
const COLOR_CONTROL_CMD_STEP_SATURATION: u8 = 0x05;
/// Colour control cluster command, Move to hue and saturation
const COLOR_CONTROL_CMD_MOVE_TO_HUE_AND_SATURATION: u8 = 0x06;
/// Colour control cluster command, Move to colour
const COLOR_CONTROL_CMD_MOVE_TO_COLOR: u8 = 0x07;
/// Colour control cluster command, Move colour
const COLOR_CONTROL_CMD_MOVE_COLOR: u8 = 0x08;
/// Colour control cluster command, Step colour
const COLOR_CONTROL_CMD_STEP_COLOR: u8 = 0x09;
/// Colour control cluster command, Stop move step
const COLOR_CONTROL_CMD_STOP_MOVE_STEP: u8 = 0x47;

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
                PROFILE_HOME_AUTOMATION,
                DEVICE_COLOR_DIMMABLE_LIGHT,
                0,
                &[CLUSTER_BASIC, CLUSTER_ON_OFF, CLUSTER_LEVEL_CONTROL, CLUSTER_COLOR_CONTROL],
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
            (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_LIBRARY_VERSION) => {
                value[0] = 0x02;
                Ok((AttributeDataType::Unsigned8, 1))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_MANUFACTURER_NAME) => {
                value[0] = MANUFACTURER_NAME.len() as u8;
                let end = MANUFACTURER_NAME.len() + 1;
                value[1..end].copy_from_slice(MANUFACTURER_NAME.as_bytes());
                Ok((AttributeDataType::CharacterString, end))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_MODEL_IDENTIFIER) => {
                value[0] = MODEL_IDENTIFIER.len() as u8;
                let end = MODEL_IDENTIFIER.len() + 1;
                value[1..end].copy_from_slice(MODEL_IDENTIFIER.as_bytes());
                Ok((AttributeDataType::CharacterString, end))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_POWER_SOURCE) => {
                value[0] = 0x01;
                Ok((AttributeDataType::Enumeration8, 1))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_ATTR_ON_OFF_STATE) => {
                value[0] = if self.on_off { 0x01 } else { 0x00 };
                Ok((AttributeDataType::Boolean, 1))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_ATTR_CURRENT_LEVEL) => {
                // current level
                value[0] = self.get_level();
                Ok((AttributeDataType::Unsigned8, 1))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_ATTR_CURRENT_X) => {
                // current x
                LittleEndian::write_u16(&mut value[0..=2], self.get_x());
                Ok((AttributeDataType::Unsigned16, 2))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_ATTR_CURRENT_Y) => {
                // current y
                LittleEndian::write_u16(&mut value[0..=2], self.get_y());
                Ok((AttributeDataType::Unsigned16, 2))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_ATTR_COLOR_MODE) => {
                // color mode
                value[0] = 0x01; // Current X, Current Y
                Ok((AttributeDataType::Enumeration8, 1))
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_ATTR_COLOR_CAPABILITIES) => {
                // color capabilities
                let capabilities = 0b_0000_0000_0000_1000; // XY
                LittleEndian::write_u16(&mut value[0..=2], capabilities);
                Ok((AttributeDataType::Bitmap16, 2))
            }
            (_, _, _) => {
                defmt::info!(
                    "Read attribute: {=u16} {=u16} {=u16}",
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
            (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_LIBRARY_VERSION, _) | (PROFILE_HOME_AUTOMATION, CLUSTER_BASIC, BASIC_ATTR_POWER_SOURCE, _) => {
                Err(ClusterLibraryStatus::ReadOnly)
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_ATTR_ON_OFF_STATE, AttributeDataType::Boolean) => {
                self.set_on_off(value[0] == 0x01);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_ATTR_ON_OFF_STATE, _) => Err(ClusterLibraryStatus::InvalidValue),
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
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_CMD_OFF) => {
                // set off
                self.set_on_off(false);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_CMD_ON) => {
                // set on
                self.set_on_off(true);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_ON_OFF, ON_OFF_CMD_TOGGLE) => {
                // toggle
                self.set_on_off(!self.on_off);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_MOVE_TO_LEVEL) => {
                // move to level
                if arguments.len() >= 3 {
                    let level = arguments[0];
                    let transition_time = LittleEndian::read_u16(&arguments[1..=2]);
                    defmt::info!("Move to level: {=u8} {=u16}", level, transition_time);
                    self.set_level(level);
                } else {
                    defmt::warn!("Move to level ?");
                }
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_MOVE) => {
                // move
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move: {=u8} {=u8}", mode, rate);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_STEP) => {
                // step
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!("Step: {=u8} {=u8} {=u16}", mode, step, transition_time);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_STOP) | (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_STOP_ON_OFF) => {
                // stop
                defmt::info!("Stop");
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_MOVE_TO_LEVEL_ON_OFF) => {
                // move to level, on / off
                let level = arguments[0];
                let _transition_time = LittleEndian::read_u16(&arguments[1..=2]);
                self.set_on_off(level > 0);
                self.set_level(level);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_MOVE_ON_OFF) => {
                // move, on / off
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move (on/off): {=u8} {=u8}", mode, rate);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_LEVEL_CONTROL, LEVEL_CONTROL_CMD_STEP_ON_OFF) => {
                // step, on / off
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!(
                    "Step (on/off): {=u8} {=u8} {=u16}",
                    mode,
                    step,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_TO_HUE) => {
                // move to hue
                let hue = arguments[0];
                let direction = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..=3]);
                defmt::info!(
                    "Move to hue: {=u8} {=u8} {=u16}",
                    hue,
                    direction,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_HUE) => {
                // move hue
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move hue: {=u8} {=u8}", mode, rate);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_STEP_HUE) => {
                // step hue
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!("Step hue: {=u8} {=u8} {=u16}", mode, step, transition_time);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_TO_SATURATION) => {
                // move to saturation
                let saturation = arguments[0];
                let transition_time = LittleEndian::read_u16(&arguments[1..3]);
                defmt::info!(
                    "Move to saturation: {=u8} {=u16}",
                    saturation,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_SATURATION) => {
                // move saturation
                let mode = arguments[0];
                let rate = arguments[1];
                defmt::info!("Move saturation: {=u8} {=u8}", mode, rate);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_STEP_SATURATION) => {
                // step saturation
                let mode = arguments[0];
                let step = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!(
                    "Step saturation: {=u8} {=u8} {=u16}",
                    mode,
                    step,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_TO_HUE_AND_SATURATION) => {
                // move to hue and saturation
                let hue = arguments[0];
                let saturation = arguments[1];
                let transition_time = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!(
                    "Move to  hue and saturation: {=u8} {=u8} {=u16}",
                    hue,
                    saturation,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_TO_COLOR) => {
                // move to color
                if arguments.len() >= 6 {
                    let x = LittleEndian::read_u16(&arguments[0..2]);
                    let y = LittleEndian::read_u16(&arguments[2..4]);
                    let transition_time = LittleEndian::read_u16(&arguments[4..6]);
                    defmt::info!("Move to color: {=u16} {=u16} {=u16}", x, y, transition_time);
                    self.set_color(x, y);
                } else {
                    defmt::warn!("Move to color ?");
                }
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_MOVE_COLOR) => {
                // move color
                let rate_x = LittleEndian::read_u16(&arguments[0..2]);
                let rate_y = LittleEndian::read_u16(&arguments[2..4]);
                defmt::info!("Move color: {=u16} {=u16}", rate_x, rate_y);
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_STEP_COLOR) => {
                // step color
                let step_x = LittleEndian::read_u16(&arguments[0..2]);
                let step_y = LittleEndian::read_u16(&arguments[2..4]);
                let transition_time = LittleEndian::read_u16(&arguments[4..6]);
                defmt::info!(
                    "Step color: {=u16} {=u16} {=u16}",
                    step_x,
                    step_y,
                    transition_time
                );
                Ok(())
            }
            (PROFILE_HOME_AUTOMATION, CLUSTER_COLOR_CONTROL, COLOR_CONTROL_CMD_STOP_MOVE_STEP) => {
                // stop move step
                defmt::info!("Stop move step");
                Ok(())
            }
            (_, _, _) => {
                defmt::info!("Operation {=u16} {=u16} {=u8}", profile, cluster, command);
                Err(ClusterLibraryStatus::UnsupportedClusterCommand)
            }
        }
    }
}

#[app(device = nrf52840_pac, peripherals = true, dispatchers = [QDEC])]
mod app {
    use super::{pac, ClusterHandler};

    use bbqueue::{self, BBBuffer};

    use nrf52840_hal::{clocks, gpio};

    use nrf52_cryptocell::CryptoCellBackend;
    use psila_data::{security::DEFAULT_LINK_KEY, ExtendedAddress, Key};
    use psila_nrf52::{
        radio::{Radio, MAX_PACKET_LENGHT},
        timer::Timer,
    };
    use psila_service::{self, PsilaService};

    const TIMER_SECOND: u32 = 1_000_000;

    const TX_BUFFER_SIZE: usize = 1024;
    const RX_BUFFER_SIZE: usize = 1024;

    static RX_BUFFER: BBBuffer<RX_BUFFER_SIZE> = BBBuffer::new();
    static TX_BUFFER: BBBuffer<TX_BUFFER_SIZE> = BBBuffer::new();

    #[local]
    struct LocalResources {
        rx_producer: bbqueue::Producer<'static, RX_BUFFER_SIZE>,
        rx_consumer: bbqueue::Consumer<'static, RX_BUFFER_SIZE>,
        tx_consumer: bbqueue::Consumer<'static, TX_BUFFER_SIZE>,
    }

    #[shared]
    struct SharedResources {
        timer: pac::TIMER1,
        radio: Radio,
        service: PsilaService<'static, CryptoCellBackend, ClusterHandler, TX_BUFFER_SIZE>,
    }

    #[init]
    fn init(cx: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
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
        timer1.fire_in(1, TIMER_SECOND);

        let mut radio = Radio::new(cx.device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        let (rx_producer, rx_consumer) = RX_BUFFER.try_split().unwrap();
        let (tx_producer, tx_consumer) = TX_BUFFER.try_split().unwrap();

        let cryptocell = CryptoCellBackend::new(cx.device.CRYPTOCELL);
        let default_link_key = Key::from(DEFAULT_LINK_KEY);

        (
            SharedResources {
                timer: timer1,
                radio,
                service: PsilaService::new(
                    cryptocell,
                    tx_producer,
                    extended_address,
                    default_link_key,
                    handler,
                ),
            },
            LocalResources {
                rx_producer,
                rx_consumer,
                tx_consumer,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIMER1, shared = [service, timer])]
    fn timer(cx: timer::Context) {
        (cx.shared.timer, cx.shared.service).lock(|timer, service| {
            if timer.is_compare_event(1) {
                timer.ack_compare_event(1);
                let _ = service.update(timer.now());
                timer.fire_in(1, TIMER_SECOND);
            }
            let _ = radio_tx::spawn();
        });
    }

    #[task(binds = RADIO, shared = [radio, service], local = [rx_producer])]
    fn radio(cx: radio::Context) {
        let queue = cx.local.rx_producer;
        (cx.shared.radio, cx.shared.service).lock(|radio, service| {
            let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
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
            let _ = radio_tx::spawn();
        });
    }

    #[task(shared = [service, timer], local = [rx_consumer])]
    fn radio_rx(mut cx: radio_rx::Context) {
        let queue = cx.local.rx_consumer;
        let timestamp = cx.shared.timer.lock(|timer| timer.now());
        cx.shared.service.lock(|service| {
            if let Ok(grant) = queue.read() {
                let packet_length = grant[0] as usize;
                if let Err(_) = service.receive(timestamp, &grant[1..packet_length - 1]) {
                    defmt::warn!("service receive failed");
                }
                grant.release(packet_length);
                let _ = radio_tx::spawn();
            }
        });
    }

    #[task(shared = [radio], local = [tx_consumer])]
    fn radio_tx(mut cx: radio_tx::Context) {
        let queue = cx.local.tx_consumer;
        cx.shared.radio.lock(|radio| {
            if !radio.is_tx_busy() {
                if let Ok(grant) = queue.read() {
                    let packet_length = grant[0] as usize;
                    let data = &grant[1..=packet_length];
                    let _ = radio.queue_transmission(data);
                    grant.release(packet_length + 1);
                }
                let _ = radio_rx::spawn();
            }
        });
    }
}
