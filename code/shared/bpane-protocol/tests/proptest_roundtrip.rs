use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, Message};
use bpane_protocol::*;
use proptest::prelude::*;

fn arb_channel_id() -> impl Strategy<Value = ChannelId> {
    prop_oneof![
        Just(ChannelId::Video),
        Just(ChannelId::AudioOut),
        Just(ChannelId::AudioIn),
        Just(ChannelId::VideoIn),
        Just(ChannelId::Input),
        Just(ChannelId::Cursor),
        Just(ChannelId::Clipboard),
        Just(ChannelId::FileUp),
        Just(ChannelId::FileDown),
        Just(ChannelId::Control),
    ]
}

fn arb_control_message() -> impl Strategy<Value = ControlMessage> {
    prop_oneof![
        (any::<u16>(), any::<u16>()).prop_map(|(w, h)| ControlMessage::ResolutionRequest {
            width: w,
            height: h
        }),
        (any::<u16>(), any::<u16>()).prop_map(|(w, h)| ControlMessage::ResolutionAck {
            width: w,
            height: h
        }),
        (any::<u8>(), any::<u8>()).prop_map(|(v, f)| ControlMessage::SessionReady {
            version: v,
            flags: SessionFlags::new(f)
        }),
        (any::<u32>(), any::<u64>()).prop_map(|(s, t)| ControlMessage::Ping {
            seq: s,
            timestamp_ms: t
        }),
        (any::<u32>(), any::<u64>()).prop_map(|(s, t)| ControlMessage::Pong {
            seq: s,
            timestamp_ms: t
        }),
        any::<[u8; 32]>()
            .prop_map(|layout_hint| ControlMessage::KeyboardLayoutInfo { layout_hint }),
        any::<u32>().prop_map(|bps| ControlMessage::BitrateHint { target_bps: bps }),
        (any::<u16>(), any::<u16>()).prop_map(|(w, h)| ControlMessage::ResolutionLocked {
            width: w,
            height: h
        }),
    ]
}

fn arb_input_message() -> impl Strategy<Value = InputMessage> {
    prop_oneof![
        (any::<u16>(), any::<u16>()).prop_map(|(x, y)| InputMessage::MouseMove { x, y }),
        (
            prop_oneof![
                Just(MouseButton::Left),
                Just(MouseButton::Middle),
                Just(MouseButton::Right),
                Just(MouseButton::Back),
                Just(MouseButton::Forward),
            ],
            any::<bool>(),
            any::<u16>(),
            any::<u16>()
        )
            .prop_map(|(b, d, x, y)| {
                InputMessage::MouseButton {
                    button: b,
                    down: d,
                    x,
                    y,
                }
            }),
        (any::<i16>(), any::<i16>()).prop_map(|(dx, dy)| InputMessage::MouseScroll { dx, dy }),
        (any::<u32>(), any::<bool>(), any::<u8>()).prop_map(|(k, d, m)| InputMessage::KeyEvent {
            keycode: k,
            down: d,
            modifiers: Modifiers::from(m),
        }),
        (any::<u32>(), any::<bool>(), any::<u8>(), any::<u32>()).prop_map(|(k, d, m, c)| {
            InputMessage::KeyEventEx {
                keycode: k,
                down: d,
                modifiers: Modifiers::from(m),
                key_char: c,
            }
        },),
    ]
}

fn arb_cursor_message() -> impl Strategy<Value = CursorMessage> {
    prop_oneof![
        (any::<u16>(), any::<u16>()).prop_map(|(x, y)| CursorMessage::CursorMove { x, y }),
        (1..64u16, 1..64u16, any::<u8>(), any::<u8>()).prop_flat_map(|(w, h, hx, hy)| {
            let data_len = w as usize * h as usize * 4;
            proptest::collection::vec(any::<u8>(), data_len).prop_map(move |data| {
                CursorMessage::CursorShape {
                    width: w,
                    height: h,
                    hotspot_x: hx,
                    hotspot_y: hy,
                    data,
                }
            })
        }),
    ]
}

fn arb_clipboard_message() -> impl Strategy<Value = ClipboardMessage> {
    proptest::collection::vec(any::<u8>(), 0..1024)
        .prop_map(|content| ClipboardMessage::Text { content })
}

