use super::*;

fn make_header(subtype: &[u8; 3], comp: u32, decomp: u32) -> Vec<u8> {
    let mut h = vec![b'C', b'N', b'X', 0x02];
    h.extend_from_slice(subtype);
    h.push(0x10);
    h.extend_from_slice(&comp.to_be_bytes());
    h.extend_from_slice(&decomp.to_be_bytes());
    h
}

#[test]
fn parse_valid_header() {
    let data = make_header(b"bin", 100, 200);
    let hdr = parse_header(&data).unwrap();
    assert_eq!(&hdr.subtype, b"bin");
    assert_eq!(hdr.compressed_size, 100);
    assert_eq!(hdr.decompressed_size, 200);
}

#[test]
fn is_cnx_valid() {
    let data = make_header(b"bin", 0, 0);
    assert!(is_cnx(&data));
}

#[test]
fn is_cnx_invalid_magic() {
    assert!(!is_cnx(b"XYZ\x02________1234"));
}

#[test]
fn is_cnx_too_short() {
    assert!(!is_cnx(b"CNX"));
}

#[test]
fn parse_invalid_magic() {
    let data = b"XYZ\x02bin\x10\x00\x00\x00\x00\x00\x00\x00\x00";
    assert!(matches!(parse_header(data), Err(CnxError::InvalidMagic)));
}

#[test]
fn parse_too_short() {
    assert!(matches!(
        parse_header(b"CNX"),
        Err(CnxError::TooShort(3))
    ));
}

// Task 2: decompress unit tests

#[test]
fn decompress_single_literals() {
    let mut data = make_header(b"bin", 0, 4);
    data.push(0x55); // 01 01 01 01
    data.push(0xAA);
    data.push(0xBB);
    data.push(0xCC);
    data.push(0xDD);
    data.push(0x00);
    let comp_size = (data.len() - HEADER_SIZE) as u32;
    data[8..12].copy_from_slice(&comp_size.to_be_bytes());
    data[12..16].copy_from_slice(&4u32.to_be_bytes());
    assert_eq!(decompress(&data).unwrap(), vec![0xAA, 0xBB, 0xCC, 0xDD]);
}

#[test]
fn decompress_multi_literal() {
    let mut data = make_header(b"bin", 0, 3);
    data.push(0x03); // code 3
    data.push(0x03); // count
    data.push(0x11);
    data.push(0x22);
    data.push(0x33);
    data.push(0x00);
    let comp_size = (data.len() - HEADER_SIZE) as u32;
    data[8..12].copy_from_slice(&comp_size.to_be_bytes());
    data[12..16].copy_from_slice(&3u32.to_be_bytes());
    assert_eq!(decompress(&data).unwrap(), vec![0x11, 0x22, 0x33]);
}

#[test]
fn decompress_lz_reference() {
    let mut data = make_header(b"bin", 0, 8);
    data.push(0x55); // 4x code 1
    data.push(0xAA);
    data.push(0xBB);
    data.push(0xCC);
    data.push(0xDD);
    // LZ ref: distance=4, length=4 -> pair = (3<<5)|0 = 0x0060
    data.push(0x02); // code 2
    data.push(0x00);
    data.push(0x60);
    data.push(0x00); // terminator
    let comp_size = (data.len() - HEADER_SIZE) as u32;
    data[8..12].copy_from_slice(&comp_size.to_be_bytes());
    data[12..16].copy_from_slice(&8u32.to_be_bytes());
    assert_eq!(
        decompress(&data).unwrap(),
        vec![0xAA, 0xBB, 0xCC, 0xDD, 0xAA, 0xBB, 0xCC, 0xDD]
    );
}

#[test]
fn decompress_size_mismatch_errors() {
    let mut data = make_header(b"bin", 0, 99);
    data.push(0x01);
    data.push(0xFF);
    data.push(0x00);
    let comp_size = (data.len() - HEADER_SIZE) as u32;
    data[8..12].copy_from_slice(&comp_size.to_be_bytes());
    assert!(matches!(
        decompress(&data),
        Err(CnxError::SizeMismatch { .. })
    ));
}

