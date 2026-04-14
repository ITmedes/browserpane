use super::*;

#[test]
fn tile_motion_features_detect_static_tile() {
    let w = 16;
    let h = 16;
    let stride = w * 4;
    let frame = vec![128u8; stride * h];
    let features = compute_tile_motion_features(&frame, Some(&frame), stride, 0, 0, w, h);
    assert!(
        features.change_ratio < 0.01,
        "static tile should have near-zero change_ratio: {}",
        features.change_ratio
    );
    assert!(
        features.motion_magnitude < 0.01,
        "static tile should have near-zero motion_magnitude: {}",
        features.motion_magnitude
    );
}

#[test]
fn tile_motion_features_detect_dynamic_tile() {
    let w = 32;
    let h = 32;
    let stride = w * 4;
    let current: Vec<u8> = (0..stride * h)
        .map(|i| ((i * 7 + 13) % 256) as u8)
        .collect();
    let previous: Vec<u8> = (0..stride * h)
        .map(|i| ((i * 3 + 97) % 256) as u8)
        .collect();
    let features = compute_tile_motion_features(&current, Some(&previous), stride, 0, 0, w, h);
    assert!(
        features.change_ratio > 0.5,
        "dynamic tile should have high change_ratio: {}",
        features.change_ratio
    );
    assert!(
        features.motion_magnitude > 0.05,
        "dynamic tile should have motion_magnitude: {}",
        features.motion_magnitude
    );
}

#[test]
fn motion_features_zero_size_returns_default() {
    let frame = vec![0u8; 100];
    let f = compute_tile_motion_features(&frame, None, 40, 0, 0, 0, 0);
    assert_eq!(f.change_ratio, 0.0);
    assert_eq!(f.motion_magnitude, 0.0);
}

#[test]
fn bbox_iou_identical_is_one() {
    let b = (1, 1, 5, 5);
    let iou = bbox_iou(b, b);
    assert!(
        (iou - 1.0).abs() < 1e-6,
        "identical bbox iou should be 1.0: {iou}"
    );
}

#[test]
fn bbox_iou_disjoint_is_zero() {
    assert_eq!(bbox_iou((0, 0, 2, 2), (5, 5, 7, 7)), 0.0);
}

#[test]
fn bbox_center_shift_identical_is_zero() {
    let b = (2, 3, 6, 9);
    assert!(bbox_center_shift(b, b) < 1e-6);
}

#[test]
fn bbox_center_shift_measures_displacement() {
    let a = (0, 0, 4, 4);
    let b = (2, 0, 6, 4);
    let shift = bbox_center_shift(a, b);
    assert!(shift > 1.5 && shift < 2.5, "shift should be ~2.0: {shift}");
}

#[test]
fn is_photo_like_detects_gradient_tile() {
    let w = 32;
    let h = 32;
    let stride = w * 4;
    let mut frame = vec![0u8; stride * h];
    for row in 0..h {
        for col in 0..w {
            let off = row * stride + col * 4;
            let lum = ((row * 8 + col * 4) % 256) as u8;
            frame[off] = lum;
            frame[off + 1] = lum;
            frame[off + 2] = lum;
            frame[off + 3] = 255;
        }
    }
    assert!(is_photo_like_tile(&frame, stride, 0, 0, w, h, 16));
}

#[test]
fn is_photo_like_rejects_solid_tile() {
    let w = 16;
    let h = 16;
    let stride = w * 4;
    let frame = vec![200u8; stride * h]; // uniform luminance
    assert!(!is_photo_like_tile(&frame, stride, 0, 0, w, h, 16));
}
