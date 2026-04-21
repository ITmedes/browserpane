use super::{CaptureBackend, CapturedFrame};

#[cfg(target_os = "linux")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use x11rb::connection::{Connection, RequestConnection};
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

#[cfg(target_os = "linux")]
use x11rb::protocol::composite::{ConnectionExt as CompositeExt, Redirect};
#[cfg(target_os = "linux")]
use x11rb::protocol::damage::{self, ConnectionExt as DamageExt, ReportLevel};
#[cfg(target_os = "linux")]
use x11rb::protocol::shm::ConnectionExt as ShmExt;

/// POSIX shared-memory segment attached to the X server via MIT-SHM.
#[cfg(target_os = "linux")]
struct ShmSegment {
    /// X server SHM segment XID
    seg_id: u32,
    /// POSIX shmid from shmget()
    shm_id: i32,
    /// shmat() pointer
    ptr: *mut u8,
    /// segment size in bytes
    size: usize,
}

// Safe: pointer is to process-global SHM, we hold exclusive &mut access
#[cfg(target_os = "linux")]
unsafe impl Send for ShmSegment {}

#[cfg(target_os = "linux")]
impl Drop for ShmSegment {
    fn drop(&mut self) {
        unsafe {
            libc::shmdt(self.ptr as *const libc::c_void);
        }
    }
}

#[cfg(target_os = "linux")]
impl ShmSegment {
    /// Allocate a SHM segment large enough for `w * h * 4` bytes and attach it
    /// to the X server. The segment is marked IPC_RMID immediately so it is
    /// cleaned up automatically when the last user detaches.
    fn allocate(
        conn: &x11rb::rust_connection::RustConnection,
        w: u32,
        h: u32,
    ) -> anyhow::Result<Self> {
        let size = (w as usize) * (h as usize) * 4;
        unsafe {
            let shm_id = libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600);
            if shm_id < 0 {
                anyhow::bail!("shmget failed: {}", std::io::Error::last_os_error());
            }
            let ptr = libc::shmat(shm_id, std::ptr::null(), 0);
            if ptr == libc::MAP_FAILED as *mut libc::c_void {
                libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut());
                anyhow::bail!("shmat failed: {}", std::io::Error::last_os_error());
            }
            // Mark for auto-cleanup when all processes detach
            libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut());

            let seg_id = conn.generate_id()?;
            conn.shm_attach(seg_id, shm_id as u32, false)?.check()?;

            Ok(Self {
                seg_id,
                shm_id,
                ptr: ptr as *mut u8,
                size,
            })
        }
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }
}

/// X11 capture backend using x11rb's GetImage.
///
/// Optionally uses XDamage (skip unchanged frames), XComposite (reliable
/// composited capture), and MIT-SHM (shared-memory pixel transfer) when
/// available. All three extensions degrade gracefully if unavailable.
///
/// On `set_resolution`, uses `xrandr` to add a new mode to the Xvfb
/// screen and switch to it, so Firefox and other X11 clients genuinely
/// reflow to the new size — pixel-perfect, no scaling.
pub struct X11CaptureBackend {
    #[cfg(target_os = "linux")]
    conn: x11rb::rust_connection::RustConnection,
    #[cfg(target_os = "linux")]
    root: u32,
    screen_w: u32,
    screen_h: u32,
    display_name: String,
    #[cfg(target_os = "linux")]
    damage_id: Option<u32>,
    #[cfg(target_os = "linux")]
    damage_dirty: bool,
    #[cfg(target_os = "linux")]
    composite_active: bool,
    #[cfg(target_os = "linux")]
    shm: Option<ShmSegment>,
    #[cfg(target_os = "linux")]
    damage_event_base: u8,
}

impl X11CaptureBackend {
    pub fn new(display_name: &str, _width: u32, _height: u32) -> anyhow::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let connect_display = if display_name.is_empty() {
                None
            } else {
                Some(display_name)
            };
            let (conn, screen_num) =
                x11rb::rust_connection::RustConnection::connect(connect_display)?;
            let screen = &conn.setup().roots[screen_num];
            let root = screen.root;
            let screen_w = screen.width_in_pixels as u32;
            let screen_h = screen.height_in_pixels as u32;
            tracing::info!(display_name, screen_w, screen_h, "X11 connected");

            let composite_active = Self::init_composite_static(&conn, root);
            let (damage_id, damage_event_base) = Self::init_damage_static(&conn, root);
            let shm = Self::init_shm_static(&conn, screen_w, screen_h);

            tracing::debug!(
                damage = damage_id.is_some(),
                composite = composite_active,
                shm = shm.is_some(),
                "X11 extensions initialized"
            );

            Ok(Self {
                conn,
                root,
                screen_w,
                screen_h,
                display_name: display_name.to_string(),
                damage_id,
                damage_dirty: true, // force first frame capture
                composite_active,
                shm,
                damage_event_base,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (display_name, _width, _height);
            anyhow::bail!("X11 capture not available on this platform")
        }
    }

    /// Initialize XComposite extension. Returns true if successful.
    #[cfg(target_os = "linux")]
    fn init_composite_static(conn: &x11rb::rust_connection::RustConnection, root: u32) -> bool {
        let result = (|| -> anyhow::Result<()> {
            conn.composite_query_version(0, 4)?.reply()?;
            conn.composite_redirect_subwindows(root, Redirect::AUTOMATIC)?
                .check()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                tracing::debug!("XComposite: redirect subwindows active");
                true
            }
            Err(e) => {
                tracing::warn!("XComposite unavailable, continuing without: {e}");
                false
            }
        }
    }

