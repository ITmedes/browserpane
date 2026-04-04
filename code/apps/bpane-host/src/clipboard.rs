//! X11 clipboard monitoring and setting.
//!
//! Monitors the X11 CLIPBOARD selection via XFixes SelectionNotify events
//! and forwards text content to the gateway. Incoming clipboard text from
//! the browser is set via `xclip`.

use bpane_protocol::frame::Frame;

#[cfg(target_os = "linux")]
use {
    bpane_protocol::ClipboardMessage,
    tokio::sync::mpsc,
    x11rb::{
        atom_manager,
        connection::Connection,
        protocol::{
            xfixes::{ConnectionExt as XFixesExt, SelectionEventMask},
            xproto::{
                Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, Property,
                SelectionNotifyEvent, WindowClass,
            },
            Event,
        },
        rust_connection::RustConnection,
    },
};

/// Maximum clipboard content size (1 MiB).
const MAX_CLIPBOARD_SIZE: usize = 1024 * 1024;

/// FNV-1a hash (matches the client-side implementation).
fn fnv_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Clipboard dedup and echo-prevention state.
///
/// Tracks hashes of sent and received content so that:
/// - Duplicate outgoing content is suppressed
/// - Content we just received from the browser is not echoed back
pub struct ClipboardState {
    last_sent_hash: u64,
    last_received_hash: u64,
}

impl ClipboardState {
    pub fn new() -> Self {
        Self {
            last_sent_hash: 0,
            last_received_hash: 0,
        }
    }

    /// Check whether outgoing clipboard content should be sent.
    /// Returns `true` if the content is new (not a duplicate, not an echo).
    pub fn should_send(&mut self, content: &[u8]) -> bool {
        if content.is_empty() || content.len() > MAX_CLIPBOARD_SIZE {
            return false;
        }
        if std::str::from_utf8(content).is_err() {
            return false;
        }
        let hash = fnv_hash(content);
        if hash == self.last_sent_hash || hash == self.last_received_hash {
            return false;
        }
        self.last_sent_hash = hash;
        true
    }

    /// Record the hash of content received from the browser, so the monitor
    /// can suppress the echo when XFixes fires for the same content.
    pub fn record_received(&mut self, content: &[u8]) {
        self.last_received_hash = fnv_hash(content);
    }
}

#[cfg(target_os = "linux")]
atom_manager! {
    pub ClipboardAtoms: AtomsCookie {
        CLIPBOARD,
        UTF8_STRING,
        BPANE_CLIPBOARD_PROP: b"BPANE_CLIPBOARD_PROP",
    }
}

/// Spawn a blocking task that monitors the X11 CLIPBOARD selection and sends
/// changes to the gateway.
#[cfg(target_os = "linux")]
pub fn spawn_clipboard_task(
    display: String,
    to_gateway: mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = clipboard_event_loop(&display, &to_gateway) {
            tracing::warn!("clipboard monitor exited: {e}");
        }
    })
}

