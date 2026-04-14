use alloc::vec::Vec;

use bytes::{BufMut, Bytes, BytesMut};

use crate::channel::ChannelId;

use super::error::FrameError;

/// Maximum payload size: 16 MiB.
pub(crate) const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// Minimum frame header size: 1 (channel) + 4 (length) = 5 bytes.
pub const FRAME_HEADER_SIZE: usize = 5;

/// A framed message with channel ID and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel: ChannelId,
    pub payload: Bytes,
}

struct FrameHeader {
    channel: ChannelId,
    total_size: usize,
}

impl FrameHeader {
    fn parse(buf: &[u8]) -> Result<Self, FrameError> {
        if buf.len() < FRAME_HEADER_SIZE {
            return Err(FrameError::BufferTooShort {
                expected: FRAME_HEADER_SIZE,
                available: buf.len(),
            });
        }

        let channel_byte = buf[0];
        let channel =
            ChannelId::from_u8(channel_byte).ok_or(FrameError::UnknownChannel(channel_byte))?;
        let length = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        if length > MAX_PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooLarge(length));
        }

        let total_size = FRAME_HEADER_SIZE + length as usize;
        if buf.len() < total_size {
            return Err(FrameError::BufferTooShort {
                expected: total_size,
                available: buf.len(),
            });
        }

        Ok(Self {
            channel,
            total_size,
        })
    }
}

impl Frame {
    /// Create a frame from a channel and payload.
    pub fn new(channel: ChannelId, payload: impl Into<Bytes>) -> Self {
        Self {
            channel,
            payload: payload.into(),
        }
    }

    /// Encode the frame into wire format bytes.
    ///
    /// # Panics
    /// Panics if the payload exceeds the 16 MiB protocol limit.
    pub fn encode(&self) -> Bytes {
        let payload_len = self.payload.len();
        assert!(
            payload_len <= MAX_PAYLOAD_SIZE as usize,
            "frame payload too large: {payload_len} bytes (max {MAX_PAYLOAD_SIZE})"
        );

        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + payload_len);
        buf.put_u8(self.channel.as_u8());
        buf.put_slice(&(payload_len as u32).to_le_bytes());
        buf.put_slice(&self.payload);
        buf.freeze()
    }

    /// Decode a frame from wire format bytes, copying the payload.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if the header is truncated, the channel is
    /// unknown, the declared payload exceeds the protocol limit, or the
    /// declared payload is not fully present in `buf`.
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), FrameError> {
        let header = FrameHeader::parse(buf)?;
        let payload = Bytes::copy_from_slice(&buf[FRAME_HEADER_SIZE..header.total_size]);
        Ok((Frame::new(header.channel, payload), header.total_size))
    }

    /// Decode a frame from a [`Bytes`] buffer with zero-copy payload slicing.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::decode`].
    pub fn decode_bytes(buf: Bytes) -> Result<(Self, usize), FrameError> {
        let header = FrameHeader::parse(&buf)?;
        let payload = buf.slice(FRAME_HEADER_SIZE..header.total_size);
        Ok((Frame::new(header.channel, payload), header.total_size))
    }

    /// Decode as many complete frames as possible from a buffer.
    ///
    /// Incomplete trailing data is left unconsumed and reported via the
    /// returned byte count.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] for malformed complete frames. Incomplete trailing
    /// bytes do not produce an error.
    pub fn decode_all(buf: &[u8]) -> Result<(Vec<Frame>, usize), FrameError> {
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset < buf.len() {
            if buf.len() - offset < FRAME_HEADER_SIZE {
                break;
            }
            match Self::decode(&buf[offset..]) {
                Ok((frame, consumed)) => {
                    frames.push(frame);
                    offset += consumed;
                }
                Err(FrameError::BufferTooShort { .. }) => break,
                Err(err) => return Err(err),
            }
        }
        Ok((frames, offset))
    }
}
