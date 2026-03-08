use super::*;

#[test]
fn vli_roundtrip_small() {
    for n in 0..128 {
        let mut buf = Vec::new();
        vli_encode(&mut buf, n);
        let (val, _) = vli_decode(&buf).unwrap();
        assert_eq!(val, n, "roundtrip failed for {n}");
    }
}

#[test]
fn vli_roundtrip_large() {
    let values = [127, 128, 255, 256, 1000, 65535, 0x100000, 0xFFFFFFFF];
    for &n in &values {
        let mut buf = Vec::new();
        vli_encode(&mut buf, n);
        let (val, _) = vli_decode(&buf).unwrap();
        assert_eq!(val, n, "roundtrip failed for {n}");
    }
}

#[test]
fn bps_roundtrip_simple() {
    let source = b"Hello, World!".to_vec();
    let target = b"Hello, Rust!!".to_vec();
    let patch = generate_bps(&source, &target);
    let result = apply_bps(&source, &patch).unwrap();
    assert_eq!(result, target);
}

#[test]
fn bps_roundtrip_size_change() {
    let source = vec![0u8; 100];
    let target = vec![0xFFu8; 200];
    let patch = generate_bps(&source, &target);
    let result = apply_bps(&source, &patch).unwrap();
    assert_eq!(result, target);
}

#[test]
fn bps_roundtrip_large_scattered() {
    let mut source = vec![0xFFu8; 0x10000];
    for i in (0..source.len()).step_by(0x100) {
        source[i] = (i >> 8) as u8;
    }
    let mut target = source.clone();
    for i in 0..16 {
        target[i * 0x1000 + 0x42] = 0xAA;
    }
    let patch = generate_bps(&source, &target);
    let result = apply_bps(&source, &patch).unwrap();
    assert_eq!(result, target);
    assert!(patch.len() < source.len() / 10);
}

#[test]
fn bps_identical_files() {
    let source = vec![0u8; 256];
    let patch = generate_bps(&source, &source);
    let result = apply_bps(&source, &patch).unwrap();
    assert_eq!(result, source);
}

#[test]
fn bps_validates_magic() {
    let result = apply_bps(&[], b"XXXX1234567890ab");
    assert!(result.is_err());
}

#[test]
fn bps_validates_source_crc() {
    let source = b"hello".to_vec();
    let target = b"world".to_vec();
    let patch = generate_bps(&source, &target);
    let result = apply_bps(b"wrong", &patch);
    assert!(result.is_err());
}
