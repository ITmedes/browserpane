use alloc::vec::Vec;

use crate::{
    channel::ChannelId,
    types::{ControlMessage, SessionFlags},
};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{decode_tagged, Writer},
};

const RESOLUTION_REQUEST: u8 = 0x01;
const RESOLUTION_ACK: u8 = 0x02;
const SESSION_READY: u8 = 0x03;
const PING: u8 = 0x04;
const PONG: u8 = 0x05;
const KEYBOARD_LAYOUT_INFO: u8 = 0x06;
const BITRATE_HINT: u8 = 0x07;
const RESOLUTION_LOCKED: u8 = 0x08;

impl ControlMessage {
    /// Encode a control message payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::ResolutionRequest { width, height } => {
                w.write_u8(RESOLUTION_REQUEST);
                w.write_u16(*width);
                w.write_u16(*height);
            }
            Self::ResolutionAck { width, height } => {
                w.write_u8(RESOLUTION_ACK);
                w.write_u16(*width);
                w.write_u16(*height);
            }
            Self::SessionReady { version, flags } => {
                w.write_u8(SESSION_READY);
                w.write_u8(*version);
                w.write_u8(flags.0);
            }
            Self::Ping { seq, timestamp_ms } | Self::Pong { seq, timestamp_ms } => {
                w.write_u8(if matches!(self, Self::Ping { .. }) {
                    PING
                } else {
                    PONG
                });
                w.write_u32(*seq);
                w.write_u64(*timestamp_ms);
            }
            Self::KeyboardLayoutInfo { layout_hint } => {
                w.write_u8(KEYBOARD_LAYOUT_INFO);
                w.write_bytes(layout_hint);
            }
            Self::BitrateHint { target_bps } => {
                w.write_u8(BITRATE_HINT);
                w.write_u32(*target_bps);
            }
            Self::ResolutionLocked { width, height } => {
                w.write_u8(RESOLUTION_LOCKED);
                w.write_u16(*width);
                w.write_u16(*height);
            }
        }
        w.finish()
    }

    /// Decode a control message payload.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if the payload is truncated, has an unknown
    /// control tag, or contains trailing bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        decode_tagged(buf, |tag, r| match tag {
            RESOLUTION_REQUEST => Ok(Self::ResolutionRequest {
                width: r.read_u16()?,
                height: r.read_u16()?,
            }),
            RESOLUTION_ACK => Ok(Self::ResolutionAck {
                width: r.read_u16()?,
                height: r.read_u16()?,
            }),
            SESSION_READY => Ok(Self::SessionReady {
                version: r.read_u8()?,
                flags: SessionFlags(r.read_u8()?),
            }),
            PING => Ok(Self::Ping {
                seq: r.read_u32()?,
                timestamp_ms: r.read_u64()?,
            }),
            PONG => Ok(Self::Pong {
                seq: r.read_u32()?,
                timestamp_ms: r.read_u64()?,
            }),
            KEYBOARD_LAYOUT_INFO => Ok(Self::KeyboardLayoutInfo {
                layout_hint: r.read_fixed_array::<32>()?,
            }),
            BITRATE_HINT => Ok(Self::BitrateHint {
                target_bps: r.read_u32()?,
            }),
            RESOLUTION_LOCKED => Ok(Self::ResolutionLocked {
                width: r.read_u16()?,
                height: r.read_u16()?,
            }),
            _ => Err(FrameError::UnknownMessageType {
                channel: ChannelId::Control.as_u8(),
                tag,
            }),
        })
    }

    /// Wrap this message in a frame on the control channel.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Control, self.encode())
    }
}
