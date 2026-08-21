use std::sync::Arc;
use std::time::Duration;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{
    ClientHello, Frame, FrameDecoder, FrameDecoderError, FrameError, Message, NegotiationMessage,
    ProtocolCapability, ProtocolFailure, ProtocolNegotiator, ProtocolSupport, ServerSelection,
};
use bpane_protocol::{AudioFrame, ControlMessage, InputMessage, TileMessage, VideoDatagram};
use tokio::sync::Mutex;
use tracing::{info, warn};
use wtransport::{Connection, RecvStream, SendStream};

use crate::metrics::GatewayMetrics;

const MAX_NEGOTIATION_FRAME_BYTES: usize = 153;
const NEGOTIATION_READ_BYTES: usize = 4 * 1024;
const PROTOCOL_REJECT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const AUDIO_PAYLOAD_MAGIC: &[u8; 4] = b"WRA1";

#[derive(Clone, Debug)]
pub(super) struct ProtocolNegotiationConfig {
    pub timeout: Duration,
    pub legacy_compatibility: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ConnectionProtocol {
    V1 { selection: ServerSelection },
    Legacy,
}

impl ConnectionProtocol {
    pub fn allows(&self, capability: ProtocolCapability) -> bool {
        match self {
            Self::V1 { selection } => selection
                .capabilities()
                .binary_search(&capability.id())
                .is_ok(),
            Self::Legacy => true,
        }
    }

    pub fn validate_client_frame(&self, frame: &Frame) -> Result<(), ProtocolFailure> {
        if matches!(self, Self::Legacy) {
            return Ok(());
        }

        let message =
            Message::from_frame(frame).map_err(|_| ProtocolFailure::UnexpectedProtocolFrame)?;
        let required = match message {
            Message::Control(ControlMessage::ResolutionRequest { .. })
            | Message::Control(ControlMessage::Ping { .. })
            | Message::Control(ControlMessage::Pong { .. })
            | Message::Input(InputMessage::MouseMove { .. })
            | Message::Input(InputMessage::MouseButton { .. })
            | Message::Input(InputMessage::MouseScroll { .. })
            | Message::Input(InputMessage::KeyEvent { .. }) => None,
            Message::Control(ControlMessage::KeyboardLayoutInfo { .. })
            | Message::Input(InputMessage::KeyEventEx { .. }) => {
                Some(ProtocolCapability::ExtendedKeyboard)
            }
            Message::Clipboard(_) => Some(ProtocolCapability::ClipboardText),
            Message::FileUp(_) => Some(ProtocolCapability::FileTransfer),
            Message::AudioIn(payload) => {
                validate_audio_codec(&payload, ProtocolCapability::MicrophoneOpus)?;
                Some(ProtocolCapability::MicrophoneOpus)
            }
            Message::VideoIn(_) => Some(ProtocolCapability::CameraH264AnnexB),
            Message::Tiles(TileMessage::CacheMiss { .. }) => Some(ProtocolCapability::TileCache),
            Message::Control(_)
            | Message::Video(_)
            | Message::AudioOut(_)
            | Message::Cursor(_)
            | Message::FileDown(_)
            | Message::Tiles(_) => return Err(ProtocolFailure::UnexpectedProtocolFrame),
        };

        if required.is_some_and(|capability| !self.allows(capability)) {
            return Err(ProtocolFailure::UnexpectedProtocolFrame);
        }
        Ok(())
    }

