use alloc::vec::Vec;

use crate::{channel::ChannelId, types::InputMessage};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{decode_tagged, Writer},
};

const MOUSE_MOVE: u8 = 0x01;
const MOUSE_BUTTON: u8 = 0x02;
const MOUSE_SCROLL: u8 = 0x03;
const KEY_EVENT: u8 = 0x04;
const KEY_EVENT_EX: u8 = 0x05;

impl InputMessage {
    /// Encode an input message payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::MouseMove { x, y } => {
                w.write_u8(MOUSE_MOVE);
                w.write_u16(*x);
                w.write_u16(*y);
            }
            Self::MouseButton { button, down, x, y } => {
                w.write_u8(MOUSE_BUTTON);
                w.write_u8(*button);
                w.write_bool(*down);
                w.write_u16(*x);
                w.write_u16(*y);
            }
            Self::MouseScroll { dx, dy } => {
                w.write_u8(MOUSE_SCROLL);
                w.write_i16(*dx);
                w.write_i16(*dy);
            }
            Self::KeyEvent {
                keycode,
                down,
                modifiers,
            } => {
                w.write_u8(KEY_EVENT);
                w.write_u32(*keycode);
                w.write_bool(*down);
                w.write_u8(*modifiers);
            }
            Self::KeyEventEx {
                keycode,
                down,
                modifiers,
                key_char,
            } => {
                w.write_u8(KEY_EVENT_EX);
                w.write_u32(*keycode);
                w.write_bool(*down);
                w.write_u8(*modifiers);
                w.write_u32(*key_char);
            }
        }
        w.finish()
    }

    /// Decode an input message payload.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if the payload is truncated, has an unknown
    /// input tag, or contains trailing bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        decode_tagged(buf, |tag, r| match tag {
            MOUSE_MOVE => Ok(Self::MouseMove {
                x: r.read_u16()?,
                y: r.read_u16()?,
            }),
            MOUSE_BUTTON => Ok(Self::MouseButton {
                button: r.read_u8()?,
                down: r.read_bool()?,
                x: r.read_u16()?,
                y: r.read_u16()?,
            }),
            MOUSE_SCROLL => Ok(Self::MouseScroll {
                dx: r.read_i16()?,
                dy: r.read_i16()?,
            }),
            KEY_EVENT => Ok(Self::KeyEvent {
                keycode: r.read_u32()?,
                down: r.read_bool()?,
                modifiers: r.read_u8()?,
            }),
            KEY_EVENT_EX => Ok(Self::KeyEventEx {
                keycode: r.read_u32()?,
                down: r.read_bool()?,
                modifiers: r.read_u8()?,
                key_char: r.read_u32()?,
            }),
            _ => Err(FrameError::UnknownMessageType {
                channel: ChannelId::Input.as_u8(),
                tag,
            }),
        })
    }

    /// Wrap this message in a frame on the input channel.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Input, self.encode())
    }
}
