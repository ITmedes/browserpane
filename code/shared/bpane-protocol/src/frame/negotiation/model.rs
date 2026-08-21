use alloc::vec::Vec;
use core::fmt;

/// Maximum number of protocol versions in a client hello.
pub const MAX_PROTOCOL_VERSIONS: usize = 8;
/// Maximum total number of required and optional capabilities in a hello.
pub const MAX_PROTOCOL_CAPABILITIES: usize = 64;

/// A protocol-v1 capability identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum ProtocolCapability {
    TileZstd = 0x0001,
    TileCache = 0x0002,
    TileScroll = 0x0003,
    H264Video = 0x0004,
    RoiVideo = 0x0005,
    AudioPcmS16Le = 0x0006,
    AudioAdpcmImaStereo = 0x0007,
    AudioOpus = 0x0008,
    MicrophoneOpus = 0x0009,
    CameraH264AnnexB = 0x000A,
    ClipboardText = 0x000B,
    FileTransfer = 0x000C,
    ExtendedKeyboard = 0x000D,
    ClientAccessState = 0x000E,
}

impl ProtocolCapability {
    /// All capabilities defined by protocol v1, in numeric order.
    pub const ALL: [Self; 14] = [
        Self::TileZstd,
        Self::TileCache,
        Self::TileScroll,
        Self::H264Video,
        Self::RoiVideo,
        Self::AudioPcmS16Le,
        Self::AudioAdpcmImaStereo,
        Self::AudioOpus,
        Self::MicrophoneOpus,
        Self::CameraH264AnnexB,
        Self::ClipboardText,
        Self::FileTransfer,
        Self::ExtendedKeyboard,
        Self::ClientAccessState,
    ];

    /// Return the numeric wire identifier.
    pub const fn id(self) -> u16 {
        self as u16
    }

    /// Return the stable wire-contract name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::TileZstd => "tile_zstd",
            Self::TileCache => "tile_cache",
            Self::TileScroll => "tile_scroll",
            Self::H264Video => "h264_video",
            Self::RoiVideo => "roi_video",
            Self::AudioPcmS16Le => "audio_pcm_s16le",
            Self::AudioAdpcmImaStereo => "audio_adpcm_ima_stereo",
            Self::AudioOpus => "audio_opus",
            Self::MicrophoneOpus => "microphone_opus",
            Self::CameraH264AnnexB => "camera_h264_annex_b",
            Self::ClipboardText => "clipboard_text",
            Self::FileTransfer => "file_transfer",
            Self::ExtendedKeyboard => "extended_keyboard",
            Self::ClientAccessState => "client_access_state",
        }
    }

    pub(crate) const fn dependency(self) -> Option<Self> {
        match self {
            Self::RoiVideo => Some(Self::H264Video),
            _ => None,
        }
    }

    pub(crate) const fn is_desktop_audio(self) -> bool {
        matches!(
            self,
            Self::AudioPcmS16Le | Self::AudioAdpcmImaStereo | Self::AudioOpus
        )
    }
}

impl TryFrom<u16> for ProtocolCapability {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.id() == value)
            .ok_or(value)
    }
}

impl From<ProtocolCapability> for u16 {
    fn from(value: ProtocolCapability) -> Self {
        value.id()
    }
}

/// Stable protocol negotiation and connection failure identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ProtocolFailure {
    UnsupportedProtocolVersion = 0x0001,
    RequiredProtocolCapabilityMissing = 0x0002,
    MalformedProtocolHello = 0x0003,
    ProtocolDowngradeRefused = 0x0004,
    ProtocolHandshakeTimeout = 0x0005,
    ProtocolSelectionMismatch = 0x0006,
    UnexpectedProtocolFrame = 0x0007,
    ProtocolFrameTooLarge = 0x0008,
    ProtocolPendingBufferLimit = 0x0009,
}

impl ProtocolFailure {
    /// All failure values defined by protocol v1, in numeric order.
    pub const ALL: [Self; 9] = [
        Self::UnsupportedProtocolVersion,
        Self::RequiredProtocolCapabilityMissing,
        Self::MalformedProtocolHello,
        Self::ProtocolDowngradeRefused,
        Self::ProtocolHandshakeTimeout,
        Self::ProtocolSelectionMismatch,
        Self::UnexpectedProtocolFrame,
        Self::ProtocolFrameTooLarge,
        Self::ProtocolPendingBufferLimit,
    ];

    /// Return the numeric wire identifier.
    pub const fn id(self) -> u16 {
        self as u16
    }

    /// Return the stable failure code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedProtocolVersion => "unsupported_protocol_version",
            Self::RequiredProtocolCapabilityMissing => "required_protocol_capability_missing",
            Self::MalformedProtocolHello => "malformed_protocol_hello",
            Self::ProtocolDowngradeRefused => "protocol_downgrade_refused",
            Self::ProtocolHandshakeTimeout => "protocol_handshake_timeout",
            Self::ProtocolSelectionMismatch => "protocol_selection_mismatch",
            Self::UnexpectedProtocolFrame => "unexpected_protocol_frame",
            Self::ProtocolFrameTooLarge => "protocol_frame_too_large",
            Self::ProtocolPendingBufferLimit => "protocol_pending_buffer_limit",
        }
    }
}

