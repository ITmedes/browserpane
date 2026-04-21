//! Tile command emitter.
//!
//! Converts tile classifications and pixel data into wire-protocol
//! TileMessage frames. Implements the multi-codec tile strategy:
//!
//! - Solid color tiles -> Fill (~9 bytes)
//! - Unchanged tiles -> CacheHit (~13 bytes)
//! - UI/text tiles -> QOI (lossless, ~1-10 KB)
//! - Video tiles -> deferred to H.264 on Video channel (VideoRegion message)
//!
//! Hash-based deduplication: each tile's pixel data is hashed (xxHash3).
//! If the hash matches the last-sent hash, a CacheHit is emitted instead
//! of re-encoding.

use indexmap::IndexSet;

use bpane_protocol::frame::Frame;
use bpane_protocol::TileMessage;

use super::{fnv_hash, Rect, TileClass, TileCoord, TileGrid};

/// Which lossless codec to use for non-solid, non-video tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileCodec {
    /// QOI (Quite OK Image) - fast, lossless, good for UI content.
    Qoi,
    /// Zstd-compressed raw RGBA - better ratio at cost of slightly more CPU.
    Zstd,
}

impl TileCodec {
    /// Parse from a string (case-insensitive). Defaults to `Qoi` on unknown input.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "zstd" => Self::Zstd,
            _ => Self::Qoi,
        }
    }
}

/// Maximum number of content hashes to track in the "client knows" set.
/// Matches the client-side TileCache capacity (8192).
const MAX_SENT_HASHES: usize = 8192;

/// Result of emitting tile commands for one frame.
pub struct EmitResult {
    /// Frames to send on the Tiles channel.
    pub tile_frames: Vec<Frame>,
    /// Active video region sideband, if any.
    /// The caller sends H.264 NALs on the Video channel for this region.
    pub video_region: Option<Rect>,
    /// Stats for this frame.
    pub stats: EmitStats,
}

#[derive(Debug, Default)]
pub struct EmitStats {
    pub fills: u32,
    pub cache_hits: u32,
    pub qoi_tiles: u32,
    pub video_tiles: u32,
    pub qoi_bytes: usize,
    /// Tiles skipped because unchanged at same position.
    pub skipped: u32,
}

/// Emits tile commands based on classification and pixel data.
///
/// Uses two-level deduplication:
/// 1. **Per-position**: `last_hashes[pos]` - if unchanged at same grid slot, skip
///    entirely (canvas already shows correct pixels).
/// 2. **Content-addressable**: `sent_hashes` - tracks QOI hashes the client has
///    cached. If the same content appears at a *different* position, emit CacheHit
///    instead of re-encoding. Works for scrolls, window moves, any content reuse.
pub struct TileEmitter {
    frame_seq: u32,
    /// Per-position hash: if tile at (col,row) still has this hash, skip entirely.
    /// Shifted by `shift_hashes()` during scroll to match content displacement.
    last_hashes: Vec<u64>,
    /// Per-position hash for static (non-scrolling) tiles (header, scrollbar).
    /// Never shifted - these tiles are drawn at fixed screen positions.
    static_last_hashes: Vec<u64>,
    cols: u16,
    /// Content hashes of QOI tiles the client has in its tile cache.
    /// Used for cross-position CacheHit (e.g. scroll reuse).
    /// IndexSet provides O(1) lookup, O(1) insert, and preserves insertion
    /// order for bounded eviction (oldest = index 0).
    sent_hashes: IndexSet<u64>,
    sent_hash_evictions_total: u64,
    /// Last video region sideband sent to the client.
    /// Used to send updates and a clearing VideoRegion(0,0,0,0) on exit.
    last_video_region: Option<Rect>,
    /// Reused scratch buffer for tile pixel extraction to reduce per-tile allocations.
    tile_scratch: Vec<u8>,
    /// Which lossless codec to use for non-solid tiles.
    codec: TileCodec,
}

