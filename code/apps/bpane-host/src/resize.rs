use std::time::{Duration, Instant};
use tracing::info;

use crate::capture::CaptureBackend;
use crate::encode::EncodeBackend;

/// Handles resolution change requests from the client.
/// Coordinates capture backend reconfiguration, encoder restart,
/// and tracks resize timing.
pub struct ResizeHandler {
    current_width: u32,
    current_height: u32,
    last_resize: Option<Instant>,
    /// Maximum time allowed for a resize operation.
    pub max_resize_duration: Duration,
}

impl ResizeHandler {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            current_width: width,
            current_height: height,
            last_resize: None,
            max_resize_duration: Duration::from_millis(200),
        }
    }

    pub fn current_resolution(&self) -> (u32, u32) {
        (self.current_width, self.current_height)
    }

    /// Apply a resolution change. Returns true if the resolution actually changed.
    pub fn apply(
        &mut self,
        width: u32,
        height: u32,
        capture: &mut dyn CaptureBackend,
        encoder: &mut dyn EncodeBackend,
    ) -> anyhow::Result<bool> {
        if width == self.current_width && height == self.current_height {
            return Ok(false);
        }

        if width == 0 || height == 0 || width > 7680 || height > 4320 {
            anyhow::bail!("invalid resolution: {width}x{height}");
        }

        let start = Instant::now();
        info!(
            "resize: {}x{} -> {}x{}",
            self.current_width, self.current_height, width, height
        );

        capture.set_resolution(width, height)?;
        encoder.reconfigure(width, height)?;
        encoder.force_keyframe();

        self.current_width = width;
        self.current_height = height;
        self.last_resize = Some(start);

        let elapsed = start.elapsed();
        if elapsed > self.max_resize_duration {
            tracing::warn!(
                "resize took {:.0}ms (target: {:.0}ms)",
                elapsed.as_millis(),
                self.max_resize_duration.as_millis()
            );
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::TestCaptureBackend;
    use crate::encode::TestEncoder;

    #[test]
    fn resize_handler_applies_new_resolution() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let changed = handler
            .apply(1920, 1080, &mut capture, &mut encoder)
            .unwrap();
        assert!(changed);
        assert_eq!(handler.current_resolution(), (1920, 1080));
        assert_eq!(capture.resolution(), (1920, 1080));
    }

    #[test]
    fn resize_handler_no_change_same_resolution() {
        let mut handler = ResizeHandler::new(800, 600);
        let mut capture = TestCaptureBackend::new(800, 600);
        let mut encoder = TestEncoder::new(800, 600);

        let changed = handler.apply(800, 600, &mut capture, &mut encoder).unwrap();
        assert!(!changed);
    }

    #[test]
    fn resize_handler_accepts_max_resolution() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        // 7680x4320 is the max allowed (8K UHD)
        let changed = handler
            .apply(7680, 4320, &mut capture, &mut encoder)
            .unwrap();
        assert!(changed);
        assert_eq!(handler.current_resolution(), (7680, 4320));
    }

    #[test]
    fn resize_handler_rejects_just_over_max() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        assert!(handler
            .apply(7681, 4320, &mut capture, &mut encoder)
            .is_err());
        assert!(handler
            .apply(7680, 4321, &mut capture, &mut encoder)
            .is_err());
        // Original resolution should be unchanged
        assert_eq!(handler.current_resolution(), (640, 480));
    }

    #[test]
    fn resize_handler_multiple_resizes() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let resolutions = [
            (800, 600),
            (1024, 768),
            (1920, 1080),
            (1280, 720),
            (640, 480),
        ];
        for (w, h) in &resolutions {
            let changed = handler.apply(*w, *h, &mut capture, &mut encoder).unwrap();
            assert!(changed);
            assert_eq!(handler.current_resolution(), (*w, *h));
            assert_eq!(capture.resolution(), (*w, *h));
        }
    }

    #[test]
    fn resize_handler_rejects_invalid_resolution() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        assert!(handler.apply(0, 480, &mut capture, &mut encoder).is_err());
        assert!(handler.apply(640, 0, &mut capture, &mut encoder).is_err());
        assert!(handler
            .apply(8000, 5000, &mut capture, &mut encoder)
            .is_err());
    }

    #[test]
    fn resize_handler_forces_keyframe() {
        let mut handler = ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        // Consume initial keyframe
        let frame = capture.capture_frame().unwrap().unwrap();
        let _ = encoder.encode_frame(&frame).unwrap();

        // Should be P frame now
        let frame = capture.capture_frame().unwrap().unwrap();
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(!encoded.is_keyframe);

        // After resize, next frame should be keyframe
        handler
            .apply(1280, 720, &mut capture, &mut encoder)
            .unwrap();
        let frame = capture.capture_frame().unwrap().unwrap();
        let encoded = encoder.encode_frame(&frame).unwrap();
        assert!(encoded.is_keyframe);
    }
}
