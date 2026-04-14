//! Frame envelope handling plus per-channel codecs.
//!
//! Most callers will use [`Frame`] for transport boundaries and
//! [`Message::from_frame`] to decode typed channels.

mod audio;
mod clipboard;
mod control;
mod cursor;
mod envelope;
mod error;
mod file_transfer;
mod input;
mod io;
mod message;
mod tile;
mod video;

pub use self::envelope::{Frame, FRAME_HEADER_SIZE};
pub use self::error::FrameError;
pub use self::message::Message;

#[cfg(test)]
mod tests;