impl TileEmitter {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self::with_codec(cols, rows, TileCodec::Qoi)
    }

    pub fn with_codec(cols: u16, rows: u16, codec: TileCodec) -> Self {
        // +1 row to accommodate the extra bottom row when grid offset is active
        let count = cols as usize * (rows as usize + 1);
        Self {
            frame_seq: 0,
            last_hashes: vec![0; count],
            static_last_hashes: vec![0; count],
            cols,
            sent_hashes: IndexSet::with_capacity(MAX_SENT_HASHES),
            sent_hash_evictions_total: 0,
            last_video_region: None,
            tile_scratch: Vec::new(),
            codec,
        }
    }

    /// Resize the hash grid. Clears all cached hashes.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let count = cols as usize * (rows as usize + 1);
        self.last_hashes = vec![0; count];
        self.static_last_hashes = vec![0; count];
        self.cols = cols;
        self.sent_hashes.clear();
        self.sent_hash_evictions_total = 0;
        self.last_video_region = None;
        self.tile_scratch.clear();
    }

    pub fn current_video_region(&self) -> Option<Rect> {
        self.last_video_region
    }

    /// Shift per-position hashes by `row_shift` rows after a scroll.
    ///
    /// When scrolling causes the content at each grid position to shift by
    /// whole tile rows, this updates `last_hashes` so Level 1 skip still works.
    /// Positive `row_shift` means content moved up (scroll down).
    pub fn shift_hashes(&mut self, row_shift: i32, rows: u16) {
        if row_shift == 0 {
            return;
        }
        let total_rows = rows as usize + 1; // +1 for extra bottom row
        let cols = self.cols as usize;
        let total = cols * total_rows;
        let mut new_hashes = vec![0u64; total];
        for r in 0..total_rows {
            let src_r = r as i32 + row_shift;
            if src_r >= 0 && (src_r as usize) < total_rows {
                let dst_start = r * cols;
                let src_start = src_r as usize * cols;
                if dst_start + cols <= total && src_start + cols <= self.last_hashes.len() {
                    new_hashes[dst_start..dst_start + cols]
                        .copy_from_slice(&self.last_hashes[src_start..src_start + cols]);
                }
            }
        }
        self.last_hashes = new_hashes;
    }

    /// Zero hash entries for tile rows in the client-side exposed strip.
    ///
    /// After `shift_hashes`, the grid-level shift correctly handles rows that
    /// shift beyond the grid boundary. But when the scroll region starts at
    /// `region_top > 0`, some tile rows map to client canvas positions that are
    /// off-screen (past screen_h) after the ScrollCopy shift - even though the
    /// grid considers them valid. The client clears those pixels, so their
    /// hashes must be zeroed to force L1 miss and re-emission.
    pub fn zero_exposed_strip(
        &mut self,
        dy: i16,
        region_top: u16,
        region_bottom: u16,
        tile_size: u16,
        offset_y: u16,
    ) {
        if dy == 0 || tile_size == 0 {
            return;
        }
        let cols = self.cols as usize;
        let ts = tile_size as i32;
        let oy = offset_y as i32;
        let rb = region_bottom as i32;
        let rt = region_top as i32;

        // Compute the pixel range of the exposed (cleared) strip on the client.
        // The client clears the entire region [regionTop, regionBottom] then
        // redraws only the shifted overlap - so the exposed strip is the part
        // of the region NOT covered by the shifted content.
        let (exposed_start, exposed_end) = if dy > 0 {
            // Scroll down: exposed at bottom of scroll region.
            let start = (rb - dy as i32).max(rt);
            (start, rb)
        } else {
            // Scroll up: exposed at top of scroll region.
            let end = (rt + (-dy) as i32).min(rb);
            (rt, end)
        };

        // Zero hashes for any tile row whose pixel range overlaps the exposed strip.
        // Content tiles use offset_y for their screen position.
        let total = self.last_hashes.len();
        for r in 0..(total / cols.max(1)) {
            let tile_pixel_top = r as i32 * ts - oy;
            let tile_pixel_bot = tile_pixel_top + ts;
            if tile_pixel_bot > exposed_start && tile_pixel_top < exposed_end {
                let start = r * cols;
                let end = (start + cols).min(total);
                for h in &mut self.last_hashes[start..end] {
                    *h = 0;
                }
            }
        }

        // Also zero static_last_hashes for tile rows whose RAW position
        // (offset_y=0) overlaps the exposed strip. Static tiles (buffer row,
        // header, scrollbar) are drawn at raw grid positions. If the buffer
        // row's content is unchanged (e.g., solid white), its static hash
        // still matches - causing L1 skip even though ScrollCopy cleared
        // that area on the client canvas. Zeroing forces re-emission.
        let static_total = self.static_last_hashes.len();
        for r in 0..(static_total / cols.max(1)) {
            let tile_pixel_top = r as i32 * ts; // no offset for static tiles
            let tile_pixel_bot = tile_pixel_top + ts;
            if tile_pixel_bot > exposed_start && tile_pixel_top < exposed_end {
                let start = r * cols;
                let end = (start + cols).min(static_total);
                for h in &mut self.static_last_hashes[start..end] {
                    *h = 0;
                }
            }
        }
    }

    /// Mark a known hash as recently used in the sender-side LRU index.
    /// IndexSet: move_to_back is O(1) amortized.
    fn touch_sent_hash(&mut self, hash: u64) {
        // move_index moves the entry to the given position, shifting others.
        // For LRU we move to back (most recent).
        if let Some(idx) = self.sent_hashes.get_index_of(&hash) {
            self.sent_hashes.move_index(idx, self.sent_hashes.len() - 1);
        }
    }

    /// Record a QOI hash as "client has this cached".
    fn track_sent_hash(&mut self, hash: u64) {
        if hash == 0 {
            return;
        }
        if self.sent_hashes.contains(&hash) {
            self.touch_sent_hash(hash);
            return;
        }
        // Evict oldest (index 0) if at capacity
        while self.sent_hashes.len() >= MAX_SENT_HASHES {
            self.sent_hashes.shift_remove_index(0);
            self.sent_hash_evictions_total = self.sent_hash_evictions_total.saturating_add(1);
        }
        self.sent_hashes.insert(hash);
    }

    pub fn sent_hash_entries(&self) -> usize {
        self.sent_hashes.len()
    }

    pub fn sent_hash_evictions_total(&self) -> u64 {
        self.sent_hash_evictions_total
    }

    /// Update `last_hashes` for tiles verified correct by scroll residual analysis.
    ///
    /// During scroll, tiles that match the shifted previous frame are skipped
    /// (not emitted). But their `last_hashes` entries are stale (pre-scroll).
    /// This method updates them to the current hash so Level 1 skip works
    /// correctly on subsequent frames.
    pub fn update_hashes_for_skipped(
        &mut self,
        frame_pixels: &[u8],
        stride: usize,
        skipped_coords: &[TileCoord],
        grid: &TileGrid,
        offset_y: u16,
    ) {
        for &coord in skipped_coords {
            let rect = Self::offset_tile_rect(coord, grid, offset_y);
            if rect.w == 0 || rect.h == 0 {
                continue;
            }
            extract_tile_pixels_into(frame_pixels, stride, &rect, &mut self.tile_scratch);
            let (hash, _) = tile_content_hash(&self.tile_scratch);
            let idx = coord.row as usize * self.cols as usize + coord.col as usize;
            if idx < self.last_hashes.len() {
                self.last_hashes[idx] = hash;
            }
        }
    }

    /// Handle a client-reported cache miss by invalidating sender assumptions.
    ///
    /// - Drop the hash from the "client has this" set so future emits use QOI/Fill.
    /// - Clear the per-position hash so the next frame does not Level-1 skip.
    pub fn handle_cache_miss(&mut self, col: u16, row: u16, hash: u64) {
        if hash != 0 {
            self.sent_hashes.swap_remove(&hash);
        }
        let idx = row as usize * self.cols as usize + col as usize;
        if idx < self.last_hashes.len() {
            self.last_hashes[idx] = 0;
        }
        if idx < self.static_last_hashes.len() {
            self.static_last_hashes[idx] = 0;
        }
    }

    /// Emit a GridConfig message.
    pub fn emit_grid_config(&self, grid: &TileGrid) -> Frame {
        let msg = TileMessage::GridConfig {
            tile_size: grid.tile_size,
            cols: grid.cols,
            rows: grid.rows,
            screen_w: grid.screen_w,
            screen_h: grid.screen_h,
        };
        msg.to_frame()
    }

    /// Compute the tile extraction rect, accounting for grid offset.
    ///
    /// With `offset_y > 0`, tiles are extracted from shifted framebuffer positions:
    /// tile row R covers FB rows `[R*tile_size - offset_y, R*tile_size - offset_y + tile_size)`,
    /// clamped to `[0, screen_h)`.
    fn offset_tile_rect(coord: TileCoord, grid: &TileGrid, offset_y: u16) -> Rect {
        let ts = grid.tile_size;
        let x = coord.col as u16 * ts;
        let raw_y = coord.row as i32 * ts as i32 - offset_y as i32;
        let fb_y = raw_y.max(0) as u16;
        let fb_end_y = ((raw_y + ts as i32).min(grid.screen_h as i32)).max(0) as u16;
        let h = fb_end_y.saturating_sub(fb_y);
        let w = ts.min(grid.screen_w.saturating_sub(x));
        if w == 0 || h == 0 {
            return Rect::new(0, 0, 0, 0);
        }
        Rect::new(x, fb_y, w, h)
    }

    /// Emit tile commands for all dirty tiles in a frame.
    ///
    /// Two-level deduplication:
    /// 1. Same position, same hash -> skip (canvas already correct)
    /// 2. Different position, hash in `sent_hashes` -> CacheHit (client reuses cached bitmap)
    /// 3. New content -> Fill (solid) or QOI (complex), track hash
    ///
    /// `frame_pixels`: the full BGRA frame buffer (width * height * 4 bytes).
    /// `stride`: bytes per row (typically width * 4).
    /// `dirty_coords`: tiles that changed this frame.
    /// `grid`: the tile grid with classification info.
    /// `offset_y`: grid offset in pixels (from scroll displacement tracking).
    pub fn emit_frame(
        &mut self,
        frame_pixels: &[u8],
        stride: usize,
        dirty_coords: &[TileCoord],
        grid: &TileGrid,
        offset_y: u16,
        force_qoi_bounds: Option<(u16, u16, u16, u16)>,
        active_video_region: Option<Rect>,
    ) -> EmitResult {
        self.frame_seq = self.frame_seq.wrapping_add(1);
        let mut frames = Vec::new();
        let mut stats = EmitStats::default();
        let mut video_tiles: Vec<TileCoord> = Vec::new();

        for &coord in dirty_coords {
            let force_qoi = coord_in_tile_bounds(coord, force_qoi_bounds);
            // For the extra row (beyond grid.rows), skip classification lookup
            let tile_state = if coord.row < grid.rows {
                match grid.get(coord) {
                    Some(s) => s,
                    None => continue,
                }
            } else {
                // Extra bottom row when offset is active - treat as static
                &super::TileState::default()
            };
            let rect = Self::offset_tile_rect(coord, grid, offset_y);
            if rect.w == 0 || rect.h == 0 {
                continue;
            }

            let video_owned = matches!(tile_state.classification, TileClass::VideoMotion)
                || active_video_region.is_some_and(|region| region.overlaps(&rect));
            if video_owned {
                video_tiles.push(coord);
                stats.video_tiles += 1;
                continue;
            }

            // Extract tile pixel data and compute content hash.
            // For solid-color tiles, `tile_content_hash` returns a canonical hash
            // based on just the RGBA value - invariant across tile dimensions.
            // This ensures L1 skip works even when edge tiles change height during scroll.
            extract_tile_pixels_into(frame_pixels, stride, &rect, &mut self.tile_scratch);
            let (hash, solid_rgba) = tile_content_hash(&self.tile_scratch);

            // Level 1: same position, same hash -> skip entirely.
            // The canvas already shows the correct pixels from the previous draw.
            let idx = coord.row as usize * self.cols as usize + coord.col as usize;
            if !force_qoi
                && idx < self.last_hashes.len()
                && self.last_hashes[idx] == hash
                && hash != 0
            {
                stats.skipped += 1;
                continue;
            }

            // Update per-position hash.
            if idx < self.last_hashes.len() {
                self.last_hashes[idx] = hash;
            }

            // Level 2: content-addressable - if the client already has this hash
            // cached (from a QOI tile at a different position), emit CacheHit.
            // Skip for fills: Fill (9 bytes) < CacheHit (13 bytes).
            if !force_qoi && solid_rgba.is_none() && hash != 0 && self.sent_hashes.contains(&hash) {
                self.touch_sent_hash(hash);
                frames.push(
                    TileMessage::CacheHit {
                        col: coord.col,
                        row: coord.row,
                        hash,
                    }
                    .to_frame(),
                );
                stats.cache_hits += 1;
                continue;
            }

            // Level 3: new content - encode.
            if let Some(rgba) = solid_rgba {
                frames.push(
                    TileMessage::Fill {
                        col: coord.col,
                        row: coord.row,
                        rgba,
                    }
                    .to_frame(),
                );
                stats.fills += 1;
            } else {
                // Convert BGRA tile pixels to RGBA for encoding.
                // Only changed, non-solid tiles reach here - a small
                // subset of total pixels, so this is much cheaper than
                // the former full-frame swap.
                bgra_to_rgba_inplace(&mut self.tile_scratch);
                let codec = if force_qoi {
                    TileCodec::Qoi
                } else {
                    self.codec
                };
                if let Some(msg) = encode_tile(
                    codec,
                    &self.tile_scratch,
                    rect.w as u32,
                    rect.h as u32,
                    coord.col,
                    coord.row,
                    hash,
                ) {
                    let frame = msg.to_frame();
                    stats.qoi_bytes += frame.payload.len();
                    frames.push(frame);
                    stats.qoi_tiles += 1;
                    self.track_sent_hash(hash);
                }
            }
        }

        let next_video_region =
            active_video_region.or_else(|| grid.tiles_bounding_rect(&video_tiles));
        if next_video_region != self.last_video_region {
            let region = next_video_region.unwrap_or(Rect::new(0, 0, 0, 0));
            frames.push(
                TileMessage::VideoRegion {
                    x: region.x,
                    y: region.y,
                    w: region.w,
                    h: region.h,
                }
                .to_frame(),
            );
            self.last_video_region = next_video_region;
        }

        // BatchEnd
        frames.push(
            TileMessage::BatchEnd {
                frame_seq: self.frame_seq,
            }
            .to_frame(),
        );

        EmitResult {
            tile_frames: frames,
            video_region: next_video_region,
            stats,
        }
    }

    /// Emit static tiles for non-scrolling areas (browser header, scrollbar).
    ///
    /// These tiles are extracted at RAW framebuffer positions (no scroll offset)
    /// and use a separate hash array that is never shifted during scroll.
    /// The caller should wrap the result with `TileDrawMode { apply_offset: false }`
    /// so the client draws them at fixed screen positions.
    /// Emit static (chrome/scrollbar) tiles with no grid offset.
    ///
    /// `boundary_col` - optional column index that straddles the content/scrollbar
    /// edge. QOI hashes for that column are NOT tracked in `sent_hashes` because
    /// the mixed content+scrollbar pixels produce a unique hash per scroll position,
    /// polluting the LRU cache with never-reused entries.
    ///
    /// Boundary tiles also emit a zero hash to tell the client not to cache them.
    /// That keeps low-value chrome/content blend tiles out of the client tile cache,
    /// matching the sender-side decision to never reuse them via CacheHit.
    pub fn emit_static_tiles(
        &mut self,
        frame_pixels: &[u8],
        stride: usize,
        coords: &[TileCoord],
        grid: &TileGrid,
        boundary_col: Option<u16>,
        boundary_top_row: Option<u16>,
        boundary_bottom_row: Option<u16>,
        force_qoi_bounds: Option<(u16, u16, u16, u16)>,
        active_video_region: Option<Rect>,
    ) -> Vec<Frame> {
        let mut frames = Vec::new();
        for &coord in coords {
            let force_qoi = coord_in_tile_bounds(coord, force_qoi_bounds);
            let rect = Self::offset_tile_rect(coord, grid, 0); // no offset
            if rect.w == 0 || rect.h == 0 {
                continue;
            }
            if active_video_region.is_some_and(|region| region.overlaps(&rect)) {
                continue;
            }

            extract_tile_pixels_into(frame_pixels, stride, &rect, &mut self.tile_scratch);
            let (hash, solid_rgba) = tile_content_hash(&self.tile_scratch);

            let is_boundary = boundary_col.is_some_and(|bc| coord.col == bc)
                || boundary_top_row.is_some_and(|br| coord.row == br)
                || boundary_bottom_row.is_some_and(|br| coord.row == br);

            // Level 1: per-position dedup using static hash array (never shifted).
            let idx = coord.row as usize * self.cols as usize + coord.col as usize;
            if !force_qoi
                && !is_boundary
                && idx < self.static_last_hashes.len()
                && self.static_last_hashes[idx] == hash
                && hash != 0
            {
                continue; // unchanged since last static emit
            }
            if idx < self.static_last_hashes.len() {
                self.static_last_hashes[idx] = hash;
            }

            // Level 2: content-addressable cache (shared with content tiles).
            // Skip for fills: Fill (9 bytes) < CacheHit (13 bytes).
            // Skip L1/L2 for boundary tiles - they mix chrome with viewport
            // content, so if the client ever gets one wrong we want a fresh
            // repair emit rather than an indefinite skip/cache replay.
            if !force_qoi
                && !is_boundary
                && solid_rgba.is_none()
                && hash != 0
                && self.sent_hashes.contains(&hash)
            {
                self.touch_sent_hash(hash);
                frames.push(
                    TileMessage::CacheHit {
                        col: coord.col,
                        row: coord.row,
                        hash,
                    }
                    .to_frame(),
                );
                continue;
            }

            // Level 3: encode as fill or QOI.
            if let Some(rgba) = solid_rgba {
                frames.push(
                    TileMessage::Fill {
                        col: coord.col,
                        row: coord.row,
                        rgba,
                    }
                    .to_frame(),
                );
            } else {
                // Convert BGRA tile pixels to RGBA for encoding.
                bgra_to_rgba_inplace(&mut self.tile_scratch);
                let codec = if force_qoi {
                    TileCodec::Qoi
                } else {
                    self.codec
                };
                let cache_hash = if is_boundary { 0 } else { hash };
                if let Some(msg) = encode_tile(
                    codec,
                    &self.tile_scratch,
                    rect.w as u32,
                    rect.h as u32,
                    coord.col,
                    coord.row,
                    cache_hash,
                ) {
                    frames.push(msg.to_frame());
                    // Don't track boundary column hashes - they change every
                    // scroll position and would evict useful entries from the LRU.
                    if !is_boundary {
                        self.track_sent_hash(hash);
                    }
                }
            }
        }
        frames
    }
}

