use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

/// Session capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionFlags(u8);

impl SessionFlags {
    pub const AUDIO: Self = Self(0x01);
    pub const CLIPBOARD: Self = Self(0x02);
    pub const FILE_TRANSFER: Self = Self(0x04);
    pub const MICROPHONE: Self = Self(0x08);
    pub const CAMERA: Self = Self(0x10);
    pub const KEYBOARD_LAYOUT: Self = Self(0x20);

    /// Construct session flags from raw wire bits.
    pub const fn new(flags: u8) -> Self {
        Self(flags)
    }

    /// Construct an empty flag set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Return the raw wire bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Return whether no flags are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Return whether all bits in `other` are present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return whether any bit in `other` is present.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Return whether a specific flag bit is set.
    pub const fn has(self, flag: Self) -> bool {
        self.contains(flag)
    }

    /// Insert the given flags.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Remove the given flags.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Return all currently defined session capability flags.
    pub const fn all() -> Self {
        Self(
            Self::AUDIO.0
                | Self::CLIPBOARD.0
                | Self::FILE_TRANSFER.0
                | Self::MICROPHONE.0
                | Self::CAMERA.0
                | Self::KEYBOARD_LAYOUT.0,
        )
    }
}

impl From<u8> for SessionFlags {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

impl From<SessionFlags> for u8 {
    fn from(flags: SessionFlags) -> Self {
        flags.bits()
    }
}

impl BitOr for SessionFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SessionFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SessionFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for SessionFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

/// Gateway-managed client access state flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClientAccessFlags(u8);

impl ClientAccessFlags {
    /// Client is in a read-only viewer mode.
    pub const VIEW_ONLY: Self = Self(0x01);
    /// Client must keep the current remote resolution and not drive resize.
    pub const RESIZE_LOCKED: Self = Self(0x02);

    /// Construct access-state flags from raw wire bits.
    pub const fn new(flags: u8) -> Self {
        Self(flags)
    }

    /// Construct an empty flag set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Return the raw wire bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Return whether no flags are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Return whether all bits in `other` are present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return whether a specific flag bit is set.
    pub const fn has(self, flag: Self) -> bool {
        self.contains(flag)
    }
}

impl From<u8> for ClientAccessFlags {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

impl From<ClientAccessFlags> for u8 {
    fn from(flags: ClientAccessFlags) -> Self {
        flags.bits()
    }
}

impl BitOr for ClientAccessFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ClientAccessFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ClientAccessFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ClientAccessFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

/// Modifier key bitmask for keyboard events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const CTRL: Self = Self(0x01);
    pub const ALT: Self = Self(0x02);
    pub const SHIFT: Self = Self(0x04);
    pub const META: Self = Self(0x08);
    pub const ALTGR: Self = Self(0x10);

    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl From<u8> for Modifiers {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

impl From<Modifiers> for u8 {
    fn from(modifiers: Modifiers) -> Self {
        modifiers.bits()
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Modifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Modifiers {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
    Back = 3,
    Forward = 4,
}

impl TryFrom<u8> for MouseButton {
    type Error = u8;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Left),
            1 => Ok(Self::Middle),
            2 => Ok(Self::Right),
            3 => Ok(Self::Back),
            4 => Ok(Self::Forward),
            _ => Err(val),
        }
    }
}

impl From<MouseButton> for u8 {
    fn from(button: MouseButton) -> Self {
        button as u8
    }
}

impl MouseButton {
    /// Convert a raw wire value into a mouse button.
    pub fn from_u8(val: u8) -> Option<Self> {
        Self::try_from(val).ok()
    }

    /// Return the raw wire value for this mouse button.
    pub fn as_u8(self) -> u8 {
        self.into()
    }
}

// ── Control Channel Messages ────────────────────────────────────────

/// Messages on the CONTROL channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    /// C->S: Container resized, request new resolution.
    ResolutionRequest { width: u16, height: u16 },
    /// S->C: Resolution change applied.
    ResolutionAck { width: u16, height: u16 },
    /// S->C: Session established.
    SessionReady { version: u8, flags: SessionFlags },
    /// Bidirectional ping.
    Ping { seq: u32, timestamp_ms: u64 },
    /// Bidirectional pong.
    Pong { seq: u32, timestamp_ms: u64 },
    /// C->S: Client keyboard layout hint (informational).
    KeyboardLayoutInfo { layout_hint: [u8; 32] },
    /// S->C or gateway->host: Suggested target bitrate in bits/sec.
    /// Used for network adaptation.
    BitrateHint { target_bps: u32 },
    /// S->C (gateway-injected): Resolution is locked by the session owner.
    /// Non-owner clients must display at this resolution without resizing.
    ResolutionLocked { width: u16, height: u16 },
    /// S->C (gateway-injected): Current client access state.
    /// This lets the gateway independently control resize ownership and
    /// read-only viewer restrictions.
    ClientAccessState {
        flags: ClientAccessFlags,
        width: u16,
        height: u16,
    },
}

// ── Input Channel Messages ──────────────────────────────────────────

/// Messages on the INPUT channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMessage {
    MouseMove {
        x: u16,
        y: u16,
    },
    MouseButton {
        button: MouseButton,
        down: bool,
        x: u16,
        y: u16,
    },
    MouseScroll {
        dx: i16,
        dy: i16,
    },
    KeyEvent {
        keycode: u32,
        down: bool,
        modifiers: Modifiers,
    },
    /// Extended key event with character annotation for keyboard layout passthrough.
    KeyEventEx {
        keycode: u32,
        down: bool,
        modifiers: Modifiers,
        /// Unicode codepoint from KeyboardEvent.key, or 0 if non-printable.
        key_char: u32,
    },
}

