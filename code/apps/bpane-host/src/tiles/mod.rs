//! Tile-based streaming optimization layer.
//!
//! Core types (TileGrid, TileCoord, Rect) and the tile emitter live here.
//! The emitter handles L1/L2 deduplication, QOI/Zstd encoding, and scroll-aware
//! hash management. The production tile loop lives in main.rs.

pub mod emitter;

use std::time::Instant;

// ── Core Types ──────────────────────────────────────────────────────

/// Grid coordinate of a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub col: u16,
    pub row: u16,
}

impl TileCoord {
    pub fn new(col: u16, row: u16) -> Self {
        Self { col, row }
    }
}

/// Pixel-space rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub fn area(&self) -> u32 {
        self.w as u32 * self.h as u32
    }

    /// Returns true if this rect overlaps `other`.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    /// Merge two rects into a bounding rect.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        Rect {
            x,
            y,
            w: x2 - x,
            h: y2 - y,
        }
    }
}

/// Classification of a tile's visual content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileClass {
    /// Unchanged UI chrome, static text.
    Static,
    /// Scrolling text or document content.
    TextScroll,
    /// Animation, video, canvas, rapid updates.
    VideoMotion,
    /// Cursor movement only (small change near cursor).
    CursorOnly,
    /// Uncertain or mixed content — conservative fallback.
    Mixed,
}

/// Per-tile state tracked across frames.
#[derive(Debug, Clone)]
pub struct TileState {
    pub dirty: bool,
    pub last_changed_frame: u64,
    /// Number of frames this tile changed in the last N frames.
    pub change_count_window: u16,
    pub last_change_ratio: f32,
    pub last_edge_density: f32,
    pub motion_magnitude: f32,
    pub temporal_stability: f32,
    pub classification: TileClass,
    pub confidence: f32,
    pub last_sent_frame: u64,
    /// Hash of the tile pixels when last sent, for recovery validation.
    pub last_sent_hash: u64,
    pub needs_recovery: bool,
}

impl Default for TileState {
    fn default() -> Self {
        Self {
            dirty: false,
            last_changed_frame: 0,
            change_count_window: 0,
            last_change_ratio: 0.0,
            last_edge_density: 0.0,
            motion_magnitude: 0.0,
            temporal_stability: 1.0,
            classification: TileClass::Static,
            confidence: 1.0,
            last_sent_frame: 0,
            last_sent_hash: 0,
            needs_recovery: false,
        }
    }
}

// ── TileGrid ────────────────────────────────────────────────────────

/// A grid of tiles overlaid on the captured screen surface.
pub struct TileGrid {
    pub tile_size: u16,
    pub cols: u16,
    pub rows: u16,
    pub screen_w: u16,
    pub screen_h: u16,
    pub tiles: Vec<TileState>,
    pub frame_counter: u64,
}

impl TileGrid {
    /// Create a new tile grid for the given screen dimensions.
    pub fn new(screen_w: u16, screen_h: u16, tile_size: u16) -> Self {
        assert!(tile_size > 0, "tile_size must be > 0");
        let cols = (screen_w + tile_size - 1) / tile_size;
        let rows = (screen_h + tile_size - 1) / tile_size;
        let count = cols as usize * rows as usize;
        Self {
            tile_size,
            cols,
            rows,
            screen_w,
            screen_h,
            tiles: vec![TileState::default(); count],
            frame_counter: 0,
        }
    }

    /// Advance the frame counter. Called once per capture cycle.
    pub fn advance_frame(&mut self) {
        self.frame_counter += 1;
    }

    /// Get the tile state at grid coordinates.
    pub fn get(&self, coord: TileCoord) -> Option<&TileState> {
        if coord.col < self.cols && coord.row < self.rows {
            Some(&self.tiles[coord.row as usize * self.cols as usize + coord.col as usize])
        } else {
            None
        }
    }

    /// Get mutable tile state at grid coordinates.
    pub fn get_mut(&mut self, coord: TileCoord) -> Option<&mut TileState> {
        if coord.col < self.cols && coord.row < self.rows {
            Some(&mut self.tiles[coord.row as usize * self.cols as usize + coord.col as usize])
        } else {
            None
        }
    }

