pub mod ffmpeg;
pub mod x11;

/// A captured framebuffer.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// XRGB8888 pixel data (4 bytes per pixel).
    pub data: Vec<u8>,
    pub timestamp_us: u64,
}

/// Trait for screen capture backends.
pub trait CaptureBackend: Send {
    /// Capture a single frame. Returns None if no frame is available (e.g., no damage).
    fn capture_frame(&mut self) -> anyhow::Result<Option<CapturedFrame>>;

    /// Set the capture resolution. Backend should reconfigure as needed.
    fn set_resolution(&mut self, width: u32, height: u32) -> anyhow::Result<()>;

    /// Get the current resolution.
    fn resolution(&self) -> (u32, u32);
}

/// A test/dummy capture backend that produces solid-color frames.
pub struct TestCaptureBackend {
    width: u32,
    height: u32,
    frame_count: u64,
}

impl TestCaptureBackend {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
        }
    }
}

impl CaptureBackend for TestCaptureBackend {
    fn capture_frame(&mut self) -> anyhow::Result<Option<CapturedFrame>> {
        self.frame_count += 1;
        let pixel_count = (self.width * self.height) as usize;
        let mut data = Vec::with_capacity(pixel_count * 4);

        // Generate a test pattern: alternating colors per frame
        let color = match self.frame_count % 3 {
            0 => [0xFF, 0x00, 0x00, 0xFF], // Red
            1 => [0x00, 0xFF, 0x00, 0xFF], // Green
            _ => [0x00, 0x00, 0xFF, 0xFF], // Blue
        };

        for _ in 0..pixel_count {
            data.extend_from_slice(&color);
        }

        Ok(Some(CapturedFrame {
            width: self.width,
            height: self.height,
            data,
            timestamp_us: self.frame_count * 33_333, // ~30fps
        }))
    }

    fn set_resolution(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_backend_produces_frames() {
        let mut backend = TestCaptureBackend::new(100, 100);
        let frame = backend.capture_frame().unwrap().unwrap();
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert_eq!(frame.data.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_capture_backend_resize() {
        let mut backend = TestCaptureBackend::new(100, 100);
        backend.set_resolution(200, 150).unwrap();
        assert_eq!(backend.resolution(), (200, 150));
        let frame = backend.capture_frame().unwrap().unwrap();
        assert_eq!(frame.width, 200);
        assert_eq!(frame.height, 150);
        assert_eq!(frame.data.len(), 200 * 150 * 4);
    }

    #[test]
    fn test_capture_timestamps_are_monotonic() {
        let mut backend = TestCaptureBackend::new(64, 64);
        let mut prev_ts = 0u64;
        for _ in 0..100 {
            let frame = backend.capture_frame().unwrap().unwrap();
            assert!(
                frame.timestamp_us > prev_ts,
                "timestamps must be strictly increasing"
            );
            prev_ts = frame.timestamp_us;
        }
    }

    #[test]
    fn test_capture_1x1_frame() {
        let mut backend = TestCaptureBackend::new(1, 1);
        let frame = backend.capture_frame().unwrap().unwrap();
        assert_eq!(frame.data.len(), 4); // 1 pixel * 4 bytes
    }

    #[test]
    fn test_capture_resize_resets_dimensions() {
        let mut backend = TestCaptureBackend::new(800, 600);
        assert_eq!(backend.resolution(), (800, 600));

        backend.set_resolution(1920, 1080).unwrap();
        assert_eq!(backend.resolution(), (1920, 1080));

        let frame = backend.capture_frame().unwrap().unwrap();
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.data.len(), 1920 * 1080 * 4);

        // Resize down
        backend.set_resolution(320, 240).unwrap();
        let frame = backend.capture_frame().unwrap().unwrap();
        assert_eq!(frame.data.len(), 320 * 240 * 4);
    }

    #[test]
    fn test_capture_frame_data_pattern() {
        let mut backend = TestCaptureBackend::new(2, 2);
        let f1 = backend.capture_frame().unwrap().unwrap();
        let f2 = backend.capture_frame().unwrap().unwrap();
        let f3 = backend.capture_frame().unwrap().unwrap();
        // Different colors per frame
        assert_ne!(f1.data[0..4], f2.data[0..4]);
        assert_ne!(f2.data[0..4], f3.data[0..4]);
    }
}
