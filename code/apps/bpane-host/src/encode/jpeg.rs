use super::{EncodeBackend, EncodedFrame};
use crate::capture::CapturedFrame;
use image::codecs::jpeg::JpegEncoder;
use image::ImageEncoder;
use std::io::Cursor;

/// JPEG encoder backend.
///
/// Encodes raw RGBA frames to JPEG. This is a pragmatic interim solution
/// while the full H.264 pipeline (VAAPI/libx264) is being built out.
/// Every frame is a keyframe (JPEG has no inter-frame compression).
pub struct JpegEncoderBackend {
    width: u32,
    height: u32,
    quality: u8,
    frame_count: u64,
    /// Reusable RGB buffer to avoid allocation per frame.
    rgb_buf: Vec<u8>,
}

impl JpegEncoderBackend {
    pub fn new(width: u32, height: u32, quality: u8) -> Self {
        Self {
            width,
            height,
            quality,
            frame_count: 0,
            rgb_buf: Vec::new(),
        }
    }
}

impl EncodeBackend for JpegEncoderBackend {
    fn encode_frame(&mut self, frame: &CapturedFrame) -> anyhow::Result<EncodedFrame> {
        self.frame_count += 1;

        let pixel_count = (frame.width * frame.height) as usize;

        // Convert RGBA -> RGB (JPEG doesn't support alpha)
        self.rgb_buf.clear();
        self.rgb_buf.reserve(pixel_count * 3);
        for pixel in frame.data.chunks_exact(4) {
            self.rgb_buf.push(pixel[0]); // R
            self.rgb_buf.push(pixel[1]); // G
            self.rgb_buf.push(pixel[2]); // B
        }

        let mut buf = Cursor::new(Vec::with_capacity(pixel_count / 4));
        let encoder = JpegEncoder::new_with_quality(&mut buf, self.quality);
        encoder.write_image(
            &self.rgb_buf,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgb8,
        )?;

        Ok(EncodedFrame {
            data: buf.into_inner(),
            is_keyframe: true,
            pts_us: frame.timestamp_us,
            width: frame.width,
            height: frame.height,
        })
    }

    fn force_keyframe(&mut self) {
        // Every frame is already a keyframe with JPEG
    }

    fn reconfigure(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_encode_produces_valid_jpeg() {
        let frame = CapturedFrame {
            width: 64,
            height: 64,
            data: vec![0xFF; 64 * 64 * 4], // white RGBA pixels
            timestamp_us: 1000,
        };
        let mut enc = JpegEncoderBackend::new(64, 64, 50);
        let encoded = enc.encode_frame(&frame).unwrap();
        // JPEG files start with FF D8
        assert_eq!(encoded.data[0], 0xFF);
        assert_eq!(encoded.data[1], 0xD8);
        assert!(encoded.is_keyframe);
        assert!(encoded.data.len() > 100);
        assert!(encoded.data.len() < 64 * 64 * 4); // compressed
    }

    #[test]
    fn jpeg_encode_reuses_rgb_buffer() {
        let frame = CapturedFrame {
            width: 32,
            height: 32,
            data: vec![0x80; 32 * 32 * 4],
            timestamp_us: 0,
        };
        let mut enc = JpegEncoderBackend::new(32, 32, 75);
        let e1 = enc.encode_frame(&frame).unwrap();
        let e2 = enc.encode_frame(&frame).unwrap();
        // Both should produce valid JPEG
        assert_eq!(e1.data[0..2], [0xFF, 0xD8]);
        assert_eq!(e2.data[0..2], [0xFF, 0xD8]);
    }
}
