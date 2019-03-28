/// Signals an error that occured while processing data
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Buffer does not contain enough bytes
    NotEnoughBytes,
    /// Last end marker not found
    EndNotFound,
    /// A escape sequence is invalid
    InvalidEscapeSequence,
    /// The length check failed
    InvalidLength(usize),
}