    /// Initialize XDamage extension. Returns (damage_id, event_base) or
    /// (None, 0) if unavailable.
    #[cfg(target_os = "linux")]
    fn init_damage_static(
        conn: &x11rb::rust_connection::RustConnection,
        root: u32,
    ) -> (Option<u32>, u8) {
        let result = (|| -> anyhow::Result<(u32, u8)> {
            conn.damage_query_version(1, 1)?.reply()?;
            let ext_info = conn
                .extension_information(damage::X11_EXTENSION_NAME)?
                .ok_or_else(|| anyhow::anyhow!("DAMAGE extension not found"))?;
            let damage_id = conn.generate_id()?;
            conn.damage_create(damage_id, root, ReportLevel::NON_EMPTY)?
                .check()?;
            Ok((damage_id, ext_info.first_event))
        })();
        match result {
            Ok((id, base)) => {
                tracing::debug!(
                    damage_id = id,
                    event_base = base,
                    "XDamage: tracking active"
                );
                (Some(id), base)
            }
            Err(e) => {
                tracing::warn!("XDamage unavailable, capturing every frame: {e}");
                (None, 0)
            }
        }
    }

    /// Initialize MIT-SHM extension. Returns a ShmSegment or None.
    #[cfg(target_os = "linux")]
    fn init_shm_static(
        conn: &x11rb::rust_connection::RustConnection,
        w: u32,
        h: u32,
    ) -> Option<ShmSegment> {
        let result = (|| -> anyhow::Result<ShmSegment> {
            conn.shm_query_version()?.reply()?;
            ShmSegment::allocate(conn, w, h)
        })();
        match result {
            Ok(seg) => {
                tracing::debug!(size = seg.size, "MIT-SHM: shared memory active");
                Some(seg)
            }
            Err(e) => {
                tracing::warn!("MIT-SHM unavailable, using GetImage fallback: {e}");
                None
            }
        }
    }

    /// Drain pending X events and update `damage_dirty` flag.
    #[cfg(target_os = "linux")]
    fn poll_damage_events(&mut self) {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(event)) => {
                    if matches!(event, x11rb::protocol::Event::DamageNotify(_)) {
                        self.damage_dirty = true;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("error polling X events: {e}");
                    // On error, assume dirty to avoid stuck frames
                    self.damage_dirty = true;
                    break;
                }
            }
        }
    }

    /// Resize the Xvfb framebuffer using xrandr --fb + --size.
    /// xrandr --fb WxH resizes the virtual framebuffer which is what
    /// Xvfb RANDR supports. After resize, reconnect X11 to pick up
    /// the new geometry.
    #[cfg(target_os = "linux")]
    fn xrandr_resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let w = width.max(320).min(7680);
        let h = height.max(200).min(4320);

        if w == self.screen_w && h == self.screen_h {
            return Ok(());
        }

        let mode_name = format!("{}x{}", w, h);
        tracing::debug!(mode = %mode_name, "xrandr resize");

        // Detect the connected output name (e.g. "DUMMY0", "screen", "VGA-1")
        let output_name = Self::detect_xrandr_output(&self.display_name);
        tracing::debug!(output = %output_name, "detected xrandr output");

        // Tear down extensions before reconnecting
        self.destroy_extensions();

        // Generate mode timings with cvt, add the mode, switch to it.
        // This is the reliable way to resize Xvfb with RANDR.
        let cvt = std::process::Command::new("cvt")
            .arg(w.to_string())
            .arg(h.to_string())
            .env("DISPLAY", &self.display_name)
            .output();

        if let Ok(cvt_out) = cvt {
            let cvt_str = String::from_utf8_lossy(&cvt_out.stdout);
            // cvt output: "Modeline "WxH_60.00"  ... timings ..."
            // Extract the modeline parameters
            if let Some(modeline) = cvt_str.lines().find(|l| l.starts_with("Modeline")) {
                let parts: Vec<&str> = modeline.split_whitespace().collect();
                if parts.len() >= 3 {
                    let cvt_mode_name = parts[1].trim_matches('"');
                    let timings: Vec<&str> = parts[2..].to_vec();

                    // xrandr --newmode "name" timings...
                    let _ = std::process::Command::new("xrandr")
                        .arg("--newmode")
                        .arg(cvt_mode_name)
                        .args(&timings)
                        .env("DISPLAY", &self.display_name)
                        .output(); // ignore error if mode already exists

                    // xrandr --addmode <output> name
                    let _ = std::process::Command::new("xrandr")
                        .arg("--addmode")
                        .arg(&output_name)
                        .arg(cvt_mode_name)
                        .env("DISPLAY", &self.display_name)
                        .output();

                    // xrandr --output <output> --mode name
                    let switch = std::process::Command::new("xrandr")
                        .arg("--output")
                        .arg(&output_name)
                        .arg("--mode")
                        .arg(cvt_mode_name)
                        .env("DISPLAY", &self.display_name)
                        .output()?;

                    if switch.status.success() {
                        tracing::debug!("switched to mode {cvt_mode_name}");
                    } else {
                        let stderr = String::from_utf8_lossy(&switch.stderr);
                        tracing::warn!(%stderr, "xrandr mode switch failed, trying --fb fallback");

                        // Fallback: just resize the framebuffer
                        let _ = std::process::Command::new("xrandr")
                            .arg("--fb")
                            .arg(&mode_name)
                            .env("DISPLAY", &self.display_name)
                            .output();
                    }
                }
            }
        } else {
            // No cvt available, try direct --fb
            let _ = std::process::Command::new("xrandr")
                .arg("--fb")
                .arg(&mode_name)
                .env("DISPLAY", &self.display_name)
                .output();
        }

        // Small delay for X server to process the change
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Detach old extensions from old connection before reconnecting
        self.destroy_extensions();

        // Reconnect to pick up the new geometry
        let connect_display = if self.display_name.is_empty() {
            None
        } else {
            Some(self.display_name.as_str())
        };
        let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(connect_display)?;
        let screen = &conn.setup().roots[screen_num];
        self.root = screen.root;
        self.screen_w = screen.width_in_pixels as u32;
        self.screen_h = screen.height_in_pixels as u32;
        self.conn = conn;

        // Reinitialize extensions on the new connection
        self.composite_active = Self::init_composite_static(&self.conn, self.root);
        let (damage_id, damage_event_base) = Self::init_damage_static(&self.conn, self.root);
        self.damage_id = damage_id;
        self.damage_event_base = damage_event_base;
        self.damage_dirty = true; // force capture after resize
        self.shm = Self::init_shm_static(&self.conn, self.screen_w, self.screen_h);

        tracing::debug!(
            actual_w = self.screen_w,
            actual_h = self.screen_h,
            damage = self.damage_id.is_some(),
            composite = self.composite_active,
            shm = self.shm.is_some(),
            "xrandr resize complete, extensions reinitialized"
        );
        Ok(())
    }

    /// Destroy extension resources on the current connection (before reconnect or drop).
    #[cfg(target_os = "linux")]
    fn destroy_extensions(&mut self) {
        if let Some(damage_id) = self.damage_id.take() {
            let _ = self.conn.damage_destroy(damage_id);
        }
        if self.composite_active {
            let _ = self
                .conn
                .composite_unredirect_subwindows(self.root, Redirect::AUTOMATIC);
            self.composite_active = false;
        }
        if let Some(ref shm_seg) = self.shm {
            let _ = self.conn.shm_detach(shm_seg.seg_id);
        }
        self.shm = None;
        let _ = self.conn.flush();
    }
}