    pub fn allows_server_frame(&self, frame: &Frame) -> bool {
        if matches!(self, Self::Legacy) {
            return true;
        }

        let Ok(message) = Message::from_frame(frame) else {
            return false;
        };
        match message {
            Message::Control(ControlMessage::ResolutionAck { .. })
            | Message::Control(ControlMessage::SessionReady { .. })
            | Message::Control(ControlMessage::Ping { .. })
            | Message::Control(ControlMessage::Pong { .. })
            | Message::Control(ControlMessage::BitrateHint { .. })
            | Message::Control(ControlMessage::ResolutionLocked { .. })
            | Message::Cursor(_)
            | Message::Tiles(TileMessage::GridConfig { .. })
            | Message::Tiles(TileMessage::Fill { .. })
            | Message::Tiles(TileMessage::Qoi { .. })
            | Message::Tiles(TileMessage::BatchEnd { .. }) => true,
            Message::Control(ControlMessage::ClientAccessState { .. }) => {
                self.allows(ProtocolCapability::ClientAccessState)
            }
            Message::Tiles(TileMessage::Zstd { .. }) => self.allows(ProtocolCapability::TileZstd),
            Message::Tiles(TileMessage::CacheHit { .. }) => {
                self.allows(ProtocolCapability::TileCache)
            }
            Message::Tiles(
                TileMessage::ScrollCopy { .. }
                | TileMessage::GridOffset { .. }
                | TileMessage::TileDrawMode { .. }
                | TileMessage::ScrollStats { .. },
            ) => self.allows(ProtocolCapability::TileScroll),
            Message::Tiles(TileMessage::VideoRegion { .. }) => {
                self.allows(ProtocolCapability::RoiVideo)
            }
            Message::Video(payload) => self.allows_video_payload(&payload),
            Message::AudioOut(payload) => self.allows_audio_payload(&payload),
            Message::Control(ControlMessage::ResolutionRequest { .. })
            | Message::Control(ControlMessage::KeyboardLayoutInfo { .. })
            | Message::Input(_)
            | Message::AudioIn(_)
            | Message::VideoIn(_)
            | Message::Clipboard(_)
            | Message::FileUp(_)
            | Message::Tiles(TileMessage::CacheMiss { .. }) => false,
            Message::FileDown(_) => self.allows(ProtocolCapability::FileTransfer),
        }
    }

    pub fn normalize_session_ready(&self, frame: &Frame) -> Frame {
        let Self::V1 { selection } = self else {
            return frame.clone();
        };
        let Ok(ControlMessage::SessionReady { mut flags, .. }) =
            ControlMessage::decode(&frame.payload)
        else {
            return frame.clone();
        };

        let audio_selected = [
            ProtocolCapability::AudioPcmS16Le,
            ProtocolCapability::AudioAdpcmImaStereo,
            ProtocolCapability::AudioOpus,
        ]
        .into_iter()
        .any(|capability| self.allows(capability));
        if !audio_selected {
            flags.remove(bpane_protocol::SessionFlags::AUDIO);
        }
        if !self.allows(ProtocolCapability::ClipboardText) {
            flags.remove(bpane_protocol::SessionFlags::CLIPBOARD);
        }
        if !self.allows(ProtocolCapability::FileTransfer) {
            flags.remove(bpane_protocol::SessionFlags::FILE_TRANSFER);
        }
        if !self.allows(ProtocolCapability::MicrophoneOpus) {
            flags.remove(bpane_protocol::SessionFlags::MICROPHONE);
        }
        if !self.allows(ProtocolCapability::CameraH264AnnexB) {
            flags.remove(bpane_protocol::SessionFlags::CAMERA);
        }
        if !self.allows(ProtocolCapability::ExtendedKeyboard) {
            flags.remove(bpane_protocol::SessionFlags::KEYBOARD_LAYOUT);
        }

        ControlMessage::SessionReady {
            version: selection.selected_version() as u8,
            flags,
        }
        .to_frame()
    }

    fn allows_audio_payload(&self, payload: &[u8]) -> bool {
        let Ok(frame) = AudioFrame::decode(payload) else {
            return false;
        };
        audio_codec_capability(&frame.data).is_some_and(|capability| self.allows(capability))
    }