// ── Cursor Channel Messages ────────────────────────────────────────

/// Messages on the CURSOR channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorMessage {
    CursorMove {
        x: u16,
        y: u16,
    },
    CursorShape {
        width: u16,
        height: u16,
        hotspot_x: u8,
        hotspot_y: u8,
        data: Vec<u8>,
    },
}

// ── Clipboard Channel Messages ──────────────────────────────────────

/// Messages on the CLIPBOARD channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardMessage {
    Text { content: Vec<u8> },
}

// ── File Transfer Messages ──────────────────────────────────────────

/// Messages for FILE_UP and FILE_DOWN channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileMessage {
    FileHeader {
        id: u32,
        filename: Box<[u8; 256]>,
        size: u64,
        mime: Box<[u8; 64]>,
    },
    FileChunk {
        id: u32,
        seq: u32,
        data: Vec<u8>,
    },
    FileComplete {
        id: u32,
    },
}

impl FileMessage {
    /// Construct a file header message without exposing the internal boxed layout.
    pub fn header(id: u32, filename: [u8; 256], size: u64, mime: [u8; 64]) -> Self {
        Self::FileHeader {
            id,
            filename: Box::new(filename),
            size,
            mime: Box::new(mime),
        }
    }

    /// Construct a file chunk message.
    pub fn chunk(id: u32, seq: u32, data: Vec<u8>) -> Self {
        Self::FileChunk { id, seq, data }
    }

    /// Construct a file completion message.
    pub fn complete(id: u32) -> Self {
        Self::FileComplete { id }
    }
}

// ── Tile Channel Messages ──────────────────────────────────────────

/// Messages on the TILES channel for multi-codec tile rendering.
///
/// The tile system uses a fixed grid overlaid on the screen. Each tile is
/// encoded independently using the most efficient codec for its content:
/// - Solid color tiles → fill commands (~6 bytes)
/// - Unchanged tiles → cache hit by hash (~12 bytes)
/// - UI/text tiles → QOI (~1-10 KB, lossless)
/// - Video tiles → H.264 via the VIDEO channel (only the video region)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileMessage {
    /// Sent once on connect and after resize. Defines the tile grid.
    GridConfig {
        tile_size: u16,
        cols: u16,
        rows: u16,
        screen_w: u16,
        screen_h: u16,
    },
    /// Tile unchanged since last send — reuse client cache.
    CacheHit { col: u16, row: u16, hash: u64 },
    /// Browser cache miss for a previously announced hash.
    /// Sent from client to host when a CacheHit cannot be applied locally.
    CacheMiss {
        frame_seq: u32,
        col: u16,
        row: u16,
        hash: u64,
    },
    /// Solid color fill. Cheapest possible tile update.
    Fill { col: u16, row: u16, rgba: u32 },
    /// Lossless QOI tile for UI elements (text, icons, buttons).
    Qoi {
        col: u16,
        row: u16,
        hash: u64,
        data: Vec<u8>,
    },
    /// Zstd-compressed raw RGBA tile (alternative lossless codec).
    Zstd {
        col: u16,
        row: u16,
        hash: u64,
        data: Vec<u8>,
    },
    /// Defines the bounding box where H.264 video data is composited.
    /// The actual H.264 NALs continue on the VIDEO channel with VideoTileInfo
    /// matching this region. Sent when the video region changes.
    VideoRegion { x: u16, y: u16, w: u16, h: u16 },
    /// Marks the end of a tile update batch for one frame.
    /// The client should composite all received updates after this.
    BatchEnd { frame_seq: u32 },
    /// Scroll copy: shift existing canvas pixels by (dx, dy) pixels,
    /// limited to the scroll region [region_top..region_bottom, 0..region_right].
    /// Pixels outside this region (e.g., browser toolbar, scrollbar) are not shifted.
    /// If region covers the full screen (0, 0, screenW, screenH), all pixels shift.
    ScrollCopy {
        dx: i16,
        dy: i16,
        region_top: u16,
        region_bottom: u16,
        region_right: u16,
    },
    /// Grid offset: tells the client where to draw subsequent tiles.
    /// Tiles are drawn at (col*tileSize - offset_x, row*tileSize - offset_y).
    /// Sent after ScrollCopy when the tile grid is shifted to align with content.
    GridOffset { offset_x: i16, offset_y: i16 },
    /// Tile draw mode: controls whether subsequent tiles in this batch
    /// are drawn with grid offset (content area) or at fixed positions
    /// (static areas like browser header / scrollbar).
    /// When apply_offset = false, tiles are drawn at (col*ts, row*ts).
    /// Resets to true at the start of each batch.
    TileDrawMode { apply_offset: bool },
    /// Host-side scroll residual telemetry snapshot (cumulative counters).
    /// Lets clients report full-fallback rate and saved-tile efficiency.
    ScrollStats {
        scroll_batches_total: u32,
        scroll_full_fallbacks_total: u32,
        scroll_potential_tiles_total: u32,
        scroll_saved_tiles_total: u32,
        scroll_non_quantized_fallbacks_total: u32,
        scroll_residual_full_repaints_total: u32,
        scroll_zero_saved_batches_total: u32,
    },
}

