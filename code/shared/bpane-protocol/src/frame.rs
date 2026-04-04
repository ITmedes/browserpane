use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use bytes::{BufMut, Bytes, BytesMut};

use crate::channel::ChannelId;
use crate::types::*;

/// Maximum payload size: 16 MiB.
const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// Errors during frame encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Not enough data to read the expected field.
    BufferTooShort { expected: usize, available: usize },
    /// Unknown channel ID.
    UnknownChannel(u8),
    /// Unknown message type tag within a channel.
    UnknownMessageType { channel: u8, tag: u8 },
    /// Payload exceeds maximum allowed size.
    PayloadTooLarge(u32),
    /// Data remaining after parsing.
    TrailingData(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort {
                expected,
                available,
            } => write!(f, "buffer too short: need {expected}, have {available}"),
            Self::UnknownChannel(ch) => write!(f, "unknown channel: 0x{ch:02x}"),
            Self::UnknownMessageType { channel, tag } => {
                write!(
                    f,
                    "unknown message type 0x{tag:02x} on channel 0x{channel:02x}"
                )
            }
            Self::PayloadTooLarge(size) => write!(f, "payload too large: {size} bytes"),
            Self::TrailingData(n) => write!(f, "{n} trailing bytes after message"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}

/// A framed message with channel ID and payload.
///
/// Wire format:
/// ```text
/// ┌─────────────┬──────────────┬─────────────────────┐
/// │ channel: u8 │ length: u32  │ payload: [u8; length]│
/// └─────────────┴──────────────┴─────────────────────┘
/// ```
/// All integers are little-endian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel: ChannelId,
    pub payload: Bytes,
}

/// Minimum frame header size: 1 (channel) + 4 (length) = 5 bytes.
pub const FRAME_HEADER_SIZE: usize = 5;

impl Frame {
    pub fn new(channel: ChannelId, payload: impl Into<Bytes>) -> Self {
        Self {
            channel,
            payload: payload.into(),
        }
    }

    /// Encode the frame into wire format bytes.
    ///
    /// # Panics
    /// Panics if the payload exceeds [`MAX_PAYLOAD_SIZE`] (16 MiB).
    pub fn encode(&self) -> Bytes {
        let payload_len = self.payload.len();
        assert!(
            payload_len <= MAX_PAYLOAD_SIZE as usize,
            "frame payload too large: {payload_len} bytes (max {MAX_PAYLOAD_SIZE})"
        );
        let len = payload_len as u32;
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + payload_len);
        buf.put_u8(self.channel.as_u8());
        buf.put_slice(&len.to_le_bytes());
        buf.put_slice(&self.payload);
        buf.freeze()
    }

    /// Decode a frame from wire format bytes (copies payload from slice).
    /// Returns the frame and the number of bytes consumed.
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), FrameError> {
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

        let payload = Bytes::copy_from_slice(&buf[FRAME_HEADER_SIZE..total_size]);
        Ok((Frame { channel, payload }, total_size))
    }

    /// Decode a frame from a `Bytes` buffer with zero-copy payload slicing.
    /// Returns the frame and the number of bytes consumed.
    pub fn decode_bytes(buf: Bytes) -> Result<(Self, usize), FrameError> {
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

        let payload = buf.slice(FRAME_HEADER_SIZE..total_size);
        Ok((Frame { channel, payload }, total_size))
    }

    /// Try to decode multiple frames from a buffer.
    /// Returns all successfully decoded frames and the total bytes consumed.
    pub fn decode_all(buf: &[u8]) -> Result<(Vec<Frame>, usize), FrameError> {
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset < buf.len() {
            if buf.len() - offset < FRAME_HEADER_SIZE {
                break;
            }
            match Frame::decode(&buf[offset..]) {
                Ok((frame, consumed)) => {
                    frames.push(frame);
                    offset += consumed;
                }
                Err(FrameError::BufferTooShort { .. }) => break,
                Err(e) => return Err(e),
            }
        }
        Ok((frames, offset))
    }
}

// ── Message-type tags ───────────────────────────────────────────────

// Control channel message tags
const CTRL_RESOLUTION_REQUEST: u8 = 0x01;
const CTRL_RESOLUTION_ACK: u8 = 0x02;
const CTRL_SESSION_READY: u8 = 0x03;
const CTRL_PING: u8 = 0x04;
const CTRL_PONG: u8 = 0x05;
const CTRL_KEYBOARD_LAYOUT_INFO: u8 = 0x06;
const CTRL_BITRATE_HINT: u8 = 0x07;
const CTRL_RESOLUTION_LOCKED: u8 = 0x08;

// Input channel message tags
const INPUT_MOUSE_MOVE: u8 = 0x01;
const INPUT_MOUSE_BUTTON: u8 = 0x02;
const INPUT_MOUSE_SCROLL: u8 = 0x03;
const INPUT_KEY_EVENT: u8 = 0x04;
const INPUT_KEY_EVENT_EX: u8 = 0x05;

// Cursor channel message tags
const CURSOR_MOVE: u8 = 0x01;
const CURSOR_SHAPE: u8 = 0x02;

// Clipboard channel message tags
const CLIPBOARD_TEXT: u8 = 0x01;

// File channel message tags
const FILE_HEADER: u8 = 0x01;
const FILE_CHUNK: u8 = 0x02;
const FILE_COMPLETE: u8 = 0x03;

// Tile channel message tags
const TILE_GRID_CONFIG: u8 = 0x01;
const TILE_CACHE_HIT: u8 = 0x02;
const TILE_FILL: u8 = 0x03;
const TILE_QOI: u8 = 0x04;
const TILE_VIDEO_REGION: u8 = 0x05;
const TILE_BATCH_END: u8 = 0x06;
const TILE_SCROLL_COPY: u8 = 0x07;
const TILE_GRID_OFFSET: u8 = 0x08;
const TILE_CACHE_MISS: u8 = 0x09;
const TILE_SCROLL_STATS: u8 = 0x0A;
const TILE_DRAW_MODE: u8 = 0x0B;
const TILE_ZSTD: u8 = 0x0C;

// ── Read helpers ────────────────────────────────────────────────────

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn read_u8(&mut self) -> Result<u8, FrameError> {
        if self.remaining() < 1 {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + 1,
                available: self.buf.len(),
            });
        }
        let val = self.buf[self.pos];
        self.pos += 1;
        Ok(val)
    }

    fn read_u16(&mut self) -> Result<u16, FrameError> {
        if self.remaining() < 2 {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + 2,
                available: self.buf.len(),
            });
        }
        let val = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(val)
    }

    fn read_i16(&mut self) -> Result<i16, FrameError> {
        if self.remaining() < 2 {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + 2,
                available: self.buf.len(),
            });
        }
        let val = i16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(val)
    }

    fn read_u32(&mut self) -> Result<u32, FrameError> {
        if self.remaining() < 4 {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + 4,
                available: self.buf.len(),
            });
        }
        let val = u32::from_le_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(val)
    }

    fn read_u64(&mut self) -> Result<u64, FrameError> {
        if self.remaining() < 8 {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + 8,
                available: self.buf.len(),
            });
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.buf[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_bool(&mut self) -> Result<bool, FrameError> {
        Ok(self.read_u8()? != 0)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], FrameError> {
        if self.remaining() < len {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + len,
                available: self.buf.len(),
            });
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_fixed_array<const N: usize>(&mut self) -> Result<[u8; N], FrameError> {
        if self.remaining() < N {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + N,
                available: self.buf.len(),
            });
        }
        let mut arr = [0u8; N];
        arr.copy_from_slice(&self.buf[self.pos..self.pos + N]);
        self.pos += N;
        Ok(arr)
    }
}

