mod decode;

use alloc::vec::Vec;

use crate::{channel::ChannelId, types::TileMessage};

use super::{envelope::Frame, error::FrameError, io::Writer};

const GRID_CONFIG: u8 = 0x01;
const CACHE_HIT: u8 = 0x02;
const FILL: u8 = 0x03;
const QOI: u8 = 0x04;
const VIDEO_REGION: u8 = 0x05;
const BATCH_END: u8 = 0x06;
const SCROLL_COPY: u8 = 0x07;
const GRID_OFFSET: u8 = 0x08;
const CACHE_MISS: u8 = 0x09;
const SCROLL_STATS: u8 = 0x0A;
const TILE_DRAW_MODE: u8 = 0x0B;
const ZSTD: u8 = 0x0C;

impl TileMessage {
    /// Encode a tile command payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::GridConfig {
                tile_size,
                cols,
                rows,
                screen_w,
                screen_h,
            } => {
                w.write_u8(GRID_CONFIG);
                w.write_u16(*tile_size);
                w.write_u16(*cols);
                w.write_u16(*rows);
                w.write_u16(*screen_w);
                w.write_u16(*screen_h);
            }
            Self::CacheHit { col, row, hash } => {
                w.write_u8(CACHE_HIT);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
            }
            Self::CacheMiss {
                frame_seq,
                col,
                row,
                hash,
            } => {
                w.write_u8(CACHE_MISS);
                w.write_u32(*frame_seq);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
            }
            Self::Fill { col, row, rgba } => {
                w.write_u8(FILL);
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u32(*rgba);
            }
            Self::Qoi {
                col,
                row,
                hash,
                data,
            }
            | Self::Zstd {
                col,
                row,
                hash,
                data,
            } => {
                w.write_u8(if matches!(self, Self::Qoi { .. }) {
                    QOI
                } else {
                    ZSTD
                });
                w.write_u16(*col);
                w.write_u16(*row);
                w.write_u64(*hash);
                w.write_vec_u32(data);
            }
            Self::VideoRegion {
                x,
                y,
                w: width,
                h: height,
            } => {
                w.write_u8(VIDEO_REGION);
                w.write_u16(*x);
                w.write_u16(*y);
                w.write_u16(*width);
                w.write_u16(*height);
            }
            Self::BatchEnd { frame_seq } => {
                w.write_u8(BATCH_END);
                w.write_u32(*frame_seq);
            }
            Self::ScrollCopy {
                dx,
                dy,
                region_top,
                region_bottom,
                region_right,
            } => {
                w.write_u8(SCROLL_COPY);
                w.write_i16(*dx);
                w.write_i16(*dy);
                w.write_u16(*region_top);
                w.write_u16(*region_bottom);
                w.write_u16(*region_right);
            }
            Self::GridOffset { offset_x, offset_y } => {
                w.write_u8(GRID_OFFSET);
                w.write_i16(*offset_x);
                w.write_i16(*offset_y);
            }
            Self::TileDrawMode { apply_offset } => {
                w.write_u8(TILE_DRAW_MODE);
                w.write_bool(*apply_offset);
            }
            Self::ScrollStats {
                scroll_batches_total,
                scroll_full_fallbacks_total,
                scroll_potential_tiles_total,
                scroll_saved_tiles_total,
                scroll_non_quantized_fallbacks_total,
                scroll_residual_full_repaints_total,
                scroll_zero_saved_batches_total,
            } => {
                w.write_u8(SCROLL_STATS);
                w.write_u32(*scroll_batches_total);
                w.write_u32(*scroll_full_fallbacks_total);
                w.write_u32(*scroll_potential_tiles_total);
                w.write_u32(*scroll_saved_tiles_total);
                w.write_u32(*scroll_non_quantized_fallbacks_total);
                w.write_u32(*scroll_residual_full_repaints_total);
                w.write_u32(*scroll_zero_saved_batches_total);
            }
        }
        w.finish()
    }

    /// Decode a tile command payload.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] if the payload is truncated, has an unknown tile
    /// tag, or contains trailing bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        decode::decode(buf)
    }

    /// Wrap this message in a frame on the tiles channel.
    pub fn to_frame(&self) -> Frame {
        Frame::new(ChannelId::Tiles, self.encode())
    }
}