fn arb_file_message() -> impl Strategy<Value = FileMessage> {
    prop_oneof![
        (any::<u32>(), any::<u64>())
            .prop_map(|(id, size)| { FileMessage::header(id, [0u8; 256], size, [0u8; 64]) }),
        (
            any::<u32>(),
            any::<u32>(),
            proptest::collection::vec(any::<u8>(), 0..4096)
        )
            .prop_map(|(id, seq, data)| FileMessage::chunk(id, seq, data)),
        any::<u32>().prop_map(FileMessage::complete),
    ]
}

fn arb_tile_info() -> impl Strategy<Value = Option<VideoTileInfo>> {
    prop_oneof![
        Just(None),
        (
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
        )
            .prop_map(|(tx, ty, tw, th, sw, sh)| Some(VideoTileInfo {
                tile_x: tx,
                tile_y: ty,
                tile_w: tw,
                tile_h: th,
                screen_w: sw,
                screen_h: sh,
            })),
    ]
}

fn arb_video_datagram() -> impl Strategy<Value = VideoDatagram> {
    (
        any::<u32>(),
        any::<u16>(),
        any::<u16>(),
        any::<bool>(),
        any::<u64>(),
        proptest::collection::vec(any::<u8>(), 0..2048),
        arb_tile_info(),
    )
        .prop_map(
            |(nal_id, fragment_seq, fragment_total, is_keyframe, pts_us, data, tile_info)| {
                VideoDatagram {
                    nal_id,
                    fragment_seq,
                    fragment_total,
                    is_keyframe,
                    pts_us,
                    data,
                    tile_info,
                }
            },
        )
}

fn arb_audio_frame() -> impl Strategy<Value = AudioFrame> {
    (
        any::<u32>(),
        any::<u64>(),
        proptest::collection::vec(any::<u8>(), 0..1024),
    )
        .prop_map(|(seq, timestamp_us, data)| AudioFrame {
            seq,
            timestamp_us,
            data,
        })
}

fn arb_tile_message() -> impl Strategy<Value = TileMessage> {
    prop_oneof![
        (
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>()
        )
            .prop_map(|(ts, c, r, sw, sh)| TileMessage::GridConfig {
                tile_size: ts,
                cols: c,
                rows: r,
                screen_w: sw,
                screen_h: sh,
            }),
        (any::<u16>(), any::<u16>(), any::<u64>()).prop_map(|(c, r, h)| TileMessage::CacheHit {
            col: c,
            row: r,
            hash: h
        }),
        (any::<u32>(), any::<u16>(), any::<u16>(), any::<u64>()).prop_map(|(fs, c, r, h)| {
            TileMessage::CacheMiss {
                frame_seq: fs,
                col: c,
                row: r,
                hash: h,
            }
        }),
        (any::<u16>(), any::<u16>(), any::<u32>()).prop_map(|(c, r, rgba)| TileMessage::Fill {
            col: c,
            row: r,
            rgba
        }),
        (
            any::<u16>(),
            any::<u16>(),
            any::<u64>(),
            proptest::collection::vec(any::<u8>(), 0..512),
        )
            .prop_map(|(c, r, h, d)| TileMessage::Qoi {
                col: c,
                row: r,
                hash: h,
                data: d,
            }),
        (
            any::<u16>(),
            any::<u16>(),
            any::<u64>(),
            proptest::collection::vec(any::<u8>(), 0..512),
        )
            .prop_map(|(c, r, h, d)| TileMessage::Zstd {
                col: c,
                row: r,
                hash: h,
                data: d,
            }),
        (any::<u16>(), any::<u16>(), any::<u16>(), any::<u16>())
            .prop_map(|(x, y, w, h)| TileMessage::VideoRegion { x, y, w, h }),
        any::<u32>().prop_map(|fs| TileMessage::BatchEnd { frame_seq: fs }),
        (
            any::<i16>(),
            any::<i16>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>()
        )
            .prop_map(|(dx, dy, rt, rb, rr)| TileMessage::ScrollCopy {
                dx,
                dy,
                region_top: rt,
                region_bottom: rb,
                region_right: rr,
            }),
        (any::<i16>(), any::<i16>()).prop_map(|(ox, oy)| TileMessage::GridOffset {
            offset_x: ox,
            offset_y: oy
        }),
        any::<bool>().prop_map(|ao| TileMessage::TileDrawMode { apply_offset: ao }),
        (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
        )
            .prop_map(|(a, b, c, d, e, f, g, h, i, j)| {
                TileMessage::ScrollStats {
                    scroll_batches_total: a,
                    scroll_full_fallbacks_total: b,
                    scroll_potential_tiles_total: c,
                    scroll_saved_tiles_total: d,
                    scroll_non_quantized_fallbacks_total: e,
                    scroll_residual_full_repaints_total: f,
                    scroll_zero_saved_batches_total: g,
                    host_sent_hash_entries: h,
                    host_sent_hash_evictions_total: i,
                    host_cache_miss_reports_total: j,
                }
            }),
    ]
}

