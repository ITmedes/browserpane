use crate::{channel::ChannelId, types::TileMessage};

use super::{
    BATCH_END, CACHE_HIT, CACHE_MISS, FILL, GRID_CONFIG, GRID_OFFSET, QOI, SCROLL_COPY,
    SCROLL_STATS, TILE_DRAW_MODE, VIDEO_REGION, ZSTD,
};
use crate::frame::{io::Reader, FrameError};

pub(super) fn decode(buf: &[u8]) -> Result<TileMessage, FrameError> {
    let mut r = Reader::new(buf);
    let tag = r.read_u8()?;
    match tag {
        GRID_CONFIG => Ok(TileMessage::GridConfig {
            tile_size: r.read_u16()?,
            cols: r.read_u16()?,
            rows: r.read_u16()?,
            screen_w: r.read_u16()?,
            screen_h: r.read_u16()?,
        }),
        CACHE_HIT => Ok(TileMessage::CacheHit {
            col: r.read_u16()?,
            row: r.read_u16()?,
            hash: r.read_u64()?,
        }),
        CACHE_MISS => Ok(TileMessage::CacheMiss {
            frame_seq: r.read_u32()?,
            col: r.read_u16()?,
            row: r.read_u16()?,
            hash: r.read_u64()?,
        }),
        FILL => Ok(TileMessage::Fill {
            col: r.read_u16()?,
            row: r.read_u16()?,
            rgba: r.read_u32()?,
        }),
        QOI => Ok(TileMessage::Qoi {
            col: r.read_u16()?,
            row: r.read_u16()?,
            hash: r.read_u64()?,
            data: r.read_vec_u32()?,
        }),
        ZSTD => Ok(TileMessage::Zstd {
            col: r.read_u16()?,
            row: r.read_u16()?,
            hash: r.read_u64()?,
            data: r.read_vec_u32()?,
        }),
        VIDEO_REGION => Ok(TileMessage::VideoRegion {
            x: r.read_u16()?,
            y: r.read_u16()?,
            w: r.read_u16()?,
            h: r.read_u16()?,
        }),
        BATCH_END => Ok(TileMessage::BatchEnd {
            frame_seq: r.read_u32()?,
        }),
        SCROLL_COPY => Ok(TileMessage::ScrollCopy {
            dx: r.read_i16()?,
            dy: r.read_i16()?,
            region_top: r.read_u16()?,
            region_bottom: r.read_u16()?,
            region_right: r.read_u16()?,
        }),
        GRID_OFFSET => Ok(TileMessage::GridOffset {
            offset_x: r.read_i16()?,
            offset_y: r.read_i16()?,
        }),
        TILE_DRAW_MODE => Ok(TileMessage::TileDrawMode {
            apply_offset: r.read_bool()?,
        }),
        SCROLL_STATS => Ok(TileMessage::ScrollStats {
            scroll_batches_total: r.read_u32()?,
            scroll_full_fallbacks_total: r.read_u32()?,
            scroll_potential_tiles_total: r.read_u32()?,
            scroll_saved_tiles_total: r.read_u32()?,
            scroll_non_quantized_fallbacks_total: r.read_u32()?,
            scroll_residual_full_repaints_total: r.read_u32()?,
            scroll_zero_saved_batches_total: r.read_u32()?,
        }),
        _ => Err(FrameError::UnknownMessageType {
            channel: ChannelId::Tiles.as_u8(),
            tag,
        }),
    }
}