    /// Get the pixel-space rect for a tile coordinate.
    pub fn tile_rect(&self, coord: TileCoord) -> Rect {
        let x = coord.col * self.tile_size;
        let y = coord.row * self.tile_size;
        let w = self.tile_size.min(self.screen_w.saturating_sub(x));
        let h = self.tile_size.min(self.screen_h.saturating_sub(y));
        Rect { x, y, w, h }
    }

    /// Map a pixel-space damage rect to the set of affected tile coordinates.
    pub fn damage_to_tiles(&self, damage: &Rect) -> Vec<TileCoord> {
        let mut tiles = Vec::new();
        if damage.w == 0 || damage.h == 0 {
            return tiles;
        }
        let col_start = damage.x / self.tile_size;
        let col_end = ((damage.x + damage.w).saturating_sub(1) / self.tile_size).min(self.cols - 1);
        let row_start = damage.y / self.tile_size;
        let row_end = ((damage.y + damage.h).saturating_sub(1) / self.tile_size).min(self.rows - 1);
        for row in row_start..=row_end {
            for col in col_start..=col_end {
                tiles.push(TileCoord::new(col, row));
            }
        }
        tiles
    }

    /// Mark tiles as dirty from a damage rect.
    pub fn mark_damaged(&mut self, damage: &Rect) {
        let coords = self.damage_to_tiles(damage);
        let frame = self.frame_counter;
        for coord in coords {
            if let Some(tile) = self.get_mut(coord) {
                tile.dirty = true;
                tile.last_changed_frame = frame;
            }
        }
    }

    /// Get all currently dirty tile coordinates.
    pub fn dirty_tiles(&self) -> Vec<TileCoord> {
        let mut result = Vec::new();
        for row in 0..self.rows {
            for col in 0..self.cols {
                let idx = row as usize * self.cols as usize + col as usize;
                if self.tiles[idx].dirty {
                    result.push(TileCoord::new(col, row));
                }
            }
        }
        result
    }

    /// Clear dirty flags on all tiles.
    pub fn clear_dirty(&mut self) {
        for tile in &mut self.tiles {
            tile.dirty = false;
        }
    }

    /// Clear dirty flag on specific tiles.
    pub fn clear_dirty_tiles(&mut self, coords: &[TileCoord]) {
        for coord in coords {
            if let Some(tile) = self.get_mut(*coord) {
                tile.dirty = false;
            }
        }
    }

    /// Rebuild the grid for a new screen resolution. Marks all tiles dirty.
    pub fn resize(&mut self, screen_w: u16, screen_h: u16) {
        let cols = (screen_w + self.tile_size - 1) / self.tile_size;
        let rows = (screen_h + self.tile_size - 1) / self.tile_size;
        let count = cols as usize * rows as usize;
        self.screen_w = screen_w;
        self.screen_h = screen_h;
        self.cols = cols;
        self.rows = rows;
        self.tiles = vec![TileState::default(); count];
        // Mark all dirty after resize
        for tile in &mut self.tiles {
            tile.dirty = true;
            tile.last_changed_frame = self.frame_counter;
        }
    }

    /// Compute bounding rect of a set of tile coordinates.
    pub fn tiles_bounding_rect(&self, coords: &[TileCoord]) -> Option<Rect> {
        if coords.is_empty() {
            return None;
        }
        let first = self.tile_rect(coords[0]);
        let mut result = first;
        for &coord in &coords[1..] {
            let r = self.tile_rect(coord);
            result = result.union(&r);
        }
        Some(result)
    }

    /// Merge adjacent dirty tiles into larger rectangular regions.
    /// Returns a list of merged rects covering all dirty tiles.
    pub fn merge_dirty_regions(&self) -> Vec<Rect> {
        let dirty = self.dirty_tiles();
        if dirty.is_empty() {
            return Vec::new();
        }
        // Group by contiguous row spans
        merge_tile_coords(&dirty, self.tile_size, self.screen_w, self.screen_h)
    }

    /// Total number of tiles.
    pub fn tile_count(&self) -> usize {
        self.cols as usize * self.rows as usize
    }

