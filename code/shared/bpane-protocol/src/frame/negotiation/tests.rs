use alloc::vec;

use crate::channel::ChannelId;

use super::{
    ClientHello, NegotiationMessage, ProtocolCapability as Capability, ProtocolFailure as Failure,
    ProtocolNegotiator, ProtocolSupport, ProtocolSupportError, ServerSelection,
};

fn support() -> ProtocolSupport {
    ProtocolSupport::new(
        vec![1],
        vec![
            Capability::H264Video,
            Capability::RoiVideo,
            Capability::AudioOpus,
            Capability::ClipboardText,
            Capability::FileTransfer,
        ],
    )
    .expect("valid support")
}

#[test]
fn negotiation_messages_have_exact_v1_bytes() {
    let hello = ClientHello::new(vec![1], vec![0x000B], vec![0x0004, 0x0005]).expect("valid hello");
    assert_eq!(
        hello.encode(),
        [0x0A, 0x01, 0x01, 0x00, 0x01, 0x0B, 0x00, 0x02, 0x04, 0x00, 0x05, 0x00,]
    );
    assert_eq!(hello.to_frame().channel, ChannelId::Control);
    assert_eq!(ClientHello::decode(&hello.encode()), Ok(hello.clone()));

    let selection = ServerSelection::new(1, vec![0x0004, 0x0005, 0x000B]).expect("valid selection");
    assert_eq!(
        selection.encode(),
        [0x0B, 0x01, 0x00, 0x03, 0x04, 0x00, 0x05, 0x00, 0x0B, 0x00]
    );
    assert_eq!(
        ServerSelection::decode(&selection.encode()),
        Ok(selection.clone())
    );

    let rejection = NegotiationMessage::ProtocolReject(Failure::RequiredProtocolCapabilityMissing);
    assert_eq!(rejection.encode(), [0x0C, 0x02, 0x00]);
    assert_eq!(
        NegotiationMessage::decode(&rejection.encode()),
        Ok(rejection)
    );
}

#[test]
fn hello_enforces_every_bound_and_canonical_rule() {
    let maximum =
        ClientHello::new((1..=8).collect(), vec![], (1..=64).collect()).expect("maximum hello");
    assert_eq!(maximum.encode().len(), 148);
    assert_eq!(ClientHello::decode(&maximum.encode()), Ok(maximum));

    for invalid in [
        ClientHello::new(vec![], vec![], vec![]),
        ClientHello::new((1..=9).collect(), vec![], vec![]),
        ClientHello::new(vec![0], vec![], vec![]),
        ClientHello::new(vec![2, 1], vec![], vec![]),
        ClientHello::new(vec![1, 1], vec![], vec![]),
        ClientHello::new(vec![1], vec![2, 1], vec![]),
        ClientHello::new(vec![1], vec![1], vec![1]),
        ClientHello::new(vec![1], (1..=64).collect(), vec![65]),
    ] {
        assert_eq!(
            invalid.expect_err("invalid hello").failure(),
            Failure::MalformedProtocolHello
        );
    }
}

#[test]
fn hello_decode_maps_truncated_trailing_and_oversized_inputs() {
    let valid = ClientHello::new(vec![1], vec![], vec![])
        .expect("hello")
        .encode();
    for end in 1..valid.len() {
        assert_eq!(
            ClientHello::decode(&valid[..end])
                .expect_err("truncated hello")
                .failure(),
            Failure::MalformedProtocolHello
        );
    }

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        ClientHello::decode(&trailing)
            .expect_err("trailing hello")
            .failure(),
        Failure::MalformedProtocolHello
    );

    let mut oversized = vec![0x0A, 1, 1, 0, 0, 64];
    oversized.extend((1u16..=64).flat_map(u16::to_le_bytes));
    oversized.push(1);
    oversized.extend_from_slice(&65u16.to_le_bytes());
    assert_eq!(
        ClientHello::decode(&oversized)
            .expect_err("oversized hello")
            .failure(),
        Failure::MalformedProtocolHello
    );
}

