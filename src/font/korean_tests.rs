use super::*;

#[test]
fn coverage_to_tiles_all_zero() {
    let coverage = [0u8; 256];
    let tiles = coverage_to_4bpp_tiles(&coverage);
    assert_eq!(tiles, [0u8; 128]);
}

#[test]
fn coverage_to_tiles_all_max() {
    let coverage = [255u8; 256];
    let tiles = coverage_to_4bpp_tiles(&coverage);
    // 255 / 17 = 15 = 0xF. Each byte = 0xFF.
    assert!(tiles.iter().all(|&b| b == 0xFF));
}

#[test]
fn coverage_to_tiles_size() {
    let coverage = [128u8; 256];
    let tiles = coverage_to_4bpp_tiles(&coverage);
    assert_eq!(tiles.len(), 128);
    // 128 / 17 = 7. Each byte = 0x77.
    assert!(tiles.iter().all(|&b| b == 0x77));
}

#[test]
fn patch_font_cel_writes_correct_offset() {
    // Glyph 0 starts at tile 438, byte offset 438 * 32 = 14016.
    let mut cel = vec![0u8; 122048];
    let tile_data = [0xAB; 128];
    patch_font_cel(&mut cel, 0, &tile_data).unwrap();
    assert_eq!(&cel[14016..14016 + 128], &[0xAB; 128]);
    // Other bytes untouched.
    assert_eq!(cel[14015], 0);
    assert_eq!(cel[14016 + 128], 0);
}

#[test]
fn patch_font_cel_glyph_index_1() {
    let mut cel = vec![0u8; 122048];
    let tile_data = [0xCD; 128];
    patch_font_cel(&mut cel, 1, &tile_data).unwrap();
    // Glyph 1 → tile 442, byte offset 442 * 32 = 14144.
    assert_eq!(&cel[14144..14144 + 128], &[0xCD; 128]);
}