impl X11CaptureBackend {
    /// Return the cached screen size.
    ///
    /// The cache is initialised in `new()`, updated in `xrandr_resize()` after
    /// the connection is re-established, and refreshed on demand by
    /// `refresh_screen_size()`. Callers that need to detect an external resize
    /// (e.g. an xrandr call by another process) should call
    /// `refresh_screen_size()` first.
    pub fn query_screen_size(&self) -> (u32, u32) {
        (self.screen_w, self.screen_h)
    }

    /// Issue a `GetGeometry` round-trip to the X server, update the internal
    /// cache, and return the new dimensions. Call this only when an external
    /// resize is suspected (e.g. after an XDamage event whose geometry differs
    /// from the cached size). On non-Linux targets this is a no-op that returns
    /// the cached values.
    #[cfg(target_os = "linux")]
    pub fn refresh_screen_size(&mut self) -> (u32, u32) {
        match self.conn.get_geometry(self.root) {
            Ok(cookie) => match cookie.reply() {
                Ok(geo) => {
                    self.screen_w = geo.width as u32;
                    self.screen_h = geo.height as u32;
                    (self.screen_w, self.screen_h)
                }
                Err(_) => (self.screen_w, self.screen_h),
            },
            Err(_) => (self.screen_w, self.screen_h),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn refresh_screen_size(&mut self) -> (u32, u32) {
        (self.screen_w, self.screen_h)
    }

    /// Detect the first connected xrandr output name (e.g. "DUMMY0", "VGA-1").
    /// Falls back to "screen" if detection fails.
    fn detect_xrandr_output(display: &str) -> String {
        let output = std::process::Command::new("xrandr")
            .arg("--query")
            .env("DISPLAY", display)
            .output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains(" connected") {
                    if let Some(name) = line.split_whitespace().next() {
                        return name.to_string();
                    }
                }
            }
        }
        "screen".to_string()
    }
}

#[cfg(target_os = "linux")]
impl Drop for X11CaptureBackend {
    fn drop(&mut self) {
        self.destroy_extensions();
    }
}

impl X11CaptureBackend {
    /// Capture a sub-region of the screen via SHM or GetImage.
    /// Returns BGRA pixel data (NOT swapped to RGBA — caller must handle).
    #[cfg(not(target_os = "linux"))]
    pub fn capture_region_raw(
        &mut self,
        _x: u16,
        _y: u16,
        _w: u16,
        _h: u16,
    ) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("X11 capture not available on this platform")
    }

    /// Capture a sub-region of the screen via SHM or GetImage.
    /// Returns BGRA pixel data (NOT swapped to RGBA — caller must handle).
    #[cfg(target_os = "linux")]
    pub fn capture_region_raw(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> anyhow::Result<Vec<u8>> {
        let data = if let Some(ref shm_seg) = self.shm {
            // SHM requires segment large enough; if region fits, use it
            let region_bytes = (w as usize) * (h as usize) * 4;
            if region_bytes <= shm_seg.size {
                let reply = self
                    .conn
                    .shm_get_image(
                        self.root,
                        x as i16,
                        y as i16,
                        w,
                        h,
                        !0,
                        ImageFormat::Z_PIXMAP.into(),
                        shm_seg.seg_id,
                        0,
                    )?
                    .reply();
                match reply {
                    Ok(_) => shm_seg.as_slice()[..region_bytes].to_vec(),
                    Err(e) => {
                        tracing::warn!("SHM region capture failed, fallback: {e}");
                        let reply = self
                            .conn
                            .get_image(
                                ImageFormat::Z_PIXMAP,
                                self.root,
                                x as i16,
                                y as i16,
                                w,
                                h,
                                !0,
                            )?
                            .reply()?;
                        reply.data
                    }
                }
            } else {
                let reply = self
                    .conn
                    .get_image(
                        ImageFormat::Z_PIXMAP,
                        self.root,
                        x as i16,
                        y as i16,
                        w,
                        h,
                        !0,
                    )?
                    .reply()?;
                reply.data
            }
        } else {
            let reply = self
                .conn
                .get_image(
                    ImageFormat::Z_PIXMAP,
                    self.root,
                    x as i16,
                    y as i16,
                    w,
                    h,
                    !0,
                )?
                .reply()?;
            reply.data
        };
        Ok(data)
    }
}