    /// Fraction of tiles currently dirty.
    pub fn dirty_fraction(&self) -> f32 {
        let total = self.tile_count();
        if total == 0 {
            return 0.0;
        }
        self.dirty_tiles().len() as f32 / total as f32
    }
}

/// Merge a list of tile coordinates into bounding rects.
/// Uses a greedy row-scan approach to merge horizontally contiguous tiles.
fn merge_tile_coords(
    coords: &[TileCoord],
    tile_size: u16,
    screen_w: u16,
    screen_h: u16,
) -> Vec<Rect> {
    if coords.is_empty() {
        return Vec::new();
    }

    // Build a set for O(1) lookup
    let mut set = std::collections::HashSet::with_capacity(coords.len());
    for c in coords {
        set.insert((c.col, c.row));
    }

    let mut visited = std::collections::HashSet::new();
    let mut rects = Vec::new();

    // Find bounding box of grid coords
    let max_col = coords.iter().map(|c| c.col).max().unwrap();
    let max_row = coords.iter().map(|c| c.row).max().unwrap();

    for row in 0..=max_row {
        for col in 0..=max_col {
            if !set.contains(&(col, row)) || visited.contains(&(col, row)) {
                continue;
            }
            // Find max horizontal span
            let mut end_col = col;
            while end_col <= max_col
                && set.contains(&(end_col, row))
                && !visited.contains(&(end_col, row))
            {
                end_col += 1;
            }
            // Extend downward while full row span matches
            let mut end_row = row + 1;
            'outer: while end_row <= max_row {
                for c in col..end_col {
                    if !set.contains(&(c, end_row)) || visited.contains(&(c, end_row)) {
                        break 'outer;
                    }
                }
                end_row += 1;
            }
            // Mark visited
            for r in row..end_row {
                for c in col..end_col {
                    visited.insert((c, r));
                }
            }
            let x = col * tile_size;
            let y = row * tile_size;
            let w = ((end_col - col) * tile_size).min(screen_w.saturating_sub(x));
            let h = ((end_row - row) * tile_size).min(screen_h.saturating_sub(y));
            rects.push(Rect::new(x, y, w, h));
        }
    }

    rects
}

// ── DamageCollector ─────────────────────────────────────────────────

/// Collects raw damage rectangles and maps them onto the tile grid
/// with configurable coalescing window.
pub struct DamageCollector {
    pending_rects: Vec<Rect>,
    first_damage_at: Option<Instant>,
    coalesce_window: std::time::Duration,
}

impl DamageCollector {
    pub fn new(coalesce_window_ms: u64) -> Self {
        Self {
            pending_rects: Vec::new(),
            first_damage_at: None,
            coalesce_window: std::time::Duration::from_millis(coalesce_window_ms),
        }
    }

    /// Add a damage rectangle. Deduplicates overlapping rects.
    pub fn add_damage(&mut self, rect: Rect) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        if self.first_damage_at.is_none() {
            self.first_damage_at = Some(Instant::now());
        }
        // Check if this rect is fully contained by an existing rect
        let dominated = self.pending_rects.iter().any(|existing| {
            rect.x >= existing.x
                && rect.y >= existing.y
                && rect.x + rect.w <= existing.x + existing.w
                && rect.y + rect.h <= existing.y + existing.h
        });
        if !dominated {
            self.pending_rects.push(rect);
        }
    }

    /// Returns true if the coalescing window has elapsed and there is pending damage.
    pub fn is_ready(&self) -> bool {
        match self.first_damage_at {
            Some(t) => !self.pending_rects.is_empty() && t.elapsed() >= self.coalesce_window,
            None => false,
        }
    }

    /// Check if there is any pending damage (regardless of window).
    pub fn has_damage(&self) -> bool {
        !self.pending_rects.is_empty()
    }

    /// Flush pending damage into the tile grid, marking affected tiles dirty.
    /// Returns the list of newly dirtied tile coordinates.
    pub fn flush(&mut self, grid: &mut TileGrid) -> Vec<TileCoord> {
        let mut all_coords = Vec::new();
        let frame = grid.frame_counter;
        for rect in &self.pending_rects {
            let coords = grid.damage_to_tiles(rect);
            for coord in &coords {
                if let Some(tile) = grid.get_mut(*coord) {
                    if !tile.dirty {
                        tile.dirty = true;
                        tile.last_changed_frame = frame;
                        all_coords.push(*coord);
                    }
                }
            }
        }
        self.pending_rects.clear();
        self.first_damage_at = None;
        all_coords
    }

    /// Get the bounding box of all pending damage.
    pub fn bounding_box(&self) -> Option<Rect> {
        if self.pending_rects.is_empty() {
            return None;
        }
        let mut result = self.pending_rects[0];
        for r in &self.pending_rects[1..] {
            result = result.union(r);
        }
        Some(result)
    }

    /// Number of pending damage rects.
    pub fn pending_count(&self) -> usize {
        self.pending_rects.len()
    }

    /// Discard all pending damage without flushing.
    pub fn clear(&mut self) {
        self.pending_rects.clear();
        self.first_damage_at = None;
    }
}

