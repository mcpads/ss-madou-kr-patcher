use super::*;

/// Build a minimal 256-byte fake Saturn header.
fn make_fake_header() -> Vec<u8> {
    let mut data = vec![0x20u8; 2048]; // fill with ASCII space

    // Hardware ID at 0x00 (16 bytes)
    let hw = b"SEGA SEGASATURN ";
    data[0x00..0x10].copy_from_slice(hw);

    // Maker ID at 0x10 (16 bytes)
    data[0x10..0x20].copy_from_slice(b"SEGA ENTERPRISES");

    // Product number at 0x20 (10 bytes)
    data[0x20..0x2A].copy_from_slice(b"T-6607G   ");

    // Version at 0x2A (6 bytes)
    data[0x2A..0x30].copy_from_slice(b"V1.003");

    // Release date at 0x30 (8 bytes)
    data[0x30..0x38].copy_from_slice(b"19980305");

    // Device info at 0x38 (8 bytes)
    data[0x38..0x40].copy_from_slice(b"CD-1/1  ");

    // Area code at 0x40 (8 bytes)
    data[0x40..0x48].copy_from_slice(b"JT      ");

    // Game title at 0x60 (128 bytes) -- fill first portion
    let title = b"MADOUMONOGATARI";
    data[0x60..0x60 + title.len()].copy_from_slice(title);

    // IP Size at 0xE0 (4 bytes, big-endian)
    data[0xE0..0xE4].copy_from_slice(&0x00001800u32.to_be_bytes());

    // 1st Read Address at 0xF0 (4 bytes, big-endian)
    data[0xF0..0xF4].copy_from_slice(&0x06004000u32.to_be_bytes());

    // 1st Read Size at 0xF4 (4 bytes, big-endian)
    data[0xF4..0xF8].copy_from_slice(&0x00080000u32.to_be_bytes());

    data
}

#[test]
fn parse_fake_header() {
    let data = make_fake_header();
    let header = SaturnHeader::parse(&data).unwrap();

    assert!(header.is_valid());
    assert_eq!(header.hardware_id, "SEGA SEGASATURN");
    assert_eq!(header.maker_id, "SEGA ENTERPRISES");
    assert_eq!(header.product_number, "T-6607G");
    assert_eq!(header.version, "V1.003");
    assert_eq!(header.release_date, "19980305");
    assert_eq!(header.device_info, "CD-1/1");
    assert_eq!(header.area_code, "JT");
    assert!(header.game_title.contains("MADOUMONOGATARI"));
    assert_eq!(header.first_read_addr, 0x06004000);
    assert_eq!(header.first_read_size, 0x00080000);
    assert_eq!(header.ip_size, 0x00001800);
}

#[test]
fn invalid_header_not_valid() {
    let mut data = make_fake_header();
    // Corrupt hardware ID
    data[0] = b'X';
    let header = SaturnHeader::parse(&data).unwrap();
    assert!(!header.is_valid());
}

#[test]
fn too_short_data_rejected() {
    let data = vec![0u8; 100];
    assert!(SaturnHeader::parse(&data).is_err());
}

#[test]
fn display_format() {
    let data = make_fake_header();
    let header = SaturnHeader::parse(&data).unwrap();
    let output = format!("{header}");
    assert!(output.contains("SEGA SEGASATURN"));
    assert!(output.contains("T-6607G"));
    assert!(output.contains("0x06004000"));
}

// -- Integration test against the real ROM --

#[test]
#[ignore]
fn parse_real_rom_header() {
    use super::super::sector::DiscImage;
    use std::path::Path;

    let rom_path = std::env::var("SS_MADOU_ROM")
        .unwrap_or_else(|_| "roms/Madou_Monogatari_JAP.bin".to_string());
    let path = Path::new(&rom_path);
    if !path.exists() {
        eprintln!("ROM not found at {rom_path}, skipping");
        return;
    }

    let disc = DiscImage::from_bin_file(path).unwrap();
    let user_data = disc.read_user_data(0).unwrap();
    let header = SaturnHeader::parse(user_data).unwrap();

    assert!(header.is_valid());
    assert_eq!(header.hardware_id, "SEGA SEGASATURN");
    assert_eq!(header.product_number, "T-6607G");
    assert_eq!(header.version, "V1.003");
    assert_eq!(header.first_read_addr, 0x06004000);
    assert!(header.game_title.contains("MADOUMONOGATARI"));
    eprintln!("{header}");
}
