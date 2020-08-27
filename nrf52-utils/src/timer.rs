use nrf52840_pac::{TIMER0, TIMER1};

pub trait Timer {
    fn init(&mut self);
    fn fire_at(&mut self, id: usize, at: u32);
    fn fire_plus(&mut self, id: usize, elapsed: u32);
    fn stop(&mut self, id: usize);
    fn now(&self) -> u32;
    fn ack_compare_event(&mut self, id: usize);
    fn is_compare_event(&self, id: usize) -> bool;
}

macro_rules! impl_timer {
    ($ty:ident) => {
        impl Timer for $ty {
            fn init(&mut self) {
                // tick resolution is 1 us
                self.tasks_stop.write(|w| w.tasks_stop().set_bit());
                self.mode.write(|w| w.mode().timer());
                self.bitmode.write(|w| w.bitmode()._32bit());
                self.prescaler.write(|w| unsafe { w.prescaler().bits(4) });
                self.tasks_clear.write(|w| w.tasks_clear().set_bit());
                self.tasks_start.write(|w| w.tasks_start().set_bit());
            }

            fn fire_at(&mut self, id: usize, at: u32) {
                assert!(id > 0 && id <= 5);
                let now = self.now();
                let later = now.wrapping_add(at);
                self.cc[id].write(|w| unsafe { w.bits(later) });
                self.events_compare[id].reset();
                match id {
                    1 => {
                        self.intenset.write(|w| w.compare1().set_bit());
                    }
                    2 => {
                        self.intenset.write(|w| w.compare2().set_bit());
                    }
                    3 => {
                        self.intenset.write(|w| w.compare3().set_bit());
                    }
                    _ => (),
                }
            }

            fn fire_plus(&mut self, id: usize, elapsed: u32) {
                assert!(id > 0 && id <= 5);
                let current = self.cc[id].read().bits();
                let later = current.wrapping_add(elapsed);
                self.cc[id].write(|w| unsafe { w.bits(later) });
                self.events_compare[id].reset();
                match id {
                    1 => {
                        self.intenset.write(|w| w.compare1().set_bit());
                    }
                    2 => {
                        self.intenset.write(|w| w.compare2().set_bit());
                    }
                    3 => {
                        self.intenset.write(|w| w.compare3().set_bit());
                    }
                    _ => (),
                }
            }

            fn stop(&mut self, id: usize) {
                assert!(id > 0 && id <= 5);
                match id {
                    1 => {
                        self.intenclr.write(|w| w.compare1().clear_bit());
                    }
                    2 => {
                        self.intenclr.write(|w| w.compare2().clear_bit());
                    }
                    3 => {
                        self.intenclr.write(|w| w.compare3().clear_bit());
                    }
                    _ => (),
                }
                self.events_compare[id].reset();
            }

            fn now(&self) -> u32 {
                self.tasks_capture[0].write(|w| w.tasks_capture().set_bit());
                self.cc[0].read().bits()
            }

            fn ack_compare_event(&mut self, id: usize) {
                self.events_compare[id].reset();
            }

            fn is_compare_event(&self, id: usize) -> bool {
                self.events_compare[id].read().events_compare().bit_is_set()
            }
        }
    };
}

impl_timer!(TIMER0);
impl_timer!(TIMER1);
