use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::SessionFlags;

pub(super) fn adapt_frame_for_client(frame: &Frame, is_owner: bool) -> Frame {
    if is_owner
        || frame.channel != ChannelId::Control
        || frame.payload.len() < 3
        || frame.payload[0] != 0x03
    {
        return frame.clone();
    }

    let mut payload = frame.payload.to_vec();
    let restricted = SessionFlags::CLIPBOARD
        | SessionFlags::FILE_TRANSFER
        | SessionFlags::MICROPHONE
        | SessionFlags::CAMERA
        | SessionFlags::KEYBOARD_LAYOUT;
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
