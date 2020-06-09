#![no_main]
#![no_std]

use core::fmt::Write;

#[allow(unused_imports)]
use panic_halt;

use rtfm::app;

use embedded_hal::digital::v2::{InputPin, OutputPin};

use crate::hal::target as pac;
use nrf52840_hal as hal;

use hal::{clocks, gpio, timer::Instance, uarte};
use pac::{RTC0, TIMER0, UARTE0};

#[app(device = crate::hal::target, peripherals = true)]
const APP: () = {
    struct Resources {
        button_1: gpio::Pin<gpio::Input<gpio::PullUp>>,
        button_2: gpio::Pin<gpio::Input<gpio::PullUp>>,
        button_3: gpio::Pin<gpio::Input<gpio::PullUp>>,
        button_4: gpio::Pin<gpio::Input<gpio::PullUp>>,
        led_1: gpio::Pin<gpio::Output<gpio::PushPull>>,
        led_2: gpio::Pin<gpio::Output<gpio::PushPull>>,
        led_3: gpio::Pin<gpio::Output<gpio::PushPull>>,
        led_4: gpio::Pin<gpio::Output<gpio::PushPull>>,
        #[init(false)]
        on_off: bool,
        rtc_0: hal::rtc::Rtc<RTC0, hal::rtc::Started>,
        timer_0: TIMER0,
        uart: uarte::Uarte<UARTE0>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        cx.device.TIMER0.set_periodic();
        cx.device.TIMER0.enable_interrupt();
        cx.device.TIMER0.timer_start(1_000_000u32);

        let mut rtc_0 = hal::rtc::Rtc::new(cx.device.RTC0);
        let _ = rtc_0.set_prescaler(4095);
        rtc_0.enable_event(hal::rtc::RtcInterrupt::Tick);
        rtc_0.enable_interrupt(hal::rtc::RtcInterrupt::Tick, None);
        let rtc_0 = rtc_0.enable_counter();

        let port0 = gpio::p0::Parts::new(cx.device.P0);
        let button_1 = port0.p0_11.into_pullup_input().degrade();
        let button_2 = port0.p0_12.into_pullup_input().degrade();
        let button_3 = port0.p0_24.into_pullup_input().degrade();
        let button_4 = port0.p0_25.into_pullup_input().degrade();
        let led_1 = port0
            .p0_13
            .into_push_pull_output(gpio::Level::Low)
            .degrade();
        let led_2 = port0
            .p0_14
            .into_push_pull_output(gpio::Level::High)
            .degrade();
        let led_3 = port0
            .p0_15
            .into_push_pull_output(gpio::Level::High)
            .degrade();
        let led_4 = port0
            .p0_16
            .into_push_pull_output(gpio::Level::High)
            .degrade();

        let uart = uarte::Uarte::new(
            cx.device.UARTE0,
            uarte::Pins {
                txd: port0
                    .p0_06
                    .into_push_pull_output(gpio::Level::High)
                    .degrade(),
                rxd: port0.p0_08.into_floating_input().degrade(),
                cts: Some(port0.p0_07.into_floating_input().degrade()),
                rts: Some(
                    port0
                        .p0_05
                        .into_push_pull_output(gpio::Level::High)
                        .degrade(),
                ),
            },
            uarte::Parity::EXCLUDED,
            uarte::Baudrate::BAUD115200,
        );

        init::LateResources {
            timer_0: cx.device.TIMER0,
            button_1,
            button_2,
            button_3,
            button_4,
            led_1,
            led_2,
            led_3,
            led_4,
            rtc_0,
            uart,
        }
    }

    #[task(binds = TIMER0, resources = [timer_0, led_3, on_off])]
    fn timer(cx: timer::Context) {
        cx.resources.timer_0.timer_reset_event();
        if *cx.resources.on_off {
            let _ = cx.resources.led_3.set_low();
        } else {
            let _ = cx.resources.led_3.set_high();
        }
        *cx.resources.on_off = !*cx.resources.on_off;
    }

    #[task(binds = RTC0, resources = [rtc_0, button_4, led_4, uart])]
    fn rtc(cx: rtc::Context) {
        let _ = cx
            .resources
            .rtc_0
            .get_event_triggered(hal::rtc::RtcInterrupt::Tick, true);
        let button_4 = cx.resources.button_4;
        let led_4 = cx.resources.led_4;
        let uart = cx.resources.uart;

        match button_4.is_low() {
            Ok(true) => {
                let _ = write!(uart, "Button 4\r\n");
                let _ = led_4.set_low();
            }
            Ok(false) => {
                let _ = led_4.set_high();
            }
            Err(_) => {}
        }
    }

    #[idle(resources = [button_2, led_2])]
    fn idle(cx: idle::Context) -> ! {
        let button_2 = cx.resources.button_2;
        let led_2 = cx.resources.led_2;

        loop {
            match button_2.is_low() {
                Ok(true) => {
                    let _ = led_2.set_low();
                }
                Ok(false) => {
                    let _ = led_2.set_high();
                }
                Err(_) => {}
            }
        }
    }
};