// ── Video Datagram Header ───────────────────────────────────────────

/// Tile metadata for partial-screen video updates.
/// When present, the video frame covers only a sub-region of the screen.
/// The client composites the tile at `(tile_x, tile_y)` on the main canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoTileInfo {
    /// Pixel offset from left edge of the full screen.
    pub tile_x: u16,
    /// Pixel offset from top edge of the full screen.
    pub tile_y: u16,
    /// Tile width in pixels.
    pub tile_w: u16,
    /// Tile height in pixels.
    pub tile_h: u16,
    /// Full screen width (for client to know the coordinate space).
    pub screen_w: u16,
    /// Full screen height.
    pub screen_h: u16,
}

/// Header for video datagrams. Supports NAL unit fragmentation
/// for payloads exceeding the QUIC MTU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDatagram {
    /// Unique identifier for this NAL unit.
    pub nal_id: u32,
    /// Fragment sequence number within the NAL unit.
    pub fragment_seq: u16,
    /// Total number of fragments for this NAL unit.
    pub fragment_total: u16,
    /// Whether this is a keyframe (IDR).
    pub is_keyframe: bool,
    /// Presentation timestamp in microseconds.
    pub pts_us: u64,
    /// The encoded video data fragment.
    pub data: Vec<u8>,
    /// Optional tile info for partial-screen updates.
    /// When Some, this datagram contains a tile (sub-region) update.
    pub tile_info: Option<VideoTileInfo>,
}

// ── Audio Frame ─────────────────────────────────────────────────────

/// An audio frame carrying codec-tagged audio payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    /// Sequence number for ordering.
    pub seq: u32,
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
    /// Encoded audio payload bytes.
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_flags_support_bitmask_operations() {
        let mut flags = SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER;
        assert!(flags.contains(SessionFlags::CLIPBOARD));
        assert!(!flags.contains(SessionFlags::AUDIO));

        flags.insert(SessionFlags::AUDIO | SessionFlags::MICROPHONE);
        assert!(flags.contains(SessionFlags::AUDIO));
        assert!(flags.intersects(SessionFlags::MICROPHONE | SessionFlags::CAMERA));

        flags.remove(SessionFlags::FILE_TRANSFER);
        assert!(!flags.contains(SessionFlags::FILE_TRANSFER));
        assert_eq!(u8::from(flags), flags.bits());
    }

    #[test]
    fn modifiers_support_bitmask_operations() {
        let mut modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(modifiers.contains(Modifiers::CTRL));
        assert!(modifiers.intersects(Modifiers::SHIFT | Modifiers::ALT));

        modifiers.insert(Modifiers::ALTGR);
        assert!(modifiers.contains(Modifiers::ALTGR));

        modifiers.remove(Modifiers::SHIFT);
        assert!(!modifiers.contains(Modifiers::SHIFT));
        assert_eq!(Modifiers::from(modifiers.bits()), modifiers);
    }

    #[test]
    fn mouse_button_try_from_rejects_invalid_values() {
        assert_eq!(MouseButton::try_from(0), Ok(MouseButton::Left));
        assert_eq!(MouseButton::try_from(4), Ok(MouseButton::Forward));
        assert_eq!(MouseButton::try_from(5), Err(5));
    }
}
