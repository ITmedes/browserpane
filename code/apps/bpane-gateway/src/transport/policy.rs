use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::SessionFlags;

use crate::session_control::ProjectPolicy;

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

    fn exposes_file_transfer_capability(self) -> bool {
        self.allow_browser_uploads && self.allow_browser_downloads
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

pub(super) fn adapt_frame_for_client(frame: &Frame, is_owner: bool) -> Frame {
    adapt_frame_for_client_with_file_policy(frame, is_owner, SessionFileTransportPolicy::default())
}

pub(super) fn adapt_frame_for_client_with_file_policy(
    frame: &Frame,
    is_owner: bool,
    file_policy: SessionFileTransportPolicy,
) -> Frame {
    if is_owner
        || frame.channel != ChannelId::Control
        || frame.payload.len() < 3
        || frame.payload[0] != 0x03
    {
        return adapt_session_ready_for_file_policy(frame, file_policy);
    }

    let mut payload = frame.payload.to_vec();
    let restricted = SessionFlags::CLIPBOARD
        | SessionFlags::FILE_TRANSFER
        | SessionFlags::MICROPHONE
        | SessionFlags::CAMERA
        | SessionFlags::KEYBOARD_LAYOUT;
    payload[2] &= !restricted.bits();
    adapt_session_ready_for_file_policy(&Frame::new(frame.channel, payload), file_policy)
}

fn adapt_session_ready_for_file_policy(
    frame: &Frame,
    file_policy: SessionFileTransportPolicy,
) -> Frame {
    if file_policy.exposes_file_transfer_capability()
        || frame.channel != ChannelId::Control
        || frame.payload.len() < 3
        || frame.payload[0] != 0x03
    {
        return frame.clone();
    }

    let mut payload = frame.payload.to_vec();
    payload[2] &= !SessionFlags::FILE_TRANSFER.bits();
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