/// Extract pixels for a tile from the full frame buffer (preserves pixel format).
fn extract_tile_pixels(frame: &[u8], stride: usize, rect: &Rect) -> Vec<u8> {
    let w = rect.w as usize;
    let h = rect.h as usize;
    let mut out = Vec::with_capacity(w * h * 4);
    extract_tile_pixels_into(frame, stride, rect, &mut out);
    out
}

/// Extract pixels for a tile into a reusable output buffer (preserves pixel format).
fn extract_tile_pixels_into(frame: &[u8], stride: usize, rect: &Rect, out: &mut Vec<u8>) {
    let w = rect.w as usize;
    let h = rect.h as usize;
    out.clear();
    let needed = w * h * 4;
    if out.capacity() < needed {
        out.reserve(needed - out.capacity());
    }
    for row in 0..h {
        let y = rect.y as usize + row;
        let start = y * stride + rect.x as usize * 4;
        let end = start + w * 4;
        if end <= frame.len() {
            out.extend_from_slice(&frame[start..end]);
        }
    }
}

/// Compute a content hash for tile pixel data.
///
/// For solid-color tiles, returns a **canonical hash** based solely on the RGBA
/// value - independent of tile dimensions. This is critical for scroll performance:
/// edge tiles change height as `offset_y` shifts, but a white tile is still white
/// regardless of whether it's 64x64 or 27x64. The canonical hash ensures L1 skip
/// works across dimension changes, sending zero bytes for unchanged solid-color tiles.
fn tile_content_hash(tile_data: &[u8]) -> (u64, Option<u32>) {
    if let Some(rgba) = detect_solid_color(tile_data) {
        // Canonical: hash just the 4-byte color, not the full pixel buffer.
        (fnv_hash(&rgba.to_le_bytes()), Some(rgba))
    } else {
        (fnv_hash(tile_data), None)
    }
}

