#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_hal::{gpio, prelude::*};

use nrf52840_pac as pac;

use ieee802154::mac;
use ieee802154::mac::{FrameContent};
use ieee802154::mac::command::{Command};
use nrf52_radio_802154::radio::{Radio, MAX_PACKET_LENGHT};

pub fn build_beacon_request(sequence: u8, mut data: &mut [u8]) -> usize {
    let frame = mac::Frame {
        header: mac::Header {
            seq: sequence,
            frame_type: mac::FrameType::MacCommand,
            security: mac::Security::None,
            frame_pending: false,
            ack_request: false,
            pan_id_compress: false,
            version: mac::FrameVersion::Ieee802154_2003,
            destination: mac::Address::broadcast(&mac::AddressMode::Short),
            source: mac::Address::None,
        },
        content: mac::FrameContent::Command(mac::command::Command::BeaconRequest),
        payload: &[0u8; 0],
        footer: [0u8; 2],
    };
    frame.encode(&mut data, mac::WriteFooter::No)
}

#[app(device = nrf52840_pac)]
const APP: () = {
    static mut BEACON_TIMER: pac::TIMER1 = ();
    static mut LED_RED: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut LED_BLUE: gpio::Pin<gpio::Output<gpio::PushPull>> = ();
    static mut RADIO: Radio = ();
    static mut SEQUENCE: u8 = 0u8;

    #[init]
    fn init() {
        let p0 = device.P0.split();
        let mut led_green = p0.p0_22.degrade().into_push_pull_output(gpio::Level::High);
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
        led_green.set_low();
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

        let mut radio = Radio::new(device.RADIO);
        radio.set_channel(15);
        radio.set_transmission_power(8);
        radio.receive_prepare();

        BEACON_TIMER = timer1;
        LED_RED = p0.p0_23.degrade().into_push_pull_output(gpio::Level::High);
        LED_BLUE = p0.p0_24.degrade().into_push_pull_output(gpio::Level::Low);
        RADIO = radio;
    }

    #[interrupt(resources = [BEACON_TIMER, RADIO, SEQUENCE, LED_BLUE],)]
    fn TIMER1() {
        let timer = resources.BEACON_TIMER;
        // Clear event and restart
        timer.events_compare[0].write(|w| w.events_compare().clear_bit());
        timer.tasks_clear.write(|w| w.tasks_clear().set_bit());
        timer.tasks_start.write(|w| w.tasks_start().set_bit());

        (*resources.LED_BLUE).set_low();
        let mut radio = resources.RADIO;
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let size = build_beacon_request(*resources.SEQUENCE, &mut packet);
        let used = radio.queue_transmission(&mut packet[..size]);
        if used != size {
            hprintln!("Failed to send beacon").unwrap();
        }
        *resources.SEQUENCE = resources.SEQUENCE.wrapping_add(1);
    }

    #[interrupt(resources = [RADIO, LED_RED, LED_BLUE],)]
    fn RADIO() {
        let mut packet = [0u8; MAX_PACKET_LENGHT as usize];
        let mut radio = resources.RADIO;
        (*resources.LED_BLUE).set_high();
        (*resources.LED_RED).set_high();
        let packet_len = radio.receive(&mut packet);
        if packet_len > 0 {
            match mac::Frame::decode(&packet[1..packet_len - 1], false) {
                Ok(frame) => match frame.content {
                    FrameContent::Acknowledgement => {
                        hprintln!("Acknowledge").unwrap();
                    },
                    FrameContent::Beacon(_) => {
                        hprintln!("Beacon").unwrap();
                    }
                    FrameContent::Data => {
                        hprintln!("Data").unwrap();
                    },
                    FrameContent::Command(command) => {
                        match command {
                            Command::AssociationRequest(_) => {
                                hprintln!("AssociationRequest").unwrap();
                            },
                            Command::AssociationResponse(_, _) => {
                                hprintln!("AssociationResponse").unwrap();
                            },
                            Command::DisassociationNotification(_) => {
                                hprintln!("DisassociationNotification").unwrap();
                            },
                            Command::DataRequest => {
                                hprintln!("DataRequest").unwrap();
                            },
                            Command::PanIdConflictNotification => {
                                hprintln!("PanIdConflictNotification").unwrap();
                            },
                            Command::OrphanNotification => {
                                hprintln!("OrphanNotification").unwrap();
                            },
                            Command::BeaconRequest => {
                                hprintln!("BeaconRequest").unwrap();
                            },
                            Command::CoordinatorRealignment(_) => {
                                hprintln!("CoordinatorRealignment").unwrap();
                            },
                            Command::GuaranteedTimeSlotRequest(_) => {
                                hprintln!("GuaranteedTimeSlotRequest").unwrap();
                            },
                        }
                    },
                },
                Err(_) => {
                    hprintln!("Failed to decode frame").unwrap();
                },
            }
            (*resources.LED_RED).set_low();
        }
    }
};
