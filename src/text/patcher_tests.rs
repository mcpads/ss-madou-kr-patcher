use super::*;

fn make_char_table() -> HashMap<char, u16> {
    let mut t = HashMap::new();
    t.insert('\u{AC00}', 0x01B6); // 가
    t.insert('\u{B098}', 0x01BA); // 나
    t
}

fn entry(offset: usize, orig_len: usize, tokens: Vec<TextToken>) -> TranslationEntry {
    TranslationEntry {
        offset,
        orig_len,
        tokens,
        entry_id: String::new(),
        expected_bytes: None,
        pad_to_original: false,
    }
}

// -- SeqType detection ------------------------------------------------

#[test]
fn seq_type_from_filename() {
    assert_eq!(SeqType::from_filename("MP0001.SEQ"), SeqType::Mp);
    assert_eq!(SeqType::from_filename("mp0101.seq"), SeqType::Mp);
    assert_eq!(SeqType::from_filename("PT0001.SEQ"), SeqType::Pt);
    assert_eq!(SeqType::from_filename("PARTY_B0.SEQ"), SeqType::Pt);
    assert_eq!(SeqType::from_filename("COMMON.SEQ"), SeqType::Common);
    assert_eq!(SeqType::from_filename("DIARY.SEQ"), SeqType::Diary);
    assert_eq!(SeqType::from_filename("BTLMAP.SEQ"), SeqType::Other);
    assert_eq!(SeqType::from_filename("DG9901.SEQ"), SeqType::Other);
    assert_eq!(SeqType::from_filename("out/dec/MP0001.SEQ"), SeqType::Mp);
}

#[test]
fn seq_type_pointer_patterns() {
    let mp = SeqType::Mp.pointer_pattern().unwrap();
    assert_eq!(mp.prefix, [0x00, 0x24]);
    assert_eq!(mp.suffix, [0x00, 0x00, 0x00, 0x05]);

    let pt = SeqType::Pt.pointer_pattern().unwrap();
    assert_eq!(pt.prefix, [0x00, 0x25]);
    assert_eq!(pt.suffix, [0x05, 0x00, 0x00, 0x00]);

    assert!(SeqType::Common.pointer_pattern().is_none());
    assert!(SeqType::Diary.pointer_pattern().is_none());
    // Other SEQ types (MG_, DG*, etc.) also use 00 24 pointers
    let other = SeqType::Other.pointer_pattern().unwrap();
    assert_eq!(other.prefix, [0x00, 0x24]);
}

// -- Preserved glyph slots --------------------------------------------

#[test]
fn preserved_slots_correct() {
    let slots = preserved_glyph_slots();
    assert_eq!(slots.len(), 19); // 15 symbols + 3 icons + 1 bank boundary
    assert!(slots.contains(&161));
    assert!(slots.contains(&175));
    assert!(!slots.contains(&160));
    assert!(!slots.contains(&176));
    assert!(slots.contains(&832));
    assert!(slots.contains(&833));
    assert!(slots.contains(&834));
    assert!(!slots.contains(&835));
    assert!(slots.contains(&914)); // bank 0/1 boundary (tiles 4094-4097)
}

#[test]
fn build_char_table_skips_preserved() {
    let chars = vec!['\u{AC00}', '\u{B098}']; // 가, 나
    let preserve = preserved_glyph_slots();
    let table = build_char_table(&chars, 160, &preserve);
    let tile_160 = (GLYPH_TILE_START + 160 * TILES_PER_GLYPH) as u16;
    let tile_176 = (GLYPH_TILE_START + 176 * TILES_PER_GLYPH) as u16;
    assert_eq!(table[&'\u{AC00}'], tile_160);
    assert_eq!(table[&'\u{B098}'], tile_176);
}

// -- apply_patches (MP type) ------------------------------------------

#[test]
fn apply_patches_same_size() {
    let mut data = vec![0xAA; 20];
    data[10] = 0x01;
    data[11] = 0xC6;
    data[12] = 0xFF;
    data[13] = 0x05;

    let entries = vec![entry(10, 4, vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)])];
    let ct = make_char_table();
    let (patched, ptrs) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    assert_eq!(patched.len(), 20);
    assert_eq!(ptrs, 0);
    assert_eq!(&patched[10..14], &[0xFF, 0x02, 0xFF, 0x05]);
}

