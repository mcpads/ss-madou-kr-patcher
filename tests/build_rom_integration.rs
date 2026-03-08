//! Integration tests for the ROM build pipeline.
//!
//! Synthetic tests run without a real ROM. Tests marked `#[ignore]`
//! require the actual Madou Monogatari ROM at `roms/Madou_Monogatari_JAP.bin`.

use ss_madou::disc::test_helpers::*;
use ss_madou::disc::tracked::TrackedDisc;
use ss_madou::disc::Iso9660;

// ---------------------------------------------------------------------------
// Synthetic tests (no ROM required)
// ---------------------------------------------------------------------------

#[test]
fn tracked_disc_write_and_check_no_collision() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 25, 2048, false),
        ("MP0001.SEQ;1", 28, 2048, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 50);
    let mut tracked = TrackedDisc::new(disc);

    // Simulate writing to non-overlapping regions
    let font_data = vec![0xAA; 3000];
    let seq_data = vec![0xBB; 1500];
    tracked
        .write_file_at(30, &font_data, "FONT.CEL:inplace")
        .unwrap();
    tracked
        .write_file_at(35, &seq_data, "MP0001.SEQ:inplace")
        .unwrap();

    assert!(tracked.check().is_ok());
    assert_eq!(tracked.region_count(), 2);
}

#[test]
fn tracked_disc_detects_collision() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("A.BIN;1", 25, 2048, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 50);
    let mut tracked = TrackedDisc::new(disc);

    // Write overlapping regions with different labels
    let data_a = vec![0xAA; 4096]; // 2 sectors: 30..32
    let data_b = vec![0xBB; 4096]; // 2 sectors: 31..33
    tracked.write_file_at(30, &data_a, "font").unwrap();
    tracked.write_file_at(31, &data_b, "seq").unwrap();

    let err = tracked.check().unwrap_err();
    assert!(err.contains("OVERLAP"));
}

#[test]
fn iso9660_relocate_tracked_updates_directory() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("FONT.CEL;1", 25, 2048, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 50);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    let new_data = vec![0xCC; 5000]; // 3 sectors
    let new_lba = iso
        .relocate_file_tracked(&mut tracked, "FONT.CEL", &new_data)
        .unwrap();

    // Verify the directory entry was updated
    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let entry = iso2
        .find_file(tracked.disc(), "FONT.CEL")
        .unwrap()
        .unwrap();
    assert_eq!(entry.lba, new_lba);
    assert_eq!(entry.size, 5000);

    // Verify data
    let extracted = tracked.extract_file(new_lba, 5000).unwrap();
    assert_eq!(extracted, new_data);

    assert!(tracked.check().is_ok());
}

#[test]
fn multiple_relocations_no_collision() {
    let pvd = make_fake_pvd("TEST", 20, 2048);
    let dir = make_fake_directory(&[
        ("\x00", 20, 2048, true),
        ("\x01", 18, 2048, true),
        ("A.BIN;1", 25, 2048, false),
        ("B.BIN;1", 27, 2048, false),
    ]);
    let disc = build_test_disc_sized(&pvd, 20, &dir, 60);
    let mut tracked = TrackedDisc::new(disc);
    let iso = Iso9660::parse(tracked.disc()).unwrap();

    let data_a = vec![0xAA; 4096]; // 2 sectors
    let lba_a = iso
        .relocate_file_tracked(&mut tracked, "A.BIN", &data_a)
        .unwrap();

    // Re-parse after first relocation
    let iso2 = Iso9660::parse(tracked.disc()).unwrap();
    let data_b = vec![0xBB; 6144]; // 3 sectors
    let lba_b = iso2
        .relocate_file_tracked(&mut tracked, "B.BIN", &data_b)
        .unwrap();

    // B should start after A
    assert!(lba_b >= lba_a + 2);
    assert!(tracked.check().is_ok());
}

// ---------------------------------------------------------------------------
// Real ROM tests (require actual ROM file)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn isolation_descriptor_only() {
    use ss_madou::pipeline;
    use std::path::Path;

    let rom_path = std::env::var("SS_MADOU_ROM")
        .unwrap_or_else(|_| "roms/Madou_Monogatari_JAP.bin".to_string());
    if !Path::new(&rom_path).exists() {
        eprintln!("ROM not found at {}, skipping", rom_path);
        return;
    }

    let ctx = pipeline::load_disc(Path::new(&rom_path)).unwrap();

    // Just loading should produce zero tracked writes
    assert_eq!(ctx.disc.region_count(), 0);
    assert!(ctx.disc.check().is_ok());
    eprintln!("Isolation test: descriptor only — 0 regions, no collisions");
}

#[test]
#[ignore]
fn isolation_font_only() {
    use ss_madou::font::korean::{GLYPH_TILE_START, TILES_PER_GLYPH, TILE_BYTES_PUB};
    use ss_madou::pipeline;
    use std::path::Path;

    let rom_path = std::env::var("SS_MADOU_ROM")
        .unwrap_or_else(|_| "roms/Madou_Monogatari_JAP.bin".to_string());
    let font_path = "assets/fonts/neodgm.ttf";
    if !Path::new(&rom_path).exists() || !Path::new(font_path).exists() {
        eprintln!("ROM or font not found, skipping");
        return;
    }

    let mut ctx = pipeline::load_disc(Path::new(&rom_path)).unwrap();
    let mut font_ctx = pipeline::extract_font(&ctx).unwrap();

    // Extend font buffer for testing
    let test_chars = vec!['가', '나', '다', '라', '마'];
    let preserve = ss_madou::text::patcher::preserved_glyph_slots();
    let char_table = ss_madou::text::patcher::build_char_table(&test_chars, 0, &preserve);

    let max_slot = char_table
        .values()
        .filter(|&&tc| tc >= GLYPH_TILE_START as u16)
        .map(|&tc| ((tc as usize) - GLYPH_TILE_START) / TILES_PER_GLYPH)
        .max()
        .unwrap_or(0)
        + 1;
    let required_size = (GLYPH_TILE_START + max_slot * TILES_PER_GLYPH) * TILE_BYTES_PUB;
    if font_ctx.font_cel.len() < required_size {
        font_ctx.font_cel.resize(required_size, 0);
    }

    let glyph_tiles =
        pipeline::generate_korean_glyphs(Path::new(font_path), &test_chars, 16.0).unwrap();
    pipeline::patch_font(&mut ctx, &mut font_ctx, &glyph_tiles, &char_table).unwrap();

    if font_ctx.font_cel.len() != 122_048 {
        pipeline::patch_first_read_combined(&mut ctx, font_ctx.font_cel.len(), &[]).unwrap();
    }

    ctx.disc.dump_regions();
    assert!(ctx.disc.check().is_ok());
    eprintln!(
        "Isolation test: font only — {} regions, no collisions",
        ctx.disc.region_count()
    );
}
