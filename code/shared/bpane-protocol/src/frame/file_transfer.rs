use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::{channel::ChannelId, types::FileMessage};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{decode_tagged, Writer},
};

const FILE_HEADER: u8 = 0x01;
const FILE_CHUNK: u8 = 0x02;
const FILE_COMPLETE: u8 = 0x03;

impl FileMessage {
    /// Encode a file-transfer message payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::FileHeader {
                id,
                filename,
                size,
                mime,
            } => {
                w.write_u8(FILE_HEADER);
                w.write_u32(*id);
                w.write_bytes(filename.as_ref());
                w.write_u64(*size);
                w.write_bytes(mime.as_ref());
            }
            Self::FileChunk { id, seq, data } => {
                w.write_u8(FILE_CHUNK);
                w.write_u32(*id);
                w.write_u32(*seq);
                w.write_vec_u32(data);
            }
            Self::FileComplete { id } => {
                w.write_u8(FILE_COMPLETE);
                w.write_u32(*id);
            }
        }
        w.finish()
    }

    /// Decode a file-transfer payload for either file channel.
    ///
    /// Prefer [`super::message::Message::from_frame`] when the surrounding frame
    /// is available and channel-specific error reporting is useful.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if the payload is truncated, has an unknown tag,
    /// or contains trailing bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        Self::decode_on_channel(buf, ChannelId::FileDown)
    }

    /// Decode a file-transfer payload for a specific file channel.
    pub fn decode_on_channel(buf: &[u8], channel: ChannelId) -> Result<Self, FrameError> {
        if !matches!(channel, ChannelId::FileUp | ChannelId::FileDown) {
            return Err(FrameError::InvalidFieldValue {
                field: "file channel",
                value: u64::from(channel.as_u8()),
            });
        }

        decode_tagged(buf, |tag, r| match tag {
            FILE_HEADER => Ok(Self::FileHeader {
                id: r.read_u32()?,
                filename: Box::new(r.read_fixed_array::<256>()?),
                size: r.read_u64()?,
                mime: Box::new(r.read_fixed_array::<64>()?),
            }),
            FILE_CHUNK => Ok(Self::FileChunk {
                id: r.read_u32()?,
                seq: r.read_u32()?,
                data: r.read_vec_u32()?,
            }),
            FILE_COMPLETE => Ok(Self::FileComplete { id: r.read_u32()? }),
            _ => Err(FrameError::UnknownMessageType {
                channel: channel.as_u8(),
                tag,
            }),
        })
    }

    /// Wrap this payload in a frame on the chosen file channel.
    ///
    /// This method does not validate that `channel` is one of the two file
    /// channels; callers should pass either [`ChannelId::FileUp`] or
    /// [`ChannelId::FileDown`].
    pub fn to_frame(&self, channel: ChannelId) -> Frame {
        Frame::new(channel, self.encode())
    }
}
