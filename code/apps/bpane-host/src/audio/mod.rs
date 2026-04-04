/// Audio subsystem for the host agent.
///
/// Handles desktop audio capture (PipeWire/PulseAudio) via FFmpeg
/// and streams compressed audio frames to the gateway.
pub mod input;

use bpane_protocol::frame::Frame;

/// Audio capture state - graceful degradation if PipeWire/PulseAudio is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioState {
    Available,
    Unavailable(String),
}

/// PCM parameters for audio capture.
const SAMPLE_RATE: u32 = 48000;
const CHANNELS: u32 = 2;
const FRAME_DURATION_MS: u32 = 20;
/// 960 samples * 2 channels * 2 bytes (s16le) = 3840 bytes per 20ms frame.
const BYTES_PER_FRAME: usize = (SAMPLE_RATE / 1000 * FRAME_DURATION_MS * CHANNELS * 2) as usize;
const SILENCE_DBFS_FLOOR: f32 = -120.0;
const SILENCE_GATE_DEFAULT_THRESHOLD_DBFS: f32 = -50.0;
const SILENCE_GATE_DEFAULT_HANGOVER_MS: u32 = 220;
const AUDIO_PAYLOAD_MAGIC: [u8; 4] = *b"WRA1";
const AUDIO_CODEC_PCM_S16LE: u8 = 0x00;
const AUDIO_CODEC_ADPCM_IMA_STEREO: u8 = 0x01;
const AUDIO_CODEC_OPUS: u8 = 0x02;
/// Opus CBR target bitrate in bits per second.
const OPUS_BITRATE_BPS: i32 = 64_000;
/// Samples per channel in a 20ms frame at 48kHz.
const SAMPLES_PER_CHANNEL: usize = (SAMPLE_RATE / 1000 * FRAME_DURATION_MS) as usize; // 960
const IMA_INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioCodec {
    PcmS16le,
    AdpcmImaStereo,
    Opus,
}

