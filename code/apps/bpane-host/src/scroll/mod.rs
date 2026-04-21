//! Scroll displacement detection and ScrollCopy optimisation.
//!
//! Submodules:
//! - `detect`: pixel-based column scroll detection
//! - `residual`: tile-level residual dirty-set analysis
//! - `policy`: heuristic decision functions for scroll behaviour

pub mod detect;
pub mod policy;
pub mod residual;

#[cfg(test)]
mod tests;

// Re-export the most commonly used items.
pub use detect::{
    content_scroll_search_limit_px, detect_column_scroll, select_wheel_trusted_scroll,
};
pub use policy::{
    can_emit_scroll_copy, has_scroll_region_split, is_content_tile_in_scroll_region,
    next_scroll_active_capture_frames, select_capture_frame_interval, should_defer_scroll_repair,
    should_emit_scroll_copy,
};
pub use residual::{analyze_scroll_residual_emit_coords, build_scroll_exposed_strip_emit_coords};

/// Top-level constants shared across scroll submodules.
pub const CONTENT_SCROLL_SEARCH_MAX_PX: usize = 384;
pub const SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT: f32 = 0.70;
pub const SCROLL_DEFER_REPAIR_MAX_INTERIOR_RATIO: f32 = 0.82;
pub const SCROLL_DEFER_REPAIR_MIN_SAVED_RATIO: f32 = 0.20;
pub const SCROLL_DEFER_REPAIR_MAX_ROW_SHIFT: i32 = 2;
