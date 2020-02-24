//! Logging-related utilities

use core::{cell::RefCell, fmt};

use bbqueue::{ArrayLength, BBBuffer, ConstBBBuffer, Consumer, Producer};
use cortex_m::interrupt::{self, Mutex};
use log::{LevelFilter, Log, Metadata, Record};

use crate::timer::Timer;

use nrf52840_pac::TIMER0;

/// A `fmt::Write` adapter that prints a timestamp before each line.
pub struct TimeStampLogger<T: Timer, L: fmt::Write> {
    timer: T,
    inner: L,
}

impl<T: Timer, L: fmt::Write> TimeStampLogger<T, L> {
    /// Creates a new `StampedLogger` that will print to `inner` and obtains timestamps using
    /// `timer`.
    pub fn new(inner: L, timer: T) -> Self {
        Self { inner, timer }
    }
}

impl<T: Timer, L: fmt::Write> fmt::Write for TimeStampLogger<T, L> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if i != 0 {
                write!(self.inner, "\n{:10.10} - ", self.timer.now())?;
            }

            self.inner.write_str(line)?;
        }
        Ok(())
    }
}

/// A `fmt::Write` sink that writes to a `BBQueue`.
///
/// The sink will panic when the `BBQueue` doesn't have enough space to the data. This is to ensure
/// that we never block or drop data.
pub struct BbqLogger<'a, N: ArrayLength<u8>> {
    p: Producer<'a, N>,
}

impl<'a, N: ArrayLength<u8>> BbqLogger<'a, N> {
    pub fn new(p: Producer<'a, N>) -> Self {
        Self { p }
    }
}

impl<'a, N: ArrayLength<u8>> fmt::Write for BbqLogger<'a, N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        match self.p.grant_exact(bytes.len()) {
            Ok(mut grant) => {
                grant.buf().copy_from_slice(&bytes);
                grant.commit(bytes.len());
            }
            Err(_) => {
                panic!("log buffer overflow: failed to grant {} bytes", bytes.len());
            }
        }
        Ok(())
    }
}

/// Wraps a `fmt::Write` implementor and forwards the `log` crates logging macros to it.
///
/// The inner `fmt::Write` is made `Sync` by wrapping it in a `Mutex` from the `cortex_m` crate.
pub struct WriteLogger<W: fmt::Write + Send> {
    writer: Mutex<RefCell<W>>,
}

impl<W: fmt::Write + Send> WriteLogger<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(RefCell::new(writer)),
        }
    }
}

impl<W: fmt::Write + Send> Log for WriteLogger<W> {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            interrupt::free(|cs| {
                let mut writer = self.writer.borrow(cs).borrow_mut();
                writeln!(writer, "{} - {}", record.level(), record.args()).unwrap();
            })
        }
    }

    fn flush(&self) {}
}

pub use bbqueue::consts::U1024 as LogBufferSize;

static mut LOGGER: Option<WriteLogger<TimeStampLogger<TIMER0, BbqLogger<'static, LogBufferSize>>>> =
    None;

static BUFFER: BBBuffer<LogBufferSize> = BBBuffer(ConstBBBuffer::new());

pub fn init(timer: TIMER0) -> Consumer<'static, LogBufferSize> {
    let (producer, consumer) = BUFFER.try_split().unwrap();
    let logger = TimeStampLogger::new(BbqLogger::new(producer), timer);

    let log = WriteLogger::new(logger);
    interrupt::free(|_| unsafe {
        LOGGER = Some(log);
        log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
    });
    log::set_max_level(LevelFilter::max());

    consumer
}
