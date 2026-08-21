//! Frame envelope handling plus per-channel codecs.
//!
//! Most callers will use [`Frame`] for transport boundaries and
//! [`Message::from_frame`] to decode current runtime channels. Negotiation is
//! intentionally separate in [`NegotiationMessage`] until connection-state
//! integration is implemented.

mod audio;
mod clipboard;
mod control;
mod cursor;
mod decoder;
mod envelope;
mod error;
mod file_transfer;
mod input;
mod io;
mod message;
mod negotiation;
mod tile;
mod video;

pub use self::decoder::{FrameDecoder, FrameDecoderError};
pub use self::envelope::{Frame, FRAME_HEADER_SIZE};
pub use self::error::FrameError;
pub use self::message::Message;
pub use self::negotiation::{
    ClientHello, NegotiationCodecError, NegotiationMessage, ProtocolCapability, ProtocolFailure,
    ProtocolNegotiator, ProtocolSupport, ProtocolSupportError, ServerSelection,
};

#[cfg(test)]
mod tests;