// ── Write helpers ───────────────────────────────────────────────────

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    fn write_u8(&mut self, val: u8) {
        self.buf.push(val);
    }

    fn write_u16(&mut self, val: u16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    fn write_i16(&mut self, val: i16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    fn write_u32(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    fn write_u64(&mut self, val: u64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    fn write_bool(&mut self, val: bool) {
        self.buf.push(if val { 1 } else { 0 });
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

// ── Control message encode/decode ───────────────────────────────────

impl ControlMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::ResolutionRequest { width, height } => {
                w.write_u8(CTRL_RESOLUTION_REQUEST);
                w.write_u16(*width);
                w.write_u16(*height);
            }
            Self::ResolutionAck { width, height } => {
                w.write_u8(CTRL_RESOLUTION_ACK);
                w.write_u16(*width);
                w.write_u16(*height);
            }
            Self::SessionReady { version, flags } => {
                w.write_u8(CTRL_SESSION_READY);
                w.write_u8(*version);
                w.write_u8(flags.0);
            }
            Self::Ping { seq, timestamp_ms } => {
                w.write_u8(CTRL_PING);
                w.write_u32(*seq);
                w.write_u64(*timestamp_ms);
            }
            Self::Pong { seq, timestamp_ms } => {
                w.write_u8(CTRL_PONG);
                w.write_u32(*seq);
                w.write_u64(*timestamp_ms);
            }
            Self::KeyboardLayoutInfo { layout_hint } => {
                w.write_u8(CTRL_KEYBOARD_LAYOUT_INFO);
                w.write_bytes(layout_hint);
            }
            Self::BitrateHint { target_bps } => {
                w.write_u8(CTRL_BITRATE_HINT);
                w.write_u32(*target_bps);
            }
            Self::ResolutionLocked { width, height } => {
                w.write_u8(CTRL_RESOLUTION_LOCKED);
                w.write_u16(*width);
                w.write_u16(*height);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        let msg = match tag {
            CTRL_RESOLUTION_REQUEST => {
                let width = r.read_u16()?;
                let height = r.read_u16()?;
                Self::ResolutionRequest { width, height }
            }
            CTRL_RESOLUTION_ACK => {
                let width = r.read_u16()?;
                let height = r.read_u16()?;
                Self::ResolutionAck { width, height }
            }
            CTRL_SESSION_READY => {
                let version = r.read_u8()?;
                let flags = SessionFlags(r.read_u8()?);
                Self::SessionReady { version, flags }
            }
            CTRL_PING => {
                let seq = r.read_u32()?;
                let timestamp_ms = r.read_u64()?;
                Self::Ping { seq, timestamp_ms }
            }
            CTRL_PONG => {
                let seq = r.read_u32()?;
                let timestamp_ms = r.read_u64()?;
                Self::Pong { seq, timestamp_ms }
            }
            CTRL_KEYBOARD_LAYOUT_INFO => {
                let layout_hint = r.read_fixed_array::<32>()?;
                Self::KeyboardLayoutInfo { layout_hint }
            }
            CTRL_BITRATE_HINT => {
                let target_bps = r.read_u32()?;
                Self::BitrateHint { target_bps }
            }
            CTRL_RESOLUTION_LOCKED => {
                let width = r.read_u16()?;
                let height = r.read_u16()?;
                Self::ResolutionLocked { width, height }
            }
            _ => {
                return Err(FrameError::UnknownMessageType {
                    channel: ChannelId::Control.as_u8(),
                    tag,
                })
            }
        };
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(msg)
    }

    /// Encode into a full wire frame.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Control, self.encode())
    }
}

// ── Input message encode/decode ─────────────────────────────────────

impl InputMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::MouseMove { x, y } => {
                w.write_u8(INPUT_MOUSE_MOVE);
                w.write_u16(*x);
                w.write_u16(*y);
            }
            Self::MouseButton { button, down, x, y } => {
                w.write_u8(INPUT_MOUSE_BUTTON);
                w.write_u8(*button);
                w.write_bool(*down);
                w.write_u16(*x);
                w.write_u16(*y);
            }
            Self::MouseScroll { dx, dy } => {
                w.write_u8(INPUT_MOUSE_SCROLL);
                w.write_i16(*dx);
                w.write_i16(*dy);
            }
            Self::KeyEvent {
                keycode,
                down,
                modifiers,
            } => {
                w.write_u8(INPUT_KEY_EVENT);
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
                w.write_u8(INPUT_KEY_EVENT_EX);
                w.write_u32(*keycode);
                w.write_bool(*down);
                w.write_u8(*modifiers);
                w.write_u32(*key_char);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        let msg = match tag {
            INPUT_MOUSE_MOVE => {
                let x = r.read_u16()?;
                let y = r.read_u16()?;
                Self::MouseMove { x, y }
            }
            INPUT_MOUSE_BUTTON => {
                let button = r.read_u8()?;
                let down = r.read_bool()?;
                let x = r.read_u16()?;
                let y = r.read_u16()?;
                Self::MouseButton { button, down, x, y }
            }
            INPUT_MOUSE_SCROLL => {
                let dx = r.read_i16()?;
                let dy = r.read_i16()?;
                Self::MouseScroll { dx, dy }
            }
            INPUT_KEY_EVENT => {
                let keycode = r.read_u32()?;
                let down = r.read_bool()?;
                let modifiers = r.read_u8()?;
                Self::KeyEvent {
                    keycode,
                    down,
                    modifiers,
                }
            }
            INPUT_KEY_EVENT_EX => {
                let keycode = r.read_u32()?;
                let down = r.read_bool()?;
                let modifiers = r.read_u8()?;
                let key_char = r.read_u32()?;
                Self::KeyEventEx {
                    keycode,
                    down,
                    modifiers,
                    key_char,
                }
            }
            _ => {
                return Err(FrameError::UnknownMessageType {
                    channel: ChannelId::Input.as_u8(),
                    tag,
                })
            }
        };
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(msg)
    }

    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Input, self.encode())
    }
}

// ── Cursor message encode/decode ────────────────────────────────────

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
                w.write_u32(data.len() as u32);
                w.write_bytes(data);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        let msg = match tag {
            CURSOR_MOVE => {
                let x = r.read_u16()?;
                let y = r.read_u16()?;
                Self::CursorMove { x, y }
            }
            CURSOR_SHAPE => {
                let width = r.read_u16()?;
                let height = r.read_u16()?;
                let hotspot_x = r.read_u8()?;
                let hotspot_y = r.read_u8()?;
                let data_len = r.read_u32()? as usize;
                let data = r.read_bytes(data_len)?.to_vec();
                Self::CursorShape {
                    width,
                    height,
                    hotspot_x,
                    hotspot_y,
                    data,
                }
            }
            _ => {
                return Err(FrameError::UnknownMessageType {
                    channel: ChannelId::Cursor.as_u8(),
                    tag,
                })
            }
        };
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(msg)
    }

    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Cursor, self.encode())
    }
}

// ── Clipboard message encode/decode ─────────────────────────────────

impl ClipboardMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::Text { content } => {
                w.write_u8(CLIPBOARD_TEXT);
                w.write_u32(content.len() as u32);
                w.write_bytes(content);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        let msg = match tag {
            CLIPBOARD_TEXT => {
                let len = r.read_u32()? as usize;
                let content = r.read_bytes(len)?.to_vec();
                Self::Text { content }
            }
            _ => {
                return Err(FrameError::UnknownMessageType {
                    channel: ChannelId::Clipboard.as_u8(),
                    tag,
                })
            }
        };
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(msg)
    }

    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Clipboard, self.encode())
    }
}

// ── File message encode/decode ──────────────────────────────────────

