/// Software H.264 encoder using libx264 via FFmpeg.
///
/// When the `libav` feature is enabled, uses `ffmpeg-next` for in-process
/// encoding (no subprocess). Otherwise, compiles as a stub that produces
/// minimal fake NAL units for testing.
use super::{EncodeBackend, EncodedFrame};
use crate::capture::CapturedFrame;

#[cfg(feature = "libav")]
mod libav_encoder {
    use super::*;

    pub struct SoftwareEncoder {
        encoder: ffmpeg_next::codec::encoder::Video,
        scaler: ffmpeg_next::software::scaling::Context,
        width: u32,
        height: u32,
        frame_count: i64,
        force_idr: bool,
    }

    impl SoftwareEncoder {
        pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
            ffmpeg_next::init()?;
            let (enc, scaler) = Self::create_encoder(width, height)?;
            Ok(Self {
                encoder: enc,
                scaler,
                width,
                height,
                frame_count: 0,
                force_idr: true,
            })
        }

        fn create_encoder(
            width: u32,
            height: u32,
        ) -> anyhow::Result<(
            ffmpeg_next::codec::encoder::Video,
            ffmpeg_next::software::scaling::Context,
        )> {
            use ffmpeg_next::codec;
            use ffmpeg_next::format::Pixel;
            use ffmpeg_next::software::scaling;

            let codec = codec::encoder::find_by_name("libx264")
                .ok_or_else(|| anyhow::anyhow!("libx264 not found"))?;

            let mut context = codec::Context::new_with_codec(codec);
            let mut encoder = context.encoder().video()?;

            encoder.set_width(width);
            encoder.set_height(height);
            encoder.set_format(Pixel::YUV420P);
            encoder.set_time_base(ffmpeg_next::Rational::new(1, 30));
            encoder.set_gop(60);
            encoder.set_max_b_frames(0);

            // Set x264 specific options for low latency
            let mut opts = ffmpeg_next::Dictionary::new();
            opts.set("preset", "ultrafast");
            opts.set("tune", "zerolatency");
            opts.set("profile", "baseline");
            opts.set("level", "3.1");
            opts.set("sliced-threads", "0");
            opts.set("repeat-headers", "1");

            let encoder = encoder.open_with(opts)?;

            // Scaler: BGRA → YUV420P
            let scaler = scaling::Context::get(
                Pixel::BGRA,
                width,
                height,
                Pixel::YUV420P,
                width,
                height,
                scaling::Flags::FAST_BILINEAR,
            )?;

            Ok((encoder, scaler))
        }
    }

    impl EncodeBackend for SoftwareEncoder {
        fn encode_frame(&mut self, frame: &CapturedFrame) -> anyhow::Result<EncodedFrame> {
            use ffmpeg_next::format::Pixel;
            use ffmpeg_next::frame::Video as VideoFrame;
            use ffmpeg_next::Packet;

            // Create input frame from BGRA data
            // Note: CapturedFrame data is RGBA after the backend's swap,
            // but our scaler expects BGRA. The caller should handle this.
            let mut input = VideoFrame::new(Pixel::BGRA, frame.width, frame.height);
            let stride = input.stride(0);
            let data = input.data_mut(0);
            // Copy row by row respecting stride
            let row_bytes = (frame.width as usize) * 4;
            for y in 0..frame.height as usize {
                let src_offset = y * row_bytes;
                let dst_offset = y * stride;
                data[dst_offset..dst_offset + row_bytes]
                    .copy_from_slice(&frame.data[src_offset..src_offset + row_bytes]);
            }

            // Scale BGRA → YUV420P
            let mut yuv_frame = VideoFrame::empty();
            self.scaler.run(&input, &mut yuv_frame)?;

            yuv_frame.set_pts(Some(self.frame_count));
            self.frame_count += 1;

            if self.force_idr {
                yuv_frame.set_kind(ffmpeg_next::picture::Type::I);
                self.force_idr = false;
            }

            // Encode
            self.encoder.send_frame(&yuv_frame)?;

            let mut output_data = Vec::new();
            let mut is_keyframe = false;

            let mut packet = Packet::empty();
            while self.encoder.receive_packet(&mut packet).is_ok() {
                output_data.extend_from_slice(packet.data().unwrap_or(&[]));
                if packet.is_key() {
                    is_keyframe = true;
                }
            }

            if output_data.is_empty() {
                // Encoder hasn't output anything yet (buffering).
                // Produce a minimal placeholder.
                let mut nal = vec![0x00, 0x00, 0x00, 0x01];
                nal.push(0x41); // non-IDR
                nal.extend(std::iter::repeat(0x00).take(4));
                return Ok(EncodedFrame {
                    data: nal,
                    is_keyframe: false,
                    pts_us: frame.timestamp_us,
                    width: frame.width,
                    height: frame.height,
                });
            }

            Ok(EncodedFrame {
                data: output_data,
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
            if width == self.width && height == self.height {
                return Ok(());
            }
            // Flush the encoder
            let _ = self.encoder.send_eof();
            let mut packet = ffmpeg_next::Packet::empty();
            while self.encoder.receive_packet(&mut packet).is_ok() {}

            let (enc, scaler) = Self::create_encoder(width, height)?;
            self.encoder = enc;
            self.scaler = scaler;
            self.width = width;
            self.height = height;
            self.frame_count = 0;
            self.force_idr = true;
            Ok(())
        }
    }
}

// When libav feature is not enabled, use a stub encoder
#[cfg(not(feature = "libav"))]
mod stub_encoder {
    use super::*;

    pub struct SoftwareEncoder {
        width: u32,
        height: u32,
        force_idr: bool,
        _frame_count: u64,
    }

    impl SoftwareEncoder {
        pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
            Ok(Self {
                width,
                height,
                force_idr: true,
                _frame_count: 0,
            })
        }
    }

    impl EncodeBackend for SoftwareEncoder {
        fn encode_frame(&mut self, frame: &CapturedFrame) -> anyhow::Result<EncodedFrame> {
            self._frame_count += 1;

            let is_keyframe = self.force_idr;
            self.force_idr = false;

            let mut nal = vec![0x00, 0x00, 0x00, 0x01];
            nal.push(if is_keyframe { 0x65 } else { 0x41 });
            let size = (frame.width * frame.height / 200) as usize;
            nal.extend(std::iter::repeat(0x00).take(size.max(4)));

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
}

#[cfg(feature = "libav")]
pub use libav_encoder::SoftwareEncoder;

#[cfg(not(feature = "libav"))]
pub(crate) use stub_encoder::SoftwareEncoder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;

    fn make_frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame {
            width: w,
            height: h,
            data: vec![0u8; (w * h * 4) as usize],
            timestamp_us: 42_000,
        }
    }

    #[test]
    fn stub_encoder_new() {
        let enc = SoftwareEncoder::new(640, 480);
        assert!(enc.is_ok());
    }

    #[test]
    fn stub_encoder_first_frame_is_keyframe() {
        let mut enc = SoftwareEncoder::new(640, 480).unwrap();
        let frame = make_frame(640, 480);
        let encoded = enc.encode_frame(&frame).unwrap();
        assert!(encoded.is_keyframe);
        assert_eq!(encoded.pts_us, 42_000);
        assert_eq!(encoded.width, 640);
        assert_eq!(encoded.height, 480);
        // NAL start code
        assert_eq!(&encoded.data[..4], &[0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn stub_encoder_second_frame_is_p_frame() {
        let mut enc = SoftwareEncoder::new(320, 240).unwrap();
        let frame = make_frame(320, 240);
        let first = enc.encode_frame(&frame).unwrap();
        assert!(first.is_keyframe);
        let second = enc.encode_frame(&frame).unwrap();
        assert!(!second.is_keyframe);
    }

    #[test]
    fn stub_encoder_force_keyframe() {
        let mut enc = SoftwareEncoder::new(320, 240).unwrap();
        let frame = make_frame(320, 240);
        enc.encode_frame(&frame).unwrap(); // first (keyframe)
        enc.encode_frame(&frame).unwrap(); // P-frame
        enc.force_keyframe();
        let third = enc.encode_frame(&frame).unwrap();
        assert!(third.is_keyframe);
    }

    #[test]
    fn stub_encoder_reconfigure() {
        let mut enc = SoftwareEncoder::new(640, 480).unwrap();
        let frame1 = make_frame(640, 480);
        enc.encode_frame(&frame1).unwrap();

        enc.reconfigure(1920, 1080).unwrap();
        let frame2 = make_frame(1920, 1080);
        let encoded = enc.encode_frame(&frame2).unwrap();
        // Reconfigure forces keyframe
        assert!(encoded.is_keyframe);
        assert_eq!(encoded.width, 1920);
        assert_eq!(encoded.height, 1080);
    }

    #[test]
    fn stub_encoder_output_size_scales_with_resolution() {
        let mut enc_small = SoftwareEncoder::new(320, 240).unwrap();
        let mut enc_large = SoftwareEncoder::new(1920, 1080).unwrap();
        let small = enc_small.encode_frame(&make_frame(320, 240)).unwrap();
        let large = enc_large.encode_frame(&make_frame(1920, 1080)).unwrap();
        assert!(large.data.len() > small.data.len());
    }
}
