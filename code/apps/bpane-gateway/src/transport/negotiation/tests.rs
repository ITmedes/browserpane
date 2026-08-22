use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{ClientHello, Frame, NegotiationMessage, ProtocolCapability};
use bpane_protocol::{AudioFrame, ControlMessage, InputMessage, SessionFlags};

use super::*;

fn encoded_hello(required: Vec<u16>, optional: Vec<u16>) -> Vec<u8> {
    ClientHello::new(vec![1], required, optional)
        .unwrap()
        .to_frame()
        .encode()
        .to_vec()
}

#[test]
fn chunked_v1_hello_selects_highest_common_capability_subset() {
    let bytes = encoded_hello(
        vec![ProtocolCapability::ClipboardText.id()],
        vec![ProtocolCapability::FileTransfer.id(), 0x00FF],
    );
    let mut state = GatewayNegotiation::new(true);

    assert_eq!(state.ingest(&bytes[..4]).unwrap(), None);
    let decision = state.ingest(&bytes[4..]).unwrap().unwrap();
    let HandshakeDecision::Negotiated(selection) = decision else {
        panic!("expected negotiated v1");
    };

    assert_eq!(selection.selected_version(), 1);
    assert_eq!(
        selection.capabilities(),
        &[
            ProtocolCapability::ClipboardText.id(),
            ProtocolCapability::FileTransfer.id()
        ]
    );
}

#[test]
fn malformed_hello_never_falls_back_to_legacy() {
    let malformed = Frame::new(ChannelId::Control, vec![0x0A, 2, 1, 0, 1, 0, 0, 0]).encode();
    let mut state = GatewayNegotiation::new(true);

    assert_eq!(
        state.ingest(&malformed),
        Err(ProtocolFailure::MalformedProtocolHello)
    );
}

#[test]
fn negotiated_hello_rejects_coalesced_application_frame() {
    let mut bytes = encoded_hello(vec![], vec![]);
    bytes.extend_from_slice(
        &ControlMessage::ResolutionRequest {
            width: 1280,
            height: 720,
        }
        .to_frame()
        .encode(),
    );
    let mut state = GatewayNegotiation::new(true);

    assert_eq!(
        state.ingest(&bytes),
        Err(ProtocolFailure::UnexpectedProtocolFrame)
    );
}

#[test]
fn checked_legacy_resize_is_preserved_only_when_enabled() {
    let resize = ControlMessage::ResolutionRequest {
        width: 1280,
        height: 720,
    }
    .to_frame();
    let encoded = resize.encode();

    let mut enabled = GatewayNegotiation::new(true);
    assert_eq!(
        enabled.ingest(&encoded).unwrap(),
        Some(HandshakeDecision::Legacy(resize))
    );

    let mut disabled = GatewayNegotiation::new(false);
    assert_eq!(
        disabled.ingest(&encoded),
        Err(ProtocolFailure::ProtocolDowngradeRefused)
    );
}

#[test]
fn checked_legacy_selection_preserves_coalesced_following_frames() {
    let resize = ControlMessage::ResolutionRequest {
        width: 1280,
        height: 720,
    }
    .to_frame();
    let ping = ControlMessage::Ping {
        seq: 7,
        timestamp_ms: 11,
    }
    .to_frame();
    let mut bytes = resize.encode().to_vec();
    bytes.extend_from_slice(&ping.encode());
    let mut state = GatewayNegotiation::new(true);

    assert_eq!(
        state.ingest(&bytes).unwrap(),
        Some(HandshakeDecision::Legacy(resize))
    );
    assert_eq!(state.into_pending_bytes(), ping.encode());
}

#[test]
fn checked_legacy_selection_preserves_partial_following_frame() {
    let resize = ControlMessage::ResolutionRequest {
        width: 1280,
        height: 720,
    }
    .to_frame();
    let ping = ControlMessage::Ping {
        seq: 7,
        timestamp_ms: 11,
    }
    .to_frame();
    let ping_wire = ping.encode();
    let mut bytes = resize.encode().to_vec();
    bytes.extend_from_slice(&ping_wire[..6]);
    let mut state = GatewayNegotiation::new(true);

    assert_eq!(
        state.ingest(&bytes).unwrap(),
        Some(HandshakeDecision::Legacy(resize))
    );
    assert_eq!(state.into_pending_bytes(), ping_wire[..6]);
}

#[test]
fn silent_timeout_never_falls_back_to_legacy() {
    let enabled = GatewayNegotiation::new(true);
    assert_eq!(
        enabled.on_timeout(),
        Err(ProtocolFailure::ProtocolHandshakeTimeout)
    );

    let disabled = GatewayNegotiation::new(false);
    assert_eq!(
        disabled.on_timeout(),
        Err(ProtocolFailure::ProtocolHandshakeTimeout)
    );
}

#[test]
fn partial_hello_times_out_without_legacy_fallback() {
    let mut state = GatewayNegotiation::new(true);
    assert_eq!(state.ingest(&[ChannelId::Control.as_u8()]).unwrap(), None);
    assert_eq!(
        state.on_timeout(),
        Err(ProtocolFailure::ProtocolHandshakeTimeout)
    );
}

