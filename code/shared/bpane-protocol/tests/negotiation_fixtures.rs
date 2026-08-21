use std::collections::BTreeSet;

use bpane_protocol::{
    ChannelId, ClientHello, Frame, NegotiationMessage, ProtocolCapability, ProtocolFailure,
    ProtocolNegotiator, ProtocolSupport, ServerSelection,
};
use serde::Deserialize;

const EXPECTED_CURRENT_VECTOR_COUNT: usize = 15;
const EXPECTED_NEGOTIATION_VECTOR_COUNT: usize = 41;
const EXPECTED_SELECTION_VECTOR_COUNT: usize = 10;
const CATALOG_JSON: &str = include_str!("fixtures/wire-fixtures.json");

#[derive(Debug, Deserialize)]
struct Catalog {
    schema_version: u8,
    catalog: String,
    vectors: Vec<serde_json::Value>,
    negotiation_vectors: Vec<NegotiationVector>,
    selection_vectors: Vec<SelectionVector>,
}

#[derive(Debug, Deserialize)]
struct NegotiationVector {
    name: String,
    direction: String,
    wire_hex: String,
    expected: NegotiationExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum NegotiationExpectation {
    Valid {
        message: String,
        #[serde(default)]
        versions: Vec<u16>,
        #[serde(default)]
        required_capabilities: Vec<u16>,
        #[serde(default)]
        optional_capabilities: Vec<u16>,
        selected_version: Option<u16>,
        #[serde(default)]
        capabilities: Vec<u16>,
        failure: Option<String>,
    },
    Invalid {
        error: String,
    },
}

#[derive(Debug, Deserialize)]
struct SelectionVector {
    name: String,
    operation: SelectionOperation,
    hello: RawHello,
    support: RawSupport,
    selection: Option<RawSelection>,
    expected: SelectionExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SelectionOperation {
    Select,
    ValidateSelection,
}

#[derive(Debug, Deserialize)]
struct RawHello {
    versions: Vec<u16>,
    required_capabilities: Vec<u16>,
    optional_capabilities: Vec<u16>,
}

#[derive(Debug, Deserialize)]
struct RawSupport {
    versions: Vec<u16>,
    capabilities: Vec<u16>,
}

#[derive(Debug, Deserialize)]
struct RawSelection {
    selected_version: u16,
    capabilities: Vec<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum SelectionExpectation {
    Selected {
        selected_version: u16,
        capabilities: Vec<u16>,
    },
    Accepted,
    Rejected {
        error: String,
    },
}

#[test]
fn every_negotiation_message_vector_has_matching_bytes_and_outcome() {
    let catalog = parse_catalog(CATALOG_JSON).expect("supported catalog");
    assert_eq!(catalog.vectors.len(), EXPECTED_CURRENT_VECTOR_COUNT);
    assert_eq!(
        catalog.negotiation_vectors.len(),
        EXPECTED_NEGOTIATION_VECTOR_COUNT
    );
    assert_unique_names(&catalog);

    for vector in &catalog.negotiation_vectors {
        assert!(matches!(
            vector.direction.as_str(),
            "client_to_server" | "server_to_client" | "bidirectional"
        ));
        let wire = hex_to_bytes(&vector.wire_hex);
        let (frame, consumed) = Frame::decode(&wire).expect("complete negotiation frame");
        assert_eq!(consumed, wire.len(), "vector {}", vector.name);
        assert_eq!(frame.channel, ChannelId::Control, "vector {}", vector.name);

        match (&vector.expected, NegotiationMessage::decode(&frame.payload)) {
            (expected @ NegotiationExpectation::Valid { .. }, Ok(message)) => {
                assert_valid_message(expected, &message, &vector.name);
                assert_eq!(
                    message.to_frame().encode().as_ref(),
                    wire,
                    "vector {} exact encoder bytes",
                    vector.name
                );
            }
            (NegotiationExpectation::Invalid { error }, Err(actual)) => {
                assert_eq!(actual.failure().code(), error, "vector {}", vector.name);
            }
            (expected, actual) => panic!(
                "negotiation vector {} mismatch: expected {expected:?}, got {actual:?}",
                vector.name
            ),
        }
    }
}

#[test]
fn every_selection_vector_has_the_fixed_cross_language_outcome() {
    let catalog = parse_catalog(CATALOG_JSON).expect("supported catalog");
    assert_eq!(
        catalog.selection_vectors.len(),
        EXPECTED_SELECTION_VECTOR_COUNT
    );

    for vector in &catalog.selection_vectors {
        let hello = ClientHello::new(
            vector.hello.versions.clone(),
            vector.hello.required_capabilities.clone(),
            vector.hello.optional_capabilities.clone(),
        )
        .expect("valid vector hello");
        let support = ProtocolSupport::new(
            vector.support.versions.clone(),
            vector
                .support
                .capabilities
                .iter()
                .map(|id| ProtocolCapability::try_from(*id).expect("known support capability"))
                .collect(),
        )
        .expect("valid vector support");

        let actual = match vector.operation {
            SelectionOperation::Select => ProtocolNegotiator::select(&hello, &support),
            SelectionOperation::ValidateSelection => {
                let raw = vector.selection.as_ref().expect("selection required");
                let selection =
                    ServerSelection::new(raw.selected_version, raw.capabilities.clone())
                        .expect("valid vector selection");
                ProtocolNegotiator::validate_selection(&hello, &support, &selection)
                    .map(|()| selection)
            }
        };
        assert_selection_outcome(&vector.expected, actual, &vector.name);
    }
}

#[test]
fn fixture_schema_and_unknown_outcomes_fail_closed() {
    let wrong_schema = CATALOG_JSON.replacen("\"schema_version\": 2", "\"schema_version\": 3", 1);
    assert!(parse_catalog(&wrong_schema).is_err());

    let unknown_outcome =
        CATALOG_JSON.replacen("\"outcome\": \"accepted\"", "\"outcome\": \"unknown\"", 1);
    assert!(parse_catalog(&unknown_outcome).is_err());
}

fn parse_catalog(input: &str) -> Result<Catalog, &'static str> {
    let catalog: Catalog = serde_json::from_str(input).map_err(|_| "invalid_catalog")?;
    if catalog.schema_version != 2 || catalog.catalog != "browserpane-v1-conformance" {
        return Err("unsupported_catalog_schema");
    }
    Ok(catalog)
}

fn assert_unique_names(catalog: &Catalog) {
    let mut names = BTreeSet::new();
    for vector in &catalog.vectors {
        let name = vector
            .get("name")
            .and_then(serde_json::Value::as_str)
            .expect("current vector name");
        assert!(
            names.insert(name.to_owned()),
            "duplicate vector name {name}"
        );
    }
    for name in catalog
        .negotiation_vectors
        .iter()
        .map(|vector| &vector.name)
        .chain(catalog.selection_vectors.iter().map(|vector| &vector.name))
    {
        assert!(names.insert(name.clone()), "duplicate vector name {name}");
    }
}

fn assert_valid_message(
    expected: &NegotiationExpectation,
    actual: &NegotiationMessage,
    name: &str,
) {
    match (expected, actual) {
        (
            NegotiationExpectation::Valid {
                message,
                versions,
                required_capabilities,
                optional_capabilities,
                ..
            },
            NegotiationMessage::ClientHello(hello),
        ) => {
            assert_eq!(message, "client_hello");
            assert_eq!(hello.versions(), versions, "vector {name}");
            assert_eq!(hello.required_capabilities(), required_capabilities);
            assert_eq!(hello.optional_capabilities(), optional_capabilities);
        }
        (
            NegotiationExpectation::Valid {
                message,
                selected_version,
                capabilities,
                ..
            },
            NegotiationMessage::ServerSelection(selection),
        ) => {
            assert_eq!(message, "server_selection");
            assert_eq!(Some(selection.selected_version()), *selected_version);
            assert_eq!(selection.capabilities(), capabilities);
        }
        (
            NegotiationExpectation::Valid {
                message, failure, ..
            },
            NegotiationMessage::ProtocolReject(actual_failure),
        ) => {
            assert_eq!(message, "protocol_reject");
            assert_eq!(Some(actual_failure.code()), failure.as_deref());
        }
        _ => panic!("valid negotiation vector {name} has mismatched fields"),
    }
}

fn assert_selection_outcome(
    expected: &SelectionExpectation,
    actual: Result<ServerSelection, ProtocolFailure>,
    name: &str,
) {
    match (expected, actual) {
        (
            SelectionExpectation::Selected {
                selected_version,
                capabilities,
            },
            Ok(selection),
        ) => {
            assert_eq!(selection.selected_version(), *selected_version, "{name}");
            assert_eq!(selection.capabilities(), capabilities, "{name}");
        }
        (SelectionExpectation::Accepted, Ok(_)) => {}
        (SelectionExpectation::Rejected { error }, Err(failure)) => {
            assert_eq!(failure.code(), error, "{name}");
        }
        (expected, actual) => panic!("selection vector {name}: {expected:?} != {actual:?}"),
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    assert!(hex
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("hex utf8"), 16).expect("hex byte")
        })
        .collect()
}
