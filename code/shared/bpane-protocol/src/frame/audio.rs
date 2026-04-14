use alloc::vec::Vec;

use crate::{channel::ChannelId, types::AudioFrame};

use super::{
    envelope::Frame,
    error::FrameError,
    io::{Reader, Writer},
};

impl AudioFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::with_capacity(16 + self.data.len());
        w.write_u32(self.seq);
        w.write_u64(self.timestamp_us);
        w.write_vec_u32(&self.data);
        w.finish()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(buf);
        let frame = Self {
            seq: r.read_u32()?,
            timestamp_us: r.read_u64()?,
            data: r.read_vec_u32()?,
        };
        r.finish(frame)
    }

    pub fn to_frame_out(&self) -> Frame {
        Frame::new(ChannelId::AudioOut, self.encode())
    }

    pub fn to_frame_in(&self) -> Frame {
        Frame::new(ChannelId::AudioIn, self.encode())
    }
}