// ── Tile Content Hash ────────────────────────────────────────────────

/// Tile content hash using xxHash3 (64-bit).
/// Named `fnv_hash` for historical compatibility — was FNV-1a, now xxHash3.
pub fn fnv_hash(data: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(data)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Rect tests ──

    #[test]
    fn rect_area() {
        assert_eq!(Rect::new(0, 0, 10, 20).area(), 200);
        assert_eq!(Rect::new(0, 0, 0, 20).area(), 0);
    }

    #[test]
    fn rect_overlaps() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        let c = Rect::new(20, 20, 5, 5);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(10, 10, 5, 5);
        let b = Rect::new(20, 20, 5, 5);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(10, 10, 15, 15));
    }

    #[test]
    fn rect_union_overlapping() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0, 0, 15, 15));
    }

    // ── TileCoord tests ──

    #[test]
    fn tile_coord_equality() {
        assert_eq!(TileCoord::new(1, 2), TileCoord::new(1, 2));
        assert_ne!(TileCoord::new(1, 2), TileCoord::new(2, 1));
    }

    // ── TileGrid tests ──

    #[test]
    fn grid_dimensions() {
        let grid = TileGrid::new(640, 480, 64);
        assert_eq!(grid.cols, 10);
        assert_eq!(grid.rows, 8); // ceil(480/64) = 7.5 → 8
        assert_eq!(grid.tile_count(), 80);
    }

    #[test]
    fn grid_dimensions_not_divisible() {
        let grid = TileGrid::new(100, 100, 64);
        assert_eq!(grid.cols, 2); // ceil(100/64) = 2
        assert_eq!(grid.rows, 2);
    }

    #[test]
    fn grid_tile_rect() {
        let grid = TileGrid::new(640, 480, 64);
        assert_eq!(
            grid.tile_rect(TileCoord::new(0, 0)),
            Rect::new(0, 0, 64, 64)
        );
        assert_eq!(
            grid.tile_rect(TileCoord::new(9, 7)),
            Rect::new(576, 448, 64, 32)
        );
    }

    #[test]
    fn grid_tile_rect_clamps_to_screen() {
        let grid = TileGrid::new(100, 100, 64);
        // Last tile should be clamped
        assert_eq!(
            grid.tile_rect(TileCoord::new(1, 1)),
            Rect::new(64, 64, 36, 36)
        );
    }

    #[test]
    fn grid_damage_to_tiles_single() {
        let grid = TileGrid::new(640, 480, 64);
        let damage = Rect::new(10, 10, 5, 5);
        let tiles = grid.damage_to_tiles(&damage);
        assert_eq!(tiles, vec![TileCoord::new(0, 0)]);
    }

    #[test]
    fn grid_damage_to_tiles_spanning() {
        let grid = TileGrid::new(640, 480, 64);
        let damage = Rect::new(60, 60, 10, 10);
        let tiles = grid.damage_to_tiles(&damage);
        assert_eq!(tiles.len(), 4);
        assert!(tiles.contains(&TileCoord::new(0, 0)));
        assert!(tiles.contains(&TileCoord::new(1, 0)));
        assert!(tiles.contains(&TileCoord::new(0, 1)));
        assert!(tiles.contains(&TileCoord::new(1, 1)));
    }

    #[test]
    fn grid_damage_to_tiles_empty() {
        let grid = TileGrid::new(640, 480, 64);
        let damage = Rect::new(10, 10, 0, 0);
        assert!(grid.damage_to_tiles(&damage).is_empty());
    }

    #[test]
    fn grid_mark_damaged() {
        let mut grid = TileGrid::new(640, 480, 64);
        assert!(grid.dirty_tiles().is_empty());
        grid.mark_damaged(&Rect::new(0, 0, 10, 10));
        let dirty = grid.dirty_tiles();
        assert_eq!(dirty, vec![TileCoord::new(0, 0)]);
    }

    #[test]
    fn grid_clear_dirty() {
        let mut grid = TileGrid::new(640, 480, 64);
        grid.mark_damaged(&Rect::new(0, 0, 200, 200));
        assert!(!grid.dirty_tiles().is_empty());
        grid.clear_dirty();
        assert!(grid.dirty_tiles().is_empty());
    }

    #[test]
    fn grid_clear_dirty_specific() {
        let mut grid = TileGrid::new(640, 480, 64);
        grid.mark_damaged(&Rect::new(0, 0, 200, 200));
        let dirty_before = grid.dirty_tiles().len();
        assert!(dirty_before > 1);
        grid.clear_dirty_tiles(&[TileCoord::new(0, 0)]);
        let dirty_after = grid.dirty_tiles().len();
        assert_eq!(dirty_after, dirty_before - 1);
    }

    #[test]
    fn grid_resize() {
        let mut grid = TileGrid::new(640, 480, 64);
        assert_eq!(grid.tile_count(), 80);
        grid.resize(1920, 1080);
        assert_eq!(grid.cols, 30);
        assert_eq!(grid.rows, 17);
        // All tiles should be dirty after resize
        assert_eq!(grid.dirty_tiles().len(), grid.tile_count());
    }

    #[test]
    fn grid_dirty_fraction() {
        let mut grid = TileGrid::new(128, 128, 64);
        assert_eq!(grid.dirty_fraction(), 0.0);
        grid.mark_damaged(&Rect::new(0, 0, 64, 64));
        assert!((grid.dirty_fraction() - 0.25).abs() < 0.01);
    }

    #[test]
    fn grid_advance_frame() {
        let mut grid = TileGrid::new(128, 128, 64);
        assert_eq!(grid.frame_counter, 0);
        grid.advance_frame();
        assert_eq!(grid.frame_counter, 1);
    }

    #[test]
    fn grid_tiles_bounding_rect() {
        let grid = TileGrid::new(640, 480, 64);
        let coords = vec![TileCoord::new(0, 0), TileCoord::new(2, 2)];
        let bbox = grid.tiles_bounding_rect(&coords).unwrap();
        assert_eq!(bbox, Rect::new(0, 0, 192, 192));
    }

    #[test]
    fn grid_tiles_bounding_rect_empty() {
        let grid = TileGrid::new(640, 480, 64);
        assert!(grid.tiles_bounding_rect(&[]).is_none());
    }

    #[test]
    fn grid_merge_dirty_regions() {
        let mut grid = TileGrid::new(256, 256, 64);
        // Mark a 2×2 block
        grid.mark_damaged(&Rect::new(0, 0, 128, 128));
        let regions = grid.merge_dirty_regions();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(0, 0, 128, 128));
    }

    #[test]
    fn grid_merge_dirty_regions_disjoint() {
        let mut grid = TileGrid::new(256, 256, 64);
        grid.mark_damaged(&Rect::new(0, 0, 10, 10));
        grid.mark_damaged(&Rect::new(192, 192, 10, 10));
        let regions = grid.merge_dirty_regions();
        assert_eq!(regions.len(), 2);
    }

    // ── merge_tile_coords tests ──

    #[test]
    fn merge_single_tile() {
        let coords = vec![TileCoord::new(0, 0)];
        let rects = merge_tile_coords(&coords, 64, 640, 480);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], Rect::new(0, 0, 64, 64));
    }

    #[test]
    fn merge_horizontal_span() {
        let coords = vec![
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
        ];
        let rects = merge_tile_coords(&coords, 64, 640, 480);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], Rect::new(0, 0, 192, 64));
    }

    #[test]
    fn merge_rect_block() {
        let coords = vec![
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            TileCoord::new(0, 1),
            TileCoord::new(1, 1),
        ];
        let rects = merge_tile_coords(&coords, 64, 640, 480);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], Rect::new(0, 0, 128, 128));
    }

    #[test]
    fn merge_empty() {
        let rects = merge_tile_coords(&[], 64, 640, 480);
        assert!(rects.is_empty());
    }

    // ── DamageCollector tests ──

    #[test]
    fn collector_empty() {
        let collector = DamageCollector::new(0);
        assert!(!collector.has_damage());
        assert!(!collector.is_ready());
        assert_eq!(collector.pending_count(), 0);
    }

    #[test]
    fn collector_add_damage() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(10, 10, 5, 5));
        assert!(collector.has_damage());
        assert_eq!(collector.pending_count(), 1);
    }

    #[test]
    fn collector_skips_empty_rects() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(10, 10, 0, 5));
        assert!(!collector.has_damage());
    }

    #[test]
    fn collector_deduplicates_contained() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(0, 0, 100, 100));
        collector.add_damage(Rect::new(10, 10, 5, 5)); // contained
        assert_eq!(collector.pending_count(), 1);
    }

    #[test]
    fn collector_flush() {
        let mut collector = DamageCollector::new(0);
        let mut grid = TileGrid::new(256, 256, 64);
        collector.add_damage(Rect::new(10, 10, 5, 5));
        let coords = collector.flush(&mut grid);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], TileCoord::new(0, 0));
        assert!(grid.get(TileCoord::new(0, 0)).unwrap().dirty);
        assert!(!collector.has_damage());
    }

    #[test]
    fn collector_flush_deduplicates_tiles() {
        let mut collector = DamageCollector::new(0);
        let mut grid = TileGrid::new(256, 256, 64);
        // Two damage rects hitting the same tile
        collector.add_damage(Rect::new(10, 10, 5, 5));
        collector.add_damage(Rect::new(20, 20, 5, 5));
        let coords = collector.flush(&mut grid);
        // Should only report tile (0,0) once
        assert_eq!(coords.len(), 1);
    }

    #[test]
    fn collector_bounding_box() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(10, 10, 5, 5));
        collector.add_damage(Rect::new(100, 100, 20, 20));
        let bbox = collector.bounding_box().unwrap();
        assert_eq!(bbox, Rect::new(10, 10, 110, 110));
    }

    #[test]
    fn collector_clear() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(10, 10, 5, 5));
        collector.clear();
        assert!(!collector.has_damage());
    }

    #[test]
    fn collector_is_ready_zero_window() {
        let mut collector = DamageCollector::new(0);
        collector.add_damage(Rect::new(10, 10, 5, 5));
        assert!(collector.is_ready());
    }

    #[test]
    fn collector_is_ready_with_window() {
        let mut collector = DamageCollector::new(1000); // 1s window
        collector.add_damage(Rect::new(10, 10, 5, 5));
        // Should NOT be ready immediately
        assert!(!collector.is_ready());
    }

    // ── Tile content hash tests ──

    #[test]
    fn fnv_hash_deterministic() {
        let data = b"hello world";
        assert_eq!(fnv_hash(data), fnv_hash(data));
    }

    #[test]
    fn fnv_hash_different_inputs() {
        assert_ne!(fnv_hash(b"hello"), fnv_hash(b"world"));
    }

    #[test]
    fn fnv_hash_empty() {
        // xxHash3 of empty input — deterministic, just not FNV's initial value.
        let empty_hash = fnv_hash(b"");
        assert_eq!(fnv_hash(b""), empty_hash); // consistent
    }

    // ── TileState default ──

    #[test]
    fn tile_state_default() {
        let state = TileState::default();
        assert!(!state.dirty);
        assert_eq!(state.classification, TileClass::Static);
        assert_eq!(state.confidence, 1.0);
        assert_eq!(state.temporal_stability, 1.0);
    }
}
