use super::*;

#[test]
fn sync_pattern_is_correct() {
    assert_eq!(SYNC_PATTERN[0], 0x00);
    for i in 1..11 {
        assert_eq!(SYNC_PATTERN[i], 0xFF, "byte {i} should be 0xFF");
    }
    assert_eq!(SYNC_PATTERN[11], 0x00);
}

#[test]
fn constants_are_consistent() {
    assert_eq!(USER_DATA_OFFSET, SYNC_SIZE + HEADER_SIZE);
    assert_eq!(USER_DATA_OFFSET, 16);
    assert_eq!(USER_DATA_SIZE, 2048);
    assert_eq!(RAW_SECTOR_SIZE, 2352);
}

#[test]
fn bcd_conversion() {
    assert_eq!(bcd_to_dec(0x00), 0);
    assert_eq!(bcd_to_dec(0x02), 2);
    assert_eq!(bcd_to_dec(0x12), 12);
    assert_eq!(bcd_to_dec(0x59), 59);
    assert_eq!(bcd_to_dec(0x74), 74);
    assert_eq!(bcd_to_dec(0x99), 99);
}

/// Build a minimal fake sector with a valid sync pattern and header.
fn make_fake_sector(minutes: u8, seconds: u8, frames: u8, fill: u8) -> Vec<u8> {
    let mut sector = vec![0u8; RAW_SECTOR_SIZE];
    sector[..SYNC_SIZE].copy_from_slice(&SYNC_PATTERN);
    sector[SYNC_SIZE] = minutes;
    sector[SYNC_SIZE + 1] = seconds;
    sector[SYNC_SIZE + 2] = frames;
    sector[SYNC_SIZE + 3] = 0x01; // Mode 1
    // Fill user data with the given byte.
    for b in &mut sector[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE] {
        *b = fill;
    }
    sector
}

#[test]
fn disc_image_from_bytes_single_sector() {
    let raw = make_fake_sector(0x00, 0x02, 0x00, 0xAB);
    let disc = DiscImage::from_bytes(raw).unwrap();
    assert_eq!(disc.sector_count(), 1);

    let sector = disc.read_sector(0).unwrap();
    assert_eq!(sector.minutes, 0x00);
    assert_eq!(sector.seconds, 0x02);
    assert_eq!(sector.frames, 0x00);
    assert_eq!(sector.mode, 0x01);
    assert_eq!(sector.user_data.len(), USER_DATA_SIZE);
    assert!(sector.user_data.iter().all(|&b| b == 0xAB));
}

#[test]
fn disc_image_from_bytes_multiple_sectors() {
    let mut raw = Vec::new();
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x00, 0x11));
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x01, 0x22));
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x02, 0x33));

    let disc = DiscImage::from_bytes(raw).unwrap();
    assert_eq!(disc.sector_count(), 3);

    assert!(disc.read_user_data(0).unwrap().iter().all(|&b| b == 0x11));
    assert!(disc.read_user_data(1).unwrap().iter().all(|&b| b == 0x22));
    assert!(disc.read_user_data(2).unwrap().iter().all(|&b| b == 0x33));
}

#[test]
fn sector_out_of_range() {
    let raw = make_fake_sector(0x00, 0x02, 0x00, 0x00);
    let disc = DiscImage::from_bytes(raw).unwrap();

    assert!(disc.read_sector(1).is_err());
    assert!(disc.read_user_data(1).is_err());
}

#[test]
fn invalid_file_size_rejected() {
    let data = vec![0u8; RAW_SECTOR_SIZE + 1];
    let result = DiscImage::from_bytes(data);
    assert!(result.is_err());
}

#[test]
fn invalid_sync_detected() {
    let mut raw = make_fake_sector(0x00, 0x02, 0x00, 0x00);
    raw[5] = 0x00; // corrupt sync pattern
    let disc = DiscImage::from_bytes(raw).unwrap();
    assert!(disc.read_sector(0).is_err());
}

#[test]
fn extract_file_across_sectors() {
    let mut raw = Vec::new();
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x00, 0xAA));
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x01, 0xBB));

    let disc = DiscImage::from_bytes(raw).unwrap();
    // Extract 3000 bytes starting at sector 0 (spans 2 sectors).
    let data = disc.extract_file(0, 3000).unwrap();
    assert_eq!(data.len(), 3000);
    assert!(data[..USER_DATA_SIZE].iter().all(|&b| b == 0xAA));
    assert!(data[USER_DATA_SIZE..].iter().all(|&b| b == 0xBB));
}

#[test]
fn extract_file_exact_sector_boundary() {
    let mut raw = Vec::new();
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x00, 0xCC));
    raw.extend_from_slice(&make_fake_sector(0x00, 0x02, 0x01, 0xDD));

    let disc = DiscImage::from_bytes(raw).unwrap();
    let data = disc.extract_file(0, USER_DATA_SIZE as u32).unwrap();
    assert_eq!(data.len(), USER_DATA_SIZE);
    assert!(data.iter().all(|&b| b == 0xCC));
}

// -- Integration test against the real ROM --

#[test]
#[ignore]
fn read_real_rom_sector_0() {
    let rom_path = std::env::var("SS_MADOU_ROM")
        .unwrap_or_else(|_| "roms/Madou_Monogatari_JAP.bin".to_string());
    let path = Path::new(&rom_path);
    if !path.exists() {
        eprintln!("ROM not found at {rom_path}, skipping");
        return;
    }

    let disc = DiscImage::from_bin_file(path).unwrap();
    // Total file size: 146,661,312 / 2352 = 62,356 sectors
    assert_eq!(disc.sector_count(), 62_356);

    let sector = disc.read_sector(0).unwrap();
    assert_eq!(sector.mode, 0x01);
    // Sector 0 user data starts with "SEGA SEGASATURN "
    assert_eq!(&sector.user_data[..16], b"SEGA SEGASATURN ");
}
