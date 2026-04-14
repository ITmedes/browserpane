use core::fmt;

/// Errors during frame encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Not enough data to read the expected field.
    BufferTooShort { expected: usize, available: usize },
    /// Unknown channel ID.
    UnknownChannel(u8),
    /// Unknown message type tag within a channel.
    UnknownMessageType { channel: u8, tag: u8 },
    /// Field contained an invalid enum value or unsupported discriminator.
    InvalidFieldValue { field: &'static str, value: u64 },
    /// Payload exceeds maximum allowed size.
    PayloadTooLarge(u32),
    /// Data remaining after parsing.
    TrailingData(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort {
                expected,
                available,
            } => write!(f, "buffer too short: need {expected}, have {available}"),
            Self::UnknownChannel(ch) => write!(f, "unknown channel: 0x{ch:02x}"),
            Self::UnknownMessageType { channel, tag } => {
                write!(
                    f,
                    "unknown message type 0x{tag:02x} on channel 0x{channel:02x}"
                )
            }
            Self::InvalidFieldValue { field, value } => {
                write!(f, "invalid {field}: {value}")
            }
            Self::PayloadTooLarge(size) => write!(f, "payload too large: {size} bytes"),
            Self::TrailingData(n) => write!(f, "{n} trailing bytes after message"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}