proptest! {
    #[test]
    fn frame_envelope_round_trip(channel in arb_channel_id(), payload in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let frame = Frame::new(channel, payload);
        let encoded = frame.encode();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap();
        prop_assert_eq!(&frame, &decoded);
        prop_assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn control_message_round_trip(msg in arb_control_message()) {
        let encoded = msg.encode();
        let decoded = ControlMessage::decode(&encoded).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn control_frame_dispatch_round_trip(msg in arb_control_message()) {
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        prop_assert_eq!(decoded, Message::Control(msg));
    }

    #[test]
    fn input_message_round_trip(msg in arb_input_message()) {
        let encoded = msg.encode();
        let decoded = InputMessage::decode(&encoded).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn cursor_message_round_trip(msg in arb_cursor_message()) {
        let encoded = msg.encode();
        let decoded = CursorMessage::decode(&encoded).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn clipboard_message_round_trip(msg in arb_clipboard_message()) {
        let encoded = msg.encode();
        let decoded = ClipboardMessage::decode(&encoded).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn file_message_round_trip(msg in arb_file_message()) {
        let encoded = msg.encode();
        let decoded = FileMessage::decode_on_channel(&encoded, ChannelId::FileDown).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn video_datagram_round_trip(dg in arb_video_datagram()) {
        let encoded = dg.encode();
        let decoded = VideoDatagram::decode(&encoded).unwrap();
        prop_assert_eq!(&dg, &decoded);
    }

    #[test]
    fn audio_frame_round_trip(frame in arb_audio_frame()) {
        let encoded = frame.encode();
        let decoded = AudioFrame::decode(&encoded).unwrap();
        prop_assert_eq!(&frame, &decoded);
    }

    #[test]
    fn audio_frame_to_frame_out_round_trip(af in arb_audio_frame()) {
        let wire_frame = af.to_frame_out();
        prop_assert_eq!(wire_frame.channel, ChannelId::AudioOut);
        let decoded = AudioFrame::decode(&wire_frame.payload).unwrap();
        prop_assert_eq!(&af, &decoded);

        // Also verify full wire encode/decode
        let wire = wire_frame.encode();
        let (decoded_frame, consumed) = Frame::decode(&wire).unwrap();
        prop_assert_eq!(consumed, wire.len());
        prop_assert_eq!(decoded_frame.channel, ChannelId::AudioOut);
        let decoded_af = AudioFrame::decode(&decoded_frame.payload).unwrap();
        prop_assert_eq!(&af, &decoded_af);
    }

    #[test]
    fn video_fragment_reassemble(data in proptest::collection::vec(any::<u8>(), 1..8192), mtu in 100..2000usize) {
        let fragments = VideoDatagram::fragment(1, false, 100_000, &data, mtu);
        let reassembled = VideoDatagram::reassemble(&fragments).unwrap();
        prop_assert_eq!(&data, &reassembled);
    }

    #[test]
    fn tile_message_round_trip(msg in arb_tile_message()) {
        let encoded = msg.encode();
        let decoded = TileMessage::decode(&encoded).unwrap();
        prop_assert_eq!(&msg, &decoded);
    }

    #[test]
    fn tile_frame_dispatch_round_trip(msg in arb_tile_message()) {
        let frame = msg.to_frame();
        prop_assert_eq!(frame.channel, ChannelId::Tiles);
        let dispatched = Message::from_frame(&frame).unwrap();
        match dispatched {
            Message::Tiles(decoded) => {
                prop_assert_eq!(&decoded, &msg);
            }
            _ => prop_assert!(false, "expected Message::Tiles"),
        }
    }

    #[test]
    fn frame_decode_all_concatenated(msgs in proptest::collection::vec(arb_control_message(), 1..10)) {
        let mut buf = Vec::new();
        for msg in &msgs {
            buf.extend_from_slice(&msg.to_frame().encode());
        }
        let (frames, consumed) = Frame::decode_all(&buf).unwrap();
        prop_assert_eq!(frames.len(), msgs.len());
        prop_assert_eq!(consumed, buf.len());
        for (frame, msg) in frames.iter().zip(msgs.iter()) {
            let decoded = ControlMessage::decode(&frame.payload).unwrap();
            prop_assert_eq!(&decoded, msg);
        }
    }
}