#[test]
fn apply_patches_shrink() {
    let mut data = vec![0xAA; 20];
    data[10..16].copy_from_slice(&[0x01, 0xC6, 0x01, 0xC6, 0xFF, 0x05]);

    let entries = vec![entry(10, 6, vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)])];
    let ct = make_char_table();
    let (patched, _) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // Per-entry padding: delta -2 (%4=2) → space tile inserted before trailing ctrl → delta 0
    assert_eq!(patched.len(), 20);
    // [space 00B2][FF02][FF05] = 6 bytes, same as orig_len
    assert_eq!(&patched[10..16], &[0x00, 0xB2, 0xFF, 0x02, 0xFF, 0x05]);
    assert_eq!(patched[16], 0xAA);
}

#[test]
fn apply_patches_grow() {
    let mut data = vec![0xAA; 20];
    data[10..14].copy_from_slice(&[0x01, 0xC6, 0xFF, 0x05]);

    let entries = vec![entry(
        10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, _) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // Per-entry padding: delta +2 (%4=2) → space tile before trailing ctrl → delta +4; 20+4=24
    assert_eq!(patched.len(), 24);
    // [가(01B6)][space 00B2][FF02][FF05] = 8 bytes
    assert_eq!(&patched[10..18], &[0x01, 0xB6, 0x00, 0xB2, 0xFF, 0x02, 0xFF, 0x05]);
}

// -- MP pointer fixing ------------------------------------------------

#[test]
fn fix_pointers_adjusts_shifted_targets() {
    let mut data = vec![0x00u8; 0x28];
    data[0] = 0x00;
    data[1] = 0x24;
    data[2] = 0x00;
    data[3] = 0x20;
    data[7] = 0x05;

    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[2], 0x00);
    // Per-entry padding: delta +2 (%4=2) → +2 pad → delta +4; shift is +4
    assert_eq!(patched[3], 0x24);
}

#[test]
fn fix_pointers_no_change_before_patches() {
    let mut data = vec![0x00u8; 0x30];
    data[0] = 0x00;
    data[1] = 0x24;
    data[2] = 0x00;
    data[3] = 0x08;
    data[7] = 0x05;

    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(0x10, 4, vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)])];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 0);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x08);
}

#[test]
fn fix_pointers_multiple_patches_cumulative() {
    let mut data = vec![0x00u8; 0x40];
    data[0] = 0x00;
    data[1] = 0x24;
    data[2] = 0x00;
    data[3] = 0x20;
    data[7] = 0x05;

    data[8] = 0x00;
    data[9] = 0x24;
    data[10] = 0x00;
    data[11] = 0x38;
    data[15] = 0x05;

    data[0x18] = 0x01;
    data[0x19] = 0xC6;
    data[0x1A] = 0xFF;
    data[0x1B] = 0x05;

    data[0x28] = 0x01;
    data[0x29] = 0xC6;
    data[0x2A] = 0x01;
    data[0x2B] = 0xC6;
    data[0x2C] = 0xFF;
    data[0x2D] = 0x05;

    let ct = make_char_table();
    let entries = vec![
        entry(
            0x18,
            4,
            vec![
                TextToken::Text("\u{AC00}".into()),
                TextToken::Ctrl(0xFF02),
                TextToken::Ctrl(0xFF05),
            ],
        ),
        entry(0x28, 6, vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)]),
    ];
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // Per-entry padding: entry 1 delta +2→+4, entry 2 delta -2→0
    // Cumulative shift at ptr1 target (0x20): +4, at ptr2 target (0x38): +4
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x24);
    assert_eq!(patched[10], 0x00);
    assert_eq!(patched[11], 0x3C);
    assert_eq!(ptrs_fixed, 2);
}

// -- PT pointer fixing ------------------------------------------------

#[test]
fn fix_pt_pointers_reversed_suffix() {
    let mut data = vec![0x00u8; 0x28];
    // PT pattern: 00 25 XX XX 05 00 00 00
    data[0] = 0x00;
    data[1] = 0x25;
    data[2] = 0x00;
    data[3] = 0x20;
    data[4] = 0x05; // reversed suffix starts here

    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Pt, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[2], 0x00);
    // Per-entry padding: delta +2→+4; shift is +4
    assert_eq!(patched[3], 0x24);
}

#[test]
fn fix_pt_ignores_mp_pattern() {
    let mut data = vec![0x00u8; 0x28];
    // MP prefix (0x24), but SeqType::Pt — should NOT match
    data[0] = 0x00;
    data[1] = 0x24;
    data[2] = 0x00;
    data[3] = 0x20;
    data[7] = 0x05;

    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (_, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Pt, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 0);
}

// -- MP/PT extended pointer patterns ----------------------------------

