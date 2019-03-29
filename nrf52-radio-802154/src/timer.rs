use nrf52840_pac::{TIMER0, TIMER1};

pub trait Timer {
    fn init(&mut self);
    fn fire_at(&mut self, id: usize, at: u32);
    fn stop(&mut self, id: usize);
    fn now(&self) -> u32;
    fn ack_compare_event(&mut self, id: usize);
}

macro_rules! impl_timer {
    ($ty:ident) => {
        impl Timer for $ty {
            fn init(&mut self) {
                // tick resolution is 1 us
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
                        self.intenset.write(|w| w.compare1().set());
                    }
                    2 => {
                        self.intenset.write(|w| w.compare2().set());
                    }
                    3 => {
                        self.intenset.write(|w| w.compare3().set());
                    }
                    4 => {
                        self.intenset.write(|w| w.compare4().set());
                    }
                    5 => {
                        self.intenset.write(|w| w.compare5().set());
                    }
                    _ => (),
                }
            }

            fn stop(&mut self, id: usize) {
                assert!(id > 0 && id <= 5);
                match id {
                    1 => {
                        self.intenclr.write(|w| w.compare1().clear());
                    }
                    2 => {
                        self.intenclr.write(|w| w.compare2().clear());
                    }
                    3 => {
                        self.intenclr.write(|w| w.compare3().clear());
                    }
                    4 => {
                        self.intenclr.write(|w| w.compare4().clear());
                    }
                    5 => {
                        self.intenclr.write(|w| w.compare5().clear());
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
        }
    };
}

impl_timer!(TIMER0);
impl_timer!(TIMER1);