    fn allows_video_payload(&self, payload: &[u8]) -> bool {
        if !self.allows(ProtocolCapability::H264Video) {
            return false;
        }
        VideoDatagram::decode(payload).is_ok_and(|video| {
            video.tile_info.is_none() || self.allows(ProtocolCapability::RoiVideo)
        })
    }
}

pub(super) struct NegotiatedConnection {
    pub send_stream: Arc<Mutex<SendStream>>,
    pub recv_stream: RecvStream,
    pub protocol: ConnectionProtocol,
    pub initial_client_frames: Vec<Frame>,
}

pub(super) async fn negotiate_connection(
    connection: &Connection,
    config: &ProtocolNegotiationConfig,
    metrics: &GatewayMetrics,
) -> anyhow::Result<Option<NegotiatedConnection>> {
    let (mut send_stream, mut recv_stream) = connection.open_bi().await?.await?;
    let started_at = metrics.begin_protocol_negotiation();
    let mut state = GatewayNegotiation::new(config.legacy_compatibility);
    let deadline = tokio::time::Instant::now() + config.timeout;
    let mut read_buffer = [0u8; NEGOTIATION_READ_BYTES];

    let decision = loop {
        let read = tokio::time::timeout_at(deadline, recv_stream.read(&mut read_buffer)).await;
        match read {
            Ok(Ok(Some(read))) => match state.ingest(&read_buffer[..read]) {
                Ok(Some(decision)) => break Ok(decision),
                Ok(None) => {}
                Err(failure) => break Err(failure),
            },
            Ok(Ok(None)) => break Err(ProtocolFailure::ProtocolHandshakeTimeout),
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => break state.on_timeout(),
        }
    };

    match decision {
        Ok(HandshakeDecision::Negotiated(selection)) => {
            send_stream
                .write_all(
                    &NegotiationMessage::ServerSelection(selection.clone())
                        .to_frame()
                        .encode(),
                )
                .await?;
            metrics.record_protocol_negotiation_success(started_at);
            info!(
                protocol_mode = "negotiated",
                selected_version = selection.selected_version(),
                capability_count = selection.capabilities().len(),
                "browser protocol negotiation completed"
            );
            Ok(Some(NegotiatedConnection {
                send_stream: Arc::new(Mutex::new(send_stream)),
                recv_stream,
                protocol: ConnectionProtocol::V1 { selection },
                initial_client_frames: Vec::new(),
            }))
        }
        Ok(HandshakeDecision::Legacy(initial_client_frames)) => {
            metrics.record_protocol_legacy_selection(started_at);
            warn!(
                protocol_mode = "legacy",
                "temporary browser protocol legacy compatibility selected"
            );
            Ok(Some(NegotiatedConnection {
                send_stream: Arc::new(Mutex::new(send_stream)),
                recv_stream,
                protocol: ConnectionProtocol::Legacy,
                initial_client_frames,
            }))
        }
        Err(failure) => {
            metrics.record_protocol_negotiation_failure(failure, started_at);
            warn!(
                protocol_mode = "rejected",
                failure = failure.code(),
                "browser protocol negotiation rejected"
            );
            send_protocol_reject(&mut send_stream, failure).await;
            connection.close(protocol_close_code(failure), failure.code().as_bytes());
            Ok(None)
        }
    }
}

pub(super) async fn reject_active_connection(
    connection: &Connection,
    send_stream: &Arc<Mutex<SendStream>>,
    failure: ProtocolFailure,
) {
    let mut stream = send_stream.lock().await;
    send_protocol_reject(&mut stream, failure).await;
    connection.close(protocol_close_code(failure), failure.code().as_bytes());
}

async fn send_protocol_reject(send_stream: &mut SendStream, failure: ProtocolFailure) {
    let reject = NegotiationMessage::ProtocolReject(failure)
        .to_frame()
        .encode();
    if send_stream.write_all(&reject).await.is_ok() {
        let _ = tokio::time::timeout(PROTOCOL_REJECT_DRAIN_TIMEOUT, send_stream.finish()).await;
    }
}

fn protocol_close_code(failure: ProtocolFailure) -> wtransport::VarInt {
    wtransport::VarInt::from_u32(0x4250 + u32::from(failure.id()))
}

#[derive(Debug, PartialEq, Eq)]
enum HandshakeDecision {
    Negotiated(ServerSelection),
    Legacy(Vec<Frame>),
}

struct GatewayNegotiation {
    decoder: FrameDecoder,
    legacy_compatibility: bool,
}

impl GatewayNegotiation {
    fn new(legacy_compatibility: bool) -> Self {
        Self {
            decoder: FrameDecoder::with_max_pending(MAX_NEGOTIATION_FRAME_BYTES),
            legacy_compatibility,
        }
    }