#[test]
fn fix_mp_event_descriptor_non_standard_suffix() {
    // Event descriptor: 41 03 00 00 [00 24 XX XX] [00 00 00 1D]
    // The 00 24 XX XX pointer should be fixed even though suffix != 00000005.
    let mut data = vec![0x00u8; 0x40];
    // Event descriptor at offset 0x04 (embedded pointer at offset 0x08)
    data[0x04] = 0x41;
    data[0x05] = 0x03;
    data[0x06] = 0x00;
    data[0x07] = 0x00;
    data[0x08] = 0x00; // prefix
    data[0x09] = 0x24;
    data[0x0A] = 0x00; // ptr_val = 0x0030
    data[0x0B] = 0x30;
    data[0x0C] = 0x00; // suffix 0000001D (NOT 00000005)
    data[0x0D] = 0x00;
    data[0x0E] = 0x00;
    data[0x0F] = 0x1D;

    // Text entry at 0x20, with scene start marker at 0x30
    data[0x20] = 0x01;
    data[0x21] = 0xC6;
    data[0x22] = 0xFF;
    data[0x23] = 0x05;
    // Target of pointer at 0x30 (FF0F = scene start)
    data[0x30] = 0xFF;
    data[0x31] = 0x0F;

    let entries = vec![entry(
        0x20, 4,
        vec![TextToken::Text("\u{AC00}".into()), TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // Per-entry padding: delta +2→+4; pointer target 0x30 + 4 = 0x34
    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[0x0A], 0x00);
    assert_eq!(patched[0x0B], 0x34);
}

#[test]
fn fix_mp_jump_table_consecutive_pointers() {
    // Jump table: consecutive 4-byte 0024XXXX pointers without any suffix.
    let mut data = vec![0x00u8; 0x50];
    // Jump table at offset 0x00: two consecutive pointers
    data[0x00] = 0x00;
    data[0x01] = 0x24;
    data[0x02] = 0x00;
    data[0x03] = 0x30; // ptr to 0x30
    data[0x04] = 0x00;
    data[0x05] = 0x24;
    data[0x06] = 0x00;
    data[0x07] = 0x40; // ptr to 0x40

    // Text entry at 0x20
    data[0x20] = 0x01;
    data[0x21] = 0xC6;
    data[0x22] = 0xFF;
    data[0x23] = 0x05;

    // Targets
    data[0x30] = 0xFF;
    data[0x31] = 0x0F;
    data[0x40] = 0xFF;
    data[0x41] = 0x0F;

    let entries = vec![entry(
        0x20, 4,
        vec![TextToken::Text("\u{AC00}".into()), TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // Per-entry padding: delta +2→+4; pointers shift by +4 each
    assert_eq!(ptrs_fixed, 2);
    assert_eq!(patched[0x02], 0x00);
    assert_eq!(patched[0x03], 0x34);
    assert_eq!(patched[0x06], 0x00);
    assert_eq!(patched[0x07], 0x44);
}

#[test]
fn fix_mp_skips_pointers_inside_patch_data() {
    // Ensure 0024XXXX inside a text entry is not treated as a pointer.
    let mut data = vec![0x00u8; 0x40];
    // Text entry at 0x10: contains bytes that look like 0024XXXX
    data[0x10] = 0x00;
    data[0x11] = 0x24;
    data[0x12] = 0x00;
    data[0x13] = 0x30; // looks like ptr to 0x30, but it's text data!
    data[0x14] = 0xFF;
    data[0x15] = 0x05;

    // Another text entry at 0x20
    data[0x20] = 0x01;
    data[0x21] = 0xC6;
    data[0x22] = 0xFF;
    data[0x23] = 0x05;

    data[0x30] = 0xFF;
    data[0x31] = 0x0F;

    let entries = vec![
        entry(0x10, 6, vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)]),
        entry(0x20, 4, vec![TextToken::Text("\u{AC00}".into()), TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)]),
    ];
    let ct = make_char_table();
    let (_, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default()).unwrap();

    // The 0024 inside the first entry should NOT be matched.
    assert_eq!(ptrs_fixed, 0);
}

#[test]
fn fix_pt_event_descriptor_non_standard_suffix() {
    // Same as MP test but with PT prefix (00 25).
    let mut data = vec![0x00u8; 0x40];
    data[0x08] = 0x00;
    data[0x09] = 0x25; // PT prefix
    data[0x0A] = 0x00;
    data[0x0B] = 0x30;
    data[0x0C] = 0x00; // non-standard suffix
    data[0x0D] = 0x00;
    data[0x0E] = 0x00;
    data[0x0F] = 0x11;

    data[0x20] = 0x01;
    data[0x21] = 0xC6;
    data[0x22] = 0xFF;
    data[0x23] = 0x05;
    data[0x30] = 0xFF;
    data[0x31] = 0x0F;

    let entries = vec![entry(
        0x20, 4,
        vec![TextToken::Text("\u{AC00}".into()), TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Pt, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[0x0A], 0x00);
    // Per-entry padding: delta +2→+4; pointer target 0x30 + 4 = 0x34
    assert_eq!(patched[0x0B], 0x34);
}

// -- COMMON pointer fixing -------------------------------------------

#[test]
fn fix_common_pointers_basic() {
    // Layout: [pointer at 0x00] [text at 0x10] [target at 0x20]
    // Pointer 00 20 00 20 → points to offset 0x20
    // Text at 0x10 grows by 2 bytes → target shifts to 0x22
    let mut data = vec![0x00u8; 0x28];
    // Pointer: 00 20 00 20 (points to 0x0020)
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x20;

    // Text at 0x10: 2 bytes of tile data + FF05 end
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()), // 가 → 2 bytes
            TextToken::Ctrl(0xFF02),             // +2 bytes
            TextToken::Ctrl(0xFF05),             // +2 bytes = 6 total (was 4, +2 growth)
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 1);
    // Pointer should shift from 0x0020 to 0x0024 (+4, entry padded from +2 to +4)
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x24);
}

#[test]
fn fix_common_pointers_no_false_positives() {
    // 00 20 pattern exists but points outside text table range — should NOT be fixed.
    let mut data = vec![0x00u8; 0x28];
    // Pointer with value 0x0004 (before the first patch at 0x10) — out of range
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x04;

    // Another 00 20 pattern with value pointing past file end
    data[4] = 0x00;
    data[5] = 0x20;
    data[6] = 0xFF;
    data[7] = 0xFF;

    // Text at 0x10
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Neither pointer should be modified
    assert_eq!(ptrs_fixed, 0);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x04);
    assert_eq!(patched[6], 0xFF);
    assert_eq!(patched[7], 0xFF);
}

