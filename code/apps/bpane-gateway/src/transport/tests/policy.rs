use super::*;

#[test]
fn adapt_frame_for_client_strips_viewer_only_capabilities() {
    let frame = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::AUDIO
            | SessionFlags::CLIPBOARD
            | SessionFlags::FILE_TRANSFER
            | SessionFlags::MICROPHONE
            | SessionFlags::CAMERA
            | SessionFlags::KEYBOARD_LAYOUT,
    }
    .to_frame();

    let adapted = adapt_frame_for_client(&frame, false);

    assert_eq!(adapted.payload[0], 0x03);
    assert_ne!(adapted.payload[2] & SessionFlags::AUDIO.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CLIPBOARD.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::FILE_TRANSFER.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::MICROPHONE.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CAMERA.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::KEYBOARD_LAYOUT.bits(), 0);
}

#[test]
fn adapt_frame_for_client_leaves_owner_flags_unchanged() {
    let frame = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
    }
    .to_frame();

    assert_eq!(adapt_frame_for_client(&frame, true), frame);
}

#[test]
fn adapt_frame_for_client_strips_disabled_owner_capabilities() {
    let frame = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::AUDIO
            | SessionFlags::CLIPBOARD
            | SessionFlags::FILE_TRANSFER
            | SessionFlags::MICROPHONE
            | SessionFlags::CAMERA
            | SessionFlags::KEYBOARD_LAYOUT,
    }
    .to_frame();
    let policy = SessionTransportPolicy::from_project_policy_and_capabilities(
        None,
        SessionCapabilities {
            browser_input: false,
            clipboard: false,
            audio: true,
            microphone: false,
            camera: false,
            file_transfer: true,
            resize: true,
        },
    );

    let adapted = adapt_frame_for_client_with_policy(&frame, true, &policy);

    assert_eq!(adapted.payload[0], 0x03);
    assert_ne!(adapted.payload[2] & SessionFlags::AUDIO.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CLIPBOARD.bits(), 0);
    assert_ne!(adapted.payload[2] & SessionFlags::FILE_TRANSFER.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::MICROPHONE.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CAMERA.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::KEYBOARD_LAYOUT.bits(), 0);
}

#[test]
fn adapt_control_message_for_client_adds_disabled_access_flags() {
    let policy = SessionTransportPolicy::from_project_policy_and_capabilities(
        None,
        SessionCapabilities {
            browser_input: false,
            clipboard: true,
            audio: true,
            microphone: true,
            camera: true,
            file_transfer: true,
            resize: false,
        },
    );

    let adapted = adapt_control_message_for_client(
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::empty(),
            width: 0,
            height: 0,
        },
        &policy,
    );

    assert_eq!(
        adapted,
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
            width: 0,
            height: 0,
        }
    );
}

#[test]
fn viewer_can_receive_frame_blocks_clipboard_and_download() {
    let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
    let download = Frame::new(ChannelId::FileDown, vec![0x01]);
    let video = Frame::new(ChannelId::Video, vec![0x00]);

    assert!(!viewer_can_receive_frame(&clipboard));
    assert!(!viewer_can_receive_frame(&download));
    assert!(viewer_can_receive_frame(&video));
}

#[test]
fn client_can_forward_frame_blocks_disabled_owner_capability_channels() {
    let policy = SessionTransportPolicy::from_project_policy_and_capabilities(
        None,
        SessionCapabilities {
            browser_input: false,
            clipboard: false,
            audio: true,
            microphone: false,
            camera: false,
            file_transfer: false,
            resize: true,
        },
    );

    assert!(!client_can_forward_frame(
        &Frame::new(ChannelId::Input, vec![0x01]),
        true,
        &policy,
    ));
    assert!(!client_can_forward_frame(
        &Frame::new(ChannelId::Clipboard, vec![0x01]),
        true,
        &policy,
    ));
    assert!(!client_can_forward_frame(
        &Frame::new(ChannelId::AudioIn, vec![0x01]),
        true,
        &policy,
    ));
    assert!(!client_can_forward_frame(
        &Frame::new(ChannelId::VideoIn, vec![0x01]),
        true,
        &policy,
    ));
    assert!(!client_can_forward_frame(
        &Frame::new(ChannelId::FileUp, vec![0x01]),
        true,
        &policy,
    ));
    assert!(client_can_forward_frame(
        &Frame::new(ChannelId::Control, vec![0x05]),
        true,
        &policy,
    ));
}

#[test]
fn viewer_can_forward_frame_blocks_interactive_channels() {
    let input = Frame::new(ChannelId::Input, vec![0x01]);
    let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
    let audio_in = Frame::new(ChannelId::AudioIn, vec![0x01]);
    let video_in = Frame::new(ChannelId::VideoIn, vec![0x01]);
    let file_up = Frame::new(ChannelId::FileUp, vec![0x01]);
    let layout = Frame::new(ChannelId::Control, vec![0x06, 0x00]);
    let pong = Frame::new(ChannelId::Control, vec![0x05]);

    assert!(!viewer_can_forward_frame(&input));
    assert!(!viewer_can_forward_frame(&clipboard));
    assert!(!viewer_can_forward_frame(&audio_in));
    assert!(!viewer_can_forward_frame(&video_in));
    assert!(!viewer_can_forward_frame(&file_up));
    assert!(!viewer_can_forward_frame(&layout));
    assert!(viewer_can_forward_frame(&pong));
}
