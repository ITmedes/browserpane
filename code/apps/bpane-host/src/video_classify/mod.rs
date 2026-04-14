//! Per-tile video vs. text/UI classification with motion features and
//! bounding-box stability analysis.

#[cfg(test)]
mod tests;

// ── Motion features ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct TileMotionFeatures {
    pub change_ratio: f32,
    pub motion_magnitude: f32,
    pub edge_density: f32,
    pub entropy_hint: f32,
}

/// Compute lightweight motion/content features for one tile by sampling
/// the current and (optionally) previous frame buffer.
pub fn compute_tile_motion_features(
    current: &[u8],
    previous: Option<&[u8]>,
    stride: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> TileMotionFeatures {
    if w == 0 || h == 0 {
        return TileMotionFeatures::default();
    }

    let pixel_count = w * h;
    let sample_step = if pixel_count > 2048 {
        4
    } else if pixel_count > 1024 {
        2
    } else {
        1
    };
    let row_step = if h > 32 { 2 } else { 1 };

    let mut sampled = 0u32;
    let mut changed = 0u32;
    let mut sad_sum = 0u64;
    let mut edge_total = 0u32;
    let mut edge_hits = 0u32;
    let mut lum_seen = [0u64; 4];
    let mut lum_bins = 0u32;

    for row in (0..h).step_by(row_step) {
        let row_start = (y + row) * stride + x * 4;
        let row_end = row_start + w * 4;
        if row_end > current.len() {
            continue;
        }

        let mut prev_sample_off: Option<usize> = None;
        for col in (0..w).step_by(sample_step) {
            let off = row_start + col * 4;
            if off + 2 >= current.len() {
                break;
            }
            sampled += 1;

            let cr = current[off] as i32;
            let cg = current[off + 1] as i32;
            let cb = current[off + 2] as i32;

            if let Some(prev_buf) = previous {
                if off + 2 < prev_buf.len() {
                    let pr = prev_buf[off] as i32;
                    let pg = prev_buf[off + 1] as i32;
                    let pb = prev_buf[off + 2] as i32;
                    let dr = (cr - pr).unsigned_abs();
                    let dg = (cg - pg).unsigned_abs();
                    let db = (cb - pb).unsigned_abs();
                    if dr > 6 || dg > 6 || db > 6 {
                        changed += 1;
                    }
                    sad_sum += (dr + dg + db) as u64;
                } else {
                    changed += 1;
                    sad_sum += 765;
                }
            } else {
                changed += 1;
                sad_sum += 765;
            }

            let lum = ((cr as u16 + (2 * cg as u16) + cb as u16) >> 2) as u8;
            let word = (lum >> 6) as usize;
            let bit = 1u64 << (lum & 63);
            if lum_seen[word] & bit == 0 {
                lum_seen[word] |= bit;
                lum_bins += 1;
            }

            if let Some(prev_off) = prev_sample_off {
                edge_total += 1;
                let lr = current[prev_off] as i32;
                let lg = current[prev_off + 1] as i32;
                let lb = current[prev_off + 2] as i32;
                let gradient =
                    (cr - lr).unsigned_abs() + (cg - lg).unsigned_abs() + (cb - lb).unsigned_abs();
                if gradient > 42 {
                    edge_hits += 1;
                }
            }
            prev_sample_off = Some(off);
        }
    }

    if sampled == 0 {
        return TileMotionFeatures::default();
    }

    TileMotionFeatures {
        change_ratio: changed as f32 / sampled as f32,
        motion_magnitude: (sad_sum as f32 / (sampled as f32 * 765.0)).min(1.0),
        edge_density: if edge_total == 0 {
            0.0
        } else {
            (edge_hits as f32 / edge_total as f32).min(1.0)
        },
        entropy_hint: (lum_bins as f32 / 64.0).min(1.0),
    }
}

// ── Bounding box utilities ──────────────────────────────────────────

/// IoU for tile-space bounding boxes encoded as (min_col, min_row, max_col, max_row).
pub fn bbox_iou(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> f32 {
    let (a_min_c, a_min_r, a_max_c, a_max_r) = a;
    let (b_min_c, b_min_r, b_max_c, b_max_r) = b;
    let ic_min = a_min_c.max(b_min_c);
    let ic_max = a_max_c.min(b_max_c);
    let ir_min = a_min_r.max(b_min_r);
    let ir_max = a_max_r.min(b_max_r);
    if ic_min > ic_max || ir_min > ir_max {
        return 0.0;
    }
    let inter = (ic_max - ic_min + 1) as f32 * (ir_max - ir_min + 1) as f32;
    let a_area = (a_max_c - a_min_c + 1) as f32 * (a_max_r - a_min_r + 1) as f32;
    let b_area = (b_max_c - b_min_c + 1) as f32 * (b_max_r - b_min_r + 1) as f32;
    let union = a_area + b_area - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Max-axis center displacement between two tile-space bounding boxes.
pub fn bbox_center_shift(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> f32 {
    let acx = (a.0 as f32 + a.2 as f32) * 0.5;
    let acy = (a.1 as f32 + a.3 as f32) * 0.5;
    let bcx = (b.0 as f32 + b.2 as f32) * 0.5;
    let bcy = (b.1 as f32 + b.3 as f32) * 0.5;
    (acx - bcx).abs().max((acy - bcy).abs())
}

// ── Photo-like content detection ────────────────────────────────────

/// Check if a tile contains photographic/video content vs text/UI
/// using unique luminance count. Uses a stack-allocated 256-bit bitset.
pub fn is_photo_like_tile(
    frame: &[u8],
    stride: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    threshold: usize,
) -> bool {
    let mut seen = [0u64; 4];
    let mut count = 0usize;
    for row in 0..h {
        let start = (y + row) * stride + x * 4;
        let end = start + w * 4;
        if end <= frame.len() {
            for pixel in frame[start..end].chunks_exact(4) {
                let lum = ((pixel[0] as u16) + 2 * (pixel[1] as u16) + (pixel[2] as u16)) >> 2;
                let lum = lum as u8;
                let word = (lum >> 6) as usize;
                let bit = 1u64 << (lum & 63);
                if seen[word] & bit == 0 {
                    seen[word] |= bit;
                    count += 1;
                    if count > threshold {
                        return true;
                    }
                }
            }
        }
    }
    false
}