#[test]
fn fix_common_pointers_shrink() {
    // Text shrinks: original 6 bytes → patched 4 bytes (−2 shift)
    let mut data = vec![0x00u8; 0x30];
    // Pointer at 0x00: 00 20 00 28 → points to 0x28
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x28;

    // Text at 0x10: 6 bytes
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0x01;
    data[0x13] = 0xC6;
    data[0x14] = 0xFF;
    data[0x15] = 0x05;

    let entries = vec![entry(
        0x10,
        6,
        vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)], // 4 bytes (shrink by 2)
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Per-entry padding: delta -2 → pad +2 → net delta 0. No pointer shift.
    assert_eq!(ptrs_fixed, 0);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x28);
    // Buffer unchanged (delta=0): 0x30
    assert_eq!(patched.len(), 0x30);
}

#[test]
fn fix_common_pointers_fixes_inter_patch_pointers() {
    // v3: pointers BETWEEN and AFTER patches are also fixed, using
    // position-shifted writes to account for splice layout changes.
    let mut data = vec![0x00u8; 0x34];

    // Pre-patch pointer at 0x00 → points to 0x28
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x28;

    // Text patch at 0x10 (4 bytes → 6 bytes, +2 growth)
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    // Post-patch pointer at 0x20 → points to 0x28 (also needs fixing)
    data[0x20] = 0x00;
    data[0x21] = 0x20;
    data[0x22] = 0x00;
    data[0x23] = 0x28;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Both pointers should be fixed: target 0x28 → 0x2C (+4, padded entry)
    assert_eq!(ptrs_fixed, 2);
    // Pre-patch pointer at position 0x00 (unchanged position in patched)
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x2C);
    // Post-patch pointer: original pos 0x20, shifted by +4 → patched pos 0x24
    assert_eq!(patched[0x26], 0x00);
    assert_eq!(patched[0x27], 0x2C);
}

#[test]
fn fix_common_pointers_skips_within_patch() {
    // A `00 20` pattern INSIDE a patched text entry must NOT be treated as a
    // pointer — it's text tile data that happens to match the prefix.
    let mut data = vec![0x00u8; 0x30];

    // Pre-patch pointer at 0x00 → points to 0x28
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x28;

    // Text patch at 0x10 (6 bytes), contains 00 20 at 0x12 — NOT a pointer
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0x00; // looks like 00 20 XX XX but is inside text
    data[0x13] = 0x20;
    data[0x14] = 0x00;
    data[0x15] = 0x28;

    let entries = vec![entry(
        0x10,
        6,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ], // 8 bytes (was 6, +2 growth)
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Only the pre-patch pointer at 0x00 should be fixed, not the one inside text
    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x2C); // target 0x28 + 4 = 0x2C (padded entry)
}

