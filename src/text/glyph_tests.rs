use super::*;

fn sample_csv() -> &'static str {
    "index,char,confidence,source\n\
     0,あ,CONFIRMED,visual\n\
     1,い,CONFIRMED,visual\n\
     2,う,CONFIRMED,visual\n\
     25,は,CONFIRMED,visual"
}

#[test]
fn load_csv_basic() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    assert_eq!(table.glyph_count(), 26); // 0..25 inclusive
}

#[test]
fn decode_16x16_glyph() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // glyph 0 (あ) → tile 438
    assert_eq!(table.decode(438), "あ");
    // glyph 1 (い) → tile 442
    assert_eq!(table.decode(442), "い");
    // glyph 2 (う) → tile 446
    assert_eq!(table.decode(446), "う");
    // glyph 25 (は) → tile 438 + 25*4 = 538
    assert_eq!(table.decode(538), "は");
}

#[test]
fn decode_misaligned_tile_returns_raw() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // tile 439 is not aligned (438 + 1, not multiple of 4)
    assert_eq!(table.decode(439), "{tile:01B7}");
}

#[test]
fn decode_wide_space() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // wide 25 = space, tile = 128 + 25*2 = 178
    assert_eq!(table.decode(178), " ");
}

#[test]
fn decode_wide_fullwidth_space() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // wide 64 = fullwidth space, tile = 128 + 64*2 = 256
    assert_eq!(table.decode(256), "\u{3000}");
}

#[test]
fn decode_wide_unknown() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // wide 30, tile = 128 + 30*2 = 188
    assert_eq!(table.decode(188), "{wide:030}");
}

#[test]
fn decode_out_of_range() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    assert_eq!(table.decode(5000), "{tile:1388}");
}

#[test]
fn is_text_glyph_valid() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    assert!(table.is_text_glyph(438));  // あ
    assert!(table.is_text_glyph(538));  // は
    assert!(!table.is_text_glyph(439)); // misaligned
    assert!(!table.is_text_glyph(178)); // wide char, not glyph
    assert!(!table.is_text_glyph(50));  // ASCII range
}

#[test]
fn empty_table() {
    let table = GlyphTable::empty();
    assert_eq!(table.glyph_count(), 0);
    assert_eq!(table.decode(438), "{tile:01B6}");
    assert!(!table.is_text_glyph(438));
}

#[test]
fn max_tile_correct() {
    let table = GlyphTable::from_csv(sample_csv()).unwrap();
    // 26 glyphs (indices 0-25) → max_tile = 438 + 26 * 4 = 542
    assert_eq!(table.max_tile(), 542);
}

#[test]
fn max_tile_empty() {
    let table = GlyphTable::empty();
    assert_eq!(table.max_tile(), 438); // GLYPH_START
}

#[test]
fn csv_with_gaps() {
    let csv = "index,char,confidence,source\n\
               0,あ,CONFIRMED,visual\n\
               5,か,CONFIRMED,visual";
    let table = GlyphTable::from_csv(csv).unwrap();
    assert_eq!(table.glyph_count(), 6); // indices 0-5
    assert_eq!(table.decode(438), "あ");          // glyph 0
    assert_eq!(table.decode(438 + 4), "{tile:01BA}"); // glyph 1 (empty)
    assert_eq!(table.decode(438 + 20), "か");     // glyph 5
}