// Compressor unit tests

#[test]
fn compress_roundtrip_simple() {
    let data = b"ABCDABCDABCD";
    let compressed = compress(data, b"bin");
    let decompressed = decompress(&compressed).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_roundtrip_zeros() {
    let data = vec![0u8; 1024];
    let compressed = compress(&data, b"bin");
    let decompressed = decompress(&compressed).unwrap();
    assert_eq!(decompressed, data);
    // Zeros should compress well (LZ matches against initial ring buffer).
    assert!(compressed.len() < data.len() / 2);
}

#[test]
fn compress_roundtrip_random_pattern() {
    let data: Vec<u8> = (0..512).map(|i| (i * 37 + 13) as u8).collect();
    let compressed = compress(&data, b"bin");
    let decompressed = decompress(&compressed).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_roundtrip_single_byte() {
    let data = vec![0x42];
    let compressed = compress(&data, b"bin");
    let decompressed = decompress(&compressed).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_roundtrip_repeated_pattern() {
    let pattern = b"Hello, World! ";
    let data: Vec<u8> = pattern.iter().cycle().take(4096).copied().collect();
    let compressed = compress(&data, b"bin");
    let decompressed = decompress(&compressed).unwrap();
    assert_eq!(decompressed, data);
    // Repeated pattern should compress very well.
    assert!(compressed.len() < data.len() / 3);
}

#[test]
fn compress_preserves_header() {
    let data = b"test data";
    let compressed = compress(data, b"bit");
    assert!(is_cnx(&compressed));
    let hdr = parse_header(&compressed).unwrap();
    assert_eq!(&hdr.subtype, b"bit");
    assert_eq!(hdr.decompressed_size, 9);
}

// Task 3: ROM integration tests

#[test]
#[ignore]
fn decompress_real_seq_file() {
    use crate::disc::{DiscImage, Iso9660};
    use std::path::Path;
    let disc = DiscImage::from_bin_file(Path::new("roms/Madou_Monogatari_JAP.bin")).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();
    let entry = iso.find_file(&disc, "DG9904.SEQ").unwrap().unwrap();
    let data = iso.extract_file(&disc, &entry).unwrap();
    assert!(is_cnx(&data));
    let header = parse_header(&data).unwrap();
    let decompressed = decompress(&data).unwrap();
    assert_eq!(decompressed.len(), header.decompressed_size as usize);
}

#[test]
#[ignore]
fn decompress_real_font_file() {
    use crate::disc::{DiscImage, Iso9660};
    use std::path::Path;
    let disc = DiscImage::from_bin_file(Path::new("roms/Madou_Monogatari_JAP.bin")).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();
    let entry = iso.find_file(&disc, "FONT.CEL").unwrap().unwrap();
    let data = iso.extract_file(&disc, &entry).unwrap();
    assert!(is_cnx(&data));
    let header = parse_header(&data).unwrap();
    assert_eq!(header.decompressed_size, 122048);
    let decompressed = decompress(&data).unwrap();
    assert_eq!(decompressed.len(), 122048);
}

#[test]
#[ignore]
fn compress_roundtrip_real_font_cel() {
    use crate::disc::{DiscImage, Iso9660};
    use std::path::Path;
    let disc = DiscImage::from_bin_file(Path::new("roms/Madou_Monogatari_JAP.bin")).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();
    let entry = iso.find_file(&disc, "FONT.CEL").unwrap().unwrap();
    let original_compressed = iso.extract_file(&disc, &entry).unwrap();
    let original_header = parse_header(&original_compressed).unwrap();

    // Decompress
    let decompressed = decompress(&original_compressed).unwrap();
    assert_eq!(decompressed.len(), 122048);

    // Recompress
    let recompressed = compress(&decompressed, &original_header.subtype);
    eprintln!(
        "FONT.CEL: original {} bytes -> recompressed {} bytes (ratio: {:.1}%)",
        original_compressed.len(),
        recompressed.len(),
        recompressed.len() as f64 / original_compressed.len() as f64 * 100.0
    );

    // Verify roundtrip
    let re_decompressed = decompress(&recompressed).unwrap();
    assert_eq!(re_decompressed, decompressed);

    // Verify it fits in the original disc allocation (43 sectors)
    let original_sectors = (original_compressed.len() + 2047) / 2048;
    let new_sectors = (recompressed.len() + 2047) / 2048;
    assert!(
        new_sectors <= original_sectors,
        "Recompressed needs {} sectors but original only has {}",
        new_sectors,
        original_sectors
    );
}

#[test]
#[ignore]
fn compress_roundtrip_common_seq() {
    use crate::disc::{DiscImage, Iso9660};
    use std::path::Path;
    let disc = DiscImage::from_bin_file(Path::new("roms/Madou_Monogatari_JAP.bin")).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();
    let entry = iso.find_file(&disc, "COMMON.SEQ").unwrap().unwrap();
    let original_compressed = iso.extract_file(&disc, &entry).unwrap();
    let original_header = parse_header(&original_compressed).unwrap();

    // Decompress original
    let decompressed = decompress(&original_compressed).unwrap();
    assert_eq!(decompressed.len(), 110188, "Original COMMON.SEQ size");

    // Roundtrip: compress then decompress the ORIGINAL
    let recompressed = compress(&decompressed, &original_header.subtype);
    let re_decompressed = decompress(&recompressed).unwrap();
    assert_eq!(re_decompressed, decompressed, "Original roundtrip failed");
    eprintln!(
        "COMMON.SEQ original roundtrip OK: {} -> {} -> {} bytes",
        original_compressed.len(),
        recompressed.len(),
        re_decompressed.len()
    );

    // Now test with patched data (if available)
    if let Ok(patched) = std::fs::read("out/patched_COMMON.SEQ") {
        eprintln!("Testing patched COMMON.SEQ roundtrip: {} bytes", patched.len());
        let recompressed_patched = compress(&patched, &original_header.subtype);
        let re_decompressed_patched = decompress(&recompressed_patched).unwrap();
        assert_eq!(
            re_decompressed_patched.len(),
            patched.len(),
            "Patched roundtrip size mismatch: {} vs {}",
            re_decompressed_patched.len(),
            patched.len()
        );
        assert_eq!(
            re_decompressed_patched, patched,
            "Patched COMMON.SEQ roundtrip data mismatch!"
        );
        eprintln!(
            "COMMON.SEQ patched roundtrip OK: {} -> {} -> {} bytes",
            patched.len(),
            recompressed_patched.len(),
            re_decompressed_patched.len()
        );
    }
}

#[test]
#[ignore]
fn decompress_all_seq_files() {
    use crate::disc::{DiscImage, Iso9660};
    use std::path::Path;
    let disc = DiscImage::from_bin_file(Path::new("roms/Madou_Monogatari_JAP.bin")).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();
    let entries = iso.list_root(&disc).unwrap();
    let seq_entries: Vec<_> = entries.iter().filter(|e| e.name.ends_with(".SEQ")).collect();
    assert_eq!(seq_entries.len(), 112);
    for entry in &seq_entries {
        let data = iso.extract_file(&disc, entry).unwrap();
        assert!(is_cnx(&data), "Not CNX: {}", entry.name);
        let header = parse_header(&data).unwrap();
        let decompressed = decompress(&data)
            .unwrap_or_else(|e| panic!("Failed to decompress {}: {e}", entry.name));
        assert_eq!(
            decompressed.len(),
            header.decompressed_size as usize,
            "Size mismatch for {}",
            entry.name
        );
    }
}