#[test]
fn selection_and_rejection_decoders_fail_with_stable_outcomes() {
    for invalid in [
        ServerSelection::new(0, vec![]),
        ServerSelection::new(1, vec![2, 1]),
        ServerSelection::new(1, vec![1, 1]),
        ServerSelection::new(1, vec![0x0005]),
        ServerSelection::new(1, vec![0x0006, 0x0008]),
        ServerSelection::new(1, vec![0xFFFF]),
    ] {
        assert_eq!(
            invalid.expect_err("invalid selection").failure(),
            Failure::ProtocolSelectionMismatch
        );
    }

    for bytes in [vec![0x0B], vec![0x0B, 1, 0, 0, 0], vec![0x0B, 1, 0, 1]] {
        assert_eq!(
            ServerSelection::decode(&bytes)
                .expect_err("invalid selection bytes")
                .failure(),
            Failure::ProtocolSelectionMismatch
        );
    }

    for bytes in [vec![], vec![0x0C], vec![0x0C, 1, 0, 0], vec![0x0C, 10, 0]] {
        assert_eq!(
            NegotiationMessage::decode(&bytes)
                .expect_err("invalid rejection")
                .failure(),
            Failure::UnexpectedProtocolFrame
        );
    }
}

#[test]
fn selector_uses_highest_common_version_and_deterministic_intersection() {
    let hello = ClientHello::new(
        vec![1],
        vec![Capability::ClipboardText.id()],
        vec![
            Capability::H264Video.id(),
            Capability::RoiVideo.id(),
            0x00FF,
        ],
    )
    .expect("hello");
    let selected = ProtocolNegotiator::select(&hello, &support()).expect("selection");
    assert_eq!(selected.selected_version(), 1);
    assert_eq!(
        selected.capabilities(),
        &[
            Capability::H264Video.id(),
            Capability::RoiVideo.id(),
            Capability::ClipboardText.id(),
        ]
    );
    ProtocolNegotiator::validate_selection(&hello, &support(), &selected).expect("valid reply");
}

#[test]
fn selector_rejects_unknown_unsupported_and_dependency_invalid_requirements() {
    let cases = [
        ClientHello::new(vec![1], vec![0x00FF], vec![]).expect("hello"),
        ClientHello::new(vec![1], vec![Capability::TileCache.id()], vec![]).expect("hello"),
        ClientHello::new(vec![1], vec![Capability::RoiVideo.id()], vec![]).expect("hello"),
    ];
    for hello in cases {
        assert_eq!(
            ProtocolNegotiator::select(&hello, &support()),
            Err(Failure::RequiredProtocolCapabilityMissing)
        );
    }

    let unsupported = ClientHello::new(vec![2], vec![], vec![]).expect("hello");
    assert_eq!(
        ProtocolNegotiator::select(&unsupported, &support()),
        Err(Failure::UnsupportedProtocolVersion)
    );
}

#[test]
fn selector_refuses_downgrades_and_mismatched_capabilities() {
    let hello =
        ClientHello::new(vec![1, 2], vec![], vec![Capability::ClipboardText.id()]).expect("hello");
    let support =
        ProtocolSupport::new(vec![1, 2], vec![Capability::ClipboardText]).expect("support");
    let downgrade =
        ServerSelection::new(1, vec![Capability::ClipboardText.id()]).expect("selection");
    assert_eq!(
        ProtocolNegotiator::validate_selection(&hello, &support, &downgrade),
        Err(Failure::ProtocolDowngradeRefused)
    );

    let mismatch = ServerSelection::new(2, vec![]).expect("selection");
    assert_eq!(
        ProtocolNegotiator::validate_selection(&hello, &support, &mismatch),
        Err(Failure::ProtocolSelectionMismatch)
    );
}

#[test]
fn support_profile_rejects_ambiguous_or_incomplete_server_configuration() {
    assert_eq!(
        ProtocolSupport::new(vec![2, 1], vec![]),
        Err(ProtocolSupportError::VersionsNotCanonical)
    );
    assert_eq!(
        ProtocolSupport::new(
            vec![1],
            vec![Capability::AudioPcmS16Le, Capability::AudioOpus],
        ),
        Err(ProtocolSupportError::MultipleDesktopAudioCodecs)
    );
    assert_eq!(
        ProtocolSupport::new(vec![1], vec![Capability::RoiVideo]),
        Err(ProtocolSupportError::CapabilityDependencyMissing)
    );
}

#[test]
fn failure_registry_is_complete_and_stable() {
    for (index, failure) in Failure::ALL.into_iter().enumerate() {
        assert_eq!(failure.id(), index as u16 + 1);
        assert_eq!(Failure::try_from(failure.id()), Ok(failure));
        assert_eq!(
            NegotiationMessage::decode(&NegotiationMessage::ProtocolReject(failure).encode()),
            Ok(NegotiationMessage::ProtocolReject(failure))
        );
    }
}
