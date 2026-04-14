use alloc::vec::Vec;

use crate::{channel::ChannelId, types::ClipboardMessage};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{decode_tagged, Writer},
};

const TEXT: u8 = 0x01;

impl ClipboardMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::Text { content } => {
                w.write_u8(TEXT);
                w.write_vec_u32(content);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        decode_tagged(buf, |tag, r| match tag {
            TEXT => Ok(Self::Text {
                content: r.read_vec_u32()?,
            }),
            _ => Err(FrameError::UnknownMessageType {
                channel: ChannelId::Clipboard.as_u8(),
                tag,
            }),
        })
    }

    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Clipboard, self.encode())
    }
}
