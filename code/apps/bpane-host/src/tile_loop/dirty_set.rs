//! Dirty tile set construction: build full emit coordinates and apply
//! XDamage bounding box filter.

use crate::tiles;

impl super::TileCaptureThread {
    fn refresh_full_emit_coords(&mut self) {
        let emit_rows = if self.grid_offset_y > 0 {
            self.grid.rows + 1
        } else {
            self.grid.rows
        };
        let expected = emit_rows as usize * self.grid.cols as usize;
        if self.full_emit_coords.len() == expected {
            return;
        }
        self.full_emit_coords = (0..emit_rows)
            .flat_map(|r| (0..self.grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
            .collect();
    }

    /// Build the full set of emit coordinates and narrow it via XDamage
    /// bounding box filtering.
    pub(crate) fn build_dirty_set(&mut self, force_refresh: bool) -> Vec<tiles::TileCoord> {
        self.refresh_full_emit_coords();
        // Narrow dirty set to tiles overlapping XDamage bounding box.
        let all_dirty: Vec<tiles::TileCoord> = if !force_refresh {
            if let Some(ref dt) = self.damage {
                if let Some((dx, dy, dw, dh)) = dt.damage_bounding_box() {
                    let ts = self.tile_size as u16;
                    let dx2 = dx.saturating_add(dw);
                    let dy2 = dy.saturating_add(dh);
                    self.full_emit_coords
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
                    self.full_emit_coords.clone()
                }
            } else {
                self.full_emit_coords.clone()
            }
        } else {
            self.full_emit_coords.clone()
        };
        all_dirty
    }
}
