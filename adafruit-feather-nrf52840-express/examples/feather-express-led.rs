#![no_main]
#![no_std]

use core::convert::From;

use panic_rtt_target as _;

use rtt_target::{rprintln, rtt_init_print};

use rtic::app;

use nrf52840_hal::{clocks, gpio};

use nrf52840_pac as pac;

use nrf_smartled::pwm::Pwm;
use smart_leds::{gamma, RGB8};
use smart_leds_trait::SmartLedsWrite;

use palette::{Hsl, Hue, Pixel, Srgb};

use nrf52_utils::timer::Timer;

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        timer: pac::TIMER0,
        neopixel: Pwm<pac::PWM0>,
        colour: Hsl,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let port0 = gpio::p0::Parts::new(cx.device.P0);
        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        rtt_init_print!();

        let mut timer0 = cx.device.TIMER0;
        timer0.init();
        timer0.fire_plus(1, 100_000);

        let neopixel_pin = port0.p0_16.into_push_pull_output(gpio::Level::Low);

        let neopixel = Pwm::new(cx.device.PWM0, neopixel_pin.degrade());

        let colour = Hsl::new(0.0, 1.0, 0.25);

        rprintln!("Initialise late resources");

        init::LateResources {
            timer: timer0,
            neopixel,
            colour,
        }
    }

    #[task(binds = TIMER0, resources = [neopixel, timer, colour])]
    fn timer(cx: timer::Context) {
        let timer = cx.resources.timer;
        let neopixel = cx.resources.neopixel;
        let rgb = Srgb::from(*cx.resources.colour);

        let pixel: [u8; 3] = rgb.into_format().into_raw();

        let mut pixels = [RGB8::default(); 1];
        pixels[0].r = pixel[0];
        pixels[0].g = pixel[1];
        pixels[0].b = pixel[2];

        if timer.is_compare_event(1) {
            neopixel.write(gamma(pixels.iter().cloned())).ok();
            timer.ack_compare_event(1);
            timer.fire_plus(1, 100_000);
        }

        *cx.resources.colour = cx.resources.colour.shift_hue(1.0);
    }
};
