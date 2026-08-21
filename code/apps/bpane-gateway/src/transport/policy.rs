use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::{ClientAccessFlags, ControlMessage, SessionFlags};

use super::negotiation::ConnectionProtocol;
use crate::session_control::{ProjectPolicy, SessionCapabilities};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SessionFileTransportPolicy {
    pub allow_browser_uploads: bool,
    pub allow_browser_downloads: bool,
}

impl SessionFileTransportPolicy {
    pub fn from_project_policy(policy: Option<&ProjectPolicy>) -> Self {
        let Some(policy) = policy else {
            return Self::default();
        };
        Self {
            allow_browser_uploads: policy.allow_browser_uploads,
            allow_browser_downloads: policy.allow_browser_downloads,
        }
    }
}

impl Default for SessionFileTransportPolicy {
    fn default() -> Self {
        Self {
            allow_browser_uploads: true,
            allow_browser_downloads: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SessionTransportPolicy {
    pub capabilities: SessionCapabilities,
    file_transfer: SessionFileTransportPolicy,
}

impl SessionTransportPolicy {
    pub fn from_project_policy_and_capabilities(
        policy: Option<&ProjectPolicy>,
        capabilities: SessionCapabilities,
    ) -> Self {
        Self {
            capabilities,
            file_transfer: SessionFileTransportPolicy::from_project_policy(policy),
        }
    }

    pub fn allow_browser_uploads(&self) -> bool {
        self.capabilities.file_transfer && self.file_transfer.allow_browser_uploads
    }

    pub fn allow_browser_downloads(&self) -> bool {
        self.capabilities.file_transfer && self.file_transfer.allow_browser_downloads
    }

    #[cfg(test)]
    pub fn with_file_transfer_policy(file_transfer: SessionFileTransportPolicy) -> Self {
        Self {
            capabilities: SessionCapabilities::default(),
            file_transfer,
        }
    }

    fn exposes_file_transfer_capability(&self) -> bool {
        self.allow_browser_uploads() && self.allow_browser_downloads()
    }
}

#[cfg(test)]
pub(super) fn adapt_frame_for_client(frame: &Frame, is_owner: bool) -> Frame {
    adapt_frame_for_client_with_policy(frame, is_owner, &SessionTransportPolicy::default())
}

#[cfg(test)]
pub(super) fn adapt_frame_for_client_with_policy(
    frame: &Frame,
    is_owner: bool,
    policy: &SessionTransportPolicy,
) -> Frame {
    adapt_frame_for_client_with_protocol(frame, is_owner, policy, &ConnectionProtocol::Legacy)
}

pub(super) fn adapt_frame_for_client_with_protocol(
    frame: &Frame,
    is_owner: bool,
    policy: &SessionTransportPolicy,
    protocol: &ConnectionProtocol,
) -> Frame {
    let role_adapted = if is_owner
        || frame.channel != ChannelId::Control
        || frame.payload.len() < 3
        || frame.payload[0] != 0x03
    {
        frame.clone()
    } else {
        let mut payload = frame.payload.to_vec();
        let restricted = SessionFlags::CLIPBOARD
            | SessionFlags::FILE_TRANSFER
            | SessionFlags::MICROPHONE
            | SessionFlags::CAMERA
            | SessionFlags::KEYBOARD_LAYOUT;
        payload[2] &= !restricted.bits();
        Frame::new(frame.channel, payload)
    };
    let policy_adapted = adapt_session_ready_for_policy(&role_adapted, policy);
    protocol.normalize_session_ready(&policy_adapted)
}

pub(super) fn adapt_control_message_for_client(
    message: ControlMessage,
    policy: &SessionTransportPolicy,
) -> ControlMessage {
    match message {
        ControlMessage::ClientAccessState {
            mut flags,
            width,
            height,
        } => {
            if !policy.capabilities.browser_input {
                flags |= ClientAccessFlags::VIEW_ONLY;
            }
            if !policy.capabilities.resize {
                flags |= ClientAccessFlags::RESIZE_LOCKED;
            }

            ControlMessage::ClientAccessState {
                flags,
                width,
                height,
            }
        }
        other => other,
    }
}

fn adapt_session_ready_for_policy(frame: &Frame, policy: &SessionTransportPolicy) -> Frame {
    if frame.channel != ChannelId::Control || frame.payload.len() < 3 || frame.payload[0] != 0x03 {
        return frame.clone();
    }

    let mut payload = frame.payload.to_vec();
    let mut restricted = SessionFlags::empty();
    if !policy.capabilities.browser_input {
        restricted |= SessionFlags::KEYBOARD_LAYOUT;
    }
    if !policy.capabilities.clipboard {
        restricted |= SessionFlags::CLIPBOARD;
    }
    if !policy.capabilities.audio {
        restricted |= SessionFlags::AUDIO;
    }
    if !policy.capabilities.microphone {
        restricted |= SessionFlags::MICROPHONE;
    }
    if !policy.capabilities.camera {
        restricted |= SessionFlags::CAMERA;
    }
    if !policy.exposes_file_transfer_capability() {
        restricted |= SessionFlags::FILE_TRANSFER;
    }
    if restricted.is_empty() {
        return frame.clone();
    }

    payload[2] &= !restricted.bits();
    Frame::new(frame.channel, payload)
}

pub(super) fn viewer_can_receive_frame(frame: &Frame) -> bool {
    !matches!(frame.channel, ChannelId::Clipboard | ChannelId::FileDown)
}

pub(super) fn viewer_can_forward_frame(frame: &Frame) -> bool {
    match frame.channel {
        ChannelId::Input
        | ChannelId::Clipboard
        | ChannelId::AudioIn
        | ChannelId::VideoIn
        | ChannelId::FileUp => false,
        ChannelId::Control if !frame.payload.is_empty() && frame.payload[0] == 0x06 => false,
        _ => true,
    }
}

pub(super) fn client_can_receive_frame(
    frame: &Frame,
    is_owner: bool,
    policy: &SessionTransportPolicy,
) -> bool {
    if !is_owner && !viewer_can_receive_frame(frame) {
        return false;
    }

    match frame.channel {
        ChannelId::AudioOut => policy.capabilities.audio,
        ChannelId::Clipboard => policy.capabilities.clipboard,
        ChannelId::FileDown => policy.allow_browser_downloads(),
        _ => true,
    }
}

pub(super) fn client_can_forward_frame(
    frame: &Frame,
    is_owner: bool,
    policy: &SessionTransportPolicy,
) -> bool {
    if !is_owner && !viewer_can_forward_frame(frame) {
        return false;
    }

    match frame.channel {
        ChannelId::Input => policy.capabilities.browser_input,
        ChannelId::Clipboard => policy.capabilities.clipboard,
        ChannelId::AudioIn => policy.capabilities.microphone,
        ChannelId::VideoIn => policy.capabilities.camera,
        ChannelId::FileUp => policy.allow_browser_uploads(),
        _ => true,
    }
}