#[test]
fn oversized_and_pending_limit_headers_have_fixed_outcomes() {
    let mut oversized = GatewayNegotiation::new(true);
    let declared_too_large = (16_u32 * 1024 * 1024 + 1).to_le_bytes();
    let mut bytes = vec![ChannelId::Control.as_u8()];
    bytes.extend_from_slice(&declared_too_large);
    assert_eq!(
        oversized.ingest(&bytes),
        Err(ProtocolFailure::ProtocolFrameTooLarge)
    );

    let mut pending = GatewayNegotiation::new(true);
    let mut bytes = vec![ChannelId::Control.as_u8()];
    bytes.extend_from_slice(&149_u32.to_le_bytes());
    assert_eq!(
        pending.ingest(&bytes),
        Err(ProtocolFailure::ProtocolPendingBufferLimit)
    );

    let mut coalesced = GatewayNegotiation::new(true);
    let resize = ControlMessage::ResolutionRequest {
        width: 1280,
        height: 720,
    }
    .to_frame()
    .encode();
    let mut bytes = resize.to_vec();
    bytes.resize(super::MAX_NEGOTIATION_PENDING_BYTES + 1, 0);
    assert_eq!(
        coalesced.ingest(&bytes),
        Err(ProtocolFailure::ProtocolPendingBufferLimit)
    );
}

#[test]
fn unsupported_and_missing_required_capabilities_are_typed() {
    let unsupported = ClientHello::new(vec![2], vec![], vec![])
        .unwrap()
        .to_frame()
        .encode();
    let mut state = GatewayNegotiation::new(true);
    assert_eq!(
        state.ingest(&unsupported),
        Err(ProtocolFailure::UnsupportedProtocolVersion)
    );

    let required_pcm = encoded_hello(vec![ProtocolCapability::AudioPcmS16Le.id()], vec![]);
    let mut state = GatewayNegotiation::new(true);
    assert_eq!(
        state.ingest(&required_pcm),
        Err(ProtocolFailure::RequiredProtocolCapabilityMissing)
    );
}

#[test]
fn negotiated_client_direction_capability_and_duplicate_rules_are_enforced() {
    let protocol = ConnectionProtocol::V1 {
        selection: ServerSelection::new(1, vec![ProtocolCapability::ExtendedKeyboard.id()])
            .unwrap(),
    };
    let core = InputMessage::MouseMove { x: 1, y: 2 }.to_frame();
    assert_eq!(protocol.validate_client_frame(&core), Ok(()));

    let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01, 0, 0, 0, 0]);
    assert_eq!(
        protocol.validate_client_frame(&clipboard),
        Err(ProtocolFailure::UnexpectedProtocolFrame)
    );

    let server_only = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::empty(),
    }
    .to_frame();
    assert_eq!(
        protocol.validate_client_frame(&server_only),
        Err(ProtocolFailure::UnexpectedProtocolFrame)
    );

    let duplicate = ClientHello::new(vec![1], vec![], vec![])
        .unwrap()
        .to_frame();
    assert_eq!(
        protocol.validate_client_frame(&duplicate),
        Err(ProtocolFailure::UnexpectedProtocolFrame)
    );

    let microphone_protocol = ConnectionProtocol::V1 {
        selection: ServerSelection::new(1, vec![ProtocolCapability::MicrophoneOpus.id()]).unwrap(),
    };
    let microphone = AudioFrame {
        seq: 1,
        timestamp_us: 2,
        data: vec![b'W', b'R', b'A', b'1', 0x02, 0xAA],
    }
    .to_frame_in();
    assert_eq!(
        microphone_protocol.validate_client_frame(&microphone),
        Ok(())
    );

    let untagged_microphone = AudioFrame {
        seq: 1,
        timestamp_us: 2,
        data: vec![0x02, 0xAA],
    }
    .to_frame_in();
    assert_eq!(
        microphone_protocol.validate_client_frame(&untagged_microphone),
        Err(ProtocolFailure::UnexpectedProtocolFrame)
    );
}

#[test]
fn session_ready_version_and_flags_stay_within_negotiated_upper_bound() {
    let protocol = ConnectionProtocol::V1 {
        selection: ServerSelection::new(
            1,
            vec![
                ProtocolCapability::AudioOpus.id(),
                ProtocolCapability::ClipboardText.id(),
            ],
        )
        .unwrap(),
    };
    let host_ready = ControlMessage::SessionReady {
        version: 2,
        flags: SessionFlags::all(),
    }
    .to_frame();

    let normalized = protocol.normalize_session_ready(&host_ready);
    assert_eq!(
        ControlMessage::decode(&normalized.payload).unwrap(),
        ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::AUDIO | SessionFlags::CLIPBOARD,
        }
    );
}

#[test]
fn outbound_audio_codec_and_server_direction_are_gated() {
    let protocol = ConnectionProtocol::V1 {
        selection: ServerSelection::new(1, vec![ProtocolCapability::AudioOpus.id()]).unwrap(),
    };
    let opus = AudioFrame {
        seq: 1,
        timestamp_us: 2,
        data: vec![b'W', b'R', b'A', b'1', 0x02, 0xAA],
    }
    .to_frame_out();
    let pcm = AudioFrame {
        seq: 1,
        timestamp_us: 2,
        data: vec![b'W', b'R', b'A', b'1', 0x00, 0xAA],
    }
    .to_frame_out();
    let untagged = AudioFrame {
        seq: 1,
        timestamp_us: 2,
        data: vec![0x02, 0xAA],
    }
    .to_frame_out();

    assert!(protocol.allows_server_frame(&opus));
    assert!(!protocol.allows_server_frame(&pcm));
    assert!(!protocol.allows_server_frame(&untagged));
    assert!(!protocol.allows_server_frame(&InputMessage::MouseMove { x: 1, y: 2 }.to_frame()));
    assert!(!protocol.allows_server_frame(
        &NegotiationMessage::ProtocolReject(ProtocolFailure::UnexpectedProtocolFrame).to_frame()
    ));
}
