use alloc::vec;
use alloc::vec::Vec;

use crate::types::{VideoDatagram, VideoTileInfo};

use super::{
    error::FrameError,
    io::{Reader, Writer},
};

const VIDEO_FLAG_TILE: u8 = 0x01;

impl VideoDatagram {
    pub fn encode(&self) -> Vec<u8> {
        let tile_size = if self.tile_info.is_some() { 13 } else { 1 };
        let mut w = Writer::with_capacity(21 + self.data.len() + tile_size);
        w.write_u32(self.nal_id);
        w.write_u16(self.fragment_seq);
        w.write_u16(self.fragment_total);
        w.write_bool(self.is_keyframe);
        w.write_u64(self.pts_us);
        w.write_vec_u32(&self.data);
        w.write_u8(if self.tile_info.is_some() {
            VIDEO_FLAG_TILE
        } else {
            0
        });
        if let Some(tile) = self.tile_info {
            w.write_u16(tile.tile_x);
            w.write_u16(tile.tile_y);
            w.write_u16(tile.tile_w);
            w.write_u16(tile.tile_h);
            w.write_u16(tile.screen_w);
            w.write_u16(tile.screen_h);
        }
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let nal_id = r.read_u32()?;
        let fragment_seq = r.read_u16()?;
        let fragment_total = r.read_u16()?;
        let is_keyframe = r.read_bool()?;
        let pts_us = r.read_u64()?;
        let data = r.read_vec_u32()?;
        let tile_info = if r.remaining() > 0 {
            let flags = r.read_u8()?;
            if flags & VIDEO_FLAG_TILE != 0 && r.remaining() >= 12 {
                Some(VideoTileInfo {
                    tile_x: r.read_u16()?,
                    tile_y: r.read_u16()?,
                    tile_w: r.read_u16()?,
                    tile_h: r.read_u16()?,
                    screen_w: r.read_u16()?,
                    screen_h: r.read_u16()?,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            nal_id,
            fragment_seq,
            fragment_total,
            is_keyframe,
            pts_us,
            data,
            tile_info,
        })
    }

    pub fn fragment(
        nal_id: u32,
        is_keyframe: bool,
        pts_us: u64,
        nal_data: &[u8],
        max_fragment_size: usize,
    ) -> Vec<Self> {
        Self::fragment_with_tile(
            nal_id,
            is_keyframe,
            pts_us,
            nal_data,
            max_fragment_size,
            None,
        )
    }

    pub fn fragment_with_tile(
        nal_id: u32,
        is_keyframe: bool,
        pts_us: u64,
        nal_data: &[u8],
        max_fragment_size: usize,
        tile_info: Option<VideoTileInfo>,
    ) -> Vec<Self> {
        assert!(max_fragment_size > 0, "max_fragment_size must be > 0");
        if nal_data.is_empty() {
            return vec![Self {
                nal_id,
                fragment_seq: 0,
                fragment_total: 1,
                is_keyframe,
                pts_us,
                data: Vec::new(),
                tile_info,
            }];
        }

        let chunks: Vec<&[u8]> = nal_data.chunks(max_fragment_size).collect();
        assert!(
            chunks.len() <= u16::MAX as usize,
            "too many NAL fragments: {} (max {})",
            chunks.len(),
            u16::MAX
        );

        let total = chunks.len() as u16;
        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| Self {
                nal_id,
                fragment_seq: i as u16,
                fragment_total: total,
                is_keyframe,
                pts_us,
                data: chunk.to_vec(),
                tile_info,
            })
            .collect()
    }

    pub fn reassemble(fragments: &[Self]) -> Option<Vec<u8>> {
        if fragments.is_empty() {
            return None;
        }

        let total = fragments[0].fragment_total as usize;
        if fragments.len() != total {
            return None;
        }
        for (i, fragment) in fragments.iter().enumerate() {
            if fragment.fragment_seq != i as u16 {
                return None;
            }
        }

        let total_len = fragments.iter().map(|f| f.data.len()).sum();
        let mut data = Vec::with_capacity(total_len);
        for fragment in fragments {
            data.extend_from_slice(&fragment.data);
        }
        Some(data)
    }
}