#[test]
fn fix_common_pointers_between_two_patches() {
    // Pointer between two patches: position-shifted write must be correct.
    let mut data = vec![0x00u8; 0x40];

    // Patch 1 at 0x08 (4→6 bytes, +2)
    data[0x08] = 0x01;
    data[0x09] = 0xC6;
    data[0x0A] = 0xFF;
    data[0x0B] = 0x05;

    // Pointer at 0x10 (between patches) → points to 0x30
    data[0x10] = 0x00;
    data[0x11] = 0x20;
    data[0x12] = 0x00;
    data[0x13] = 0x30;

    // Patch 2 at 0x20 (4→6 bytes, +2)
    data[0x20] = 0x01;
    data[0x21] = 0xC6;
    data[0x22] = 0xFF;
    data[0x23] = 0x05;

    let entries = vec![
        entry(
            0x08,
            4,
            vec![
                TextToken::Text("\u{AC00}".into()),
                TextToken::Ctrl(0xFF02),
                TextToken::Ctrl(0xFF05),
            ],
        ),
        entry(
            0x20,
            4,
            vec![
                TextToken::Text("\u{AC00}".into()),
                TextToken::Ctrl(0xFF02),
                TextToken::Ctrl(0xFF05),
            ],
        ),
    ];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 1);
    // Pointer at original 0x10: pos_shift = +4 (patch1 padded from +2 to +4)
    // → patched position = 0x14
    // Target 0x30: target_shift = +8 (both patches padded +4 each)
    // → new value = 0x38
    assert_eq!(patched[0x16], 0x00);
    assert_eq!(patched[0x17], 0x38);
}

