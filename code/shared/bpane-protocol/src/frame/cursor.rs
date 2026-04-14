use alloc::vec::Vec;

use crate::{channel::ChannelId, types::CursorMessage};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{decode_tagged, Writer},
};

const CURSOR_MOVE: u8 = 0x01;
const CURSOR_SHAPE: u8 = 0x02;

impl CursorMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::CursorMove { x, y } => {
                w.write_u8(CURSOR_MOVE);
                w.write_u16(*x);
                w.write_u16(*y);
            }
            Self::CursorShape {
                width,
                height,
                hotspot_x,
                hotspot_y,
                data,
            } => {
                w.write_u8(CURSOR_SHAPE);
                w.write_u16(*width);
                w.write_u16(*height);
                w.write_u8(*hotspot_x);
                w.write_u8(*hotspot_y);
                w.write_vec_u32(data);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        decode_tagged(buf, |tag, r| match tag {
            CURSOR_MOVE => Ok(Self::CursorMove {
                x: r.read_u16()?,
                y: r.read_u16()?,
            }),
            CURSOR_SHAPE => Ok(Self::CursorShape {
                width: r.read_u16()?,
                height: r.read_u16()?,
                hotspot_x: r.read_u8()?,
                hotspot_y: r.read_u8()?,
                data: r.read_vec_u32()?,
            }),
            _ => Err(FrameError::UnknownMessageType {
                channel: ChannelId::Cursor.as_u8(),
                tag,
            }),
        })
    }

    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Cursor, self.encode())
    }
}