/// Detect if all pixels in a tile are the same color.
/// Input is BGRA pixel data. Returns an RGBA u32 (wire format) if solid, None otherwise.
fn detect_solid_color(tile_data: &[u8]) -> Option<u32> {
    if tile_data.len() < 4 {
        return None;
    }
    let b0 = tile_data[0]; // B in BGRA
    let b1 = tile_data[1]; // G
    let b2 = tile_data[2]; // R in BGRA
    let b3 = tile_data[3]; // A

    for pixel in tile_data.chunks_exact(4) {
        if pixel[0] != b0 || pixel[1] != b1 || pixel[2] != b2 || pixel[3] != b3 {
            return None;
        }
    }

    // Return as RGBA u32 (swap B and R for wire format)
    Some(b2 as u32 | (b1 as u32) << 8 | (b0 as u32) << 16 | (b3 as u32) << 24)
}

/// Convert BGRA pixels to RGBA in-place.
fn bgra_to_rgba_inplace(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

/// Encode RGBA pixels as QOI. Returns None on failure.
fn encode_qoi(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    qoi::encode_to_vec(rgba, width, height).ok()
}

/// Compress raw RGBA pixels with zstd. Returns None on failure.
fn encode_zstd(rgba: &[u8]) -> Option<Vec<u8>> {
    zstd::encode_all(rgba, 1).ok()
}

fn coord_in_tile_bounds(coord: TileCoord, bounds: Option<(u16, u16, u16, u16)>) -> bool {
    bounds
        .map(|(min_col, min_row, max_col, max_row)| {
            coord.col >= min_col
                && coord.col <= max_col
                && coord.row >= min_row
                && coord.row <= max_row
        })
        .unwrap_or(false)
}

/// Encode tile pixels and return the appropriate TileMessage.
fn encode_tile(
    codec: TileCodec,
    rgba: &[u8],
    width: u32,
    height: u32,
    col: u16,
    row: u16,
    hash: u64,
) -> Option<TileMessage> {
    match codec {
        TileCodec::Qoi => encode_qoi(rgba, width, height).map(|data| TileMessage::Qoi {
            col,
            row,
            hash,
            data,
        }),
        TileCodec::Zstd => encode_zstd(rgba).map(|data| TileMessage::Zstd {
            col,
            row,
            hash,
            data,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::channel::ChannelId;

    #[test]
    fn detect_solid_color_uniform() {
        // Input is BGRA: B=0x00, G=0x00, R=0xFF, A=0xFF (red pixel in BGRA)
        let data = vec![0x00, 0x00, 0xFF, 0xFF].repeat(64 * 64);
        let rgba = detect_solid_color(&data);
        assert!(rgba.is_some());
        // Output is RGBA u32 LE: R=0xFF, G=0x00, B=0x00, A=0xFF -> 0xFF0000FF
        assert_eq!(rgba.unwrap(), 0xFF0000FF);
    }

    #[test]
    fn detect_solid_color_not_uniform() {
        // BGRA input
        let mut data = vec![0x00, 0x00, 0xFF, 0xFF].repeat(64 * 64);
        data[4] = 0x01; // change one pixel
        assert!(detect_solid_color(&data).is_none());
    }

    #[test]
    fn detect_solid_color_empty() {
        assert!(detect_solid_color(&[]).is_none());
    }

    #[test]
    fn extract_tile_pixels_basic() {
        // 4x4 frame, tile at (1,1) size 2x2
        let stride = 4 * 4; // 4 pixels * 4 bytes
        let mut frame = vec![0u8; stride * 4];
        // Set pixel at (2, 2) to red
        let offset = 2 * stride + 2 * 4;
        frame[offset] = 0xFF;
        frame[offset + 3] = 0xFF;

        let rect = Rect::new(2, 2, 2, 2);
        let tile = extract_tile_pixels(&frame, stride, &rect);
        assert_eq!(tile.len(), 2 * 2 * 4);
        assert_eq!(tile[0], 0xFF); // first byte of first pixel in tile
    }

    #[test]
    fn encode_qoi_works() {
        let data = vec![0xFF; 64 * 64 * 4]; // white 64x64
        let qoi = encode_qoi(&data, 64, 64);
        assert!(qoi.is_some());
        let bytes = qoi.unwrap();
        // QOI magic bytes: "qoif"
        assert_eq!(&bytes[0..4], b"qoif");
    }

    #[test]
    fn emitter_force_qoi_bounds_override_global_zstd_codec() {
        let mut grid = TileGrid::new(64, 64, 64);
        if let Some(tile) = grid.get_mut(TileCoord::new(0, 0)) {
            tile.classification = TileClass::Static;
        }
        let stride = 64 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 3) as u8;
                frame_data[offset + 1] = (y * 5) as u8;
                frame_data[offset + 2] = (x ^ y) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }
        let mut emitter = TileEmitter::with_codec(grid.cols, grid.rows, TileCodec::Zstd);
        let result = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            0,
            Some((0, 0, 0, 0)),
            None,
        );
        let tile_msg = result
            .tile_frames
            .iter()
            .find_map(|frame| TileMessage::decode(&frame.payload).ok())
            .expect("tile frame");
        assert!(matches!(tile_msg, TileMessage::Qoi { .. }));
    }

    #[test]
    fn static_tiles_force_qoi_bounds_override_global_zstd_codec() {
        let grid = TileGrid::new(64, 64, 64);
        let stride = 64 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 7) as u8;
                frame_data[offset + 1] = (y * 3) as u8;
                frame_data[offset + 2] = x.wrapping_add(y as usize) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }
        let mut emitter = TileEmitter::with_codec(grid.cols, grid.rows, TileCodec::Zstd);
        let frames = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            None,
            None,
            None,
            Some((0, 0, 0, 0)),
            None,
        );
        let tile_msg = frames
            .iter()
            .find_map(|frame| TileMessage::decode(&frame.payload).ok())
            .expect("tile frame");
        assert!(matches!(tile_msg, TileMessage::Qoi { .. }));
    }

    #[test]
    fn emitter_force_qoi_bounds_bypass_skip_and_cache_hit() {
        let mut grid = TileGrid::new(128, 64, 64);
        for col in 0..2 {
            if let Some(tile) = grid.get_mut(TileCoord::new(col, 0)) {
                tile.classification = TileClass::Static;
            }
        }

        let stride = 128 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 3) as u8;
                frame_data[offset + 1] = (y * 5) as u8;
                frame_data[offset + 2] = (x ^ y) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::with_codec(grid.cols, grid.rows, TileCodec::Zstd);

        let first = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            0,
            Some((0, 0, 0, 0)),
            None,
        );
        assert_eq!(first.stats.qoi_tiles, 1);

        let second = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            0,
            Some((0, 0, 0, 0)),
            None,
        );
        assert_eq!(second.stats.skipped, 0);
        assert_eq!(second.stats.cache_hits, 0);
        assert_eq!(second.stats.qoi_tiles, 1);

        for y in 0..64 {
            for x in 0..64 {
                let src = y * stride + x * 4;
                let dst = y * stride + (64 + x) * 4;
                frame_data[dst] = frame_data[src];
                frame_data[dst + 1] = frame_data[src + 1];
                frame_data[dst + 2] = frame_data[src + 2];
                frame_data[dst + 3] = frame_data[src + 3];
            }
        }

        let third = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            0,
            Some((1, 0, 1, 0)),
            None,
        );
        assert_eq!(third.stats.cache_hits, 0);
        assert_eq!(third.stats.qoi_tiles, 1);
        let tile_msg = third
            .tile_frames
            .iter()
            .find_map(|frame| TileMessage::decode(&frame.payload).ok())
            .expect("tile frame");
        assert!(matches!(tile_msg, TileMessage::Qoi { .. }));
    }

    #[test]
    fn static_force_qoi_bounds_bypass_skip_and_cache_hit() {
        let grid = TileGrid::new(128, 64, 64);
        let stride = 128 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 7) as u8;
                frame_data[offset + 1] = (y * 3) as u8;
                frame_data[offset + 2] = x.wrapping_add(y as usize) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::with_codec(grid.cols, grid.rows, TileCodec::Zstd);
        let coords = [TileCoord::new(0, 0)];

        let first = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            Some((0, 0, 0, 0)),
            None,
        );
        assert!(!first.is_empty());

        let second = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            Some((0, 0, 0, 0)),
            None,
        );
        assert!(!second.is_empty());
        assert!(second.iter().all(|frame| {
            matches!(
                TileMessage::decode(&frame.payload).ok(),
                Some(TileMessage::Fill { .. }) | Some(TileMessage::Qoi { .. })
            )
        }));

        for y in 0..64 {
            for x in 0..64 {
                let src = y * stride + x * 4;
                let dst = y * stride + (64 + x) * 4;
                frame_data[dst] = frame_data[src];
                frame_data[dst + 1] = frame_data[src + 1];
                frame_data[dst + 2] = frame_data[src + 2];
                frame_data[dst + 3] = frame_data[src + 3];
            }
        }

        let third = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            None,
            None,
            None,
            Some((1, 0, 1, 0)),
            None,
        );
        assert!(!third.is_empty());
        assert!(third.iter().all(|frame| {
            matches!(
                TileMessage::decode(&frame.payload).ok(),
                Some(TileMessage::Fill { .. }) | Some(TileMessage::Qoi { .. })
            )
        }));
    }

    #[test]
    fn emitter_same_position_skip() {
        let mut grid = TileGrid::new(128, 128, 64);
        grid.mark_damaged(&Rect::new(0, 0, 64, 64));
        if let Some(tile) = grid.get_mut(TileCoord::new(0, 0)) {
            tile.classification = TileClass::Static;
        }

        let frame_data = vec![0xFF; 128 * 128 * 4]; // solid white
        let stride = 128 * 4;
        let dirty = vec![TileCoord::new(0, 0)];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);

        // First emit: should produce a Fill (solid color)
        let result1 = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);
        assert_eq!(result1.stats.fills, 1);
        assert_eq!(result1.stats.skipped, 0);

        // Second emit with same data: should skip entirely (not even CacheHit)
        let result2 = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);
        assert_eq!(result2.stats.skipped, 1);
        assert_eq!(result2.stats.fills, 0);
        assert_eq!(result2.stats.cache_hits, 0);
    }

    #[test]
    fn emitter_cross_position_cache_hit() {
        // 3-column grid, tiles at col 0 and col 1 have varied (non-solid) content.
        let mut grid = TileGrid::new(192, 64, 64);
        for col in 0..3 {
            if let Some(tile) = grid.get_mut(TileCoord::new(col, 0)) {
                tile.classification = TileClass::Static;
            }
        }

        let stride = 192 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        // Paint tile 0 with a non-solid pattern
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 4) as u8; // R varies
                frame_data[offset + 1] = (y * 4) as u8; // G varies
                frame_data[offset + 2] = 0;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);

        // First frame: emit tile 0 -> QOI (non-solid content)
        let result1 = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            0,
            None,
            None,
        );
        assert_eq!(result1.stats.qoi_tiles, 1);

        // Now copy tile 0's content to tile 1's position in the frame buffer
        for y in 0..64 {
            for x in 0..64 {
                let src = y * stride + x * 4;
                let dst = y * stride + (64 + x) * 4;
                frame_data[dst] = frame_data[src];
                frame_data[dst + 1] = frame_data[src + 1];
                frame_data[dst + 2] = frame_data[src + 2];
                frame_data[dst + 3] = frame_data[src + 3];
            }
        }

        // Second frame: emit tile 1 - should be CacheHit (same content, different position)
        let result2 = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            0,
            None,
            None,
        );
        assert_eq!(result2.stats.cache_hits, 1);
        assert_eq!(result2.stats.qoi_tiles, 0);
    }

    #[test]
    fn emitter_cache_miss_invalidates_hash_and_position() {
        let mut grid = TileGrid::new(192, 64, 64);
        for col in 0..3 {
            if let Some(tile) = grid.get_mut(TileCoord::new(col, 0)) {
                tile.classification = TileClass::Static;
            }
        }

        let stride = 192 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        // Non-solid pattern in tile 0
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 3) as u8;
                frame_data[offset + 1] = (y * 5) as u8;
                frame_data[offset + 2] = (x ^ y) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);

        // Seed sender-side known hash map with QOI tile from tile 0.
        let _ = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(0, 0)],
            &grid,
            0,
            None,
            None,
        );

        // Copy tile 0's content to tile 1 so second emit uses CacheHit.
        for y in 0..64 {
            for x in 0..64 {
                let src = y * stride + x * 4;
                let dst = y * stride + (64 + x) * 4;
                frame_data[dst] = frame_data[src];
                frame_data[dst + 1] = frame_data[src + 1];
                frame_data[dst + 2] = frame_data[src + 2];
                frame_data[dst + 3] = frame_data[src + 3];
            }
        }
        let second = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            0,
            None,
            None,
        );
        assert_eq!(second.stats.cache_hits, 1);
        let miss_hash = second
            .tile_frames
            .iter()
            .find_map(|f| match TileMessage::decode(&f.payload).ok() {
                Some(TileMessage::CacheHit { hash, .. }) => Some(hash),
                _ => None,
            })
            .expect("expected cache-hit hash");

        // Client reports cache miss for tile (1,0).
        emitter.handle_cache_miss(1, 0, miss_hash);

        // Next emit for same tile must re-encode (QOI), not skip/cache-hit.
        let third = emitter.emit_frame(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            0,
            None,
            None,
        );
        assert_eq!(third.stats.cache_hits, 0);
        assert_eq!(third.stats.skipped, 0);
        assert_eq!(third.stats.qoi_tiles, 1);
    }

    #[test]
    fn static_tile_cache_miss_invalidates_static_position() {
        let grid = TileGrid::new(128, 64, 64);
        let stride = 128 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 3) as u8;
                frame_data[offset + 1] = (y * 5) as u8;
                frame_data[offset + 2] = (x ^ y) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let coords = [TileCoord::new(0, 0)];

        let first = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(!first.is_empty());

        let second = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(second.is_empty());

        let miss_hash = first
            .iter()
            .find_map(|f| match TileMessage::decode(&f.payload).ok() {
                Some(TileMessage::Qoi { hash, .. }) => Some(hash),
                Some(TileMessage::CacheHit { hash, .. }) => Some(hash),
                _ => None,
            })
            .expect("expected hash-bearing static tile frame");

        emitter.handle_cache_miss(0, 0, miss_hash);

        let third = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(!third.is_empty());
    }

    #[test]
    fn static_boundary_row_reemits_even_when_unchanged() {
        let grid = TileGrid::new(128, 128, 64);
        let stride = 128 * 4;
        let frame_data = vec![0xFF; stride * 128];
        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let coords = [TileCoord::new(0, 1)];

        let first = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            Some(1),
            None,
            None,
            None,
        );
        assert!(!first.is_empty());

        let second = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            Some(1),
            None,
            None,
            None,
        );
        assert!(!second.is_empty());
        assert!(second.iter().all(|f| {
            matches!(
                TileMessage::decode(&f.payload).ok(),
                Some(TileMessage::Fill { .. }) | Some(TileMessage::Qoi { .. })
            )
        }));
    }

    #[test]
    fn static_boundary_tiles_emit_zero_hash_to_disable_client_caching() {
        let grid = TileGrid::new(128, 64, 64);
        let stride = 128 * 4;
        let mut frame_data = vec![0u8; stride * 64];
        for y in 0..64 {
            for x in 0..128 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x * 5) as u8;
                frame_data[offset + 1] = (y * 7) as u8;
                frame_data[offset + 2] = x.wrapping_add(y as usize) as u8;
                frame_data[offset + 3] = 0xFF;
            }
        }

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let frames = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &[TileCoord::new(1, 0)],
            &grid,
            Some(1),
            None,
            None,
            None,
            None,
        );

        let qoi_hash = frames
            .iter()
            .find_map(|f| match TileMessage::decode(&f.payload).ok() {
                Some(TileMessage::Qoi { hash, .. }) => Some(hash),
                Some(TileMessage::Zstd { hash, .. }) => Some(hash),
                _ => None,
            })
            .expect("expected hash-bearing static tile frame");
        assert_eq!(qoi_hash, 0);
    }

    #[test]
    fn emitter_video_region() {
        let mut grid = TileGrid::new(128, 128, 64);
        grid.mark_damaged(&Rect::new(0, 0, 128, 128));
        // Mark all tiles as VideoMotion
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                if let Some(tile) = grid.get_mut(TileCoord::new(col, row)) {
                    tile.classification = TileClass::VideoMotion;
                }
            }
        }

        let frame_data = vec![0xFF; 128 * 128 * 4];
        let stride = 128 * 4;
        let dirty = vec![
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            TileCoord::new(0, 1),
            TileCoord::new(1, 1),
        ];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let result = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);
        assert_eq!(result.stats.video_tiles, 4);
        assert!(result.video_region.is_some());
        let region = result.video_region.unwrap();
        assert_eq!(region.x, 0);
        assert_eq!(region.y, 0);
        assert_eq!(region.w, 128);
        assert_eq!(region.h, 128);
    }

    #[test]
    fn emitter_active_video_region_suppresses_static_tiles() {
        let mut grid = TileGrid::new(128, 64, 64);
        if let Some(tile) = grid.get_mut(TileCoord::new(0, 0)) {
            tile.classification = TileClass::Static;
        }

        let frame_data = vec![0xFF; 128 * 64 * 4];
        let stride = 128 * 4;
        let dirty = vec![TileCoord::new(0, 0)];
        let active_video_region = Some(Rect::new(0, 0, 64, 64));

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let result = emitter.emit_frame(
            &frame_data,
            stride,
            &dirty,
            &grid,
            0,
            None,
            active_video_region,
        );

        assert_eq!(result.stats.video_tiles, 1);
        assert_eq!(result.stats.fills, 0);
        assert_eq!(result.stats.qoi_tiles, 0);
        assert_eq!(result.video_region, active_video_region);
        assert_eq!(result.tile_frames.len(), 2);

        let first = TileMessage::decode(&result.tile_frames[0].payload).unwrap();
        assert!(matches!(
            first,
            TileMessage::VideoRegion {
                x: 0,
                y: 0,
                w: 64,
                h: 64
            }
        ));
    }

    #[test]
    fn emitter_video_owned_tiles_do_not_update_skip_hashes() {
        let mut grid = TileGrid::new(64, 64, 64);
        if let Some(tile) = grid.get_mut(TileCoord::new(0, 0)) {
            tile.classification = TileClass::Static;
        }

        let frame_data = vec![0xFF; 64 * 64 * 4];
        let stride = 64 * 4;
        let dirty = vec![TileCoord::new(0, 0)];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let suppressed = emitter.emit_frame(
            &frame_data,
            stride,
            &dirty,
            &grid,
            0,
            None,
            Some(Rect::new(0, 0, 64, 64)),
        );
        assert_eq!(suppressed.stats.video_tiles, 1);

        let repaired = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);
        assert_eq!(repaired.stats.skipped, 0);
        assert_eq!(repaired.stats.fills, 1);
        assert!(repaired.tile_frames.iter().any(|frame| {
            matches!(
                TileMessage::decode(&frame.payload).ok(),
                Some(TileMessage::VideoRegion {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0
                })
            )
        }));
    }

    #[test]
    fn emitter_static_tiles_respect_active_video_region() {
        let grid = TileGrid::new(128, 64, 64);
        let frame_data = vec![0xFF; 128 * 64 * 4];
        let stride = 128 * 4;
        let coords = [TileCoord::new(0, 0)];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let frames = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            Some(Rect::new(0, 0, 64, 64)),
        );

        assert!(frames.is_empty());
    }

    #[test]
    fn emitter_static_video_owned_tiles_do_not_update_skip_hashes() {
        let grid = TileGrid::new(64, 64, 64);
        let frame_data = vec![0xFF; 64 * 64 * 4];
        let stride = 64 * 4;
        let coords = [TileCoord::new(0, 0)];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let suppressed = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            Some(Rect::new(0, 0, 64, 64)),
        );
        assert!(suppressed.is_empty());

        let repaired = emitter.emit_static_tiles(
            &frame_data,
            stride,
            &coords,
            &grid,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(repaired.len(), 1);
        assert!(matches!(
            TileMessage::decode(&repaired[0].payload).ok(),
            Some(TileMessage::Fill { .. }) | Some(TileMessage::Qoi { .. })
        ));
    }

    #[test]
    fn emitter_grid_config() {
        let grid = TileGrid::new(1280, 768, 64);
        let emitter = TileEmitter::new(grid.cols, grid.rows);
        let frame = emitter.emit_grid_config(&grid);
        assert_eq!(frame.channel, ChannelId::Tiles);

        // Decode and verify
        let msg = TileMessage::decode(&frame.payload).unwrap();
        match msg {
            TileMessage::GridConfig {
                tile_size,
                cols,
                rows,
                screen_w,
                screen_h,
            } => {
                assert_eq!(tile_size, 64);
                assert_eq!(cols, 20);
                assert_eq!(rows, 12);
                assert_eq!(screen_w, 1280);
                assert_eq!(screen_h, 768);
            }
            _ => panic!("expected GridConfig"),
        }
    }

    #[test]
    fn emitter_batch_end_always_present() {
        let grid = TileGrid::new(128, 128, 64);
        let frame_data = vec![0xFF; 128 * 128 * 4];
        let stride = 128 * 4;

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let result = emitter.emit_frame(&frame_data, stride, &[], &grid, 0, None, None);

        // Even with no dirty tiles, BatchEnd should be emitted
        assert!(!result.tile_frames.is_empty());
        let last = result.tile_frames.last().unwrap();
        let msg = TileMessage::decode(&last.payload).unwrap();
        assert!(matches!(msg, TileMessage::BatchEnd { .. }));
    }

    #[test]
    fn emitter_resize_clears_hashes() {
        let mut grid = TileGrid::new(128, 128, 64);
        if let Some(tile) = grid.get_mut(TileCoord::new(0, 0)) {
            tile.classification = TileClass::Static;
        }

        let frame_data = vec![0xFF; 128 * 128 * 4];
        let stride = 128 * 4;
        let dirty = vec![TileCoord::new(0, 0)];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);

        // First emit
        emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);

        // Resize
        emitter.resize(grid.cols, grid.rows);

        // After resize, same data should NOT be a cache hit
        let result = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);
        assert_eq!(result.stats.cache_hits, 0);
        assert_eq!(result.stats.fills, 1);
    }

    #[test]
    fn emitter_mixed_tile_types() {
        let mut grid = TileGrid::new(192, 64, 64);
        // Tile (0,0) = Static (solid), (1,0) = TextScroll (QOI), (2,0) = VideoMotion
        for (col, class) in [
            (0, TileClass::Static),
            (1, TileClass::TextScroll),
            (2, TileClass::VideoMotion),
        ] {
            if let Some(tile) = grid.get_mut(TileCoord::new(col, 0)) {
                tile.classification = class;
            }
        }

        // Solid white for tile 0, varied for tile 1, anything for tile 2
        let stride = 192 * 4;
        let mut frame_data = vec![0xFF; stride * 64]; // all white initially
                                                      // Make tile 1 (pixels 64..128) non-uniform
        for x in 64..128 {
            for y in 0..64 {
                let offset = y * stride + x * 4;
                frame_data[offset] = (x & 0xFF) as u8; // varying R
            }
        }

        let dirty = vec![
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
        ];

        let mut emitter = TileEmitter::new(grid.cols, grid.rows);
        let result = emitter.emit_frame(&frame_data, stride, &dirty, &grid, 0, None, None);

        assert_eq!(result.stats.fills, 1); // tile 0
        assert_eq!(result.stats.qoi_tiles, 1); // tile 1
        assert_eq!(result.stats.video_tiles, 1); // tile 2
        assert!(result.video_region.is_some());
    }
}