#[test]
fn split_ff00_fixes_intra_entry_pointers() {
    // A single entry with 3 FF00-separated sub-items.  A pointer targets
    // the 3rd sub-item.  Without splitting, compute_shift gives the entry-
    // level shift; with splitting, it gives per-sub-item shift.
    let mut data = vec![0x00u8; 0x40];

    // Pointer at 0x00 → targets 3rd sub-item at 0x2C within entry
    data[0] = 0x00;
    data[1] = 0x20;
    data[2] = 0x00;
    data[3] = 0x2C;

    // Entry at 0x20 (28 bytes):
    //   sub-item 0: tile(あ) FF00         (4 bytes)
    //   sub-item 1: tile(い) tile(う) FF00 (6 bytes)
    //   sub-item 2: tile(え) FF05          (4 bytes)
    // Total: 14 bytes (rest is padding)
    // sub-item 0 starts at 0x20
    data[0x20] = 0x01; data[0x21] = 0xC6; // tile あ
    data[0x22] = 0xFF; data[0x23] = 0x00; // FF00
    // sub-item 1 starts at 0x24
    data[0x24] = 0x01; data[0x25] = 0xCA; // tile い
    data[0x26] = 0x01; data[0x27] = 0xCE; // tile う
    data[0x28] = 0xFF; data[0x29] = 0x00; // FF00
    // sub-item 2 starts at 0x2A (NOT 0x2C!)
    // Actually let me recalculate. 0x20+4=0x24, 0x24+6=0x2A.
    // Sub-item 2 at 0x2A:
    data[0x2A] = 0x01; data[0x2B] = 0xD2; // tile え
    data[0x2C] = 0xFF; data[0x2D] = 0x05; // FF05
    // Total orig_len = 14 bytes (0x20..0x2E)

    // Fix: pointer should target sub-item 2 start = 0x2A
    data[2] = 0x00;
    data[3] = 0x2A;

    // Translation: sub-item 0 shrinks (4→2), sub-item 1 same (6→6),
    // sub-item 2 same (4→4). Total: 12 bytes (-2).
    // Without split: compute_shift(ptr_val=0x2A) → 0 (target inside entry)
    // With split: sub-item 0 shrinks by 2, shift at 0x2A = -2
    let entries = vec![entry(
        0x20,
        14,
        vec![
            // sub-item 0: just ctrl → 2 bytes (was 4: tile+FF00)
            TextToken::Ctrl(0xFF00),
            // sub-item 1: 2 tiles + FF00 → 6 bytes (same)
            TextToken::Text("\u{AC00}".into()),
            TextToken::Text("\u{AC01}".into()),
            TextToken::Ctrl(0xFF00),
            // sub-item 2: 1 tile + FF05 → 4 bytes (same)
            TextToken::Text("\u{AC02}".into()),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Pointer should be fixed: 0x2A → 0x28 (sub-item 0 shrank by 2)
    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x28);
}

#[test]
fn fix_common_pointers_2byte_aligned() {
    // A `00 20` pattern at an odd offset should be ignored (not 2-byte aligned).
    let mut data = vec![0x00u8; 0x28];

    // Misaligned `00 20` at offset 1 (odd) — should NOT match
    data[1] = 0x00;
    data[2] = 0x20;
    data[3] = 0x00;
    data[4] = 0x20;

    // Aligned `00 20` at offset 6 — should match
    data[6] = 0x00;
    data[7] = 0x20;
    data[8] = 0x00;
    data[9] = 0x20;

    // Text patch at 0x10
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Only the aligned pointer at offset 6 should be fixed
    assert_eq!(ptrs_fixed, 1);
    // Misaligned at offset 1: unchanged
    assert_eq!(patched[3], 0x00);
    assert_eq!(patched[4], 0x20);
    // Aligned at offset 6: fixed from 0x0020 → 0x0024 (padded entry +4)
    assert_eq!(patched[8], 0x00);
    assert_eq!(patched[9], 0x24);
}

#[test]
fn fix_common_pointers_00_21_prefix() {
    // COMMON.SEQ pointers use 32-bit RAM addresses: 0x00200000 + offset.
    // For offsets >= 0x10000, the prefix becomes `00 21` instead of `00 20`.
    // The fixer must handle both prefixes.
    let size = 0x10020usize;
    let mut data = vec![0x00u8; size];

    // Pointer at 0x00: 00 21 00 10 → RAM 0x00210010 → file offset 0x10010
    data[0] = 0x00;
    data[1] = 0x21;
    data[2] = 0x00;
    data[3] = 0x10;

    // Text patch at 0x10000 (4→6 bytes, +2 growth)
    data[0x10000] = 0x01;
    data[0x10001] = 0xC6;
    data[0x10002] = 0xFF;
    data[0x10003] = 0x05;

    let entries = vec![entry(
        0x10000,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Pointer should be fixed: target 0x10010 → 0x10014 (+4, padded entry)
    // Full RAM: 0x00210010 → 0x00210014
    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[0], 0x00);
    assert_eq!(patched[1], 0x21);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x14);
}

#[test]
fn split_ff09_fixes_intra_entry_pointers() {
    // COMMON_0046/0047 use FF09 delimiters instead of FF00.
    // The sub-item splitter must handle FF09 so that intra-entry pointers
    // get correct per-sub-item shift computation.
    let mut data = vec![0x00u8; 0x40];

    // Entry at 0x20 with 3 FF09-separated sub-items:
    //   sub-item 0: tile(あ) FF09           (4 bytes)  → offset 0x20
    //   sub-item 1: tile(い) tile(う) FF09  (6 bytes)  → offset 0x24
    //   sub-item 2: tile(え) FF05           (4 bytes)  → offset 0x2A
    data[0x20] = 0x01; data[0x21] = 0xC6; // tile あ
    data[0x22] = 0xFF; data[0x23] = 0x09; // FF09
    data[0x24] = 0x01; data[0x25] = 0xCA; // tile い
    data[0x26] = 0x01; data[0x27] = 0xCE; // tile う
    data[0x28] = 0xFF; data[0x29] = 0x09; // FF09
    data[0x2A] = 0x01; data[0x2B] = 0xD2; // tile え
    data[0x2C] = 0xFF; data[0x2D] = 0x05; // FF05

    // Pointer at 0x00 → targets sub-item 2 at 0x2A
    data[0] = 0x00; data[1] = 0x20; data[2] = 0x00; data[3] = 0x2A;

    // Translation: sub-item 0 shrinks from 4→2 bytes (-2),
    // sub-item 1 same (6→6), sub-item 2 same (4→4).
    let entries = vec![entry(
        0x20,
        14,
        vec![
            // sub-item 0: just FF09 → 2 bytes (was 4)
            TextToken::Ctrl(0xFF09),
            // sub-item 1: 2 tiles + FF09 → 6 bytes
            TextToken::Text("\u{AC00}".into()),
            TextToken::Text("\u{AC01}".into()),
            TextToken::Ctrl(0xFF09),
            // sub-item 2: 1 tile + FF05 → 4 bytes
            TextToken::Text("\u{AC02}".into()),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Pointer should be fixed: 0x2A → 0x28 (sub-item 0 shrank by 2)
    assert_eq!(ptrs_fixed, 1);
    assert_eq!(patched[2], 0x00);
    assert_eq!(patched[3], 0x28);
}

// -- Record table false positive filtering -----------------------------

#[test]
fn fix_common_pointers_skips_record_data_fields() {
    // A value at a non-zero field offset in a record table looks like a
    // valid RAM pointer but is actually numeric data — must NOT be modified.
    //
    // equip_a starts at 0x0B7C, record size 24 (0x18).
    // Record 0: 0x0B7C (field 0 = pointer), 0x0B7C+8 = 0x0B84 (field 8 = data).
    // We place a RAM-range value at field 8 — it should be skipped.
    let size = 0x11000usize;
    let mut data = vec![0x00u8; size];

    // Legitimate pointer at record field 0 (0x0B7C) → target at 0x10010
    data[0x0B7C] = 0x00;
    data[0x0B7D] = 0x21;
    data[0x0B7E] = 0x00;
    data[0x0B7F] = 0x10;

    // False positive at record field 8 (0x0B84) — data, not a pointer
    data[0x0B84] = 0x00;
    data[0x0B85] = 0x21;
    data[0x0B86] = 0x00;
    data[0x0B87] = 0x04;

    // Text patch at 0x10000 (4→6 bytes, +2 growth)
    data[0x10000] = 0x01;
    data[0x10001] = 0xC6;
    data[0x10002] = 0xFF;
    data[0x10003] = 0x05;

    let entries = vec![entry(
        0x10000,
        4,
        vec![
            TextToken::Text("\u{AC00}".into()),
            TextToken::Ctrl(0xFF02),
            TextToken::Ctrl(0xFF05),
        ],
    )];
    let ct = make_char_table();
    let (patched, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Common, &PatchOptions::default()).unwrap();

    // Only the field-0 pointer should be fixed, not the field-8 data
    assert_eq!(ptrs_fixed, 1);
    // Field 0 at 0x0B7C: fixed
    assert_eq!(patched[0x0B7E], 0x00);
    assert_eq!(patched[0x0B7F], 0x14); // 0x10010 + 4 = 0x10014 (padded entry)
    // Field 8 at 0x0B84: UNCHANGED
    assert_eq!(patched[0x0B86], 0x00);
    assert_eq!(patched[0x0B87], 0x04);
}

#[test]
fn is_record_data_field_boundaries() {
    // Field 0 of each region should NOT be flagged as data.
    assert!(!is_record_data_field(0x0280)); // skill_rec start
    assert!(!is_record_data_field(0x0B7C)); // equip_a start
    assert!(!is_record_data_field(0x19BC)); // equip_b start
    assert!(!is_record_data_field(0x3030)); // consumable start

    // Field 0 of subsequent records should NOT be flagged.
    assert!(!is_record_data_field(0x0280 + 20));  // skill_rec record 1
    assert!(!is_record_data_field(0x0B7C + 24));  // equip_a record 1
    assert!(!is_record_data_field(0x19BC + 24));  // equip_b record 1
    assert!(!is_record_data_field(0x3030 + 12));  // consumable record 1

    // Non-zero field offsets inside regions SHOULD be flagged.
    assert!(is_record_data_field(0x0280 + 8));   // skill_rec field 8
    assert!(is_record_data_field(0x0B7C + 8));   // equip_a field 8 (0x0B84)
    assert!(is_record_data_field(0x19BC + 22));  // equip_b field 22
    assert!(is_record_data_field(0x3030 + 4));   // consumable field 4

    // Positions outside all regions should NOT be flagged.
    assert!(!is_record_data_field(0x0000));  // before skill_rec
    assert!(!is_record_data_field(0x0B54));  // between skill_rec and equip_a
    assert!(!is_record_data_field(0x32B8));  // after consumable
    assert!(!is_record_data_field(0x7000));  // undoc_data region
    assert!(!is_record_data_field(0xA000));  // text region
}

// -- Other seq type (no pointer fix) ----------------------------------

#[test]
fn apply_patches_other_type_no_pointer_fix() {
    let mut data = vec![0x00u8; 0x28];
    data[0] = 0x00;
    data[1] = 0x24;
    data[2] = 0x00;
    data[3] = 0x20;
    data[7] = 0x05;

    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;

    let entries = vec![entry(
        0x10,
        4,
        vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)],
    )];
    let ct = make_char_table();
    let (_, ptrs_fixed) = apply_patches(&data, &entries, &ct, SeqType::Other, &PatchOptions::default()).unwrap();

    assert_eq!(ptrs_fixed, 0);
}

// -- parse_ko_tokens ------------------------------------------------

#[test]
fn parse_ko_tokens_plain_text() {
    let tokens = parse_ko_tokens("가나다");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0], TextToken::Text(s) if s == "가나다"));
}

