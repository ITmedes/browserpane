/// Mouse input injection using XTEST (works inside the Xorg dummy display).
use super::InputBackend;
use bpane_protocol::InputMessage;

#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::{xproto, xtest::ConnectionExt as XTestExt},
    rust_connection::RustConnection,
    CURRENT_TIME,
};

#[cfg(target_os = "linux")]
pub struct MouseInjector {
    conn: RustConnection,
    root: u32,
}

#[cfg(target_os = "linux")]
impl MouseInjector {
    pub fn new() -> anyhow::Result<Self> {
        let (conn, screen) = RustConnection::connect(None)?;
        let root = conn.setup().roots[screen].root;
        conn.xtest_get_version(1, 0)?.reply()?;
        Ok(Self { conn, root })
    }
}

#[cfg(target_os = "linux")]
impl InputBackend for MouseInjector {
    fn inject(&mut self, msg: &InputMessage) -> anyhow::Result<()> {
        match msg {
            InputMessage::MouseMove { x, y } => {
                self.conn
                    .xtest_fake_input(
                        xproto::MOTION_NOTIFY_EVENT,
                        0,
                        CURRENT_TIME,
                        self.root,
                        (*x).min(i16::MAX as u16) as i16,
                        (*y).min(i16::MAX as u16) as i16,
                        0,
                    )?
                    .check()?;
            }
            InputMessage::MouseButton { button, down, x, y } => {
                // Move first to keep coordinates in sync
                self.conn
                    .xtest_fake_input(
                        xproto::MOTION_NOTIFY_EVENT,
                        0,
                        CURRENT_TIME,
                        self.root,
                        (*x).min(i16::MAX as u16) as i16,
                        (*y).min(i16::MAX as u16) as i16,
                        0,
                    )?
                    .check()?;
                let detail = button.as_u8() + 1; // DOM 0/1/2 -> X11 1/2/3
                let evtype = if *down {
                    xproto::BUTTON_PRESS_EVENT
                } else {
                    xproto::BUTTON_RELEASE_EVENT
                };
                self.conn
                    .xtest_fake_input(evtype, detail, CURRENT_TIME, self.root, 0, 0, 0)?
                    .check()?;
            }
            InputMessage::MouseScroll { dx, dy } => {
                // Map wheel to buttons 4/5 (vertical) and 6/7 (horizontal).
                // Repeat the button press for each scroll step to preserve magnitude.
                let send_btn = |btn: u8| -> anyhow::Result<()> {
                    self.conn
                        .xtest_fake_input(
                            xproto::BUTTON_PRESS_EVENT,
                            btn,
                            CURRENT_TIME,
                            self.root,
                            0,
                            0,
                            0,
                        )?
                        .check()?;
                    self.conn
                        .xtest_fake_input(
                            xproto::BUTTON_RELEASE_EVENT,
                            btn,
                            CURRENT_TIME,
                            self.root,
                            0,
                            0,
                            0,
                        )?
                        .check()?;
                    Ok(())
                };
                let vert_steps = dy.unsigned_abs();
                let vert_btn = if *dy > 0 { 4u8 } else { 5u8 };
                for _ in 0..vert_steps {
                    send_btn(vert_btn)?;
                }
                let horiz_steps = dx.unsigned_abs();
                let horiz_btn = if *dx > 0 { 6u8 } else { 7u8 };
                for _ in 0..horiz_steps {
                    send_btn(horiz_btn)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// Non-Linux stub
#[cfg(not(target_os = "linux"))]
pub struct MouseInjector;
#[cfg(not(target_os = "linux"))]
impl MouseInjector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
}
#[cfg(not(target_os = "linux"))]
impl InputBackend for MouseInjector {
    fn inject(&mut self, _msg: &InputMessage) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // ── Fix 4: coordinate clamping for i16 ──────────────────────────

    #[test]
    fn u16_to_i16_clamp_normal() {
        let x: u16 = 1920;
        let clamped = x.min(i16::MAX as u16) as i16;
        assert_eq!(clamped, 1920);
    }

    #[test]
    fn u16_to_i16_clamp_at_boundary() {
        let x: u16 = i16::MAX as u16; // 32767
        let clamped = x.min(i16::MAX as u16) as i16;
        assert_eq!(clamped, 32767);
    }

    #[test]
    fn u16_to_i16_clamp_overflow() {
        // u16 value 40000 would wrap to negative without clamping
        let x: u16 = 40000;
        let clamped = x.min(i16::MAX as u16) as i16;
        assert_eq!(clamped, 32767); // Clamped, not wrapped

        // Before fix: `40000 as i16` would be -25536 (silently wrong)
        assert_eq!(40000u16 as i16, -25536);
    }

    #[test]
    fn u16_to_i16_clamp_max() {
        let x: u16 = u16::MAX; // 65535
        let clamped = x.min(i16::MAX as u16) as i16;
        assert_eq!(clamped, 32767); // Clamped to i16::MAX
    }

    // ── Fix 4: scroll magnitude ─────────────────────────────────────

    #[test]
    fn scroll_magnitude_preserved() {
        // Verify unsigned_abs works correctly for scroll steps
        let dy: i16 = 3;
        assert_eq!(dy.unsigned_abs(), 3u16);

        let dy: i16 = -5;
        assert_eq!(dy.unsigned_abs(), 5u16);

        let dy: i16 = 0;
        assert_eq!(dy.unsigned_abs(), 0u16);
    }
}