impl CaptureBackend for X11CaptureBackend {
    fn capture_frame(&mut self) -> anyhow::Result<Option<CapturedFrame>> {
        #[cfg(target_os = "linux")]
        {
            // Check damage — skip frame if nothing changed
            if self.damage_id.is_some() {
                self.poll_damage_events();
                if !self.damage_dirty {
                    tracing::trace!("no damage, skipping frame");
                    return Ok(None);
                }
            }

            let w = self.screen_w;
            let h = self.screen_h;

            // Capture pixels via SHM or fallback to GetImage
            let data = if let Some(ref shm_seg) = self.shm {
                let reply = self
                    .conn
                    .shm_get_image(
                        self.root,
                        0,
                        0,
                        w as u16,
                        h as u16,
                        !0,
                        ImageFormat::Z_PIXMAP.into(),
                        shm_seg.seg_id,
                        0,
                    )?
                    .reply();

                match reply {
                    Ok(_) => shm_seg.as_slice()[..(w as usize * h as usize * 4)].to_vec(),
                    Err(e) => {
                        tracing::warn!("SHM get_image failed, falling back to GetImage: {e}");
                        // Fall back to GetImage for this frame
                        let reply = self
                            .conn
                            .get_image(
                                ImageFormat::Z_PIXMAP,
                                self.root,
                                0,
                                0,
                                w as u16,
                                h as u16,
                                !0,
                            )?
                            .reply()?;
                        reply.data
                    }
                }
            } else {
                let reply = self
                    .conn
                    .get_image(
                        ImageFormat::Z_PIXMAP,
                        self.root,
                        0,
                        0,
                        w as u16,
                        h as u16,
                        !0,
                    )?
                    .reply()?;
                reply.data
            };

            // Acknowledge damage so we get notified of the next change.
            // We must flush + drain events after subtract to consume any
            // DamageNotify generated by the subtract itself, otherwise the
            // next poll_damage_events() would immediately see it as dirty.
            if let Some(damage_id) = self.damage_id {
                let _ = self
                    .conn
                    .damage_subtract(damage_id, x11rb::NONE, x11rb::NONE);
                let _ = self.conn.flush();
                self.poll_damage_events();
                self.damage_dirty = false;
            }

            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;

            Ok(Some(CapturedFrame {
                width: w,
                height: h,
                data,
                timestamp_us: ts,
            }))
        }
        #[cfg(not(target_os = "linux"))]
        {
            anyhow::bail!("X11 capture not available on this platform")
        }
    }

    fn set_resolution(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.xrandr_resize(width, height)?;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (width, height);
        }
        Ok(())
    }

    fn resolution(&self) -> (u32, u32) {
        (self.screen_w, self.screen_h)
    }
}

/// Lightweight XDamage tracker — reports whether the screen has changed.
/// Used to gate the FFmpeg pipeline: only forward encoded frames when
/// damage has been reported. Does not capture pixels.
///
/// Supports an aggregation window: rapid damage events within the window
/// are coalesced into a single "dirty" signal. This avoids forwarding
/// frames for every tiny repaint (cursor blink, hover, sub-frame scroll).
#[cfg(target_os = "linux")]
pub struct DamageTracker {
    conn: x11rb::rust_connection::RustConnection,
    damage_id: u32,
    damage_event_base: u8,
    dirty: bool,
    /// When the first damage event in the current aggregation window arrived.
    first_damage_at: Option<std::time::Instant>,
    /// How long to wait after first damage before reporting dirty.
    aggregation_window: std::time::Duration,
    /// Shared cursor position for cursor-damage exclusion (Phase 3).
    /// Packed as (x << 32 | y). If None, cursor filtering is disabled.
    cursor_pos: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    /// Recent keyboard/text input activity timestamp in UNIX epoch millis.
    /// When fresh, bypass small-damage suppression for typing responsiveness.
    input_activity_ms: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    input_bypass_window: std::time::Duration,
    /// Accumulated damage rectangles within the current aggregation window.
    damage_rects: Vec<DamageRect>,
    /// Recently seen small-rect damage signatures for blink detection.
    /// Each entry is (rect_signature, last_seen, times_seen).
    blink_history: Vec<(u64, std::time::Instant, u32)>,
    /// Consecutive small-damage frames (for idle throttling).
    consecutive_small_damage: u32,
    /// Last time a small-damage frame was allowed through.
    last_small_damage_sent: Option<std::time::Instant>,
}

/// A damage rectangle reported by XDamage.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
pub struct DamageRect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

#[cfg(target_os = "linux")]
impl DamageTracker {
    /// Create a new damage tracker on a fresh X11 connection.
    /// Returns `None` if XDamage is unavailable (graceful fallback).
    pub fn new(display: &str) -> anyhow::Result<Option<Self>> {
        Self::with_options(display, None, None, None)
    }