#[test]
fn parse_ko_tokens_with_ctrl() {
    let tokens = parse_ko_tokens("가{ctrl:FF02}나");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0], TextToken::Text(s) if s == "가"));
    assert!(matches!(tokens[1], TextToken::Ctrl(0xFF02)));
    assert!(matches!(&tokens[2], TextToken::Text(s) if s == "나"));
}

#[test]
fn parse_ko_tokens_multi_ctrl_params() {
    let tokens = parse_ko_tokens("{ctrl:FF06:0012:0034}");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(tokens[0], TextToken::Ctrl(0xFF06)));
    assert!(matches!(tokens[1], TextToken::Ctrl(0x0012)));
    assert!(matches!(tokens[2], TextToken::Ctrl(0x0034)));
}

#[test]
fn parse_ko_tokens_with_tile() {
    let tokens = parse_ko_tokens("가{tile:01B6}나");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0], TextToken::Text(s) if s == "가"));
    assert!(matches!(tokens[1], TextToken::Tile(0x01B6)));
    assert!(matches!(&tokens[2], TextToken::Text(s) if s == "나"));
}

#[test]
fn parse_ko_tokens_wide_tags_encoded() {
    // {wide:NNN} → tile code 128 + NNN * 2
    let tokens = parse_ko_tokens("{wide:114}가{wide:053}");
    assert_eq!(tokens.len(), 3);
    // wide:114 → 128 + 114*2 = 356 = 0x0164
    assert!(matches!(tokens[0], TextToken::Tile(356)));
    assert!(matches!(&tokens[1], TextToken::Text(s) if s == "가"));
    // wide:053 → 128 + 53*2 = 234 = 0x00EA
    assert!(matches!(tokens[2], TextToken::Tile(234)));
}