impl FileMessage {
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
                w.write_bytes(filename);
                w.write_u64(*size);
                w.write_bytes(mime);
            }
            Self::FileChunk { id, seq, data } => {
                w.write_u8(FILE_CHUNK);
                w.write_u32(*id);
                w.write_u32(*seq);
                w.write_u32(data.len() as u32);
                w.write_bytes(data);
            }
            Self::FileComplete { id } => {
                w.write_u8(FILE_COMPLETE);
                w.write_u32(*id);
            }
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        let msg = match tag {
            FILE_HEADER => {
                let id = r.read_u32()?;
                let filename = r.read_fixed_array::<256>()?;
                let size = r.read_u64()?;
                let mime = r.read_fixed_array::<64>()?;
                Self::FileHeader {
                    id,
                    filename,
                    size,
                    mime,
                }
            }
            FILE_CHUNK => {
                let id = r.read_u32()?;
                let seq = r.read_u32()?;
                let data_len = r.read_u32()? as usize;
                let data = r.read_bytes(data_len)?.to_vec();
                Self::FileChunk { id, seq, data }
            }
            FILE_COMPLETE => {
                let id = r.read_u32()?;
                Self::FileComplete { id }
            }
            _ => {
                return Err(FrameError::UnknownMessageType {
                    channel: ChannelId::FileDown.as_u8(),
                    tag,
                })
            }
        };
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(msg)
    }

    pub fn to_frame(&self, channel: ChannelId) -> Frame {
        Frame::new(channel, self.encode())
    }
}

// ── VideoDatagram encode/decode ─────────────────────────────────────

/// Flags byte for VideoDatagram extensions (follows data in wire format).
const VIDEO_FLAG_TILE: u8 = 0x01;

impl VideoDatagram {
    pub fn encode(&self) -> Vec<u8> {
        // Base header: nal_id(4) + fragment_seq(2) + fragment_total(2) + is_keyframe(1) + pts_us(8) + data_len(4) = 21 bytes
        // After data: flags(1) + optional tile_info(12)
        let tile_size = if self.tile_info.is_some() { 1 + 12 } else { 1 };
        let mut w = Writer::with_capacity(21 + self.data.len() + tile_size);
        w.write_u32(self.nal_id);
        w.write_u16(self.fragment_seq);
        w.write_u16(self.fragment_total);
        w.write_bool(self.is_keyframe);
        w.write_u64(self.pts_us);
        w.write_u32(self.data.len() as u32);
        w.write_bytes(&self.data);
        // Extension flags + tile info
        let flags = if self.tile_info.is_some() {
            VIDEO_FLAG_TILE
        } else {
            0
        };
        w.write_u8(flags);
        if let Some(ref tile) = self.tile_info {
            w.write_u16(tile.tile_x);
            w.write_u16(tile.tile_y);
            w.write_u16(tile.tile_w);
            w.write_u16(tile.tile_h);
            w.write_u16(tile.screen_w);
            w.write_u16(tile.screen_h);
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let nal_id = r.read_u32()?;
        let fragment_seq = r.read_u16()?;
        let fragment_total = r.read_u16()?;
        let is_keyframe = r.read_bool()?;
        let pts_us = r.read_u64()?;
        let data_len = r.read_u32()? as usize;
        let data = r.read_bytes(data_len)?.to_vec();
        // Parse optional extension flags (backwards-compat: old data has no flags)
        let tile_info = if r.remaining() > 0 {
            let flags = r.read_u8()?;
            if flags & VIDEO_FLAG_TILE != 0 && r.remaining() >= 12 {
                Some(VideoTileInfo {
                    tile_x: r.read_u16()?,
                    tile_y: r.read_u16()?,
                    tile_w: r.read_u16()?,
                    tile_h: r.read_u16()?,
                    screen_w: r.read_u16()?,
                    screen_h: r.read_u16()?,
                })
            } else {
                None
            }
        } else {
            None
        };
        // Don't error on trailing data — future extensions may add more
        Ok(Self {
            nal_id,
            fragment_seq,
            fragment_total,
            is_keyframe,
            pts_us,
            data,
            tile_info,
        })
    }

    /// Fragment a NAL unit into datagrams that fit within the given MTU.
    pub fn fragment(
        nal_id: u32,
        is_keyframe: bool,
        pts_us: u64,
        nal_data: &[u8],
        max_fragment_size: usize,
    ) -> Vec<Self> {
        Self::fragment_with_tile(
            nal_id,
            is_keyframe,
            pts_us,
            nal_data,
            max_fragment_size,
            None,
        )
    }

    /// Fragment a NAL unit with optional tile info.
    pub fn fragment_with_tile(
        nal_id: u32,
        is_keyframe: bool,
        pts_us: u64,
        nal_data: &[u8],
        max_fragment_size: usize,
        tile_info: Option<VideoTileInfo>,
    ) -> Vec<Self> {
        if nal_data.is_empty() {
            return vec![Self {
                nal_id,
                fragment_seq: 0,
                fragment_total: 1,
                is_keyframe,
                pts_us,
                data: Vec::new(),
                tile_info,
            }];
        }

        let chunks: Vec<&[u8]> = nal_data.chunks(max_fragment_size).collect();
        assert!(
            chunks.len() <= u16::MAX as usize,
            "too many NAL fragments: {} (max {})",
            chunks.len(),
            u16::MAX
        );
        let total = chunks.len() as u16;
        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| Self {
                nal_id,
                fragment_seq: i as u16,
                fragment_total: total,
                is_keyframe,
                pts_us,
                data: chunk.to_vec(),
                tile_info,
            })
            .collect()
    }

    /// Reassemble fragments into a complete NAL unit.
    /// Fragments must be sorted by fragment_seq and all present.
    pub fn reassemble(fragments: &[Self]) -> Option<Vec<u8>> {
        if fragments.is_empty() {
            return None;
        }
        let total = fragments[0].fragment_total as usize;
        if fragments.len() != total {
            return None;
        }
        for (i, frag) in fragments.iter().enumerate() {
            if frag.fragment_seq != i as u16 {
                return None;
            }
        }
        let total_len: usize = fragments.iter().map(|f| f.data.len()).sum();
        let mut data = Vec::with_capacity(total_len);
        for frag in fragments {
            data.extend_from_slice(&frag.data);
        }
        Some(data)
    }
}

// ── AudioFrame encode/decode ────────────────────────────────────────

impl AudioFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::with_capacity(16 + self.data.len());
        w.write_u32(self.seq);
        w.write_u64(self.timestamp_us);
        w.write_u32(self.data.len() as u32);
        w.write_bytes(&self.data);
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let seq = r.read_u32()?;
        let timestamp_us = r.read_u64()?;
        let data_len = r.read_u32()? as usize;
        let data = r.read_bytes(data_len)?.to_vec();
        if r.remaining() > 0 {
            return Err(FrameError::TrailingData(r.remaining()));
        }
        Ok(Self {
            seq,
            timestamp_us,
            data,
        })
    }

    /// Encode into a full wire frame on the AudioOut channel.
    pub fn to_frame_out(&self) -> Frame {
        Frame::new(ChannelId::AudioOut, self.encode())
    }

    /// Encode into a full wire frame on the AudioIn channel.
    pub fn to_frame_in(&self) -> Frame {
        Frame::new(ChannelId::AudioIn, self.encode())
    }
}

// ── Dispatch helper ─────────────────────────────────────────────────

// ── TileMessage encode/decode ──────────────────────────────────────

