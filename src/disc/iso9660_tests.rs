use super::*;
use super::super::test_helpers::*;

#[test]
fn both_endian_u32_parsing() {
    let data: [u8; 8] = [0x04, 0x03, 0x02, 0x01, 0x01, 0x02, 0x03, 0x04];
    assert_eq!(read_both_u32(&data), 0x01020304);
}

#[test]
fn both_endian_u16_parsing() {
    let data: [u8; 4] = [0x00, 0x08, 0x08, 0x00];
    assert_eq!(read_both_u16(&data), 0x0800);
}

#[test]
fn both_endian_u32_small_value() {
    let data: [u8; 8] = [18, 0, 0, 0, 0, 0, 0, 18];
    assert_eq!(read_both_u32(&data), 18);
}

#[test]
fn both_endian_u16_block_size() {
    let data: [u8; 4] = [0x00, 0x08, 0x08, 0x00];
    assert_eq!(read_both_u16(&data), 2048);
}

#[test]
fn parse_pvd_from_fake_disc() {
    let pvd = make_fake_pvd("TEST_VOLUME", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),  // "." entry
        ("\x01", 18, 2048, true),  // ".." entry
        ("HELLO.TXT;1", 25, 1024, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);

    let iso = Iso9660::parse(&disc).unwrap();
    assert_eq!(iso.pvd.volume_id, "TEST_VOLUME");
    assert_eq!(iso.pvd.root_directory_lba, 20);
    assert_eq!(iso.pvd.root_directory_size, 2048);
    assert_eq!(iso.pvd.logical_block_size, 2048);
}

#[test]
fn list_root_from_fake_disc() {
    let pvd = make_fake_pvd("TEST_VOLUME", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("HELLO.TXT;1", 25, 1024, false),
        ("DATA", 30, 2048, true),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    let entries = iso.list_root(&disc).unwrap();
    assert_eq!(entries.len(), 4);

    assert_eq!(entries[0].name, ".");
    assert!(entries[0].is_directory);

    assert_eq!(entries[1].name, "..");
    assert!(entries[1].is_directory);

    assert_eq!(entries[2].name, "HELLO.TXT");
    assert!(!entries[2].is_directory);
    assert_eq!(entries[2].lba, 25);
    assert_eq!(entries[2].size, 1024);

    assert_eq!(entries[3].name, "DATA");
    assert!(entries[3].is_directory);
}

#[test]
fn find_file_from_fake_disc() {
    let pvd = make_fake_pvd("TEST_VOLUME", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("GAME.BIN;1", 50, 4096, false),
        ("README.TXT;1", 55, 256, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    // Search by name (case-insensitive, no version suffix)
    let found = iso.find_file(&disc, "GAME.BIN").unwrap();
    assert!(found.is_some());
    let entry = found.unwrap();
    assert_eq!(entry.lba, 50);
    assert_eq!(entry.size, 4096);

    // Search for something that doesn't exist.
    let not_found = iso.find_file(&disc, "NOPE.TXT").unwrap();
    assert!(not_found.is_none());
}

#[test]
fn patch_file_entry_updates_lba_and_size() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 50, 4096, false),
        ("DATA.BIN;1", 60, 1024, false),
    ]);
    let mut disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    // Patch both LBA and size.
    iso.patch_file_entry(&mut disc, "FONT.CEL", Some(200), Some(8192))
        .unwrap();

    // Re-parse and verify.
    let iso2 = Iso9660::parse(&disc).unwrap();
    let entry = iso2
        .find_file(&disc, "FONT.CEL")
        .unwrap()
        .expect("FONT.CEL should exist");
    assert_eq!(entry.lba, 200);
    assert_eq!(entry.size, 8192);

    // Verify other files are untouched.
    let data_entry = iso2
        .find_file(&disc, "DATA.BIN")
        .unwrap()
        .expect("DATA.BIN should exist");
    assert_eq!(data_entry.lba, 60);
    assert_eq!(data_entry.size, 1024);
}

