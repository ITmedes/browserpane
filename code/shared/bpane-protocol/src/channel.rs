/// Channel identifiers for the BrowserPane wire protocol.
/// Each logical channel maps to its own WebTransport stream or datagram channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChannelId {
    /// Encoded H.264 NAL units (datagrams, loss-tolerant). S->C.
    Video = 0x01,
    /// Codec-tagged audio frames, desktop audio. Reliable stream S->C.
    AudioOut = 0x02,
    /// Codec-tagged audio frames, microphone. Reliable stream C->S.
    AudioIn = 0x03,
    /// H.264 access units, webcam. Reliable stream C->S.
    VideoIn = 0x04,
    /// Mouse, keyboard, scroll events. Reliable stream C->S.
    Input = 0x05,
    /// Cursor shape + position. Reliable stream S->C.
    Cursor = 0x06,
    /// Clipboard sync. Bidirectional reliable stream.
    Clipboard = 0x07,
    /// File upload chunks. Reliable stream C->S.
    FileUp = 0x08,
    /// File download chunks. Reliable stream S->C.
    FileDown = 0x09,
    /// Session control, resize, ping. Bidirectional reliable stream.
    Control = 0x0A,
    /// Tile-based rendering commands. Reliable stream S->C.
    Tiles = 0x0B,
}

impl TryFrom<u8> for ChannelId {
    type Error = u8;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0x01 => Ok(Self::Video),
            0x02 => Ok(Self::AudioOut),
            0x03 => Ok(Self::AudioIn),
            0x04 => Ok(Self::VideoIn),
            0x05 => Ok(Self::Input),
            0x06 => Ok(Self::Cursor),
            0x07 => Ok(Self::Clipboard),
            0x08 => Ok(Self::FileUp),
            0x09 => Ok(Self::FileDown),
            0x0A => Ok(Self::Control),
            0x0B => Ok(Self::Tiles),
            _ => Err(val),
        }
    }
}

impl From<ChannelId> for u8 {
    fn from(channel: ChannelId) -> Self {
        channel as u8
    }
}

impl ChannelId {
    /// Convert a raw wire value into a channel identifier.
    ///
    /// Returns `None` for unknown or reserved channel IDs.
    pub fn from_u8(val: u8) -> Option<Self> {
        Self::try_from(val).ok()
    }

    /// Return the raw wire value for this channel.
    pub fn as_u8(self) -> u8 {
        self.into()
    }

    /// Whether this channel uses datagrams (loss-tolerant) vs reliable streams.
    pub fn is_datagram(self) -> bool {
        matches!(self, Self::Video)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_round_trip() {
        let channels = [
            ChannelId::Video,
            ChannelId::AudioOut,
            ChannelId::AudioIn,
            ChannelId::VideoIn,
            ChannelId::Input,
            ChannelId::Cursor,
            ChannelId::Clipboard,
            ChannelId::FileUp,
            ChannelId::FileDown,
            ChannelId::Control,
            ChannelId::Tiles,
        ];
        for ch in channels {
            let val = ch.as_u8();
            let recovered = ChannelId::from_u8(val).unwrap();
            assert_eq!(ch, recovered);
        }
    }

    #[test]
    fn invalid_channel_returns_none() {
        assert!(ChannelId::from_u8(0x00).is_none());
        assert!(ChannelId::from_u8(0x0C).is_none());
        assert!(ChannelId::from_u8(0xFF).is_none());
    }

    #[test]
    fn try_from_invalid_channel_returns_original_value() {
        assert_eq!(ChannelId::try_from(0x00), Err(0x00));
        assert_eq!(ChannelId::try_from(0xFF), Err(0xFF));
    }

    #[test]
    fn video_is_datagram() {
        assert!(ChannelId::Video.is_datagram());
        assert!(!ChannelId::Control.is_datagram());
        assert!(!ChannelId::Input.is_datagram());
    }
}
