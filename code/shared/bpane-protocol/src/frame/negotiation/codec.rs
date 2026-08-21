use alloc::vec::Vec;

use crate::channel::ChannelId;

use super::super::{
    envelope::Frame,
    io::{Reader, Writer},
};
use super::model::{
    ClientHello, NegotiationCodecError, NegotiationMessage, ProtocolFailure, ServerSelection,
    MAX_PROTOCOL_CAPABILITIES, MAX_PROTOCOL_VERSIONS,
};

const CLIENT_HELLO: u8 = 0x0A;
const SERVER_SELECTION: u8 = 0x0B;
const PROTOCOL_REJECT: u8 = 0x0C;
const MAX_CLIENT_HELLO_BYTES: usize = 148;
const MAX_SERVER_SELECTION_BYTES: usize = 132;
const PROTOCOL_REJECT_BYTES: usize = 3;

impl NegotiationMessage {
    /// Encode this negotiation control payload.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::ClientHello(hello) => hello.encode(),
            Self::ServerSelection(selection) => selection.encode(),
            Self::ProtocolReject(failure) => encode_reject(*failure),
        }
    }

    /// Decode one complete negotiation control payload.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free [`NegotiationCodecError`] for a
    /// malformed, mismatched, truncated, trailing, or unknown payload.
    pub fn decode(buf: &[u8]) -> Result<Self, NegotiationCodecError> {
        let tag = buf.first().copied().ok_or_else(unexpected_frame)?;
        match tag {
            CLIENT_HELLO => decode_client_hello(buf).map(Self::ClientHello),
            SERVER_SELECTION => decode_server_selection(buf).map(Self::ServerSelection),
            PROTOCOL_REJECT => decode_reject(buf).map(Self::ProtocolReject),
            _ => Err(unexpected_frame()),
        }
    }

    /// Wrap this payload in a reliable control-channel frame.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Control, self.encode())
    }
}

impl ClientHello {
    /// Encode this client hello control payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::with_capacity(
            4 + (self.versions().len()
                + self.required_capabilities().len()
                + self.optional_capabilities().len())
                * 2,
        );
        writer.write_u8(CLIENT_HELLO);
        write_list(&mut writer, self.versions());
        write_list(&mut writer, self.required_capabilities());
        write_list(&mut writer, self.optional_capabilities());
        writer.finish()
    }

    /// Decode one complete client hello control payload.
    ///
    /// # Errors
    ///
    /// Returns a stable codec error if the tag or hello is invalid.
    pub fn decode(buf: &[u8]) -> Result<Self, NegotiationCodecError> {
        match NegotiationMessage::decode(buf)? {
            NegotiationMessage::ClientHello(hello) => Ok(hello),
            _ => Err(unexpected_frame()),
        }
    }

    /// Wrap this hello in a reliable control-channel frame.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Control, self.encode())
    }
}

impl ServerSelection {
    /// Encode this server selection control payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::with_capacity(4 + self.capabilities().len() * 2);
        writer.write_u8(SERVER_SELECTION);
        writer.write_u16(self.selected_version());
        write_list(&mut writer, self.capabilities());
        writer.finish()
    }

    /// Decode one complete server selection control payload.
    ///
    /// # Errors
    ///
    /// Returns a stable codec error if the tag or selection is invalid.
    pub fn decode(buf: &[u8]) -> Result<Self, NegotiationCodecError> {
        match NegotiationMessage::decode(buf)? {
            NegotiationMessage::ServerSelection(selection) => Ok(selection),
            _ => Err(unexpected_frame()),
        }
    }

    /// Wrap this selection in a reliable control-channel frame.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Control, self.encode())
    }
}

fn decode_client_hello(buf: &[u8]) -> Result<ClientHello, NegotiationCodecError> {
    let failure = ProtocolFailure::MalformedProtocolHello;
    if buf.len() > MAX_CLIENT_HELLO_BYTES {
        return Err(NegotiationCodecError::new(failure));
    }
    let mut reader = Reader::new(&buf[1..]);
    let versions = read_list(&mut reader, MAX_PROTOCOL_VERSIONS, failure)?;
    if versions.is_empty() {
        return Err(NegotiationCodecError::new(failure));
    }
    let required = read_list(&mut reader, MAX_PROTOCOL_CAPABILITIES, failure)?;
    let optional_count = read_count(&mut reader, failure)?;
    if optional_count > MAX_PROTOCOL_CAPABILITIES
        || required.len() + optional_count > MAX_PROTOCOL_CAPABILITIES
    {
        return Err(NegotiationCodecError::new(failure));
    }
    let optional = read_values(&mut reader, optional_count, failure)?;
    finish(reader, failure)?;
    ClientHello::new(versions, required, optional)
}

fn decode_server_selection(buf: &[u8]) -> Result<ServerSelection, NegotiationCodecError> {
    let failure = ProtocolFailure::ProtocolSelectionMismatch;
    if buf.len() > MAX_SERVER_SELECTION_BYTES {
        return Err(NegotiationCodecError::new(failure));
    }
    let mut reader = Reader::new(&buf[1..]);
    let selected_version = reader
        .read_u16()
        .map_err(|_| NegotiationCodecError::new(failure))?;
    let capabilities = read_list(&mut reader, MAX_PROTOCOL_CAPABILITIES, failure)?;
    finish(reader, failure)?;
    ServerSelection::new(selected_version, capabilities)
}

fn decode_reject(buf: &[u8]) -> Result<ProtocolFailure, NegotiationCodecError> {
    if buf.len() != PROTOCOL_REJECT_BYTES {
        return Err(unexpected_frame());
    }
    let value = u16::from_le_bytes([buf[1], buf[2]]);
    ProtocolFailure::try_from(value).map_err(|_| unexpected_frame())
}

fn encode_reject(failure: ProtocolFailure) -> Vec<u8> {
    let mut writer = Writer::with_capacity(PROTOCOL_REJECT_BYTES);
    writer.write_u8(PROTOCOL_REJECT);
    writer.write_u16(failure.id());
    writer.finish()
}

fn write_list(writer: &mut Writer, values: &[u16]) {
    writer.write_u8(values.len() as u8);
    for value in values {
        writer.write_u16(*value);
    }
}

fn read_list(
    reader: &mut Reader<'_>,
    maximum: usize,
    failure: ProtocolFailure,
) -> Result<Vec<u16>, NegotiationCodecError> {
    let count = read_count(reader, failure)?;
    if count > maximum {
        return Err(NegotiationCodecError::new(failure));
    }
    read_values(reader, count, failure)
}

fn read_count(
    reader: &mut Reader<'_>,
    failure: ProtocolFailure,
) -> Result<usize, NegotiationCodecError> {
    reader
        .read_u8()
        .map(usize::from)
        .map_err(|_| NegotiationCodecError::new(failure))
}

fn read_values(
    reader: &mut Reader<'_>,
    count: usize,
    failure: ProtocolFailure,
) -> Result<Vec<u16>, NegotiationCodecError> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(
            reader
                .read_u16()
                .map_err(|_| NegotiationCodecError::new(failure))?,
        );
    }
    Ok(values)
}

fn finish(reader: Reader<'_>, failure: ProtocolFailure) -> Result<(), NegotiationCodecError> {
    reader
        .finish(())
        .map_err(|_| NegotiationCodecError::new(failure))
}

const fn unexpected_frame() -> NegotiationCodecError {
    NegotiationCodecError::new(ProtocolFailure::UnexpectedProtocolFrame)
}