#[test]
fn patch_file_entry_lba_only() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 50, 4096, false),
    ]);
    let mut disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    iso.patch_file_entry(&mut disc, "FONT.CEL", Some(300), None)
        .unwrap();

    let iso2 = Iso9660::parse(&disc).unwrap();
    let entry = iso2
        .find_file(&disc, "FONT.CEL")
        .unwrap()
        .unwrap();
    assert_eq!(entry.lba, 300);
    assert_eq!(entry.size, 4096); // size unchanged
}

#[test]
fn find_free_region_after_last_file() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        // File at LBA 30, size 4096 → 2 sectors → ends at LBA 32
        ("A.BIN;1", 30, 4096, false),
        // File at LBA 50, size 2048 → 1 sector → ends at LBA 51
        ("B.BIN;1", 50, 2048, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    let free = iso.find_free_region(&disc, 10).unwrap();
    assert_eq!(free, 51); // right after B.BIN
}

#[test]
fn find_free_region_not_enough_space() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("A.BIN;1", 30, 4096, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let iso = Iso9660::parse(&disc).unwrap();

    // Volume space is 57318 sectors. LBA 32 + 57300 should fail.
    let result = iso.find_free_region(&disc, 57300);
    assert!(result.is_err());
}

#[test]
fn relocate_file_roundtrip() {
    // Build disc with enough sectors for relocation.
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 25, 2048, false),
        ("OTHER.BIN;1", 28, 1024, false),
    ]);
    // build_test_disc creates max(17, dir_lba+dir_sectors) sectors.
    // We need at least 31 sectors (LBA 29 + 2 for the relocated data).
    let mut disc = build_test_disc_sized(&pvd, 20, &dir, 35);
    let iso = Iso9660::parse(&disc).unwrap();

    // Create test data (3000 bytes → 2 sectors).
    let test_data: Vec<u8> = (0..3000).map(|i| (i % 256) as u8).collect();

    let new_lba = iso.relocate_file(&mut disc, "FONT.CEL", &test_data).unwrap();
    assert_eq!(new_lba, 29); // after OTHER.BIN (LBA 28, 1 sector)

    // Verify ISO 9660 directory updated.
    let iso2 = Iso9660::parse(&disc).unwrap();
    let entry = iso2
        .find_file(&disc, "FONT.CEL")
        .unwrap()
        .expect("FONT.CEL should exist");
    assert_eq!(entry.lba, 29);
    assert_eq!(entry.size, 3000);

    // Verify data was written correctly.
    let extracted = disc.extract_file(29, 3000).unwrap();
    assert_eq!(extracted, test_data);
}

#[test]
fn invalid_iso_signature_rejected() {
    use super::super::sector::{RAW_SECTOR_SIZE, SYNC_PATTERN, SYNC_SIZE, USER_DATA_OFFSET};

    // Build a disc with 17 sectors but no valid PVD signature.
    let mut bin = vec![0u8; 17 * RAW_SECTOR_SIZE];
    for i in 0..17 {
        let offset = i * RAW_SECTOR_SIZE;
        bin[offset..offset + SYNC_SIZE].copy_from_slice(&SYNC_PATTERN);
        bin[offset + SYNC_SIZE + 3] = 0x01;
    }
    // Write a bogus type code.
    bin[PVD_SECTOR * RAW_SECTOR_SIZE + USER_DATA_OFFSET] = 0xFF;

    let disc = DiscImage::from_bytes(bin).unwrap();
    assert!(Iso9660::parse(&disc).is_err());
}

// -- TrackedDisc variants --

#[test]
fn patch_file_entry_tracked_updates_lba_and_size() {
    use super::super::tracked::TrackedDisc;

    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 50, 4096, false),
        ("DATA.BIN;1", 60, 1024, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    iso.patch_file_entry_tracked(&mut tracked, "FONT.CEL", Some(200), Some(8192))
        .unwrap();

    // Verify directory updated
    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let entry = iso2
        .find_file(tracked.disc(), "FONT.CEL")
        .unwrap()
        .expect("FONT.CEL should exist");
    assert_eq!(entry.lba, 200);
    assert_eq!(entry.size, 8192);

    // Verify write was tracked
    assert_eq!(tracked.region_count(), 1);
    assert!(tracked.check().is_ok());
}