    fn ingest(&mut self, bytes: &[u8]) -> Result<Option<HandshakeDecision>, ProtocolFailure> {
        self.decoder.push(bytes).map_err(map_decoder_error)?;
        let Some(first_frame) = self.decoder.next_frame().map_err(map_decoder_error)? else {
            return Ok(None);
        };

        if first_frame.channel == ChannelId::Control
            && first_frame.payload.first().copied() == Some(0x0A)
        {
            let hello =
                ClientHello::decode(&first_frame.payload).map_err(|error| error.failure())?;
            if self.decoder.pending_len() != 0 {
                return Err(ProtocolFailure::UnexpectedProtocolFrame);
            }
            let support = gateway_protocol_support();
            let selection = ProtocolNegotiator::select(&hello, &support)?;
            return Ok(Some(HandshakeDecision::Negotiated(selection)));
        }

        if !is_checked_legacy_client_frame(&first_frame) {
            return Err(ProtocolFailure::UnexpectedProtocolFrame);
        }
        if !self.legacy_compatibility {
            return Err(ProtocolFailure::ProtocolDowngradeRefused);
        }
        if self.decoder.pending_len() != 0 {
            return Err(ProtocolFailure::UnexpectedProtocolFrame);
        }
        Ok(Some(HandshakeDecision::Legacy(vec![first_frame])))
    }

    fn on_timeout(&self) -> Result<HandshakeDecision, ProtocolFailure> {
        Err(ProtocolFailure::ProtocolHandshakeTimeout)
    }
}

fn gateway_protocol_support() -> ProtocolSupport {
    ProtocolSupport::new(
        vec![1],
        vec![
            ProtocolCapability::TileZstd,
            ProtocolCapability::TileCache,
            ProtocolCapability::TileScroll,
            ProtocolCapability::H264Video,
            ProtocolCapability::RoiVideo,
            ProtocolCapability::AudioOpus,
            ProtocolCapability::MicrophoneOpus,
            ProtocolCapability::CameraH264AnnexB,
            ProtocolCapability::ClipboardText,
            ProtocolCapability::FileTransfer,
            ProtocolCapability::ExtendedKeyboard,
            ProtocolCapability::ClientAccessState,
        ],
    )
    .expect("the fixed gateway protocol support profile is valid")
}

fn is_checked_legacy_client_frame(frame: &Frame) -> bool {
    matches!(
        Message::from_frame(frame),
        Ok(Message::Control(ControlMessage::ResolutionRequest { .. }))
            | Ok(Message::Control(ControlMessage::Ping { .. }))
            | Ok(Message::Control(ControlMessage::Pong { .. }))
            | Ok(Message::Control(ControlMessage::KeyboardLayoutInfo { .. }))
            | Ok(Message::Input(_))
            | Ok(Message::AudioIn(_))
            | Ok(Message::VideoIn(_))
            | Ok(Message::Clipboard(_))
            | Ok(Message::FileUp(_))
            | Ok(Message::Tiles(TileMessage::CacheMiss { .. }))
    )
}

fn validate_audio_codec(
    payload: &[u8],
    required: ProtocolCapability,
) -> Result<(), ProtocolFailure> {
    let frame =
        AudioFrame::decode(payload).map_err(|_| ProtocolFailure::UnexpectedProtocolFrame)?;
    if audio_codec_capability(&frame.data) != Some(ProtocolCapability::AudioOpus)
        || required != ProtocolCapability::MicrophoneOpus
    {
        return Err(ProtocolFailure::UnexpectedProtocolFrame);
    }
    Ok(())
}

fn audio_codec_capability(data: &[u8]) -> Option<ProtocolCapability> {
    if data.get(..AUDIO_PAYLOAD_MAGIC.len()) != Some(AUDIO_PAYLOAD_MAGIC) {
        return None;
    }
    match data.get(AUDIO_PAYLOAD_MAGIC.len()).copied() {
        Some(0x00) => Some(ProtocolCapability::AudioPcmS16Le),
        Some(0x01) => Some(ProtocolCapability::AudioAdpcmImaStereo),
        Some(0x02) => Some(ProtocolCapability::AudioOpus),
        _ => None,
    }
}

pub(super) fn map_decoder_error(error: FrameDecoderError) -> ProtocolFailure {
    match error {
        FrameDecoderError::Frame(FrameError::PayloadTooLarge(_)) => {
            ProtocolFailure::ProtocolFrameTooLarge
        }
        FrameDecoderError::PendingTooLarge { .. } => ProtocolFailure::ProtocolPendingBufferLimit,
        FrameDecoderError::Frame(_) => ProtocolFailure::UnexpectedProtocolFrame,
    }
}

#[cfg(test)]
mod tests;
