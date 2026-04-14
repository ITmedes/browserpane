//! Dirty tile set construction: build full emit coordinates and apply
//! XDamage bounding box filter.

use crate::tiles;

impl super::TileCaptureThread {
    /// Build the full set of emit coordinates and narrow it via XDamage
    /// bounding box filtering.
    ///
    /// Returns `(full_emit_coords, initial_all_dirty)`.
    pub(crate) fn build_dirty_set(
        &self,
        force_refresh: bool,
    ) -> (Vec<tiles::TileCoord>, Vec<tiles::TileCoord>) {
        // Emit tiles for all positions, including extra bottom row when
        // grid offset is active (partial tile at bottom edge).
        let emit_rows = if self.grid_offset_y > 0 {
            self.grid.rows + 1
        } else {
            self.grid.rows
        };
        let full_emit_coords: Vec<tiles::TileCoord> = (0..emit_rows)
            .flat_map(|r| (0..self.grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
            .collect();
        // Narrow dirty set to tiles overlapping XDamage bounding box.
        let all_dirty: Vec<tiles::TileCoord> = if !force_refresh {
            if let Some(ref dt) = self.damage {
                if let Some((dx, dy, dw, dh)) = dt.damage_bounding_box() {
                    let ts = self.tile_size as u16;
                    let dx2 = dx.saturating_add(dw);
                    let dy2 = dy.saturating_add(dh);
                    full_emit_coords
                        .iter()
                        .copied()
                        .filter(|coord| {
                            let tx = coord.col * ts;
                            let ty = coord.row * ts;
                            let tx2 = tx.saturating_add(ts);
                            let ty2 = ty.saturating_add(ts);
                            // AABB overlap test
                            tx < dx2 && tx2 > dx && ty < dy2 && ty2 > dy
                        })
                        .collect()
                } else {
                    full_emit_coords.clone()
                }
            } else {
                full_emit_coords.clone()
            }
        } else {
            full_emit_coords.clone()
        };
        (full_emit_coords, all_dirty)
    }
}
