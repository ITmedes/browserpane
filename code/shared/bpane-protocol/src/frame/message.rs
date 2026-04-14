use bytes::Bytes;

use crate::{channel::ChannelId, types::*};

use super::{envelope::Frame, error::FrameError};

/// Decoded message from any channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Control(ControlMessage),
    Input(InputMessage),
    Cursor(CursorMessage),
    Clipboard(ClipboardMessage),
    FileUp(FileMessage),
    FileDown(FileMessage),
    Tiles(TileMessage),
    Video(Bytes),
    AudioOut(Bytes),
    AudioIn(Bytes),
    VideoIn(Bytes),
}

impl Message {
    /// Decode a frame payload according to its channel.
    ///
    /// Typed channels are parsed into their corresponding message enums. Raw
    /// media channels are returned as payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if a typed payload is malformed for the frame's
    /// channel.
    pub fn from_frame(frame: &Frame) -> Result<Self, FrameError> {
        match frame.channel {
            ChannelId::Control => Ok(Self::Control(ControlMessage::decode(&frame.payload)?)),
            ChannelId::Input => Ok(Self::Input(InputMessage::decode(&frame.payload)?)),
            ChannelId::Cursor => Ok(Self::Cursor(CursorMessage::decode(&frame.payload)?)),
            ChannelId::Clipboard => Ok(Self::Clipboard(ClipboardMessage::decode(&frame.payload)?)),
            ChannelId::FileUp => Ok(Self::FileUp(FileMessage::decode_for_channel(
                &frame.payload,
                ChannelId::FileUp,
            )?)),
            ChannelId::FileDown => Ok(Self::FileDown(FileMessage::decode_for_channel(
                &frame.payload,
                ChannelId::FileDown,
            )?)),
            ChannelId::Tiles => Ok(Self::Tiles(TileMessage::decode(&frame.payload)?)),
            ChannelId::Video => Ok(Self::Video(frame.payload.clone())),
            ChannelId::AudioOut => Ok(Self::AudioOut(frame.payload.clone())),
            ChannelId::AudioIn => Ok(Self::AudioIn(frame.payload.clone())),
            ChannelId::VideoIn => Ok(Self::VideoIn(frame.payload.clone())),
        }
    }
}
