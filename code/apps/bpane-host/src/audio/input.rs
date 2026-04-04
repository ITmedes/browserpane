//! Microphone input: receives browser audio frames and feeds decoded PCM
//! into a PipeWire virtual source so host applications see a microphone.
//!
//! Creates a null sink named `bpane-mic`, pipes PCM into it via `pacat`,
//! and sets its monitor as the default source. Applications see
//! `bpane-mic.monitor` as a standard microphone device.

use std::convert::{TryFrom, TryInto};

use audiopus::{packet::Packet, Channels, SampleRate};
use bpane_protocol::AudioFrame;

fn decode_audio_input_payload(
    data: &[u8],
    opus_decoder: &mut audiopus::coder::Decoder,
) -> anyhow::Result<Vec<u8>> {
    if data.len() >= 5 && data[..4] == super::AUDIO_PAYLOAD_MAGIC {
        match data[4] {
            super::AUDIO_CODEC_PCM_S16LE => Ok(data[5..].to_vec()),
            super::AUDIO_CODEC_OPUS => decode_opus_payload(&data[5..], opus_decoder),
            codec => Err(anyhow::anyhow!(
                "unsupported microphone codec: 0x{codec:02x}"
            )),
        }
    } else {
        // Backward compatibility for the previous raw PCM mic path.
        Ok(data.to_vec())
    }
}

fn decode_opus_payload(
    payload: &[u8],
    opus_decoder: &mut audiopus::coder::Decoder,
) -> anyhow::Result<Vec<u8>> {
    let packet = Packet::try_from(payload)?;
    let mut pcm = vec![0i16; super::SAMPLES_PER_CHANNEL];
    let decoded_samples = opus_decoder.decode(Some(packet), (&mut pcm[..]).try_into()?, false)?;
    let mut out = Vec::with_capacity(decoded_samples * 2);
    for sample in pcm.into_iter().take(decoded_samples) {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(out)
}

/// State for the microphone virtual source.
#[cfg(target_os = "linux")]
pub struct MicInput {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    module_id: Option<String>,
    opus_decoder: audiopus::coder::Decoder,
}

#[cfg(target_os = "linux")]
impl MicInput {
    /// Create the virtual mic source and spawn pacat. Non-blocking.
    pub fn new() -> anyhow::Result<Self> {
        use std::process::{Command, Stdio};

        // Load a null-sink — its .monitor becomes our virtual mic source
        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-null-sink",
                "sink_name=bpane-mic",
                "sink_properties=device.description=BrowserPane-Microphone",
                "format=s16le",
                "rate=48000",
                "channels=1",
            ])
            .output()?;

        let module_id = if output.status.success() {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            tracing::debug!("mic: null-sink loaded (module {id})");
            Some(id)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("mic: failed to load null-sink: {err}");
            None
        };

        // WirePlumber auto-switches defaults to newly loaded sinks.
        // Restore the desktop audio sink as default so apps and FFmpeg
        // capture keep using it, then set only the source to our mic.
        let _ = Command::new("pactl")
            .args(["set-default-sink", "bpane-desktop"])
            .output();
        let _ = Command::new("pactl")
            .args(["set-default-source", "bpane-mic.monitor"])
            .output();

        // Pipe PCM into the null-sink via pacat
        let mut child = Command::new("pacat")
            .args([
                "--playback",
                "--device=bpane-mic",
                "--format=s16le",
                "--rate=48000",
                "--channels=1",
                "--stream-name=bpane-mic-input",
                "--channel-map=mono",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("pacat: no stdin"))?;
        let opus_decoder = audiopus::coder::Decoder::new(SampleRate::Hz48000, Channels::Mono)
            .map_err(|e| anyhow::anyhow!("opus decoder init failed: {e}"))?;

        tracing::info!("mic: pacat started → bpane-mic sink (s16le 48kHz mono)");

        Ok(Self {
            child,
            stdin,
            module_id,
            opus_decoder,
        })
    }

    /// Write an incoming audio frame to pacat stdin after decoding if needed.
    pub fn write_frame(&mut self, audio_frame: &AudioFrame) {
        use std::io::Write;
        let pcm = match decode_audio_input_payload(&audio_frame.data, &mut self.opus_decoder) {
            Ok(pcm) => pcm,
            Err(e) => {
                tracing::debug!("mic: decode failed: {e}");
                return;
            }
        };
        if let Err(e) = self.stdin.write_all(&pcm) {
            tracing::debug!("mic: write failed: {e}");
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for MicInput {
    fn drop(&mut self) {
        // Stop pacat
        let _ = self.child.kill();
        let _ = self.child.wait();

        // Unload the null-sink module
        if let Some(ref id) = self.module_id {
            let _ = std::process::Command::new("pactl")
                .args(["unload-module", id])
                .output();
            tracing::debug!("mic: null-sink unloaded (module {id})");
        }

        tracing::info!("mic: stopped");
    }
}

// Non-Linux stubs
#[cfg(not(target_os = "linux"))]
pub struct MicInput;

#[cfg(not(target_os = "linux"))]
impl MicInput {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
    pub fn write_frame(&mut self, _audio_frame: &AudioFrame) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiopus::{coder::Encoder, Application, Bitrate};

    #[test]
    fn mic_input_new_does_not_panic() {
        // On non-Linux or without PulseAudio, this should not panic
        // (it may fail gracefully)
        let _ = MicInput::new();
    }

    #[test]
    fn decode_audio_input_payload_passes_legacy_pcm_through() {
        let mut decoder =
            audiopus::coder::Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap();
        let pcm = vec![0x34, 0x12, 0x78, 0x56];
        let decoded = decode_audio_input_payload(&pcm, &mut decoder).unwrap();
        assert_eq!(decoded, pcm);
    }

    #[test]
    fn decode_audio_input_payload_decodes_opus_packets() {
        let mut encoder =
            Encoder::new(SampleRate::Hz48000, Channels::Mono, Application::LowDelay).unwrap();
        encoder
            .set_bitrate(Bitrate::BitsPerSecond(32_000))
            .expect("set opus bitrate");

        let mut samples = vec![0i16; super::super::SAMPLES_PER_CHANNEL];
        for (i, sample) in samples.iter_mut().enumerate() {
            let phase = (i as f32 / 24.0).sin();
            *sample = (phase * 12_000.0) as i16;
        }

        let mut opus_buf = [0u8; 4000];
        let encoded_len = encoder.encode(&samples, &mut opus_buf).unwrap();
        let mut payload = Vec::with_capacity(5 + encoded_len);
        payload.extend_from_slice(&super::super::AUDIO_PAYLOAD_MAGIC);
        payload.push(super::super::AUDIO_CODEC_OPUS);
        payload.extend_from_slice(&opus_buf[..encoded_len]);

        let mut decoder =
            audiopus::coder::Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap();
        let decoded = decode_audio_input_payload(&payload, &mut decoder).unwrap();

        assert_eq!(decoded.len(), super::super::SAMPLES_PER_CHANNEL * 2);
        assert!(decoded.iter().any(|byte| *byte != 0));
    }
}