impl AudioCodec {
    fn from_env() -> Self {
        match std::env::var("BPANE_AUDIO_CODEC") {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "pcm" | "raw" | "s16le" => Self::PcmS16le,
                "adpcm" | "ima" | "ima-adpcm" | "ima_adpcm" => Self::AdpcmImaStereo,
                "opus" => Self::Opus,
                _ => Self::AdpcmImaStereo,
            },
            Err(_) => Self::AdpcmImaStereo,
        }
    }

    fn id(self) -> u8 {
        match self {
            Self::PcmS16le => AUDIO_CODEC_PCM_S16LE,
            Self::AdpcmImaStereo => AUDIO_CODEC_ADPCM_IMA_STEREO,
            Self::Opus => AUDIO_CODEC_OPUS,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SilenceGateConfig {
    enabled: bool,
    threshold_dbfs: f32,
    hangover_frames: u32,
}

impl SilenceGateConfig {
    fn from_env() -> Self {
        let enabled = parse_env_bool("BPANE_AUDIO_SILENCE_GATE", true);
        let threshold_dbfs = parse_env_f32(
            "BPANE_AUDIO_SILENCE_DBFS",
            SILENCE_GATE_DEFAULT_THRESHOLD_DBFS,
            -96.0,
            -3.0,
        );
        let hangover_ms = parse_env_u32(
            "BPANE_AUDIO_SILENCE_HANGOVER_MS",
            SILENCE_GATE_DEFAULT_HANGOVER_MS,
            2000,
        );
        let hangover_frames = hangover_ms.div_ceil(FRAME_DURATION_MS);
        Self {
            enabled,
            threshold_dbfs,
            hangover_frames,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct SilenceGateState {
    hold_frames: u32,
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn parse_env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    match std::env::var(name) {
        Ok(raw) => raw
            .trim()
            .parse::<f32>()
            .map(|v| v.clamp(min, max))
            .unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_env_u32(name: &str, default: u32, max: u32) -> u32 {
    match std::env::var(name) {
        Ok(raw) => raw
            .trim()
            .parse::<u32>()
            .map(|v| v.min(max))
            .unwrap_or(default),
        Err(_) => default,
    }
}

fn pcm_rms_dbfs_s16le(pcm: &[u8]) -> f32 {
    let sample_count = pcm.len() / 2;
    if sample_count == 0 {
        return SILENCE_DBFS_FLOOR;
    }

    let mut sum_sq = 0.0f64;
    for bytes in pcm.chunks_exact(2) {
        let sample = i16::from_le_bytes([bytes[0], bytes[1]]) as f64;
        sum_sq += sample * sample;
    }

    if sum_sq <= f64::EPSILON {
        return SILENCE_DBFS_FLOOR;
    }

    let rms = (sum_sq / sample_count as f64).sqrt();
    let norm = (rms / 32768.0).max(1.0e-9);
    (20.0 * norm.log10()) as f32
}

fn should_forward_audio_frame(
    level_dbfs: f32,
    gate: SilenceGateConfig,
    state: &mut SilenceGateState,
) -> bool {
    if !gate.enabled {
        return true;
    }
    if level_dbfs >= gate.threshold_dbfs {
        state.hold_frames = gate.hangover_frames;
        return true;
    }
    if state.hold_frames > 0 {
        state.hold_frames -= 1;
        return true;
    }
    false
}

fn encode_audio_payload(
    pcm: &[u8],
    codec: AudioCodec,
    opus_encoder: Option<&mut audiopus::coder::Encoder>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + pcm.len());
    out.extend_from_slice(&AUDIO_PAYLOAD_MAGIC);
    match codec {
        AudioCodec::PcmS16le => {
            out.push(codec.id());
            out.extend_from_slice(pcm);
        }
        AudioCodec::AdpcmImaStereo => {
            let compressed = encode_ima_adpcm_stereo(pcm);
            if compressed.is_empty() {
                out.push(AudioCodec::PcmS16le.id());
                out.extend_from_slice(pcm);
            } else {
                out.push(codec.id());
                out.extend_from_slice(&compressed);
            }
        }
        AudioCodec::Opus => {
            if let Some(encoder) = opus_encoder {
                // Convert &[u8] PCM S16LE to &[i16] safely.
                // Each sample is 2 bytes little-endian; we need an aligned &[i16].
                let sample_count = pcm.len() / 2;
                let mut samples = Vec::<i16>::with_capacity(sample_count);
                for chunk in pcm.chunks_exact(2) {
                    samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
                }
                // Maximum Opus frame size: 4000 bytes is generous for 20ms at 64kbps
                let mut opus_buf = [0u8; 4000];
                match encoder.encode(&samples, &mut opus_buf) {
                    Ok(encoded_len) => {
                        out.push(codec.id());
                        out.extend_from_slice(&opus_buf[..encoded_len]);
                    }
                    Err(e) => {
                        tracing::warn!("audio: opus encode failed: {e}, falling back to PCM");
                        out.push(AudioCodec::PcmS16le.id());
                        out.extend_from_slice(pcm);
                    }
                }
            } else {
                // No encoder available — fall back to PCM
                tracing::warn!("audio: opus encoder not available, falling back to PCM");
                out.push(AudioCodec::PcmS16le.id());
                out.extend_from_slice(pcm);
            }
        }
    }
    out
}

fn encode_ima_adpcm_stereo(pcm: &[u8]) -> Vec<u8> {
    if pcm.len() < 4 || !pcm.len().is_multiple_of(4) {
        return Vec::new();
    }

    let stereo_samples = pcm.len() / 4;
    let left0 = i16::from_le_bytes([pcm[0], pcm[1]]);
    let right0 = i16::from_le_bytes([pcm[2], pcm[3]]);
    let mut left_pred = left0 as i32;
    let mut right_pred = right0 as i32;
    let mut left_index = 0i32;
    let mut right_index = 0i32;

    let mut out = Vec::with_capacity(6 + stereo_samples.saturating_sub(1));
    out.extend_from_slice(&left0.to_le_bytes());
    out.push(left_index as u8);
    out.extend_from_slice(&right0.to_le_bytes());
    out.push(right_index as u8);

    for i in 1..stereo_samples {
        let off = i * 4;
        let left = i16::from_le_bytes([pcm[off], pcm[off + 1]]) as i32;
        let right = i16::from_le_bytes([pcm[off + 2], pcm[off + 3]]) as i32;
        let left_nibble = ima_encode_nibble(left, &mut left_pred, &mut left_index);
        let right_nibble = ima_encode_nibble(right, &mut right_pred, &mut right_index);
        out.push((right_nibble << 4) | left_nibble);
    }

    out
}

fn ima_encode_nibble(sample: i32, predictor: &mut i32, index: &mut i32) -> u8 {
    let step = IMA_STEP_TABLE[*index as usize];
    let mut diff = sample - *predictor;
    let mut nibble: u8 = 0;

    if diff < 0 {
        nibble |= 0x08;
        diff = -diff;
    }

    let mut vpdiff = step >> 3;
    if diff >= step {
        nibble |= 0x04;
        diff -= step;
        vpdiff += step;
    }
    if diff >= (step >> 1) {
        nibble |= 0x02;
        diff -= step >> 1;
        vpdiff += step >> 1;
    }
    if diff >= (step >> 2) {
        nibble |= 0x01;
        vpdiff += step >> 2;
    }

    if (nibble & 0x08) != 0 {
        *predictor -= vpdiff;
    } else {
        *predictor += vpdiff;
    }
    *predictor = (*predictor).clamp(-32768, 32767);

    *index += IMA_INDEX_TABLE[nibble as usize];
    *index = (*index).clamp(0, 88);

    nibble
}

pub fn detect_audio() -> AudioState {
    #[cfg(target_os = "linux")]
    {
        // Check if PulseAudio (or PipeWire-Pulse) is running via pactl
        match std::process::Command::new("pactl").arg("info").output() {
            Ok(output) if output.status.success() => AudioState::Available,
            _ => AudioState::Unavailable(
                "PulseAudio/PipeWire not detected (pactl info failed)".to_string(),
            ),
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        AudioState::Unavailable("Audio capture only available on Linux".to_string())
    }
}

/// Spawn an audio capture task that reads PCM from FFmpeg and sends AudioFrames.
///
/// Returns a JoinHandle that runs until the gateway sender is dropped or FFmpeg exits.
#[cfg(target_os = "linux")]
pub fn spawn_audio_capture(
    to_gateway: tokio::sync::mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    use bpane_protocol::AudioFrame;
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        use std::process::{Command, Stdio};

        let audio_codec = AudioCodec::from_env();
        let silence_gate = SilenceGateConfig::from_env();
        tracing::debug!(codec = ?audio_codec, "audio: transport codec");
        if silence_gate.enabled {
            tracing::debug!(
                threshold_dbfs = silence_gate.threshold_dbfs,
                hangover_ms = silence_gate.hangover_frames * FRAME_DURATION_MS,
                "audio: silence gate enabled"
            );
        } else {
            tracing::debug!("audio: silence gate disabled");
        }

        // FFmpeg captures from the dedicated bpane-desktop sink monitor.
        // Using a named sink (not "default.monitor") prevents the mic
        // virtual source from interfering with desktop audio capture.
        let mut child = match Command::new("ffmpeg")
            .args([
                "-f",
                "pulse",
                "-i",
                "bpane-desktop.monitor",
                "-ar",
                &SAMPLE_RATE.to_string(),
                "-ac",
                &CHANNELS.to_string(),
                "-f",
                "s16le",
                "-acodec",
                "pcm_s16le",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                tracing::info!(
                    "audio: ffmpeg started (s16le {}Hz {}ch)",
                    SAMPLE_RATE,
                    CHANNELS
                );
                child
            }
            Err(e) => {
                tracing::warn!("audio: failed to start ffmpeg: {e}");
                return;
            }
        };

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                tracing::warn!("audio: no stdout from ffmpeg");
                return;
            }
        };

        // Create Opus encoder if selected (it is stateful, persists across frames).
        let mut opus_encoder = if audio_codec == AudioCodec::Opus {
            match audiopus::coder::Encoder::new(
                audiopus::SampleRate::Hz48000,
                audiopus::Channels::Stereo,
                audiopus::Application::LowDelay,
            ) {
                Ok(mut enc) => {
                    if let Err(e) =
                        enc.set_bitrate(audiopus::Bitrate::BitsPerSecond(OPUS_BITRATE_BPS))
                    {
                        tracing::warn!("audio: failed to set opus bitrate: {e}");
                    }
                    tracing::info!(
                        bitrate_bps = OPUS_BITRATE_BPS,
                        "audio: opus encoder created (48kHz stereo, LowDelay)"
                    );
                    Some(enc)
                }
                Err(e) => {
                    tracing::warn!(
                        "audio: failed to create opus encoder: {e}, falling back to ADPCM"
                    );
                    None
                }
            }
        } else {
            None
        };

        // If Opus was requested but encoder creation failed, fall back to ADPCM.
        let effective_codec = if audio_codec == AudioCodec::Opus && opus_encoder.is_none() {
            AudioCodec::AdpcmImaStereo
        } else {
            audio_codec
        };

        let mut seq: u32 = 0;
        let mut buf = vec![0u8; BYTES_PER_FRAME];
        let frame_duration_us = (FRAME_DURATION_MS as u64) * 1000;
        let mut silence_state = SilenceGateState::default();
        let mut dropped_silent_frames: u64 = 0;

        loop {
            // Read exactly one 20ms PCM frame
            match stdout.read_exact(&mut buf) {
                Ok(()) => {}
                Err(e) => {
                    tracing::debug!("audio: ffmpeg stdout ended: {e}");
                    break;
                }
            }

            let level_dbfs = pcm_rms_dbfs_s16le(&buf);
            if !should_forward_audio_frame(level_dbfs, silence_gate, &mut silence_state) {
                dropped_silent_frames = dropped_silent_frames.saturating_add(1);
                continue;
            }

            seq = seq.wrapping_add(1);
            let timestamp_us = seq as u64 * frame_duration_us;

            let audio_frame = AudioFrame {
                seq,
                timestamp_us,
                data: encode_audio_payload(&buf, effective_codec, opus_encoder.as_mut()),
            };

            if to_gateway
                .blocking_send(audio_frame.to_frame_out())
                .is_err()
            {
                tracing::debug!("audio: gateway channel closed");
                break;
            }
        }

        if dropped_silent_frames > 0 {
            tracing::debug!(dropped_silent_frames, "audio: silent frames suppressed");
        }

        // Clean up FFmpeg process
        let _ = child.kill();
        let _ = child.wait();
        tracing::info!("audio: capture stopped");
    })
}

#[cfg(not(target_os = "linux"))]
pub fn spawn_audio_capture(
    _to_gateway: tokio::sync::mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::channel::ChannelId;
    use bpane_protocol::frame::Message;
    use bpane_protocol::AudioFrame;

    #[test]
    fn detect_audio_returns_state() {
        let state = detect_audio();
        match state {
            AudioState::Available => {}
            AudioState::Unavailable(reason) => {
                assert!(!reason.is_empty());
            }
        }
    }

    #[test]
    fn audio_constants_correct() {
        // 48000 / 1000 * 20 = 960 samples per frame
        assert_eq!(SAMPLE_RATE / 1000 * FRAME_DURATION_MS, 960);
        // 960 * 2ch * 2 bytes = 3840
        assert_eq!(BYTES_PER_FRAME, 3840);
    }

    #[test]
    fn audio_constants_match_spec() {
        assert_eq!(SAMPLE_RATE, 48000);
        assert_eq!(CHANNELS, 2);
        assert_eq!(FRAME_DURATION_MS, 20);
        // Verify derivation: samples_per_frame = 48000/1000*20 = 960
        let samples_per_frame = SAMPLE_RATE / 1000 * FRAME_DURATION_MS;
        assert_eq!(samples_per_frame, 960);
        // bytes = samples * channels * sizeof(s16le)
        assert_eq!(BYTES_PER_FRAME, (samples_per_frame * CHANNELS * 2) as usize);
    }

    #[test]
    fn pcm_rms_dbfs_detects_silence() {
        let pcm = vec![0u8; BYTES_PER_FRAME];
        let dbfs = pcm_rms_dbfs_s16le(&pcm);
        assert!(dbfs <= -119.0, "dbfs={dbfs}");
    }

    #[test]
    fn pcm_rms_dbfs_detects_loud_audio() {
        let mut pcm = Vec::with_capacity(BYTES_PER_FRAME);
        for i in 0..(BYTES_PER_FRAME / 2) {
            let sample: i16 = if i % 2 == 0 { 24_000 } else { -24_000 };
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        let dbfs = pcm_rms_dbfs_s16le(&pcm);
        assert!(dbfs > -6.0, "dbfs={dbfs}");
    }

    #[test]
    fn silence_gate_suppresses_quiet_frames() {
        let gate = SilenceGateConfig {
            enabled: true,
            threshold_dbfs: -50.0,
            hangover_frames: 2,
        };
        let mut state = SilenceGateState::default();

        assert!(!should_forward_audio_frame(-90.0, gate, &mut state));
        assert!(should_forward_audio_frame(-20.0, gate, &mut state));
        assert!(should_forward_audio_frame(-90.0, gate, &mut state));
        assert!(should_forward_audio_frame(-90.0, gate, &mut state));
        assert!(!should_forward_audio_frame(-90.0, gate, &mut state));
    }

    #[test]
    fn silence_gate_disabled_always_forwards() {
        let gate = SilenceGateConfig {
            enabled: false,
            threshold_dbfs: -50.0,
            hangover_frames: 0,
        };
        let mut state = SilenceGateState::default();
        assert!(should_forward_audio_frame(-120.0, gate, &mut state));
        assert!(should_forward_audio_frame(-80.0, gate, &mut state));
        assert!(should_forward_audio_frame(-20.0, gate, &mut state));
    }

    #[test]
    fn audio_payload_pcm_has_magic_and_codec_tag() {
        let pcm_data = vec![0x12u8; BYTES_PER_FRAME];
        let payload = encode_audio_payload(&pcm_data, AudioCodec::PcmS16le, None);
        assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
        assert_eq!(payload[4], AUDIO_CODEC_PCM_S16LE);
        assert_eq!(&payload[5..], &pcm_data);
    }

    #[test]
    fn audio_payload_adpcm_is_tagged_and_smaller() {
        let mut pcm = Vec::with_capacity(BYTES_PER_FRAME);
        for i in 0..(BYTES_PER_FRAME / 2) {
            let sample: i16 = ((i as i32 * 97 % 60000) - 30000) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        let payload = encode_audio_payload(&pcm, AudioCodec::AdpcmImaStereo, None);
        assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
        assert_eq!(payload[4], AUDIO_CODEC_ADPCM_IMA_STEREO);
        assert!(
            payload.len() < 5 + pcm.len(),
            "payload_len={}",
            payload.len()
        );
    }

    #[test]
    fn adpcm_encoder_rejects_invalid_frame_size() {
        assert!(encode_ima_adpcm_stereo(&[]).is_empty());
        assert!(encode_ima_adpcm_stereo(&[0u8; 3]).is_empty());
        assert!(encode_ima_adpcm_stereo(&[0u8; 5]).is_empty());
    }

    #[test]
    fn audio_frame_pcm_round_trip() {
        let pcm_data = vec![0x12u8; BYTES_PER_FRAME];
        let frame = AudioFrame {
            seq: 42,
            timestamp_us: 840_000,
            data: pcm_data.clone(),
        };
        let encoded = frame.encode();
        let decoded = AudioFrame::decode(&encoded).unwrap();
        assert_eq!(decoded.seq, 42);
        assert_eq!(decoded.timestamp_us, 840_000);
        assert_eq!(decoded.data.len(), BYTES_PER_FRAME);
        assert_eq!(decoded.data, pcm_data);
    }

    #[test]
    fn audio_frame_to_frame_out_produces_correct_channel() {
        let af = AudioFrame {
            seq: 1,
            timestamp_us: 20_000,
            data: vec![0x00; BYTES_PER_FRAME],
        };
        let frame = af.to_frame_out();
        assert_eq!(frame.channel, ChannelId::AudioOut);
    }

    #[test]
    fn audio_frame_wire_round_trip_through_message_dispatch() {
        let af = AudioFrame {
            seq: 99,
            timestamp_us: 1_980_000,
            data: vec![0xAB; BYTES_PER_FRAME],
        };
        let frame = af.to_frame_out();
        let wire = frame.encode();
        let (decoded_frame, _) = bpane_protocol::frame::Frame::decode(&wire).unwrap();
        let msg = Message::from_frame(&decoded_frame).unwrap();
        match msg {
            Message::AudioOut(payload) => {
                let decoded_af = AudioFrame::decode(&payload).unwrap();
                assert_eq!(decoded_af.seq, 99);
                assert_eq!(decoded_af.timestamp_us, 1_980_000);
                assert_eq!(decoded_af.data.len(), BYTES_PER_FRAME);
            }
            other => panic!("expected AudioOut, got {:?}", other),
        }
    }

    #[test]
    fn audio_frame_timestamp_calculation() {
        // Verify the timestamp formula: seq * frame_duration_us
        let frame_duration_us = (FRAME_DURATION_MS as u64) * 1000; // 20_000
        assert_eq!(frame_duration_us, 20_000);

        for seq in [0u32, 1, 49, 100, 999] {
            let expected_ts = seq as u64 * frame_duration_us;
            let af = AudioFrame {
                seq,
                timestamp_us: expected_ts,
                data: vec![0; BYTES_PER_FRAME],
            };
            let decoded = AudioFrame::decode(&af.encode()).unwrap();
            assert_eq!(decoded.timestamp_us, expected_ts);
        }
    }

    #[test]
    fn audio_frame_sequence_produces_ordered_stream() {
        // Simulate 50 frames (1 second of audio) and verify ordering
        let frame_count = 50u32;
        let frames: Vec<Frame> = (1..=frame_count)
            .map(|i| {
                AudioFrame {
                    seq: i,
                    timestamp_us: i as u64 * 20_000,
                    data: vec![(i & 0xFF) as u8; BYTES_PER_FRAME],
                }
                .to_frame_out()
            })
            .collect();

        // Verify sequential order
        let mut prev_seq = 0u32;
        let mut prev_ts = 0u64;
        for frame in &frames {
            let af = AudioFrame::decode(&frame.payload).unwrap();
            assert!(af.seq > prev_seq);
            assert!(af.timestamp_us > prev_ts);
            assert_eq!(af.timestamp_us - prev_ts, 20_000);
            prev_seq = af.seq;
            prev_ts = af.timestamp_us;
        }
    }

    #[test]
    fn detect_audio_unavailable_on_non_linux() {
        // On macOS (test host), audio should be unavailable
        if !cfg!(target_os = "linux") {
            match detect_audio() {
                AudioState::Unavailable(reason) => {
                    assert!(reason.contains("Linux"), "reason: {reason}");
                }
                AudioState::Available => {
                    panic!("audio should not be available on non-Linux");
                }
            }
        }
    }

    #[tokio::test]
    async fn spawn_audio_capture_non_linux_completes() {
        // On non-Linux, spawn_audio_capture returns an empty task that completes
        if !cfg!(target_os = "linux") {
            let (tx, _rx) = tokio::sync::mpsc::channel(16);
            let handle = spawn_audio_capture(tx);
            // Should complete immediately
            tokio::time::timeout(std::time::Duration::from_secs(1), handle)
                .await
                .expect("task should complete quickly")
                .expect("task should not panic");
        }
    }

    #[test]
    fn opus_codec_id_is_0x02() {
        assert_eq!(AudioCodec::Opus.id(), AUDIO_CODEC_OPUS);
        assert_eq!(AUDIO_CODEC_OPUS, 0x02);
    }

    #[test]
    fn opus_encoder_creates_successfully() {
        let encoder = audiopus::coder::Encoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::LowDelay,
        );
        assert!(
            encoder.is_ok(),
            "opus encoder creation failed: {:?}",
            encoder.err()
        );
    }

    #[test]
    fn opus_encode_produces_smaller_payload() {
        let mut encoder = audiopus::coder::Encoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::LowDelay,
        )
        .expect("opus encoder");
        encoder
            .set_bitrate(audiopus::Bitrate::BitsPerSecond(OPUS_BITRATE_BPS))
            .expect("set bitrate");

        // Generate a synthetic PCM frame (sine-ish pattern)
        let mut pcm = Vec::with_capacity(BYTES_PER_FRAME);
        for i in 0..(BYTES_PER_FRAME / 2) {
            let sample: i16 = ((i as f64 * 0.1).sin() * 10000.0) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes());
        }

        let payload = encode_audio_payload(&pcm, AudioCodec::Opus, Some(&mut encoder));
        assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
        assert_eq!(payload[4], AUDIO_CODEC_OPUS);
        // Opus at 64kbps for 20ms should be ~160 bytes, far less than PCM (3840 bytes)
        let opus_data_len = payload.len() - 5;
        assert!(
            opus_data_len < BYTES_PER_FRAME / 2,
            "opus payload should be much smaller than PCM: opus={opus_data_len} pcm={BYTES_PER_FRAME}"
        );
        assert!(opus_data_len > 0, "opus payload should not be empty");
    }

    #[test]
    fn opus_encode_silence_frame() {
        let mut encoder = audiopus::coder::Encoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::LowDelay,
        )
        .expect("opus encoder");
        encoder
            .set_bitrate(audiopus::Bitrate::BitsPerSecond(OPUS_BITRATE_BPS))
            .expect("set bitrate");

        let pcm = vec![0u8; BYTES_PER_FRAME]; // silence
        let payload = encode_audio_payload(&pcm, AudioCodec::Opus, Some(&mut encoder));
        assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
        assert_eq!(payload[4], AUDIO_CODEC_OPUS);
        // Even silence should produce a valid (small) Opus frame
        assert!(
            payload.len() > 5,
            "opus silence frame should produce output"
        );
    }

    #[test]
    fn opus_encode_multiple_frames_stateful() {
        // Verify the encoder maintains state across frames (no panic, output is valid)
        let mut encoder = audiopus::coder::Encoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::LowDelay,
        )
        .expect("opus encoder");
        encoder
            .set_bitrate(audiopus::Bitrate::BitsPerSecond(OPUS_BITRATE_BPS))
            .expect("set bitrate");

        for frame_idx in 0..50u32 {
            let mut pcm = Vec::with_capacity(BYTES_PER_FRAME);
            for i in 0..(BYTES_PER_FRAME / 2) {
                let t = (frame_idx as f64 * 960.0 + i as f64) * 0.01;
                let sample: i16 = (t.sin() * 8000.0) as i16;
                pcm.extend_from_slice(&sample.to_le_bytes());
            }
            let payload = encode_audio_payload(&pcm, AudioCodec::Opus, Some(&mut encoder));
            assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
            assert_eq!(payload[4], AUDIO_CODEC_OPUS);
            assert!(payload.len() > 5, "frame {frame_idx}: empty opus output");
        }
    }

    #[test]
    fn opus_without_encoder_falls_back_to_pcm() {
        let pcm = vec![0u8; BYTES_PER_FRAME];
        let payload = encode_audio_payload(&pcm, AudioCodec::Opus, None);
        assert_eq!(&payload[..4], &AUDIO_PAYLOAD_MAGIC);
        // Should fall back to PCM codec tag
        assert_eq!(payload[4], AUDIO_CODEC_PCM_S16LE);
        assert_eq!(&payload[5..], &pcm[..]);
    }

    #[test]
    fn opus_constants_consistent() {
        assert_eq!(SAMPLES_PER_CHANNEL, 960);
        assert_eq!(OPUS_BITRATE_BPS, 64_000);
    }

    /// Test all codec parsing in a single test to avoid env var races
    /// between parallel test threads.
    #[test]
    fn audio_codec_from_env_parsing() {
        // Direct parsing tests that don't depend on the env var:
        // The from_env function reads BPANE_AUDIO_CODEC, which is shared
        // process state. Instead, test the codec enum properties directly.
        assert_eq!(AudioCodec::Opus.id(), 0x02);
        assert_eq!(AudioCodec::PcmS16le.id(), 0x00);
        assert_eq!(AudioCodec::AdpcmImaStereo.id(), 0x01);

        // Verify the match arms are correct by checking from_env
        // when we control the env (single test avoids races).
        unsafe { std::env::set_var("BPANE_AUDIO_CODEC", "opus") };
        assert_eq!(AudioCodec::from_env(), AudioCodec::Opus);

        unsafe { std::env::set_var("BPANE_AUDIO_CODEC", "pcm") };
        assert_eq!(AudioCodec::from_env(), AudioCodec::PcmS16le);

        unsafe { std::env::set_var("BPANE_AUDIO_CODEC", "adpcm") };
        assert_eq!(AudioCodec::from_env(), AudioCodec::AdpcmImaStereo);

        unsafe { std::env::set_var("BPANE_AUDIO_CODEC", "anything_else") };
        assert_eq!(AudioCodec::from_env(), AudioCodec::AdpcmImaStereo);

        // Clean up
        unsafe { std::env::remove_var("BPANE_AUDIO_CODEC") };
        assert_eq!(AudioCodec::from_env(), AudioCodec::AdpcmImaStereo);
    }
}
