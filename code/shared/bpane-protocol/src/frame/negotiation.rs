//! Protocol negotiation message codecs and pure selection logic.

mod codec;
mod model;
mod selection;

pub use self::model::{
    ClientHello, NegotiationCodecError, NegotiationMessage, ProtocolCapability, ProtocolFailure,
    ServerSelection,
};
pub use self::selection::{ProtocolNegotiator, ProtocolSupport, ProtocolSupportError};

#[cfg(test)]
mod tests;
