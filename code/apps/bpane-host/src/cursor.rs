//! X11 cursor control and forwarding (hide/show + cursor shape/move channel).
//! This mimics the remote cursor in the browser and hides the host cursor,
//! similar to Kasm.

use bpane_protocol::frame::Frame;

#[cfg(target_os = "linux")]
use {
    bpane_protocol::CursorMessage,
    std::time::Duration,
    tokio::sync::mpsc,
    x11rb::{
        connection::Connection, protocol::xfixes::ConnectionExt as XFixesExt,
        rust_connection::RustConnection,
    },
};

#[cfg(target_os = "linux")]
pub struct CursorHider {
    conn: RustConnection,
    root: u32,
}

#[cfg(target_os = "linux")]
impl CursorHider {
    pub fn new(display_name: &str) -> anyhow::Result<Self> {
        let (conn, screen_num) = RustConnection::connect(Some(display_name))?;
        let setup = conn.setup();
        let root = setup.roots[screen_num].root;
        conn.xfixes_query_version(6, 0)?.reply()?; // XFixes 6 is common on modern Xorg
                                                   // Hide the core cursor
        conn.xfixes_hide_cursor(root)?.check()?;
        tracing::debug!(display = %display_name, "cursor: XFixes hide applied");
        Ok(Self { conn, root })
    }
}

#[cfg(target_os = "linux")]
impl Drop for CursorHider {
    fn drop(&mut self) {
        // Best-effort show cursor again
        let _ = self.conn.xfixes_show_cursor(self.root);
        let _ = self.conn.flush();
    }
}

// Stub for non-Linux targets so code compiles unchanged.
#[cfg(not(target_os = "linux"))]
pub struct CursorHider;
#[cfg(not(target_os = "linux"))]
impl CursorHider {
    pub fn new(_display: &str) -> anyhow::Result<Self> {
        Ok(Self)
    }
}

/// Spawn a task that polls the X cursor shape/position and forwards it to the client.
/// Optionally updates a shared `AtomicU64` with packed cursor position (x << 32 | y)
/// so the DamageTracker can filter cursor-movement damage.
#[cfg(target_os = "linux")]
pub fn spawn_cursor_task(
    display: String,
    to_gateway: mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    spawn_cursor_task_with_pos(display, to_gateway, None)
}

/// Like `spawn_cursor_task`, but also writes the cursor position to a shared atomic.
#[cfg(target_os = "linux")]
pub fn spawn_cursor_task_with_pos(
    display: String,
    to_gateway: mpsc::Sender<Frame>,
    cursor_pos: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let (conn, screen_num) = match RustConnection::connect(Some(&display)) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("cursor: connect failed: {e}");
                return;
            }
        };
        match conn.xfixes_query_version(6, 0) {
            Ok(cookie) => {
                if let Err(e) = cookie.reply() {
                    tracing::warn!("cursor: XFixes reply failed: {e}");
                    return;
                }
            }
            Err(e) => {
                tracing::warn!("cursor: XFixes not available: {e}");
                return;
            }
        }
        let root = conn.setup().roots[screen_num].root;
        let mut last_serial: u32 = 0;
        let mut last_pos: (u16, u16) = (0, 0);
        // Adaptive polling: start at 60Hz, back off to 10Hz when idle.
        let mut idle_streak: u32 = 0;
        const IDLE_THRESHOLD: u32 = 30; // ~500ms at 60Hz before backing off

        loop {
            let mut changed = false;

            match conn.xfixes_get_cursor_image() {
                Ok(cookie) => match cookie.reply() {
                    Ok(img) => {
                        let serial = img.cursor_serial;
                        let x = img.x as u16;
                        let y = img.y as u16;

                        // Send shape when serial changes
                        if serial != last_serial {
                            last_serial = serial;
                            changed = true;
                            let mut data =
                                Vec::with_capacity(img.width as usize * img.height as usize * 4);
                            for pixel in img.cursor_image {
                                let a = ((pixel >> 24) & 0xFF) as u8;
                                let r = ((pixel >> 16) & 0xFF) as u8;
                                let g = ((pixel >> 8) & 0xFF) as u8;
                                let b = (pixel & 0xFF) as u8;
                                data.extend_from_slice(&[r, g, b, a]); // RGBA for the client canvas
                            }
                            let msg = CursorMessage::CursorShape {
                                width: img.width as u16,
                                height: img.height as u16,
                                hotspot_x: img.xhot as u8,
                                hotspot_y: img.yhot as u8,
                                data,
                            };
                            let _ = to_gateway.blocking_send(msg.to_frame());
                        }

                        // Send move if position changed
                        if (x, y) != last_pos {
                            last_pos = (x, y);
                            changed = true;
                            let msg = CursorMessage::CursorMove { x, y };
                            let _ = to_gateway.blocking_send(msg.to_frame());

                            // Update shared cursor position for damage filtering
                            if let Some(ref pos) = cursor_pos {
                                let packed = ((x as u64) << 32) | (y as u64);
                                pos.store(packed, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => tracing::warn!("cursor: reply failed: {e}"),
                },
                Err(e) => tracing::warn!("cursor: request failed: {e}"),
            }

            // Adaptive sleep: 60Hz when active, backs off to 10Hz when idle.
            if changed {
                idle_streak = 0;
            } else {
                idle_streak = idle_streak.saturating_add(1);
            }
            let sleep_ms = if idle_streak >= IDLE_THRESHOLD {
                100
            } else {
                16
            };
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
    })
}

#[cfg(not(target_os = "linux"))]
pub fn spawn_cursor_task(
    _display: String,
    _to_gateway: tokio::sync::mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async {})
}

#[cfg(not(target_os = "linux"))]
pub fn spawn_cursor_task_with_pos(
    _display: String,
    _to_gateway: tokio::sync::mpsc::Sender<Frame>,
    _cursor_pos: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async {})
}
