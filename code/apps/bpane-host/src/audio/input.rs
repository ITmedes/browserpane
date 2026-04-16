//! Microphone input: receives browser audio frames and feeds decoded PCM
//! into a PipeWire virtual source so host applications see a microphone.
//!
//! Creates a pipe-backed source named `bpane-mic`, writes decoded PCM into its
//! FIFO, and sets it as the default source so host applications see a real
//! microphone/input device instead of a sink monitor.

use std::convert::{TryFrom, TryInto};
#[cfg(target_os = "linux")]
use std::ffi::CString;
#[cfg(target_os = "linux")]
use std::fs::{self, File, OpenOptions};
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

use audiopus::{packet::Packet, Channels, SampleRate};
use bpane_protocol::AudioFrame;

const MIC_SOURCE_NAME: &str = "bpane-mic";
const MIC_PIPE_PATH: &str = "/tmp/bpane-mic.pipe";
const MIC_SOURCE_DESCRIPTION: &str = "BrowserPane-Microphone";
const MIC_SOURCE_ICON: &str = "audio-input-microphone";

fn pipe_source_load_module_args(pipe_path: &Path) -> Vec<String> {
    vec![
        "load-module".to_string(),
        "module-pipe-source".to_string(),
        format!("source_name={MIC_SOURCE_NAME}"),
        format!(
            "source_properties=device.description={MIC_SOURCE_DESCRIPTION} device.icon_name={MIC_SOURCE_ICON}"
        ),
        format!("file={}", pipe_path.display()),
        "format=s16le".to_string(),
        "rate=48000".to_string(),
        "channels=1".to_string(),
    ]
}

fn set_default_source_args() -> [&'static str; 2] {
    ["set-default-source", MIC_SOURCE_NAME]
}

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
    pipe: File,
    pipe_path: PathBuf,
    module_id: Option<String>,
    opus_decoder: audiopus::coder::Decoder,
}

#[cfg(target_os = "linux")]
impl MicInput {
    /// Create the virtual mic source and open its backing FIFO. Non-blocking.
    pub fn new() -> anyhow::Result<Self> {
        use std::process::Command;

        let pipe_path = PathBuf::from(MIC_PIPE_PATH);
        prepare_pipe(&pipe_path)?;

        // Load a real source device backed by the FIFO.
        let load_args = pipe_source_load_module_args(&pipe_path);
        let output = Command::new("pactl")
            .args(load_args.iter().map(String::as_str))
            .output()?;

        let module_id = if output.status.success() {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            tracing::debug!("mic: pipe-source loaded (module {id})");
            Some(id)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            let _ = fs::remove_file(&pipe_path);
            return Err(anyhow::anyhow!(
                "failed to load microphone pipe-source: {err}"
            ));
        };

        // Point only the default source at the microphone device. Desktop audio
        // capture keeps using the separate bpane-desktop monitor sink.
        let _ = Command::new("pactl")
            .args(set_default_source_args())
            .output();

        // Keep the FIFO open read/write on our side so writes do not block or
        // fail before an application actually starts recording from the source.
        let pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_path)
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to open microphone pipe {}: {e}",
                    pipe_path.display()
                )
            })?;
        let opus_decoder = audiopus::coder::Decoder::new(SampleRate::Hz48000, Channels::Mono)
            .map_err(|e| anyhow::anyhow!("opus decoder init failed: {e}"))?;

        tracing::info!("mic: pipe-source ready → {MIC_SOURCE_NAME} (s16le 48kHz mono)");

        Ok(Self {
            pipe,
            pipe_path,
            module_id,
            opus_decoder,
        })
    }

    /// Write an incoming audio frame into the backing source FIFO after decoding if needed.
    pub fn write_frame(&mut self, audio_frame: &AudioFrame) {
        use std::io::Write;
        let pcm = match decode_audio_input_payload(&audio_frame.data, &mut self.opus_decoder) {
            Ok(pcm) => pcm,
            Err(e) => {
                tracing::debug!("mic: decode failed: {e}");
                return;
            }
        };
        if let Err(e) = self.pipe.write_all(&pcm) {
            tracing::debug!("mic: write failed: {e}");
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for MicInput {
    fn drop(&mut self) {
        // Unload the pipe-source module.
        if let Some(ref id) = self.module_id {
            let _ = std::process::Command::new("pactl")
                .args(["unload-module", id])
                .output();
            tracing::debug!("mic: pipe-source unloaded (module {id})");
        }
        let _ = fs::remove_file(&self.pipe_path);

        tracing::info!("mic: stopped");
    }
}

#[cfg(target_os = "linux")]
fn prepare_pipe(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| {
            anyhow::anyhow!(
                "failed to remove stale microphone pipe {}: {e}",
                path.display()
            )
        })?;
    }

    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| anyhow::anyhow!("invalid microphone pipe path: {}", path.display()))?;
    let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    if rc != 0 {
        return Err(anyhow::anyhow!(
            "failed to create microphone pipe {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
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
    use std::path::Path;

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

    #[test]
    fn pipe_source_load_module_args_expose_a_real_microphone_source() {
        let args = pipe_source_load_module_args(Path::new(MIC_PIPE_PATH));

        assert_eq!(args[0], "load-module");
        assert_eq!(args[1], "module-pipe-source");
        assert!(args
            .iter()
            .any(|arg| arg == &format!("source_name={MIC_SOURCE_NAME}")));
        assert!(args
            .iter()
            .any(|arg| arg == &format!(
                "source_properties=device.description={MIC_SOURCE_DESCRIPTION} device.icon_name={MIC_SOURCE_ICON}"
            )));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("file={MIC_PIPE_PATH}")));
    }

    #[test]
    fn set_default_source_args_point_to_the_microphone_source() {
        assert_eq!(
            set_default_source_args(),
            ["set-default-source", MIC_SOURCE_NAME]
        );
    }
}
