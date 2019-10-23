//! Logging-related utilities and adapters.

use {
    bbqueue::{BBQueue, bbq, Producer, Consumer},
    core::{cell::RefCell, fmt},
    cortex_m::interrupt::{self, Mutex},
    log::{Log, Metadata, Record, LevelFilter},
};

/// A `fmt::Write` sink that writes to a `BBQueue`.
///
/// The sink will panic when the `BBQueue` doesn't have enough space to the data. This is to ensure
/// that we never block or drop data.
pub struct BbqLogger {
    p: Producer,
}

impl BbqLogger {
    pub fn new(p: Producer) -> Self {
        Self { p }
    }
}

impl fmt::Write for BbqLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut bytes = s.as_bytes();

        while !bytes.is_empty() {
            let mut grant = match self.p.grant_max(bytes.len()) {
                Ok(grant) => grant,
                Err(_) => {
                    let cap = self.p.capacity();
                    let max_len = self
                        .p
                        .grant_max(cap)
                        .map(|mut g| g.buf().len())
                        .unwrap_or(0);
                    panic!(
                        "log buffer overflow: failed to grant {} Bytes ({} available)",
                        bytes.len(),
                        max_len
                    );
                }
            };
            let size = grant.buf().len();
            grant.buf().copy_from_slice(&bytes[..size]);
            bytes = &bytes[size..];
            self.p.commit(size, grant);
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

/// Stores the global logger used by the `log` crate.
static mut LOGGER: Option<WriteLogger<BbqLogger>> = None;

pub fn init() -> Consumer {
    let (tx, log_sink) = bbq![1024].unwrap().split();
    let logger = BbqLogger::new(tx);

    let log = WriteLogger::new(logger);
    interrupt::free(|_| unsafe {
        // Safe, since we're the only thread and interrupts are off
        LOGGER = Some(log);
        log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
    });
    log::set_max_level(LevelFilter::max());

    log::info!("Logger ready");

    log_sink
}