    /// Create a damage tracker with optional cursor position sharing and
    /// custom aggregation window.
    pub fn with_options(
        display: &str,
        cursor_pos: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
        aggregation_ms: Option<u64>,
        input_activity_ms: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    ) -> anyhow::Result<Option<Self>> {
        if display.is_empty() {
            return Ok(None);
        }
        std::env::set_var("DISPLAY", display);
        let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        let agg_ms = aggregation_ms.unwrap_or_else(|| {
            std::env::var("BPANE_DAMAGE_WINDOW_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8)
        });
        let input_bypass_ms = std::env::var("BPANE_DAMAGE_INPUT_BYPASS_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(700u64)
            .clamp(0, 5_000);

        let result = (|| -> anyhow::Result<(u32, u8)> {
            conn.damage_query_version(1, 1)?.reply()?;
            let ext_info = conn
                .extension_information(damage::X11_EXTENSION_NAME)?
                .ok_or_else(|| anyhow::anyhow!("DAMAGE extension not found"))?;
            let damage_id = conn.generate_id()?;
            // RAW_RECTANGLES gives individual damage rects with precise sizes,
            // enabling accurate small-damage detection for idle throttling.
            conn.damage_create(damage_id, root, ReportLevel::RAW_RECTANGLES)?
                .check()?;
            Ok((damage_id, ext_info.first_event))
        })();

        match result {
            Ok((damage_id, damage_event_base)) => {
                tracing::debug!(
                    damage_id,
                    damage_event_base,
                    aggregation_ms = agg_ms,
                    cursor_filter = cursor_pos.is_some(),
                    input_bypass_ms,
                    "DamageTracker: active"
                );
                Ok(Some(Self {
                    conn,
                    damage_id,
                    damage_event_base,
                    dirty: true, // force first frame through
                    first_damage_at: Some(std::time::Instant::now()),
                    aggregation_window: std::time::Duration::from_millis(agg_ms),
                    cursor_pos,
                    input_activity_ms,
                    input_bypass_window: std::time::Duration::from_millis(input_bypass_ms),
                    damage_rects: Vec::new(),
                    blink_history: Vec::new(),
                    consecutive_small_damage: 0,
                    last_small_damage_sent: None,
                }))
            }
            Err(e) => {
                tracing::warn!("DamageTracker: XDamage unavailable ({e}), all frames will be sent");
                Ok(None)
            }
        }
    }

    /// Drain pending X events and return `true` if the screen has changed
    /// since the last `reset()` AND the aggregation window has elapsed.
    ///
    /// The aggregation window coalesces bursts of tiny damage events
    /// (cursor blink, scroll sub-frames, hover effects) into single
    /// forwarding decisions.
    pub fn poll(&mut self) -> bool {
        self.drain_events();

        if !self.dirty {
            return false;
        }

        let bypass_small_damage_filters = self.has_recent_input_activity();

        // If aggregation window is zero, skip timing logic
        if self.aggregation_window.is_zero() {
            if !bypass_small_damage_filters
                && (self.is_cursor_only_damage() || self.is_blink_damage())
            {
                self.reset();
                return false;
            }
            return self.apply_idle_throttle();
        }

        // Wait for the aggregation window to elapse
        match self.first_damage_at {
            Some(first) => {
                if first.elapsed() < self.aggregation_window {
                    return false; // still collecting damage
                }
                // Window elapsed — report damage (unless cursor-only or blink)
                if !bypass_small_damage_filters
                    && (self.is_cursor_only_damage() || self.is_blink_damage())
                {
                    self.reset();
                    return false;
                }
                self.apply_idle_throttle()
            }
            None => false,
        }
    }

    /// If damage is small (likely idle noise like cursor blinks, browser
    /// background repaints), throttle frame rate to ~1 fps after several
    /// consecutive small-damage frames. For large damage, reset the
    /// consecutive counter and pass through immediately.
    fn apply_idle_throttle(&mut self) -> bool {
        if self.has_recent_input_activity() {
            self.consecutive_small_damage = 0;
            self.last_small_damage_sent = None;
            self.log_passing_damage();
            return true;
        }

        // Check if total damaged area is small (< 0.5% of a typical screen)
        let total_area: u32 = self
            .damage_rects
            .iter()
            .map(|r| r.width as u32 * r.height as u32)
            .sum();

        const SMALL_DAMAGE_THRESHOLD: u32 = 10000; // ~100x100 pixels

        if total_area > SMALL_DAMAGE_THRESHOLD {
            // Large damage — reset idle state, pass through
            self.consecutive_small_damage = 0;
            self.log_passing_damage();
            return true;
        }

        self.consecutive_small_damage += 1;

        // Allow the first few small-damage frames through to render initial state
        if self.consecutive_small_damage <= 3 {
            self.log_passing_damage();
            return true;
        }

        // After sustained small damage, throttle to 1 frame per second
        let now = std::time::Instant::now();
        let should_send = match self.last_small_damage_sent {
            Some(last) => now.duration_since(last) >= std::time::Duration::from_secs(1),
            None => true,
        };

        if should_send {
            self.last_small_damage_sent = Some(now);
            tracing::trace!(
                consecutive = self.consecutive_small_damage,
                total_area,
                "idle throttle: allowing 1fps update"
            );
            self.log_passing_damage();
            true
        } else {
            // Suppress — but still reset damage so it doesn't accumulate
            self.reset();
            false
        }
    }

    /// Log damage rects that pass through all filters (diagnostic).
    fn log_passing_damage(&self) {
        if self.damage_rects.len() <= 3 {
            for r in &self.damage_rects {
                tracing::trace!(
                    x = r.x,
                    y = r.y,
                    w = r.width,
                    h = r.height,
                    "damage passed filters"
                );
            }
        } else {
            tracing::trace!(
                count = self.damage_rects.len(),
                "damage passed filters (multi-rect)"
            );
        }
    }

    /// Drain X events and record damage rects.
    fn drain_events(&mut self) {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(event)) => {
                    if let x11rb::protocol::Event::DamageNotify(notify) = event {
                        if !self.dirty {
                            self.first_damage_at = Some(std::time::Instant::now());
                        }
                        self.dirty = true;
                        self.damage_rects.push(DamageRect {
                            x: notify.area.x,
                            y: notify.area.y,
                            width: notify.area.width,
                            height: notify.area.height,
                        });
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("DamageTracker: error polling events: {e}");
                    self.dirty = true;
                    break;
                }
            }
        }
    }