#[test]
fn relocate_file_tracked_roundtrip() {
    use super::super::tracked::TrackedDisc;

    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 25, 2048, false),
        ("OTHER.BIN;1", 28, 1024, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 35);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    let test_data: Vec<u8> = (0..3000).map(|i| (i % 256) as u8).collect();
    let new_lba = iso
        .relocate_file_tracked(&mut tracked, "FONT.CEL", &test_data)
        .unwrap();
    assert_eq!(new_lba, 29);

    // Verify ISO 9660 directory updated
    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let entry = iso2
        .find_file(tracked.disc(), "FONT.CEL")
        .unwrap()
        .expect("FONT.CEL should exist");
    assert_eq!(entry.lba, 29);
    assert_eq!(entry.size, 3000);

    // Verify data written
    let extracted = tracked.extract_file(29, 3000).unwrap();
    assert_eq!(extracted, test_data);

    // Verify writes tracked (file write + dir entry patch)
    assert_eq!(tracked.region_count(), 2);
    assert!(tracked.check().is_ok());
}

#[test]
fn patch_file_size_tracked_updates_size_only() {
    use super::super::tracked::TrackedDisc;

    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 50, 4096, false),
    ]);
    let disc = build_test_disc(&pvd, 20, &dir);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    iso.patch_file_size_tracked(&mut tracked, "FONT.CEL", 8192)
        .unwrap();

    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let entry = iso2
        .find_file(tracked.disc(), "FONT.CEL")
        .unwrap()
        .unwrap();
    assert_eq!(entry.lba, 50); // unchanged
    assert_eq!(entry.size, 8192);
}

#[test]
fn tracked_relocations_no_collision() {
    use super::super::tracked::TrackedDisc;

    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("A.BIN;1", 25, 2048, false),
        ("B.BIN;1", 28, 2048, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 50);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    let data_a = vec![0xAA; 3000]; // 2 sectors
    let data_b = vec![0xBB; 5000]; // 3 sectors

    // Relocate A first, then B
    let lba_a = iso
        .relocate_file_tracked(&mut tracked, "A.BIN", &data_a)
        .unwrap();
    // After A is relocated, re-parse to get updated directory for B's free region calc
    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let lba_b = iso2
        .relocate_file_tracked(&mut tracked, "B.BIN", &data_b)
        .unwrap();

    // They should not overlap
    assert!(lba_b >= lba_a + 2); // A takes 2 sectors
    assert!(tracked.check().is_ok());
}

// -- Integration test against the real ROM --

#[test]
#[ignore]
fn parse_real_rom_iso9660() {
    use std::path::Path;

    let rom_path = std::env::var("SS_MADOU_ROM")
        .unwrap_or_else(|_| "roms/Madou_Monogatari_JAP.bin".to_string());
    let path = Path::new(&rom_path);
    if !path.exists() {
        eprintln!("ROM not found at {rom_path}, skipping");
        return;
    }

    let disc = DiscImage::from_bin_file(path).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();

    eprintln!("Volume ID: {}", iso.pvd.volume_id);
    eprintln!("Volume Space Size: {}", iso.pvd.volume_space_size);
    eprintln!("Block Size: {}", iso.pvd.logical_block_size);
    eprintln!(
        "Root Dir LBA: {}, Size: {}",
        iso.pvd.root_directory_lba, iso.pvd.root_directory_size
    );

    assert_eq!(iso.pvd.volume_id, "MADOU_MONOGATARI");
    assert_eq!(iso.pvd.logical_block_size, 2048);

    let entries = iso.list_root(&disc).unwrap();
    assert!(!entries.is_empty(), "root directory should not be empty");

    eprintln!("\n=== Root Directory ===");
    for entry in &entries {
        let kind = if entry.is_directory { "DIR " } else { "FILE" };
        eprintln!(
            "{kind} {:30} LBA={:6}  Size={:10}",
            entry.name, entry.lba, entry.size
        );
    }
}