impl TileMessage {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::GridConfig {
                tile_size,
                cols,
                rows,
                screen_w,
                screen_h,
            } => {
                let mut w = Writer::with_capacity(11);
                w.write_u8(TILE_GRID_CONFIG);
                w.write_u16(*tile_size);
                w.write_u16(*cols);
                w.write_u16(*rows);
                w.write_u16(*screen_w);
                w.write_u16(*screen_h);
                w.finish()
            }
            Self::CacheHit { col, row, hash } => {
                let mut w = Writer::with_capacity(13);
                w.write_u8(TILE_CACHE_HIT);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
                w.finish()
            }
            Self::CacheMiss {
                frame_seq,
                col,
                row,
                hash,
            } => {
                let mut w = Writer::with_capacity(17);
                w.write_u8(TILE_CACHE_MISS);
                w.write_u32(*frame_seq);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
                w.finish()
            }
            Self::Fill { col, row, rgba } => {
                let mut w = Writer::with_capacity(9);
                w.write_u8(TILE_FILL);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u32(*rgba);
                w.finish()
            }
            Self::Qoi {
                col,
                row,
                hash,
                data,
            } => {
                let mut w = Writer::with_capacity(17 + data.len());
                w.write_u8(TILE_QOI);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
                w.write_u32(data.len() as u32);
                w.write_bytes(data);
                w.finish()
            }
            Self::Zstd {
                col,
                row,
                hash,
                data,
            } => {
                let mut w = Writer::with_capacity(17 + data.len());
                w.write_u8(TILE_ZSTD);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
                w.write_u32(data.len() as u32);
                w.write_bytes(data);
                w.finish()
            }
            Self::VideoRegion { x, y, w: vw, h: vh } => {
                let mut wr = Writer::with_capacity(9);
                wr.write_u8(TILE_VIDEO_REGION);
                wr.write_u16(*x);
                wr.write_u16(*y);
                wr.write_u16(*vw);
                wr.write_u16(*vh);
                wr.finish()
            }
            Self::BatchEnd { frame_seq } => {
                let mut w = Writer::with_capacity(5);
                w.write_u8(TILE_BATCH_END);
                w.write_u32(*frame_seq);
                w.finish()
            }
            Self::ScrollCopy {
                dx,
                dy,
                region_top,
                region_bottom,
                region_right,
            } => {
                let mut w = Writer::with_capacity(11);
                w.write_u8(TILE_SCROLL_COPY);
                w.write_i16(*dx);
                w.write_i16(*dy);
                w.write_u16(*region_top);
                w.write_u16(*region_bottom);
                w.write_u16(*region_right);
                w.finish()
            }
            Self::GridOffset { offset_x, offset_y } => {
                let mut w = Writer::with_capacity(5);
                w.write_u8(TILE_GRID_OFFSET);
                w.write_i16(*offset_x);
                w.write_i16(*offset_y);
                w.finish()
            }
            Self::TileDrawMode { apply_offset } => {
                let mut w = Writer::with_capacity(2);
                w.write_u8(TILE_DRAW_MODE);
                w.write_u8(if *apply_offset { 1 } else { 0 });
                w.finish()
            }
            Self::ScrollStats {
                scroll_batches_total,
                scroll_full_fallbacks_total,
                scroll_potential_tiles_total,
                scroll_saved_tiles_total,
            } => {
                let mut w = Writer::with_capacity(17);
                w.write_u8(TILE_SCROLL_STATS);
                w.write_u32(*scroll_batches_total);
                w.write_u32(*scroll_full_fallbacks_total);
                w.write_u32(*scroll_potential_tiles_total);
                w.write_u32(*scroll_saved_tiles_total);
                w.finish()
            }
        }
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let tag = r.read_u8()?;
        match tag {
            TILE_GRID_CONFIG => {
                let tile_size = r.read_u16()?;
                let cols = r.read_u16()?;
                let rows = r.read_u16()?;
                let screen_w = r.read_u16()?;
                let screen_h = r.read_u16()?;
                Ok(Self::GridConfig {
                    tile_size,
                    cols,
                    rows,
                    screen_w,
                    screen_h,
                })
            }
            TILE_CACHE_HIT => {
                let col = r.read_u16()?;
                let row = r.read_u16()?;
                let hash = r.read_u64()?;
                Ok(Self::CacheHit { col, row, hash })
            }
            TILE_CACHE_MISS => {
                let frame_seq = r.read_u32()?;
                let col = r.read_u16()?;
                let row = r.read_u16()?;
                let hash = r.read_u64()?;
                Ok(Self::CacheMiss {
                    frame_seq,
                    col,
                    row,
                    hash,
                })
            }
            TILE_FILL => {
                let col = r.read_u16()?;
                let row = r.read_u16()?;
                let rgba = r.read_u32()?;
                Ok(Self::Fill { col, row, rgba })
            }
            TILE_QOI => {
                let col = r.read_u16()?;
                let row = r.read_u16()?;
                let hash = r.read_u64()?;
                let data_len = r.read_u32()? as usize;
                let data = r.read_bytes(data_len)?.to_vec();
                Ok(Self::Qoi {
                    col,
                    row,
                    hash,
                    data,
                })
            }
            TILE_ZSTD => {
                let col = r.read_u16()?;
                let row = r.read_u16()?;
                let hash = r.read_u64()?;
                let data_len = r.read_u32()? as usize;
                let data = r.read_bytes(data_len)?.to_vec();
                Ok(Self::Zstd {
                    col,
                    row,
                    hash,
                    data,
                })
            }
            TILE_VIDEO_REGION => {
                let x = r.read_u16()?;
                let y = r.read_u16()?;
                let w = r.read_u16()?;
                let h = r.read_u16()?;
                Ok(Self::VideoRegion { x, y, w, h })
            }
            TILE_BATCH_END => {
                let frame_seq = r.read_u32()?;
                Ok(Self::BatchEnd { frame_seq })
            }
            TILE_SCROLL_COPY => {
                let dx = r.read_i16()?;
                let dy = r.read_i16()?;
                let region_top = r.read_u16()?;
                let region_bottom = r.read_u16()?;
                let region_right = r.read_u16()?;
                Ok(Self::ScrollCopy {
                    dx,
                    dy,
                    region_top,
                    region_bottom,
                    region_right,
                })
            }
            TILE_GRID_OFFSET => {
                let offset_x = r.read_i16()?;
                let offset_y = r.read_i16()?;
                Ok(Self::GridOffset { offset_x, offset_y })
            }
            TILE_DRAW_MODE => {
                let flag = r.read_u8()?;
                Ok(Self::TileDrawMode {
                    apply_offset: flag != 0,
                })
            }
            TILE_SCROLL_STATS => {
                let scroll_batches_total = r.read_u32()?;
                let scroll_full_fallbacks_total = r.read_u32()?;
                let scroll_potential_tiles_total = r.read_u32()?;
                let scroll_saved_tiles_total = r.read_u32()?;
                Ok(Self::ScrollStats {
                    scroll_batches_total,
                    scroll_full_fallbacks_total,
                    scroll_potential_tiles_total,
                    scroll_saved_tiles_total,
                })
            }
            _ => Err(FrameError::UnknownMessageType {
                channel: ChannelId::Tiles.as_u8(),
                tag,
            }),
        }
    }

    /// Wrap this tile message in a Frame for wire transmission.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Tiles, self.encode())
    }
}

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
    /// Decode a frame's payload based on its channel.
    pub fn from_frame(frame: &Frame) -> Result<Self, FrameError> {
        match frame.channel {
            ChannelId::Control => Ok(Self::Control(ControlMessage::decode(&frame.payload)?)),
            ChannelId::Input => Ok(Self::Input(InputMessage::decode(&frame.payload)?)),
            ChannelId::Cursor => Ok(Self::Cursor(CursorMessage::decode(&frame.payload)?)),
            ChannelId::Clipboard => Ok(Self::Clipboard(ClipboardMessage::decode(&frame.payload)?)),
            ChannelId::FileUp => Ok(Self::FileUp(FileMessage::decode(&frame.payload)?)),
            ChannelId::FileDown => Ok(Self::FileDown(FileMessage::decode(&frame.payload)?)),
            ChannelId::Tiles => Ok(Self::Tiles(TileMessage::decode(&frame.payload)?)),
            ChannelId::Video => Ok(Self::Video(frame.payload.clone())),
            ChannelId::AudioOut => Ok(Self::AudioOut(frame.payload.clone())),
            ChannelId::AudioIn => Ok(Self::AudioIn(frame.payload.clone())),
            ChannelId::VideoIn => Ok(Self::VideoIn(frame.payload.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frame envelope tests ────────────────────────────────────────

    #[test]
    fn frame_round_trip() {
        let frame = Frame::new(ChannelId::Control, vec![1, 2, 3, 4]);
        let encoded = frame.encode();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn frame_empty_payload() {
        let frame = Frame::new(ChannelId::Input, vec![]);
        let encoded = frame.encode();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
        assert_eq!(consumed, FRAME_HEADER_SIZE);
    }

    #[test]
    fn frame_decode_too_short() {
        assert!(matches!(
            Frame::decode(&[0x0A, 0x01]),
            Err(FrameError::BufferTooShort { .. })
        ));
    }

    #[test]
    fn frame_decode_unknown_channel() {
        let buf = [0xFF, 0x00, 0x00, 0x00, 0x00];
        assert!(matches!(
            Frame::decode(&buf),
            Err(FrameError::UnknownChannel(0xFF))
        ));
    }

    #[test]
    fn frame_decode_payload_too_large() {
        // 32 MiB payload length
        let buf = [0x0A, 0x00, 0x00, 0x00, 0x02];
        assert!(matches!(
            Frame::decode(&buf),
            Err(FrameError::PayloadTooLarge(_))
        ));
    }

    #[test]
    fn frame_decode_all_multiple() {
        let f1 = Frame::new(ChannelId::Control, vec![1, 2]);
        let f2 = Frame::new(ChannelId::Input, vec![3, 4, 5]);
        let mut buf = Vec::from(f1.encode().as_ref());
        buf.extend_from_slice(&f2.encode());
        let (frames, consumed) = Frame::decode_all(&buf).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], f1);
        assert_eq!(frames[1], f2);
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn frame_decode_all_partial() {
        let f1 = Frame::new(ChannelId::Control, vec![1, 2]);
        let mut buf = Vec::from(f1.encode().as_ref());
        buf.extend_from_slice(&[0x0A, 0x10]); // partial header
        let (frames, consumed) = Frame::decode_all(&buf).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], f1);
        assert!(consumed < buf.len());
    }

    // ── Control message tests ───────────────────────────────────────

    #[test]
    fn control_resolution_request_round_trip() {
        let msg = ControlMessage::ResolutionRequest {
            width: 1920,
            height: 1080,
        };
        let encoded = msg.encode();
        let decoded = ControlMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn control_resolution_ack_round_trip() {
        let msg = ControlMessage::ResolutionAck {
            width: 800,
            height: 600,
        };
        let encoded = msg.encode();
        let decoded = ControlMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn control_session_ready_round_trip() {
        let msg = ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::all(),
        };
        let encoded = msg.encode();
        let decoded = ControlMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn control_ping_pong_round_trip() {
        let ping = ControlMessage::Ping {
            seq: 42,
            timestamp_ms: 1_700_000_000_000,
        };
        let pong = ControlMessage::Pong {
            seq: 42,
            timestamp_ms: 1_700_000_000_005,
        };
        assert_eq!(ping, ControlMessage::decode(&ping.encode()).unwrap());
        assert_eq!(pong, ControlMessage::decode(&pong.encode()).unwrap());
    }

    #[test]
    fn control_to_frame_round_trip() {
        let msg = ControlMessage::Ping {
            seq: 1,
            timestamp_ms: 999,
        };
        let frame = msg.to_frame();
        assert_eq!(frame.channel, ChannelId::Control);
        let decoded = Message::from_frame(&frame).unwrap();
        assert_eq!(decoded, Message::Control(msg));
    }

    // ── Input message tests ─────────────────────────────────────────

    #[test]
    fn input_mouse_move_round_trip() {
        let msg = InputMessage::MouseMove { x: 100, y: 200 };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_mouse_button_round_trip() {
        let msg = InputMessage::MouseButton {
            button: 0,
            down: true,
            x: 50,
            y: 75,
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_mouse_scroll_round_trip() {
        let msg = InputMessage::MouseScroll { dx: -3, dy: 5 };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_key_event_round_trip() {
        let msg = InputMessage::KeyEvent {
            keycode: 0x001E, // KeyA
            down: true,
            modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_negative_scroll_values() {
        let msg = InputMessage::MouseScroll {
            dx: -32768,
            dy: 32767,
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_key_event_ex_round_trip() {
        let msg = InputMessage::KeyEventEx {
            keycode: 30, // KeyA
            down: true,
            modifiers: 0,
            key_char: 0x61, // 'a'
        };
        let encoded = msg.encode();
        assert_eq!(encoded.len(), 11); // tag(1) + keycode(4) + down(1) + mods(1) + key_char(4)
        assert_eq!(msg, InputMessage::decode(&encoded).unwrap());
    }

    #[test]
    fn input_key_event_ex_unicode_round_trip() {
        let msg = InputMessage::KeyEventEx {
            keycode: 3, // Digit2
            down: true,
            modifiers: 0,
            key_char: 0xE9, // 'é'
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_key_event_ex_non_printable_round_trip() {
        let msg = InputMessage::KeyEventEx {
            keycode: 1, // Escape
            down: true,
            modifiers: 0,
            key_char: 0, // non-printable
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn input_key_event_ex_euro_sign() {
        let msg = InputMessage::KeyEventEx {
            keycode: 18, // KeyE
            down: true,
            modifiers: Modifiers::ALT,
            key_char: 0x20AC, // '€'
        };
        assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn control_keyboard_layout_info_round_trip() {
        let mut layout_hint = [0u8; 32];
        let hint = b"fr";
        layout_hint[..hint.len()].copy_from_slice(hint);
        let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
        let encoded = msg.encode();
        assert_eq!(encoded.len(), 33); // tag(1) + layout_hint(32)
        assert_eq!(msg, ControlMessage::decode(&encoded).unwrap());
    }

    #[test]
    fn control_keyboard_layout_info_empty() {
        let msg = ControlMessage::KeyboardLayoutInfo {
            layout_hint: [0u8; 32],
        };
        assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
    }

    // ── Cursor message tests ────────────────────────────────────────

    #[test]
    fn cursor_move_round_trip() {
        let msg = CursorMessage::CursorMove { x: 400, y: 300 };
        assert_eq!(msg, CursorMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn cursor_shape_round_trip() {
        let data = vec![0xFF; 32 * 32 * 4]; // 32x32 RGBA
        let msg = CursorMessage::CursorShape {
            width: 32,
            height: 32,
            hotspot_x: 16,
            hotspot_y: 16,
            data,
        };
        assert_eq!(msg, CursorMessage::decode(&msg.encode()).unwrap());
    }

    // ── Clipboard message tests ─────────────────────────────────────

    #[test]
    fn clipboard_text_round_trip() {
        let msg = ClipboardMessage::Text {
            content: b"Hello, clipboard!".to_vec(),
        };
        assert_eq!(msg, ClipboardMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn clipboard_empty_text() {
        let msg = ClipboardMessage::Text {
            content: Vec::new(),
        };
        assert_eq!(msg, ClipboardMessage::decode(&msg.encode()).unwrap());
    }

    // ── File message tests ──────────────────────────────────────────

    #[test]
    fn file_header_round_trip() {
        let mut filename = [0u8; 256];
        let name = b"test-file.txt";
        filename[..name.len()].copy_from_slice(name);

        let mut mime = [0u8; 64];
        let mt = b"text/plain";
        mime[..mt.len()].copy_from_slice(mt);

        let msg = FileMessage::FileHeader {
            id: 1,
            filename,
            size: 1024,
            mime,
        };
        assert_eq!(msg, FileMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn file_chunk_round_trip() {
        let msg = FileMessage::FileChunk {
            id: 1,
            seq: 0,
            data: vec![0xAB; 65536],
        };
        assert_eq!(msg, FileMessage::decode(&msg.encode()).unwrap());
    }

    #[test]
    fn file_complete_round_trip() {
        let msg = FileMessage::FileComplete { id: 1 };
        assert_eq!(msg, FileMessage::decode(&msg.encode()).unwrap());
    }

    // ── VideoDatagram tests ─────────────────────────────────────────

    #[test]
    fn video_datagram_round_trip() {
        let dg = VideoDatagram {
            nal_id: 42,
            fragment_seq: 0,
            fragment_total: 1,
            is_keyframe: true,
            pts_us: 33_333,
            data: vec![0x00, 0x00, 0x00, 0x01, 0x65],
            tile_info: None,
        };
        let encoded = dg.encode();
        let decoded = VideoDatagram::decode(&encoded).unwrap();
        assert_eq!(dg, decoded);
    }

    #[test]
    fn video_datagram_fragmentation() {
        let nal_data = vec![0xAA; 3000];
        let fragments = VideoDatagram::fragment(1, false, 100_000, &nal_data, 1000);
        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments[0].fragment_seq, 0);
        assert_eq!(fragments[0].fragment_total, 3);
        assert_eq!(fragments[1].fragment_seq, 1);
        assert_eq!(fragments[2].fragment_seq, 2);
        assert_eq!(fragments[2].data.len(), 1000);

        let reassembled = VideoDatagram::reassemble(&fragments).unwrap();
        assert_eq!(reassembled, nal_data);
    }

    #[test]
    fn video_datagram_fragment_single() {
        let nal_data = vec![0xBB; 500];
        let fragments = VideoDatagram::fragment(2, true, 200_000, &nal_data, 1200);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].fragment_total, 1);
        assert!(fragments[0].is_keyframe);

        let reassembled = VideoDatagram::reassemble(&fragments).unwrap();
        assert_eq!(reassembled, nal_data);
    }

    #[test]
    fn video_datagram_reassemble_missing_fragment() {
        let fragments = VideoDatagram::fragment(1, false, 100, &vec![0; 3000], 1000);
        let partial = &fragments[0..2]; // missing fragment 2
        assert!(VideoDatagram::reassemble(partial).is_none());
    }

    #[test]
    fn video_datagram_fragment_round_trip_encode_decode() {
        let fragments = VideoDatagram::fragment(5, true, 500_000, &vec![0xCC; 2500], 1000);
        let round_tripped: Vec<VideoDatagram> = fragments
            .iter()
            .map(|f| {
                let encoded = f.encode();
                VideoDatagram::decode(&encoded).unwrap()
            })
            .collect();
        assert_eq!(fragments, round_tripped);
    }

    // ── AudioFrame tests ────────────────────────────────────────────

    #[test]
    fn audio_frame_round_trip() {
        let frame = AudioFrame {
            seq: 100,
            timestamp_us: 2_000_000,
            data: vec![0x01, 0x02, 0x03],
        };
        let encoded = frame.encode();
        let decoded = AudioFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn audio_frame_empty_data() {
        let frame = AudioFrame {
            seq: 0,
            timestamp_us: 0,
            data: Vec::new(),
        };
        assert_eq!(frame, AudioFrame::decode(&frame.encode()).unwrap());
    }

    #[test]
    fn audio_frame_to_frame_out_channel() {
        let af = AudioFrame {
            seq: 1,
            timestamp_us: 20_000,
            data: vec![0xAB; 16],
        };
        let frame = af.to_frame_out();
        assert_eq!(frame.channel, ChannelId::AudioOut);
        // Payload should decode back to the same AudioFrame
        let decoded = AudioFrame::decode(&frame.payload).unwrap();
        assert_eq!(decoded, af);
    }

    #[test]
    fn audio_frame_to_frame_out_message_dispatch() {
        let af = AudioFrame {
            seq: 42,
            timestamp_us: 840_000,
            data: vec![0x00; 3840],
        };
        let frame = af.to_frame_out();
        let msg = Message::from_frame(&frame).unwrap();
        match msg {
            Message::AudioOut(payload) => {
                let decoded = AudioFrame::decode(&payload).unwrap();
                assert_eq!(decoded, af);
            }
            other => panic!("expected AudioOut, got {:?}", other),
        }
    }

    #[test]
    fn audio_frame_pcm_20ms_round_trip() {
        // 48kHz stereo s16le = 960 samples * 2 channels * 2 bytes = 3840 bytes per 20ms
        let pcm_data = vec![0x7F; 3840];
        let af = AudioFrame {
            seq: 100,
            timestamp_us: 2_000_000,
            data: pcm_data.clone(),
        };
        let frame = af.to_frame_out();
        let wire = frame.encode();
        let (decoded_frame, consumed) = Frame::decode(&wire).unwrap();
        assert_eq!(consumed, wire.len());
        assert_eq!(decoded_frame.channel, ChannelId::AudioOut);
        let decoded_af = AudioFrame::decode(&decoded_frame.payload).unwrap();
        assert_eq!(decoded_af.seq, 100);
        assert_eq!(decoded_af.data.len(), 3840);
        assert_eq!(decoded_af.data, pcm_data);
    }

    #[test]
    fn audio_frame_decode_truncated_header() {
        // AudioFrame header: seq(4) + timestamp_us(8) + data_len(4) = 16 bytes minimum
        let buf = vec![0u8; 10]; // too short
        assert!(matches!(
            AudioFrame::decode(&buf),
            Err(FrameError::BufferTooShort { .. })
        ));
    }

    #[test]
    fn audio_frame_decode_truncated_data() {
        // Encode a valid header claiming 100 bytes of data, but only provide 50
        let af = AudioFrame {
            seq: 1,
            timestamp_us: 20_000,
            data: vec![0xAA; 100],
        };
        let encoded = af.encode();
        // Chop off the last 50 bytes of data
        let truncated = &encoded[..encoded.len() - 50];
        assert!(matches!(
            AudioFrame::decode(truncated),
            Err(FrameError::BufferTooShort { .. })
        ));
    }

    #[test]
    fn audio_frame_trailing_data() {
        let af = AudioFrame {
            seq: 1,
            timestamp_us: 20_000,
            data: vec![0xBB; 10],
        };
        let mut encoded = af.encode();
        encoded.push(0xFF); // trailing byte
        assert!(matches!(
            AudioFrame::decode(&encoded),
            Err(FrameError::TrailingData(1))
        ));
    }

    #[test]
    fn audio_frame_sequence_monotonic() {
        // Verify sequential frames encode correctly with wrapping seq
        let frames: Vec<AudioFrame> = (0..5u32)
            .map(|i| AudioFrame {
                seq: (u32::MAX - 2).wrapping_add(i),
                timestamp_us: i as u64 * 20_000,
                data: vec![i as u8; 3840],
            })
            .collect();

        for af in &frames {
            let decoded = AudioFrame::decode(&af.encode()).unwrap();
            assert_eq!(decoded.seq, af.seq);
        }
        // Verify wrapping: seq goes ..., MAX-2, MAX-1, MAX, 0, 1
        assert_eq!(frames[0].seq, u32::MAX - 2);
        assert_eq!(frames[1].seq, u32::MAX - 1);
        assert_eq!(frames[2].seq, u32::MAX);
        assert_eq!(frames[3].seq, 0); // wraps
        assert_eq!(frames[4].seq, 1);
    }

    // ── Message dispatch tests ──────────────────────────────────────

    #[test]
    fn message_dispatch_all_typed_channels() {
        let ctrl = ControlMessage::Ping {
            seq: 1,
            timestamp_ms: 100,
        };
        let input = InputMessage::MouseMove { x: 10, y: 20 };
        let input_key_ex = InputMessage::KeyEventEx {
            keycode: 16,
            down: true,
            modifiers: 0,
            key_char: 0x61,
        };
        let layout_info = ControlMessage::KeyboardLayoutInfo {
            layout_hint: [0u8; 32],
        };
        let cursor = CursorMessage::CursorMove { x: 5, y: 10 };
        let clip = ClipboardMessage::Text {
            content: b"hi".to_vec(),
        };
        let file = FileMessage::FileComplete { id: 99 };

        assert!(matches!(
            Message::from_frame(&ctrl.to_frame()),
            Ok(Message::Control(_))
        ));
        assert!(matches!(
            Message::from_frame(&input.to_frame()),
            Ok(Message::Input(_))
        ));
        // KeyEventEx dispatches as Input
        assert!(matches!(
            Message::from_frame(&input_key_ex.to_frame()),
            Ok(Message::Input(_))
        ));
        // KeyboardLayoutInfo dispatches as Control
        assert!(matches!(
            Message::from_frame(&layout_info.to_frame()),
            Ok(Message::Control(_))
        ));
        assert!(matches!(
            Message::from_frame(&cursor.to_frame()),
            Ok(Message::Cursor(_))
        ));
        assert!(matches!(
            Message::from_frame(&clip.to_frame()),
            Ok(Message::Clipboard(_))
        ));
        assert!(matches!(
            Message::from_frame(&file.to_frame(ChannelId::FileDown)),
            Ok(Message::FileDown(_))
        ));
        assert!(matches!(
            Message::from_frame(&file.to_frame(ChannelId::FileUp)),
            Ok(Message::FileUp(_))
        ));
    }

    #[test]
    fn message_dispatch_key_event_ex_preserves_fields() {
        let msg = InputMessage::KeyEventEx {
            keycode: 18,
            down: true,
            modifiers: Modifiers::ALT,
            key_char: 0x20AC, // €
        };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        assert_eq!(decoded, Message::Input(msg));
    }

    #[test]
    fn message_dispatch_keyboard_layout_info_preserves_fields() {
        let mut layout_hint = [0u8; 32];
        layout_hint[..5].copy_from_slice(b"de-CH");
        let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        assert_eq!(decoded, Message::Control(msg));
    }

    #[test]
    fn message_dispatch_raw_channels() {
        let video_frame = Frame::new(ChannelId::Video, vec![0x00, 0x00, 0x01]);
        assert!(matches!(
            Message::from_frame(&video_frame),
            Ok(Message::Video(_))
        ));

        let audio_frame = Frame::new(ChannelId::AudioOut, vec![0xAA]);
        assert!(matches!(
            Message::from_frame(&audio_frame),
            Ok(Message::AudioOut(_))
        ));
    }

    // ── Error cases ─────────────────────────────────────────────────

    #[test]
    fn control_unknown_tag() {
        assert!(matches!(
            ControlMessage::decode(&[0xFF]),
            Err(FrameError::UnknownMessageType { .. })
        ));
    }

    #[test]
    fn input_unknown_tag() {
        assert!(matches!(
            InputMessage::decode(&[0xFF]),
            Err(FrameError::UnknownMessageType { .. })
        ));
    }

    #[test]
    fn control_trailing_data() {
        let mut encoded = ControlMessage::ResolutionAck {
            width: 100,
            height: 100,
        }
        .encode();
        encoded.push(0xFF); // trailing byte
        assert!(matches!(
            ControlMessage::decode(&encoded),
            Err(FrameError::TrailingData(1))
        ));
    }

    #[test]
    fn control_truncated() {
        let encoded = ControlMessage::Ping {
            seq: 1,
            timestamp_ms: 100,
        }
        .encode();
        // Remove last byte
        assert!(matches!(
            ControlMessage::decode(&encoded[..encoded.len() - 1]),
            Err(FrameError::BufferTooShort { .. })
        ));
    }

    // ── Tile info round-trip tests ──────────────────────────────────

    #[test]
    fn video_datagram_with_tile_info_round_trip() {
        let dg = VideoDatagram {
            nal_id: 42,
            fragment_seq: 0,
            fragment_total: 1,
            is_keyframe: true,
            pts_us: 123_456,
            data: vec![0xAA; 100],
            tile_info: Some(VideoTileInfo {
                tile_x: 100,
                tile_y: 200,
                tile_w: 320,
                tile_h: 240,
                screen_w: 1920,
                screen_h: 1080,
            }),
        };
        let encoded = dg.encode();
        let decoded = VideoDatagram::decode(&encoded).unwrap();
        assert_eq!(dg, decoded);
        assert!(decoded.tile_info.is_some());
        let tile = decoded.tile_info.unwrap();
        assert_eq!(tile.tile_x, 100);
        assert_eq!(tile.tile_y, 200);
        assert_eq!(tile.tile_w, 320);
        assert_eq!(tile.tile_h, 240);
        assert_eq!(tile.screen_w, 1920);
        assert_eq!(tile.screen_h, 1080);
    }

    #[test]
    fn video_datagram_without_tile_info_round_trip() {
        let dg = VideoDatagram {
            nal_id: 7,
            fragment_seq: 0,
            fragment_total: 1,
            is_keyframe: false,
            pts_us: 999,
            data: vec![0xBB; 50],
            tile_info: None,
        };
        let encoded = dg.encode();
        let decoded = VideoDatagram::decode(&encoded).unwrap();
        assert_eq!(dg, decoded);
        assert!(decoded.tile_info.is_none());
    }

    #[test]
    fn fragment_with_tile_preserves_tile_info() {
        let tile = VideoTileInfo {
            tile_x: 64,
            tile_y: 128,
            tile_w: 256,
            tile_h: 128,
            screen_w: 1920,
            screen_h: 1080,
        };
        let data = vec![0xCC; 3000];
        let fragments = VideoDatagram::fragment_with_tile(10, true, 500, &data, 1000, Some(tile));
        assert_eq!(fragments.len(), 3);
        for frag in &fragments {
            assert_eq!(frag.tile_info, Some(tile));
            assert_eq!(frag.nal_id, 10);
            assert!(frag.is_keyframe);
        }
        // Round-trip each fragment through encode/decode
        for frag in &fragments {
            let decoded = VideoDatagram::decode(&frag.encode()).unwrap();
            assert_eq!(decoded.tile_info, Some(tile));
        }
        // Reassemble
        let reassembled = VideoDatagram::reassemble(&fragments).unwrap();
        assert_eq!(reassembled, data);
    }

    #[test]
    fn fragment_with_tile_none_is_same_as_fragment() {
        let data = vec![0xDD; 2500];
        let frags_plain = VideoDatagram::fragment(1, false, 100, &data, 1000);
        let frags_tile = VideoDatagram::fragment_with_tile(1, false, 100, &data, 1000, None);
        assert_eq!(frags_plain.len(), frags_tile.len());
        for (a, b) in frags_plain.iter().zip(frags_tile.iter()) {
            assert_eq!(a, b);
        }
    }

    // ── BitrateHint tests ───────────────────────────────────────────

    #[test]
    fn bitrate_hint_round_trip() {
        let msg = ControlMessage::BitrateHint {
            target_bps: 5_000_000,
        };
        let encoded = msg.encode();
        let decoded = ControlMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn bitrate_hint_dispatch_round_trip() {
        let msg = ControlMessage::BitrateHint {
            target_bps: 2_500_000,
        };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        assert_eq!(decoded, Message::Control(msg));
    }

    #[test]
    fn bitrate_hint_zero_bps() {
        let msg = ControlMessage::BitrateHint { target_bps: 0 };
        let decoded = ControlMessage::decode(&msg.encode()).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn bitrate_hint_max_bps() {
        let msg = ControlMessage::BitrateHint {
            target_bps: u32::MAX,
        };
        let decoded = ControlMessage::decode(&msg.encode()).unwrap();
        assert_eq!(decoded, msg);
    }

    // ── Fix 1: Frame::encode() payload size validation ──────────────

    #[test]
    #[should_panic(expected = "frame payload too large")]
    fn frame_encode_panics_on_oversized_payload() {
        let payload = vec![0u8; MAX_PAYLOAD_SIZE as usize + 1];
        let frame = Frame::new(ChannelId::Control, payload);
        let _ = frame.encode();
    }

    #[test]
    fn frame_encode_max_payload_succeeds() {
        // Exactly MAX_PAYLOAD_SIZE should work (but we use a smaller value to avoid OOM)
        let payload = vec![0u8; 1024];
        let frame = Frame::new(ChannelId::Control, payload);
        let encoded = frame.encode();
        assert_eq!(encoded.len(), FRAME_HEADER_SIZE + 1024);
    }

    // ── Fix 2: fragment_with_tile overflow guard ────────────────────

    #[test]
    fn fragment_with_small_max_fragment_size() {
        // Ensure fragmentation works correctly with very small fragment sizes
        let data = vec![0xAA; 100];
        let frags = VideoDatagram::fragment(1, false, 0, &data, 10);
        assert_eq!(frags.len(), 10);
        assert_eq!(frags[0].fragment_total, 10);
        for (i, frag) in frags.iter().enumerate() {
            assert_eq!(frag.fragment_seq, i as u16);
        }
        let reassembled = VideoDatagram::reassemble(&frags).unwrap();
        assert_eq!(reassembled, data);
    }

    // ── Tile message tests ──────────────────────────────────────────

    #[test]
    fn tile_grid_config_round_trip() {
        let msg = TileMessage::GridConfig {
            tile_size: 64,
            cols: 20,
            rows: 12,
            screen_w: 1280,
            screen_h: 768,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_cache_hit_round_trip() {
        let msg = TileMessage::CacheHit {
            col: 5,
            row: 3,
            hash: 0xDEADBEEFCAFE1234,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_cache_miss_round_trip() {
        let msg = TileMessage::CacheMiss {
            frame_seq: 77,
            col: 5,
            row: 3,
            hash: 0xDEADBEEFCAFE1234,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_fill_round_trip() {
        let msg = TileMessage::Fill {
            col: 0,
            row: 0,
            rgba: 0xFF3A3A6E,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_qoi_round_trip() {
        let qoi_data = vec![0x71, 0x6f, 0x69, 0x66, 0x00, 0x00, 0x00, 0x40]; // QOI header
        let msg = TileMessage::Qoi {
            col: 10,
            row: 7,
            hash: 0x1234567890ABCDEF,
            data: qoi_data.clone(),
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_qoi_large_payload_round_trip() {
        let qoi_data = vec![0xAA; 65536]; // 64KB tile
        let msg = TileMessage::Qoi {
            col: 1,
            row: 2,
            hash: 42,
            data: qoi_data,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_zstd_round_trip() {
        let zstd_data = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x01, 0x00]; // zstd magic + data
        let msg = TileMessage::Zstd {
            col: 3,
            row: 5,
            hash: 0xFEDCBA9876543210,
            data: zstd_data.clone(),
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_video_region_round_trip() {
        let msg = TileMessage::VideoRegion {
            x: 100,
            y: 200,
            w: 640,
            h: 480,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_batch_end_round_trip() {
        let msg = TileMessage::BatchEnd { frame_seq: 12345 };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_scroll_copy_round_trip() {
        let msg = TileMessage::ScrollCopy {
            dx: 0,
            dy: -128,
            region_top: 80,
            region_bottom: 720,
            region_right: 1260,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);

        let msg2 = TileMessage::ScrollCopy {
            dx: 64,
            dy: 0,
            region_top: 0,
            region_bottom: 600,
            region_right: 800,
        };
        let encoded2 = msg2.encode();
        let decoded2 = TileMessage::decode(&encoded2).unwrap();
        assert_eq!(msg2, decoded2);
    }

    #[test]
    fn tile_draw_mode_round_trip() {
        let msg = TileMessage::TileDrawMode { apply_offset: true };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);

        let msg2 = TileMessage::TileDrawMode {
            apply_offset: false,
        };
        let encoded2 = msg2.encode();
        let decoded2 = TileMessage::decode(&encoded2).unwrap();
        assert_eq!(msg2, decoded2);
    }

    #[test]
    fn tile_scroll_stats_round_trip() {
        let msg = TileMessage::ScrollStats {
            scroll_batches_total: 42,
            scroll_full_fallbacks_total: 9,
            scroll_potential_tiles_total: 12_345,
            scroll_saved_tiles_total: 8_765,
        };
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tile_message_unknown_tag() {
        let buf = [0xFF];
        let err = TileMessage::decode(&buf).unwrap_err();
        assert!(matches!(
            err,
            FrameError::UnknownMessageType {
                channel: 0x0B,
                tag: 0xFF
            }
        ));
    }

    #[test]
    fn tile_message_to_frame_round_trip() {
        let msg = TileMessage::Fill {
            col: 3,
            row: 7,
            rgba: 0xFFFFFFFF,
        };
        let frame = msg.to_frame();
        assert_eq!(frame.channel, ChannelId::Tiles);
        let decoded = Message::from_frame(&frame).unwrap();
        match decoded {
            Message::Tiles(tm) => assert_eq!(tm, msg),
            _ => panic!("expected Tiles message"),
        }
    }

    #[test]
    fn tile_all_variants_via_frame() {
        let messages = vec![
            TileMessage::GridConfig {
                tile_size: 64,
                cols: 20,
                rows: 12,
                screen_w: 1280,
                screen_h: 768,
            },
            TileMessage::CacheHit {
                col: 0,
                row: 0,
                hash: 0,
            },
            TileMessage::CacheMiss {
                frame_seq: 1,
                col: 0,
                row: 0,
                hash: 0,
            },
            TileMessage::Fill {
                col: 1,
                row: 1,
                rgba: 0,
            },
            TileMessage::Qoi {
                col: 2,
                row: 2,
                hash: 99,
                data: vec![1, 2, 3],
            },
            TileMessage::Zstd {
                col: 4,
                row: 3,
                hash: 123,
                data: vec![0x28, 0xB5, 0x2F, 0xFD],
            },
            TileMessage::VideoRegion {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            TileMessage::BatchEnd { frame_seq: 0 },
            TileMessage::ScrollCopy {
                dx: -64,
                dy: 128,
                region_top: 0,
                region_bottom: 768,
                region_right: 1280,
            },
            TileMessage::TileDrawMode {
                apply_offset: false,
            },
            TileMessage::ScrollStats {
                scroll_batches_total: 3,
                scroll_full_fallbacks_total: 1,
                scroll_potential_tiles_total: 100,
                scroll_saved_tiles_total: 72,
            },
        ];
        for msg in messages {
            let frame = msg.to_frame();
            let encoded = frame.encode();
            let (decoded_frame, consumed) = Frame::decode(&encoded).unwrap();
            assert_eq!(consumed, encoded.len());
            let decoded_msg = Message::from_frame(&decoded_frame).unwrap();
            match decoded_msg {
                Message::Tiles(tm) => assert_eq!(tm, msg),
                _ => panic!("expected Tiles message for {:?}", msg),
            }
        }
    }
}
