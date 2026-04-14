use bytes::Bytes;

use crate::{channel::ChannelId, types::*};

use super::{envelope::Frame, error::FrameError};

/// Decoded message from any channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Typed control message on [`ChannelId::Control`].
    Control(ControlMessage),
    /// Typed input message on [`ChannelId::Input`].
    Input(InputMessage),
    /// Typed cursor message on [`ChannelId::Cursor`].
    Cursor(CursorMessage),
    /// Typed clipboard message on [`ChannelId::Clipboard`].
    Clipboard(ClipboardMessage),
    /// Typed file-transfer message on [`ChannelId::FileUp`].
    FileUp(FileMessage),
    /// Typed file-transfer message on [`ChannelId::FileDown`].
    FileDown(FileMessage),
    /// Typed tile-rendering message on [`ChannelId::Tiles`].
    Tiles(TileMessage),
    /// Raw payload bytes from [`ChannelId::Video`].
    Video(Bytes),
    /// Raw payload bytes from [`ChannelId::AudioOut`].
    AudioOut(Bytes),
    /// Raw payload bytes from [`ChannelId::AudioIn`].
    AudioIn(Bytes),
    /// Raw payload bytes from [`ChannelId::VideoIn`].
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
