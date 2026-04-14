use alloc::vec::Vec;
use core::fmt;

use bytes::BytesMut;

use super::{
    envelope::{Frame, FrameHeader, FRAME_HEADER_SIZE, MAX_PAYLOAD_SIZE},
    error::FrameError,
};

/// Default maximum buffered bytes for incremental decoding.
///
/// This allows one complete max-size protocol frame plus its header.
pub const DEFAULT_MAX_PENDING_BYTES: usize = MAX_PAYLOAD_SIZE as usize + FRAME_HEADER_SIZE;

/// Errors produced by [`FrameDecoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameDecoderError {
    Frame(FrameError),
    PendingTooLarge { pending: usize, max_pending: usize },
}

impl fmt::Display for FrameDecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(err) => err.fmt(f),
            Self::PendingTooLarge {
                pending,
                max_pending,
            } => write!(
                f,
                "pending frame buffer too large: {pending} bytes (max {max_pending})"
            ),
        }
    }
}

impl From<FrameError> for FrameDecoderError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameDecoderError {}

/// Incremental decoder for reliable frame streams.
///
/// Feed arbitrary byte chunks into the decoder and pull complete [`Frame`]s
/// out as they become available.
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    pending: BytesMut,
    max_pending: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    /// Create a decoder with the default pending-buffer limit.
    pub fn new() -> Self {
        Self::with_max_pending(DEFAULT_MAX_PENDING_BYTES)
    }

    /// Create a decoder with a caller-defined pending-buffer limit.
    ///
    /// # Panics
    ///
    /// Panics if `max_pending == 0`.
    pub fn with_max_pending(max_pending: usize) -> Self {
        assert!(max_pending > 0, "max_pending must be > 0");
        Self {
            pending: BytesMut::new(),
            max_pending,
        }
    }

    /// Append more bytes from the transport.
    ///
    /// # Errors
    ///
    /// Returns [`FrameDecoderError::PendingTooLarge`] if the buffered bytes
    /// exceed the configured pending limit.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), FrameDecoderError> {
        if chunk.is_empty() {
            return Ok(());
        }

        self.pending.extend_from_slice(chunk);
        self.ensure_pending_limit()
    }

    /// Return the number of bytes currently buffered but not yet emitted.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Decode and return the next complete frame, if one is available.
    ///
    /// # Errors
    ///
    /// Returns [`FrameDecoderError`] if the buffered bytes contain a malformed
    /// frame header/payload or if the declared frame size exceeds the decoder's
    /// pending limit.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, FrameDecoderError> {
        let Some(header) = FrameHeader::parse_prefix(&self.pending)? else {
            return Ok(None);
        };
        if header.total_size > self.max_pending {
            return Err(FrameDecoderError::PendingTooLarge {
                pending: header.total_size,
                max_pending: self.max_pending,
            });
        }
        let header = match header.require_complete(self.pending.len()) {
            Ok(header) => header,
            Err(FrameError::BufferTooShort { .. }) => return Ok(None),
            Err(err) => return Err(FrameDecoderError::Frame(err)),
        };

        let frame_bytes = self.pending.split_to(header.total_size).freeze();
        let (frame, _) = Frame::decode_bytes(frame_bytes)?;
        Ok(Some(frame))
    }

    /// Decode all complete frames currently buffered.
    ///
    /// # Errors
    ///
    /// Returns the first error that [`Self::next_frame`] would report.
    pub fn drain_frames(&mut self) -> Result<Vec<Frame>, FrameDecoderError> {
        let mut frames = Vec::new();
        while let Some(frame) = self.next_frame()? {
            frames.push(frame);
        }
        Ok(frames)
    }

    fn ensure_pending_limit(&self) -> Result<(), FrameDecoderError> {
        if self.pending.len() > self.max_pending {
            return Err(FrameDecoderError::PendingTooLarge {
                pending: self.pending.len(),
                max_pending: self.max_pending,
            });
        }
        Ok(())
    }
}