impl TryFrom<u16> for ProtocolFailure {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|failure| failure.id() == value)
            .ok_or(value)
    }
}

/// A bounded codec failure with a stable, payload-free protocol outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiationCodecError {
    failure: ProtocolFailure,
}

impl NegotiationCodecError {
    pub(crate) const fn new(failure: ProtocolFailure) -> Self {
        Self { failure }
    }

    /// Return the stable protocol failure represented by this error.
    pub const fn failure(self) -> ProtocolFailure {
        self.failure
    }
}

impl fmt::Display for NegotiationCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.failure.code())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NegotiationCodecError {}

/// A validated client protocol offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientHello {
    versions: Vec<u16>,
    required_capabilities: Vec<u16>,
    optional_capabilities: Vec<u16>,
}

impl ClientHello {
    /// Construct a canonical, bounded client hello.
    ///
    /// Unknown capability identifiers are retained here so selection can
    /// reject unknown required values and ignore unknown optional values.
    ///
    /// # Errors
    ///
    /// Returns `malformed_protocol_hello` if any list is out of bounds,
    /// unsorted, duplicated, overlapping, or contains protocol version zero.
    pub fn new(
        versions: Vec<u16>,
        required_capabilities: Vec<u16>,
        optional_capabilities: Vec<u16>,
    ) -> Result<Self, NegotiationCodecError> {
        validate_client_hello(&versions, &required_capabilities, &optional_capabilities)?;
        Ok(Self {
            versions,
            required_capabilities,
            optional_capabilities,
        })
    }

    /// Return the ascending, unique offered versions.
    pub fn versions(&self) -> &[u16] {
        &self.versions
    }

    /// Return the ascending, unique required capability IDs.
    pub fn required_capabilities(&self) -> &[u16] {
        &self.required_capabilities
    }

    /// Return the ascending, unique optional capability IDs.
    pub fn optional_capabilities(&self) -> &[u16] {
        &self.optional_capabilities
    }

    pub(crate) fn offers_capability(&self, capability: ProtocolCapability) -> bool {
        let id = capability.id();
        self.required_capabilities.binary_search(&id).is_ok()
            || self.optional_capabilities.binary_search(&id).is_ok()
    }
}

/// A validated server protocol selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSelection {
    selected_version: u16,
    capabilities: Vec<u16>,
}

impl ServerSelection {
    /// Construct a canonical protocol selection.
    ///
    /// # Errors
    ///
    /// Returns `protocol_selection_mismatch` for version zero, an unbounded or
    /// non-canonical list, an unknown capability, an unmet dependency, or more
    /// than one selected desktop-audio codec.
    pub fn new(
        selected_version: u16,
        capabilities: Vec<u16>,
    ) -> Result<Self, NegotiationCodecError> {
        validate_server_selection(selected_version, &capabilities)?;
        Ok(Self {
            selected_version,
            capabilities,
        })
    }

    /// Return the selected protocol version.
    pub const fn selected_version(&self) -> u16 {
        self.selected_version
    }

    /// Return the ascending, unique selected capability IDs.
    pub fn capabilities(&self) -> &[u16] {
        &self.capabilities
    }
}

/// A protocol negotiation control payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationMessage {
    ClientHello(ClientHello),
    ServerSelection(ServerSelection),
    ProtocolReject(ProtocolFailure),
}

pub(crate) fn validate_client_hello(
    versions: &[u16],
    required: &[u16],
    optional: &[u16],
) -> Result<(), NegotiationCodecError> {
    let capability_total = required.len().checked_add(optional.len());
    let valid = (1..=MAX_PROTOCOL_VERSIONS).contains(&versions.len())
        && required.len() <= MAX_PROTOCOL_CAPABILITIES
        && optional.len() <= MAX_PROTOCOL_CAPABILITIES
        && capability_total.is_some_and(|total| total <= MAX_PROTOCOL_CAPABILITIES)
        && versions.iter().all(|version| *version != 0)
        && is_canonical(versions)
        && is_canonical(required)
        && is_canonical(optional)
        && are_disjoint(required, optional);
    if valid {
        Ok(())
    } else {
        Err(NegotiationCodecError::new(
            ProtocolFailure::MalformedProtocolHello,
        ))
    }
}

pub(crate) fn validate_server_selection(
    selected_version: u16,
    capabilities: &[u16],
) -> Result<(), NegotiationCodecError> {
    let structure_valid = selected_version != 0
        && capabilities.len() <= MAX_PROTOCOL_CAPABILITIES
        && is_canonical(capabilities);
    let mut desktop_audio_count = 0;
    let values_valid = structure_valid
        && capabilities.iter().all(|id| {
            ProtocolCapability::try_from(*id).is_ok_and(|capability| {
                desktop_audio_count += usize::from(capability.is_desktop_audio());
                capability
                    .dependency()
                    .is_none_or(|dependency| capabilities.binary_search(&dependency.id()).is_ok())
            })
        });
    let valid = values_valid && desktop_audio_count <= 1;
    if valid {
        Ok(())
    } else {
        Err(NegotiationCodecError::new(
            ProtocolFailure::ProtocolSelectionMismatch,
        ))
    }
}

pub(crate) fn is_canonical(values: &[u16]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn are_disjoint(left: &[u16], right: &[u16]) -> bool {
    !left.iter().any(|value| right.binary_search(value).is_ok())
}