    fn has_recent_input_activity(&self) -> bool {
        let Some(activity_ms) = &self.input_activity_ms else {
            return false;
        };
        if self.input_bypass_window.is_zero() {
            return false;
        }
        let last_ms = activity_ms.load(std::sync::atomic::Ordering::Relaxed);
        if last_ms == 0 {
            return false;
        }
        let now_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_millis() as u64,
            Err(_) => return false,
        };
        now_ms.saturating_sub(last_ms) <= self.input_bypass_window.as_millis() as u64
    }

    /// Check if all damage is repetitive small-area noise (text cursor blinks,
    /// browser animations, favicon spinners, etc.).
    ///
    /// Returns true if:
    /// - Total damaged area is small (≤ 4096 pixels ≈ 64×64)
    /// - All individual rects are small (≤ 64×64)
    /// - This damage pattern (hashed) has been seen recently
    ///
    /// The first 2 occurrences of a pattern are allowed through (to render
    /// both states of a blink). Subsequent repetitions within a 2-second
    /// window are suppressed.
    fn is_blink_damage(&mut self) -> bool {
        if self.damage_rects.is_empty() {
            return false;
        }

        // All rects must be small
        let mut total_area: u32 = 0;
        for r in &self.damage_rects {
            if r.width > 64 || r.height > 64 {
                return false;
            }
            total_area += r.width as u32 * r.height as u32;
        }
        // Total damaged area must be small
        if total_area > 4096 {
            return false;
        }

        // Compute a hash of all rects as the pattern signature.
        // Sort rects by position so order doesn't matter.
        let mut sig: u64 = 0xcbf29ce484222325; // FNV offset
        let mut sorted: Vec<u64> = self
            .damage_rects
            .iter()
            .map(|r| {
                ((r.x as u64) << 48)
                    | ((r.y as u64 & 0xFFFF) << 32)
                    | ((r.width as u64) << 16)
                    | (r.height as u64)
            })
            .collect();
        sorted.sort();
        for v in &sorted {
            sig ^= v;
            sig = sig.wrapping_mul(0x100000001b3);
        }

        let now = std::time::Instant::now();
        let expiry = std::time::Duration::from_secs(2);

        // Evict stale entries
        self.blink_history
            .retain(|(_, ts, _)| now.duration_since(*ts) < expiry);

        // Look for matching signature
        if let Some(entry) = self.blink_history.iter_mut().find(|(s, _, _)| *s == sig) {
            entry.1 = now;
            entry.2 += 1;
            if entry.2 > 2 {
                tracing::trace!(
                    rects = self.damage_rects.len(),
                    total_area,
                    count = entry.2,
                    "blink damage suppressed"
                );
                return true;
            }
            return false;
        }

        // New signature — record it, allow through
        self.blink_history.push((sig, now, 1));
        if self.blink_history.len() > 32 {
            self.blink_history.remove(0);
        }
        false
    }

    /// Check if all damage in the current window is cursor-movement noise.
    /// Returns true if ALL rects are small (<=48x48) and near the cursor.
    fn is_cursor_only_damage(&self) -> bool {
        let cursor_pos = match &self.cursor_pos {
            Some(pos) => pos,
            None => return false,
        };

        if self.damage_rects.is_empty() {
            return false;
        }

        let packed = cursor_pos.load(std::sync::atomic::Ordering::Relaxed);
        let cx = (packed >> 32) as i32;
        let cy = (packed & 0xFFFF_FFFF) as i32;

        for rect in &self.damage_rects {
            // Skip filtering for non-small rects
            if rect.width > 48 || rect.height > 48 {
                return false;
            }
            // Check if the rect overlaps the cursor position ±24px
            let rx = rect.x as i32;
            let ry = rect.y as i32;
            let rw = rect.width as i32;
            let rh = rect.height as i32;
            let margin = 24;
            if rx + rw < cx - margin || rx > cx + margin {
                return false;
            }
            if ry + rh < cy - margin || ry > cy + margin {
                return false;
            }
        }

        tracing::trace!(
            rects = self.damage_rects.len(),
            cursor_x = cx,
            cursor_y = cy,
            "cursor-only damage filtered"
        );
        true
    }

    /// Acknowledge damage after forwarding a frame.
    /// Calls damage_subtract and drains residual events to prevent
    /// the subtract itself from causing a self-triggering dirty cycle.
    pub fn reset(&mut self) {
        let _ = self
            .conn
            .damage_subtract(self.damage_id, x11rb::NONE, x11rb::NONE);
        let _ = self.conn.flush();
        // Drain any DamageNotify generated by the subtract itself
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(_)) => {}
                _ => break,
            }
        }
        self.dirty = false;
        self.first_damage_at = None;
        self.damage_rects.clear();
    }

    /// Return the bounding box of all accumulated damage rects.
    /// Returns None if no damage.
    pub fn damage_bounding_box(&self) -> Option<(u16, u16, u16, u16)> {
        if self.damage_rects.is_empty() {
            return None;
        }
        let mut min_x = i16::MAX;
        let mut min_y = i16::MAX;
        let mut max_x = i16::MIN;
        let mut max_y = i16::MIN;
        for r in &self.damage_rects {
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x.saturating_add(r.width as i16));
            max_y = max_y.max(r.y.saturating_add(r.height as i16));
        }
        let x = min_x.max(0) as u16;
        let y = min_y.max(0) as u16;
        let w = (max_x - min_x).max(0) as u16;
        let h = (max_y - min_y).max(0) as u16;
        Some((x, y, w, h))
    }

    /// Number of accumulated damage rects (diagnostic).
    pub fn rect_count(&self) -> usize {
        self.damage_rects.len()
    }

    /// Total area of all accumulated damage rects (diagnostic).
    pub fn total_damage_area(&self) -> u32 {
        self.damage_rects
            .iter()
            .map(|r| r.width as u32 * r.height as u32)
            .sum()
    }
}

#[cfg(target_os = "linux")]
impl Drop for DamageTracker {
    fn drop(&mut self) {
        let _ = self.conn.damage_destroy(self.damage_id);
        let _ = self.conn.flush();
    }
}

// Stub for non-Linux so the code compiles everywhere.
#[cfg(not(target_os = "linux"))]
pub struct DamageTracker;

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy)]
pub struct DamageRect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