// -- Pre-patch validation ------------------------------------------------

#[test]
fn apply_patches_rejects_raw_hex_mismatch() {
    let data = vec![0xAAu8; 0x20];
    let entries = vec![TranslationEntry {
        offset: 0x10,
        orig_len: 4,
        tokens: vec![TextToken::Ctrl(0xFF05)],
        entry_id: "TEST_0001".into(),
        expected_bytes: Some(vec![0x01, 0xC6, 0xFF, 0x05]), // doesn't match 0xAA
        pad_to_original: false,
    }];
    let ct = make_char_table();
    let result = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("raw_hex mismatch"), "Expected raw_hex mismatch error, got: {}", msg);
    assert!(msg.contains("TEST_0001"), "Error should contain entry_id");
}

#[test]
fn apply_patches_accepts_matching_raw_hex() {
    let mut data = vec![0x00u8; 0x20];
    data[0x10] = 0x01;
    data[0x11] = 0xC6;
    data[0x12] = 0xFF;
    data[0x13] = 0x05;
    let entries = vec![TranslationEntry {
        offset: 0x10,
        orig_len: 4,
        tokens: vec![TextToken::Ctrl(0xFF02), TextToken::Ctrl(0xFF05)],
        entry_id: "TEST_0002".into(),
        expected_bytes: Some(vec![0x01, 0xC6, 0xFF, 0x05]),
        pad_to_original: false,
    }];
    let ct = make_char_table();
    let result = apply_patches(&data, &entries, &ct, SeqType::Mp, &PatchOptions::default());
    assert!(result.is_ok());
}

#[test]
fn apply_patches_rejects_overlapping_entries() {
    let mut data = vec![0x00u8; 0x30];
    data[0x10] = 0x01; data[0x11] = 0xC6; data[0x12] = 0xFF; data[0x13] = 0x05;
    data[0x12] = 0x01; data[0x13] = 0xC6; data[0x14] = 0xFF; data[0x15] = 0x05;
    // Two entries that overlap: first at 0x10 (len 6) covers 0x10-0x15,
    // second at 0x12 (len 4) covers 0x12-0x15.
    let entries = vec![
        entry(0x10, 6, vec![TextToken::Ctrl(0xFF05)]),
        entry(0x12, 4, vec![TextToken::Ctrl(0xFF05)]),
    ];
    let ct = make_char_table();
    let result = apply_patches(&data, &entries, &ct, SeqType::Other, &PatchOptions::default());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Overlapping"), "Expected overlap error, got: {}", msg);
}

