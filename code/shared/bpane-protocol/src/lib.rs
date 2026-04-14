#![cfg_attr(not(feature = "std"), no_std)]
#![deny(rustdoc::broken_intra_doc_links, rustdoc::bare_urls)]

//! Shared BrowserPane wire protocol types and codecs.
//!
//! This crate defines the binary protocol contract used by BrowserPane Rust
//! services and the browser client. It provides:
//!
//! - channel identifiers in [`ChannelId`]
//! - typed protocol messages in [`types`]
//! - frame envelope parsing and serialization in [`frame`]
//!
//! # Compatibility policy
//!
//! - The wire format is compatibility-sensitive across BrowserPane components.
//! - Changes to channel IDs, message tags, or binary layouts must be
//!   coordinated across the Rust services and browser client.
//! - Before `1.0`, the public Rust API may still evolve for ergonomics and
//!   maintainability, but wire-format changes should be treated as explicit
//!   protocol changes rather than routine refactors.
//!
//! # Features
//!
//! - `std` (default): enables [`std::error::Error`] for [`FrameError`].
//! - without `std`: supports `no_std` with `alloc`.
//!
//! # MSRV
//!
//! This crate inherits the workspace `rust-version` and currently targets
//! Rust 1.93 or newer. That version is treated as the minimum supported Rust
//! version until it is explicitly raised.
//!
//! # Channel model
//!
//! Some channels carry typed messages decoded by
//! [`frame::Message::from_frame`], while
//! media channels intentionally remain raw payload channels:
//!
//! - typed: control, input, cursor, clipboard, file up/down, tiles
//! - raw: video, audio out, audio in, video in
//!
//! # Example
//!
//! ```
//! use bpane_protocol::ControlMessage;
//! use bpane_protocol::frame::Message;
//!
//! let frame = ControlMessage::Ping {
//!     seq: 7,
//!     timestamp_ms: 42,
//! }
//! .to_frame();
//!
//! let decoded = Message::from_frame(&frame).unwrap();
//! assert_eq!(
//!     decoded,
//!     Message::Control(ControlMessage::Ping {
//!         seq: 7,
//!         timestamp_ms: 42,
//!     })
//! );
//! ```

extern crate alloc;

pub mod channel;
pub mod frame;
pub mod types;

pub use channel::ChannelId;
pub use frame::{Frame, FrameDecoder, FrameDecoderError, FrameError};
pub use types::*;