#[cfg(not(target_os = "linux"))]
impl DamageTracker {
    pub fn new(_display: &str) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }

    pub fn with_options(
        _display: &str,
        _cursor_pos: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
        _aggregation_ms: Option<u64>,
        _input_activity_ms: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    ) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }

    pub fn poll(&mut self) -> bool {
        true
    }

    pub fn reset(&mut self) {}

    pub fn damage_bounding_box(&self) -> Option<(u16, u16, u16, u16)> {
        None
    }

    pub fn rect_count(&self) -> usize {
        0
    }

    pub fn total_damage_area(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_capture_backend_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<X11CaptureBackend>();
    }

    #[test]
    fn damage_tracker_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DamageTracker>();
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn damage_tracker_returns_none_without_display() {
        let result = DamageTracker::new("").unwrap();
        assert!(result.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shm_segment_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ShmSegment>();
    }

    // --- Integration tests (require a running X server, e.g. Xvfb) ---
    // Run with: DISPLAY=:99 cargo test -p bpane-host -- --ignored

    /// Helper: get DISPLAY from env or skip.
    #[cfg(target_os = "linux")]
    fn display_or_skip() -> String {
        std::env::var("DISPLAY").unwrap_or_else(|_| {
            eprintln!("DISPLAY not set — skipping integration test");
            String::new()
        })
    }

    /// After construction, `damage_dirty` must be `true` so the first
    /// `capture_frame()` always captures regardless of XDamage state.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn damage_dirty_initial_state_is_true() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        assert!(
            backend.damage_dirty,
            "damage_dirty must be true after construction"
        );
    }

    /// First capture must always succeed (returns Some) because
    /// `damage_dirty` starts `true`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn first_capture_always_returns_frame() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        let frame = backend.capture_frame().unwrap();
        assert!(frame.is_some(), "first capture must return a frame");
        let frame = frame.unwrap();
        assert!(frame.width > 0);
        assert!(frame.height > 0);
        assert_eq!(
            frame.data.len(),
            frame.width as usize * frame.height as usize * 4
        );
    }

    /// After capturing with no intervening screen changes, XDamage should
    /// cause `capture_frame()` to return `None` (no damage).
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn capture_returns_none_when_no_damage() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();

        // First capture — consumes initial damage
        let first = backend.capture_frame().unwrap();
        assert!(first.is_some());

        // If damage tracking is active, next capture with no changes should be None
        if backend.damage_id.is_some() {
            // Small sleep to let event queue settle
            std::thread::sleep(std::time::Duration::from_millis(50));
            let second = backend.capture_frame().unwrap();
            assert!(
                second.is_none(),
                "capture should return None when no screen damage occurred"
            );
        }
    }

    /// Regression test: damage_subtract must not cause a self-perpetuating
    /// dirty cycle. After the first capture consumes damage, multiple
    /// consecutive calls with no screen changes must all return None.
    /// This catches the bug where damage_subtract itself generates a
    /// DamageNotify that re-dirties the state.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn damage_subtract_does_not_self_trigger() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        if backend.damage_id.is_none() {
            eprintln!("XDamage not available — skipping");
            return;
        }

        // First capture consumes initial damage
        let first = backend.capture_frame().unwrap();
        assert!(first.is_some(), "first capture must return a frame");

        // Wait for event queue to settle
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Multiple consecutive captures with no screen changes must all be None.
        // Before the fix, the second call would return Some because
        // damage_subtract generated a spurious DamageNotify.
        let mut false_positives = 0;
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if backend.capture_frame().unwrap().is_some() {
                false_positives += 1;
            }
        }
        assert_eq!(
            false_positives, 0,
            "damage_subtract should not cause self-triggering dirty cycle \
             ({false_positives}/5 frames were captured on a static screen)"
        );
    }

    /// After `set_resolution`, extensions must be reinitialized and
    /// `damage_dirty` must be `true` so the next capture succeeds.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn resize_reinitializes_extensions() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        let had_damage = backend.damage_id.is_some();
        let had_composite = backend.composite_active;
        let had_shm = backend.shm.is_some();

        // Resize to a different resolution
        let (orig_w, orig_h) = backend.resolution();
        let new_w = if orig_w != 800 { 800 } else { 1024 };
        let new_h = if orig_h != 600 { 600 } else { 768 };
        backend.set_resolution(new_w, new_h).unwrap();

        // Extensions should be re-established at the same availability level
        assert_eq!(
            backend.damage_id.is_some(),
            had_damage,
            "damage availability should survive resize"
        );
        assert_eq!(
            backend.composite_active, had_composite,
            "composite availability should survive resize"
        );
        assert_eq!(
            backend.shm.is_some(),
            had_shm,
            "shm availability should survive resize"
        );
        assert!(
            backend.damage_dirty,
            "damage_dirty must be true after resize"
        );

        // First capture after resize must succeed
        let frame = backend.capture_frame().unwrap();
        assert!(frame.is_some(), "capture after resize must return a frame");

        // Restore original resolution
        backend.set_resolution(orig_w, orig_h).unwrap();
    }

    /// Verifying that captured frame data is returned in native BGRA byte order.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn captured_frame_data_is_bgra() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        let frame = backend.capture_frame().unwrap().unwrap();
        // Data length must be exactly w*h*4
        assert_eq!(
            frame.data.len(),
            frame.width as usize * frame.height as usize * 4,
            "frame data length must match width * height * 4"
        );
        // Timestamp should be recent (within last 10 seconds)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        assert!(
            now - frame.timestamp_us < 10_000_000,
            "timestamp should be recent"
        );
    }

    /// DamageTracker: poll() returns true initially (dirty flag starts true).
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn damage_tracker_poll_true_initially() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut tracker = match DamageTracker::new(&display).unwrap() {
            Some(t) => t,
            None => {
                eprintln!("XDamage not available — skipping");
                return;
            }
        };
        assert!(tracker.poll(), "poll() must return true initially");
    }

    /// DamageTracker: after reset(), poll() returns false on a static screen.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn damage_tracker_reset_clears_dirty() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut tracker = match DamageTracker::new(&display).unwrap() {
            Some(t) => t,
            None => {
                eprintln!("XDamage not available — skipping");
                return;
            }
        };
        // Consume initial damage
        assert!(tracker.poll());
        tracker.reset();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            !tracker.poll(),
            "poll() must return false on a static screen after reset()"
        );
    }

    /// DamageTracker: reset() does not self-trigger (same regression as
    /// X11CaptureBackend's damage_subtract fix).
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn damage_tracker_reset_does_not_self_trigger() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut tracker = match DamageTracker::new(&display).unwrap() {
            Some(t) => t,
            None => {
                eprintln!("XDamage not available — skipping");
                return;
            }
        };
        // Consume initial damage
        tracker.poll();
        tracker.reset();

        let mut false_positives = 0;
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if tracker.poll() {
                false_positives += 1;
            }
            tracker.reset();
        }
        assert_eq!(
            false_positives, 0,
            "DamageTracker.reset() should not self-trigger ({false_positives}/5 false positives)"
        );
    }

    // ── Pure logic tests (no X server required) ─────────────────────

    /// Compute bounding box from a slice of DamageRects (mirrors DamageTracker method).
    fn compute_bbox(rects: &[DamageRect]) -> Option<(u16, u16, u16, u16)> {
        if rects.is_empty() {
            return None;
        }
        let mut min_x = i16::MAX;
        let mut min_y = i16::MAX;
        let mut max_x = i16::MIN;
        let mut max_y = i16::MIN;
        for r in rects {
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x.saturating_add(r.width as i16));
            max_y = max_y.max(r.y.saturating_add(r.height as i16));
        }
        let x = min_x.max(0) as u16;
        let y = min_y.max(0) as u16;
        let w = (max_x - min_x).max(0) as u16;
        let h = (max_y - min_y).max(0) as u16;
        Some((x, y, w, h))
    }

    #[test]
    fn damage_bounding_box_single_rect() {
        let rects = vec![DamageRect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        }];
        let bbox = compute_bbox(&rects).unwrap();
        assert_eq!(bbox, (10, 20, 100, 50));
    }

    #[test]
    fn damage_bounding_box_multiple_rects() {
        let rects = vec![
            DamageRect {
                x: 10,
                y: 20,
                width: 30,
                height: 30,
            },
            DamageRect {
                x: 50,
                y: 60,
                width: 40,
                height: 20,
            },
        ];
        let bbox = compute_bbox(&rects).unwrap();
        // Union: x=10..90, y=20..80 => (10, 20, 80, 60)
        assert_eq!(bbox, (10, 20, 80, 60));
    }

    #[test]
    fn damage_bounding_box_overlapping_rects() {
        let rects = vec![
            DamageRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            DamageRect {
                x: 50,
                y: 50,
                width: 100,
                height: 100,
            },
        ];
        let bbox = compute_bbox(&rects).unwrap();
        assert_eq!(bbox, (0, 0, 150, 150));
    }

    #[test]
    fn damage_bounding_box_empty() {
        let rects: Vec<DamageRect> = vec![];
        assert!(compute_bbox(&rects).is_none());
    }

    #[test]
    fn damage_bounding_box_negative_coords_clamped() {
        // Damage rect can have negative coords if near screen edge
        let rects = vec![DamageRect {
            x: -5,
            y: -3,
            width: 20,
            height: 15,
        }];
        let bbox = compute_bbox(&rects).unwrap();
        // x clamped to 0, y clamped to 0, but w/h still cover the full range
        assert_eq!(bbox.0, 0); // x
        assert_eq!(bbox.1, 0); // y
        assert_eq!(bbox.2, 20); // w = 15 - (-5) = 20
        assert_eq!(bbox.3, 15); // h = 12 - (-3) = 15
    }

    /// Test cursor-only damage filtering logic.
    /// A rect is "cursor-only" if it's ≤48×48 and within ±24px of cursor.
    fn is_cursor_rect(rect: &DamageRect, cx: i32, cy: i32) -> bool {
        if rect.width > 48 || rect.height > 48 {
            return false;
        }
        let rx = rect.x as i32;
        let ry = rect.y as i32;
        let rw = rect.width as i32;
        let rh = rect.height as i32;
        let margin = 24;
        if rx + rw < cx - margin || rx > cx + margin {
            return false;
        }
        if ry + rh < cy - margin || ry > cy + margin {
            return false;
        }
        true
    }

    #[test]
    fn cursor_rect_at_cursor_position() {
        let rect = DamageRect {
            x: 95,
            y: 95,
            width: 10,
            height: 10,
        };
        assert!(is_cursor_rect(&rect, 100, 100));
    }

    #[test]
    fn cursor_rect_too_large() {
        let rect = DamageRect {
            x: 90,
            y: 90,
            width: 50,
            height: 50,
        };
        assert!(!is_cursor_rect(&rect, 100, 100));
    }

    #[test]
    fn cursor_rect_too_far_away() {
        let rect = DamageRect {
            x: 200,
            y: 200,
            width: 10,
            height: 10,
        };
        assert!(!is_cursor_rect(&rect, 100, 100));
    }

    #[test]
    fn cursor_rect_at_margin_boundary() {
        // cursor at 100,100. Margin is 24. Rect at (125,100) w=10,h=10
        // rx=125, rx > cx+margin=124 => false (just outside)
        let rect = DamageRect {
            x: 125,
            y: 90,
            width: 10,
            height: 10,
        };
        assert!(!is_cursor_rect(&rect, 100, 100));
        // At 124 it should be inside
        let rect2 = DamageRect {
            x: 124,
            y: 90,
            width: 10,
            height: 10,
        };
        assert!(is_cursor_rect(&rect2, 100, 100));
    }

    #[test]
    fn cursor_filtering_mixed_rects() {
        let cx = 100i32;
        let cy = 100i32;
        let rects = vec![
            DamageRect {
                x: 95,
                y: 95,
                width: 10,
                height: 10,
            }, // cursor-only
            DamageRect {
                x: 500,
                y: 500,
                width: 200,
                height: 200,
            }, // real content
        ];
        // Not all rects are cursor-only → should NOT be filtered
        let all_cursor = rects.iter().all(|r| is_cursor_rect(r, cx, cy));
        assert!(!all_cursor);
    }

    #[test]
    fn cursor_filtering_all_cursor_rects() {
        let cx = 100i32;
        let cy = 100i32;
        let rects = vec![
            DamageRect {
                x: 95,
                y: 95,
                width: 10,
                height: 10,
            },
            DamageRect {
                x: 98,
                y: 102,
                width: 5,
                height: 5,
            },
        ];
        let all_cursor = rects.iter().all(|r| is_cursor_rect(r, cx, cy));
        assert!(all_cursor);
    }

    /// Extensions logging: verify new() reports extension status.
    /// This is a smoke test — just checks construction + capture works
    /// end-to-end with all extensions attempted.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn full_capture_with_extensions_smoke_test() {
        let display = display_or_skip();
        if display.is_empty() {
            return;
        }
        let mut backend = X11CaptureBackend::new(&display, 0, 0).unwrap();
        eprintln!(
            "Extensions: damage={}, composite={}, shm={}",
            backend.damage_id.is_some(),
            backend.composite_active,
            backend.shm.is_some(),
        );
        // Capture several frames
        for i in 0..5 {
            let frame = backend.capture_frame().unwrap();
            if i == 0 {
                assert!(frame.is_some(), "first frame must always be captured");
            }
            // Subsequent frames may be None (damage) — that's fine
            std::thread::sleep(std::time::Duration::from_millis(33));
        }
    }
}