#[cfg(target_os = "linux")]
fn clipboard_event_loop(
    display_name: &str,
    to_gateway: &mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    let (conn, screen_num) = RustConnection::connect(Some(display_name))?;
    let setup = conn.setup();
    let root = setup.roots[screen_num].root;

    let atoms = ClipboardAtoms::new(&conn)?.reply()?;

    // XFixes version check
    conn.xfixes_query_version(6, 0)?.reply()?;

    // Create a small hidden window as our selection requestor
    let win = conn.generate_id()?;
    conn.create_window(
        0, // depth: copy from parent
        win,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?
    .check()?;

    // Subscribe to clipboard ownership changes
    conn.xfixes_select_selection_input(
        win,
        atoms.CLIPBOARD,
        SelectionEventMask::SET_SELECTION_OWNER,
    )?
    .check()?;

    tracing::debug!("clipboard: monitoring started on {display_name}");

    let state = std::sync::Arc::new(std::sync::Mutex::new(ClipboardState::new()));
    CLIPBOARD_STATE
        .lock()
        .unwrap()
        .replace(std::sync::Arc::clone(&state));

    loop {
        let event = conn.wait_for_event()?;

        match event {
            Event::XfixesSelectionNotify(ev) => {
                if ev.selection != atoms.CLIPBOARD {
                    continue;
                }
                // Request the clipboard content
                conn.convert_selection(
                    win,
                    atoms.CLIPBOARD,
                    atoms.UTF8_STRING,
                    atoms.BPANE_CLIPBOARD_PROP,
                    ev.timestamp,
                )?
                .check()?;
            }
            Event::SelectionNotify(SelectionNotifyEvent { property, .. }) => {
                if property == Atom::from(AtomEnum::NONE) {
                    continue; // conversion failed
                }

                let reply = conn
                    .get_property(
                        true, // delete after reading
                        win,
                        atoms.BPANE_CLIPBOARD_PROP,
                        atoms.UTF8_STRING,
                        0,
                        (MAX_CLIPBOARD_SIZE / 4) as u32,
                    )?
                    .reply()?;

                let content = reply.value;
                let should_send = state.lock().map_or(false, |mut s| s.should_send(&content));
                if !should_send {
                    continue;
                }

                let msg = ClipboardMessage::Text { content };
                let _ = to_gateway.blocking_send(msg.to_frame());
                tracing::trace!("clipboard: sent text to browser");
            }
            _ => {}
        }
    }
}

/// Shared clipboard state so `set_clipboard` can coordinate with the monitor task.
#[cfg(target_os = "linux")]
static CLIPBOARD_STATE: std::sync::Mutex<Option<std::sync::Arc<std::sync::Mutex<ClipboardState>>>> =
    std::sync::Mutex::new(None);

/// Set the X11 clipboard content via `xclip`. Records the hash for echo prevention.
#[cfg(target_os = "linux")]
pub fn set_clipboard(display: &str, content: &[u8]) {
    if content.len() > MAX_CLIPBOARD_SIZE {
        tracing::warn!("clipboard: incoming content too large, ignoring");
        return;
    }

    // Record received hash so the monitor task suppresses the XFixes echo
    if let Ok(guard) = CLIPBOARD_STATE.lock() {
        if let Some(arc) = guard.as_ref() {
            if let Ok(mut s) = arc.lock() {
                s.record_received(content);
            }
        }
    }

    let result = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .env("DISPLAY", display)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(content)?;
            }
            child.wait()
        });

    match result {
        Ok(status) if status.success() => {
            tracing::trace!("clipboard: set remote clipboard ({} bytes)", content.len());
        }
        Ok(status) => {
            tracing::warn!("clipboard: xclip exited with {status}");
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!("clipboard: xclip not found — install xclip for clipboard sync");
        }
        Err(e) => {
            tracing::warn!("clipboard: xclip failed: {e}");
        }
    }
}

// Non-Linux stubs
#[cfg(not(target_os = "linux"))]
pub fn spawn_clipboard_task(
    _display: String,
    _to_gateway: tokio::sync::mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async {})
}

#[cfg(not(target_os = "linux"))]
pub fn set_clipboard(_display: &str, _content: &[u8]) {}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::frame::Message;
    use bpane_protocol::ClipboardMessage;

    #[test]
    fn fnv_hash_deterministic() {
        let a = fnv_hash(b"hello world");
        let b = fnv_hash(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn fnv_hash_different_inputs() {
        let a = fnv_hash(b"hello");
        let b = fnv_hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn fnv_hash_empty() {
        let h = fnv_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325); // FNV offset basis
    }

    #[test]
    fn state_dedup_same_content() {
        let mut state = ClipboardState::new();
        assert!(state.should_send(b"hello"));
        assert!(!state.should_send(b"hello")); // duplicate suppressed
    }

    #[test]
    fn state_allows_different_content() {
        let mut state = ClipboardState::new();
        assert!(state.should_send(b"hello"));
        assert!(state.should_send(b"world")); // different content passes
    }

    #[test]
    fn state_echo_prevention() {
        let mut state = ClipboardState::new();
        // Simulate receiving "hello" from the browser
        state.record_received(b"hello");
        // Now if X11 fires with the same content, it should be suppressed
        assert!(!state.should_send(b"hello"));
        // But different content should still pass
        assert!(state.should_send(b"something else"));
    }

    #[test]
    fn state_rejects_empty() {
        let mut state = ClipboardState::new();
        assert!(!state.should_send(b""));
    }

    #[test]
    fn state_rejects_oversized() {
        let mut state = ClipboardState::new();
        let big = vec![b'x'; MAX_CLIPBOARD_SIZE + 1];
        assert!(!state.should_send(&big));
    }

    #[test]
    fn state_rejects_invalid_utf8() {
        let mut state = ClipboardState::new();
        assert!(!state.should_send(&[0xFF, 0xFE, 0x80]));
    }

    #[test]
    fn state_accepts_max_size() {
        let mut state = ClipboardState::new();
        let content = vec![b'a'; MAX_CLIPBOARD_SIZE];
        assert!(state.should_send(&content));
    }

    #[test]
    fn clipboard_message_frame_round_trip() {
        let msg = ClipboardMessage::Text {
            content: b"test clipboard".to_vec(),
        };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        match decoded {
            Message::Clipboard(ClipboardMessage::Text { content }) => {
                assert_eq!(content, b"test clipboard");
            }
            other => panic!("expected Clipboard, got {:?}", other),
        }
    }

    #[test]
    fn set_clipboard_does_not_panic_without_xclip() {
        // On macOS / CI where xclip doesn't exist, this should not panic
        set_clipboard(":99", b"hello clipboard");
    }
}
