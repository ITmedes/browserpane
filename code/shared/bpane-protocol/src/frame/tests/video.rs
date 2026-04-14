use alloc::vec;
use alloc::vec::Vec;

use crate::{VideoDatagram, VideoTileInfo};

#[test]
fn video_datagram_round_trip() {
    let datagram = VideoDatagram {
        nal_id: 42,
        fragment_seq: 0,
        fragment_total: 1,
        is_keyframe: true,
        pts_us: 33_333,
        data: vec![0x00, 0x00, 0x00, 0x01, 0x65],
        tile_info: None,
    };
    assert_eq!(datagram, VideoDatagram::decode(&datagram.encode()).unwrap());
}

#[test]
fn video_datagram_fragmentation() {
    let nal_data = vec![0xAA; 3000];
    let fragments = VideoDatagram::fragment(1, false, 100_000, &nal_data, 1000);
    assert_eq!(fragments.len(), 3);
    assert_eq!(fragments[0].fragment_seq, 0);
    assert_eq!(fragments[0].fragment_total, 3);
    assert_eq!(fragments[1].fragment_seq, 1);
    assert_eq!(fragments[2].fragment_seq, 2);
    assert_eq!(fragments[2].data.len(), 1000);
    assert_eq!(VideoDatagram::reassemble(&fragments).unwrap(), nal_data);
}

#[test]
fn video_datagram_fragment_single() {
    let nal_data = vec![0xBB; 500];
    let fragments = VideoDatagram::fragment(2, true, 200_000, &nal_data, 1200);
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].fragment_total, 1);
    assert!(fragments[0].is_keyframe);
    assert_eq!(VideoDatagram::reassemble(&fragments).unwrap(), nal_data);
}

#[test]
fn video_datagram_reassemble_missing_fragment() {
    let fragments = VideoDatagram::fragment(1, false, 100, &vec![0; 3000], 1000);
    assert!(VideoDatagram::reassemble(&fragments[..2]).is_none());
}

#[test]
fn video_datagram_fragment_round_trip_encode_decode() {
    let fragments = VideoDatagram::fragment(5, true, 500_000, &vec![0xCC; 2500], 1000);
    let decoded: Vec<VideoDatagram> = fragments
        .iter()
        .map(|fragment| VideoDatagram::decode(&fragment.encode()).unwrap())
        .collect();
    assert_eq!(fragments, decoded);
}

#[test]
fn video_datagram_with_tile_info_round_trip() {
    let tile_info = Some(VideoTileInfo {
        tile_x: 100,
        tile_y: 200,
        tile_w: 320,
        tile_h: 240,
        screen_w: 1920,
        screen_h: 1080,
    });
    let datagram = VideoDatagram {
        nal_id: 42,
        fragment_seq: 0,
        fragment_total: 1,
        is_keyframe: true,
        pts_us: 123_456,
        data: vec![0xAA; 100],
        tile_info,
    };
    assert_eq!(datagram, VideoDatagram::decode(&datagram.encode()).unwrap());
}

#[test]
fn video_datagram_without_tile_info_round_trip() {
    let datagram = VideoDatagram {
        nal_id: 7,
        fragment_seq: 0,
        fragment_total: 1,
        is_keyframe: false,
        pts_us: 999,
        data: vec![0xBB; 50],
        tile_info: None,
    };
    assert_eq!(datagram, VideoDatagram::decode(&datagram.encode()).unwrap());
}

#[test]
fn fragment_with_tile_preserves_tile_info() {
    let tile = VideoTileInfo {
        tile_x: 64,
        tile_y: 128,
        tile_w: 256,
        tile_h: 128,
        screen_w: 1920,
        screen_h: 1080,
    };
    let data = vec![0xCC; 3000];
    let fragments = VideoDatagram::fragment_with_tile(10, true, 500, &data, 1000, Some(tile));
    assert_eq!(fragments.len(), 3);
    for fragment in &fragments {
        assert_eq!(fragment.tile_info, Some(tile));
        assert_eq!(
            VideoDatagram::decode(&fragment.encode()).unwrap().tile_info,
            Some(tile)
        );
    }
    assert_eq!(VideoDatagram::reassemble(&fragments).unwrap(), data);
}

#[test]
fn fragment_with_tile_none_is_same_as_fragment() {
    let data = vec![0xDD; 2500];
    let plain = VideoDatagram::fragment(1, false, 100, &data, 1000);
    let with_none = VideoDatagram::fragment_with_tile(1, false, 100, &data, 1000, None);
    assert_eq!(plain, with_none);
}

#[test]
fn fragment_with_small_max_fragment_size() {
    let data = vec![0xAA; 100];
    let fragments = VideoDatagram::fragment(1, false, 0, &data, 10);
    assert_eq!(fragments.len(), 10);
    for (i, fragment) in fragments.iter().enumerate() {
        assert_eq!(fragment.fragment_seq, i as u16);
        assert_eq!(fragment.fragment_total, 10);
    }
    assert_eq!(VideoDatagram::reassemble(&fragments).unwrap(), data);
}
