use alloc::vec::Vec;
use core::fmt;

use super::model::{
    is_canonical, ClientHello, ProtocolCapability, ProtocolFailure, ServerSelection,
    MAX_PROTOCOL_CAPABILITIES, MAX_PROTOCOL_VERSIONS,
};

/// Invalid local protocol support configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSupportError {
    VersionsNotCanonical,
    CapabilitiesNotCanonical,
    CapabilityDependencyMissing,
    MultipleDesktopAudioCodecs,
}

impl fmt::Display for ProtocolSupportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::VersionsNotCanonical => "protocol support versions are not canonical",
            Self::CapabilitiesNotCanonical => "protocol support capabilities are not canonical",
            Self::CapabilityDependencyMissing => {
                "protocol support capability dependency is missing"
            }
            Self::MultipleDesktopAudioCodecs => {
                "protocol support contains multiple desktop audio codecs"
            }
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProtocolSupportError {}

/// A validated local protocol-version and capability profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSupport {
    versions: Vec<u16>,
    capabilities: Vec<ProtocolCapability>,
}

impl ProtocolSupport {
    /// Construct a canonical local support profile.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolSupportError`] when versions or capabilities are not
    /// bounded and canonical, a dependency is absent, or multiple mutually
    /// exclusive desktop-audio codecs are configured.
    pub fn new(
        versions: Vec<u16>,
        capabilities: Vec<ProtocolCapability>,
    ) -> Result<Self, ProtocolSupportError> {
        if !(1..=MAX_PROTOCOL_VERSIONS).contains(&versions.len())
            || versions.contains(&0)
            || !is_canonical(&versions)
        {
            return Err(ProtocolSupportError::VersionsNotCanonical);
        }
        if capabilities.len() > MAX_PROTOCOL_CAPABILITIES
            || !capabilities.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(ProtocolSupportError::CapabilitiesNotCanonical);
        }
        if capabilities
            .iter()
            .filter(|capability| capability.is_desktop_audio())
            .count()
            > 1
        {
            return Err(ProtocolSupportError::MultipleDesktopAudioCodecs);
        }
        if capabilities.iter().any(|capability| {
            capability
                .dependency()
                .is_some_and(|dependency| !capabilities.contains(&dependency))
        }) {
            return Err(ProtocolSupportError::CapabilityDependencyMissing);
        }
        Ok(Self {
            versions,
            capabilities,
        })
    }

    /// Return supported versions in ascending order.
    pub fn versions(&self) -> &[u16] {
        &self.versions
    }

    /// Return supported capabilities in ascending numeric order.
    pub fn capabilities(&self) -> &[ProtocolCapability] {
        &self.capabilities
    }

    fn contains(&self, capability: ProtocolCapability) -> bool {
        self.capabilities.binary_search(&capability).is_ok()
    }
}

/// Pure highest-common-version and capability selection.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProtocolNegotiator;

impl ProtocolNegotiator {
    /// Select the highest common version and deterministic capability set.
    ///
    /// Unknown optional capabilities are ignored. Unknown, unsupported, or
    /// dependency-invalid required capabilities are rejected.
    ///
    /// # Errors
    ///
    /// Returns the stable protocol rejection that should be sent to the peer.
    pub fn select(
        hello: &ClientHello,
        support: &ProtocolSupport,
    ) -> Result<ServerSelection, ProtocolFailure> {
        let selected_version = support
            .versions()
            .iter()
            .rev()
            .find(|version| hello.versions().binary_search(version).is_ok())
            .copied()
            .ok_or(ProtocolFailure::UnsupportedProtocolVersion)?;

        for id in hello.required_capabilities() {
            let capability = ProtocolCapability::try_from(*id)
                .map_err(|_| ProtocolFailure::RequiredProtocolCapabilityMissing)?;
            if !support.contains(capability)
                || capability.dependency().is_some_and(|dependency| {
                    !hello.offers_capability(dependency) || !support.contains(dependency)
                })
            {
                return Err(ProtocolFailure::RequiredProtocolCapabilityMissing);
            }
        }

        let mut selected = hello
            .required_capabilities()
            .iter()
            .chain(hello.optional_capabilities())
            .filter_map(|id| ProtocolCapability::try_from(*id).ok())
            .filter(|capability| support.contains(*capability))
            .collect::<Vec<_>>();
        selected.sort_unstable();
        selected.dedup();
        let selected_with_dependencies = selected.clone();
        selected.retain(|capability| {
            capability
                .dependency()
                .is_none_or(|dependency| selected_with_dependencies.contains(&dependency))
        });

        ServerSelection::new(
            selected_version,
            selected.into_iter().map(ProtocolCapability::id).collect(),
        )
        .map_err(|error| error.failure())
    }

    /// Validate a server reply against the unique expected selection.
    ///
    /// # Errors
    ///
    /// Returns `protocol_downgrade_refused` when the server selects a lower
    /// common version than the highest one. Other differences return
    /// `protocol_selection_mismatch`.
    pub fn validate_selection(
        hello: &ClientHello,
        support: &ProtocolSupport,
        selection: &ServerSelection,
    ) -> Result<(), ProtocolFailure> {
        let expected = Self::select(hello, support)?;
        let selected_is_common = hello
            .versions()
            .binary_search(&selection.selected_version())
            .is_ok()
            && support
                .versions()
                .binary_search(&selection.selected_version())
                .is_ok();
        if selected_is_common && selection.selected_version() < expected.selected_version() {
            return Err(ProtocolFailure::ProtocolDowngradeRefused);
        }
        if selection != &expected {
            return Err(ProtocolFailure::ProtocolSelectionMismatch);
        }
        Ok(())
    }
}
