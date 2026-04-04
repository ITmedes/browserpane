pub mod jpeg;
pub mod software;

use crate::capture::CapturedFrame;

/// An encoded video frame (H.264 NAL unit).
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    pub pts_us: u64,
    pub width: u32,
    pub height: u32,
}

/// Trait for video encoder backends.
pub trait EncodeBackend: Send {
    /// Encode a raw frame into H.264.
    fn encode_frame(&mut self, frame: &CapturedFrame) -> anyhow::Result<EncodedFrame>;

    /// Request an immediate keyframe on the next encode.
    fn force_keyframe(&mut self);

    /// Reconfigure the encoder for a new resolution.
    fn reconfigure(&mut self, width: u32, height: u32) -> anyhow::Result<()>;
}

/// A test encoder that produces fake H.264 NAL units.
pub struct TestEncoder {
    width: u32,
    height: u32,
    frame_count: u64,
    force_idr: bool,
    keyframe_interval: u64,
}

impl TestEncoder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
            force_idr: true,       // First frame is always IDR
            keyframe_interval: 60, // Every 2 seconds at 30fps
        }
    }
}

impl EncodeBackend for TestEncoder {
    fn encode_frame(&mut self, frame: &CapturedFrame) -> anyhow::Result<EncodedFrame> {
        self.frame_count += 1;
        let is_keyframe = self.force_idr || (self.frame_count % self.keyframe_interval == 0);
        self.force_idr = false;

        // Produce a fake H.264 NAL unit
        // Real implementation would use FFmpeg/VAAPI
        let mut nal = Vec::new();
        // Start code
        nal.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        if is_keyframe {
            // IDR NAL unit type (5)
            nal.push(0x65);
        } else {
            // Non-IDR NAL unit type (1)
            nal.push(0x41);
        }
        // Add some data proportional to frame size
        let data_size = (frame.width * frame.height / 100) as usize;
        nal.extend(std::iter::repeat(0xAA).take(data_size.max(10)));

        Ok(EncodedFrame {
            data: nal,
            is_keyframe,
            pts_us: frame.timestamp_us,
            width: frame.width,
            height: frame.height,
        })
    }

    fn force_keyframe(&mut self) {
        self.force_idr = true;
    }

    fn reconfigure(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.force_idr = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;

    fn make_test_frame(width: u32, height: u32) -> CapturedFrame {
        CapturedFrame {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
            timestamp_us: 0,
        }
    }

    #[test]
    fn test_encoder_first_frame_is_keyframe() {
        let mut encoder = TestEncoder::new(640, 480);
        let frame = make_test_frame(640, 480);
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(encoded.is_keyframe);
        // NAL starts with start code
        assert_eq!(&encoded.data[0..4], &[0x00, 0x00, 0x00, 0x01]);
        // IDR NAL type
        assert_eq!(encoded.data[4], 0x65);
    }

    #[test]
    fn test_encoder_subsequent_frames_are_p_frames() {
        let mut encoder = TestEncoder::new(640, 480);
        let frame = make_test_frame(640, 480);
        encoder.encode_frame(&frame).unwrap(); // keyframe
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(!encoded.is_keyframe);
        assert_eq!(encoded.data[4], 0x41); // non-IDR
    }

    #[test]
    fn test_encoder_force_keyframe() {
        let mut encoder = TestEncoder::new(640, 480);
        let frame = make_test_frame(640, 480);
        encoder.encode_frame(&frame).unwrap(); // first keyframe
        encoder.encode_frame(&frame).unwrap(); // P frame
        encoder.force_keyframe();
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(encoded.is_keyframe);
    }

    #[test]
    fn test_encoder_keyframe_interval() {
        let mut encoder = TestEncoder::new(100, 100);
        let frame = make_test_frame(100, 100);

        // First frame is keyframe
        assert!(encoder.encode_frame(&frame).unwrap().is_keyframe);

        // Frames 2..59 are P frames
        for i in 2..60 {
            let encoded = encoder.encode_frame(&frame).unwrap();
            assert!(!encoded.is_keyframe, "frame {i} should be P frame");
        }

        // Frame 60 should be keyframe (interval=60)
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(encoded.is_keyframe, "frame 60 should be keyframe");
    }

    #[test]
    fn test_encoder_output_size_proportional_to_resolution() {
        let mut small_encoder = TestEncoder::new(100, 100);
        let mut large_encoder = TestEncoder::new(1920, 1080);

        let small_frame = make_test_frame(100, 100);
        let large_frame = make_test_frame(1920, 1080);

        let small_encoded = small_encoder.encode_frame(&small_frame).unwrap();
        let large_encoded = large_encoder.encode_frame(&large_frame).unwrap();

        assert!(
            large_encoded.data.len() > small_encoded.data.len(),
            "larger resolution should produce larger output"
        );
    }

    #[test]
    fn test_encoder_preserves_timestamp() {
        let mut encoder = TestEncoder::new(100, 100);
        let frame = CapturedFrame {
            width: 100,
            height: 100,
            data: vec![0; 100 * 100 * 4],
            timestamp_us: 12345678,
        };
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert_eq!(encoded.pts_us, 12345678);
    }

    #[test]
    fn test_encoder_multiple_reconfigures() {
        let mut encoder = TestEncoder::new(640, 480);

        let resolutions = [(800, 600), (1024, 768), (320, 240)];
        for (w, h) in &resolutions {
            encoder.reconfigure(*w, *h).unwrap();
            let frame = make_test_frame(*w, *h);
            let encoded = encoder.encode_frame(&frame).unwrap();
            // Each reconfigure should force a keyframe
            assert!(encoded.is_keyframe);
            assert_eq!(encoded.width, *w);
            assert_eq!(encoded.height, *h);
        }
    }

    #[test]
    fn test_encoder_reconfigure() {
        let mut encoder = TestEncoder::new(640, 480);
        let frame = make_test_frame(640, 480);
        encoder.encode_frame(&frame).unwrap();
        encoder.reconfigure(1920, 1080).unwrap();
        let frame2 = make_test_frame(1920, 1080);
        let encoded = encoder.encode_frame(&frame2).unwrap();
        assert!(encoded.is_keyframe); // Reconfigure forces keyframe
        assert_eq!(encoded.width, 1920);
        assert_eq!(encoded.height, 1080);
    }
}
