use super::*;

fn test_table() -> GlyphTable {
    let csv = "index,char,confidence,source\n\
               0,あ,CONFIRMED,visual\n\
               1,い,CONFIRMED,visual\n\
               2,う,CONFIRMED,visual\n\
               15,た,CONFIRMED,visual\n\
               16,ち,CONFIRMED,visual\n\
               21,に,CONFIRMED,visual\n\
               25,は,CONFIRMED,visual\n\
               79,っ,CONFIRMED,visual\n\
               86,カ,CONFIRMED,visual";
    GlyphTable::from_csv(csv).unwrap()
}

#[test]
fn control_code_known_params() {
    assert_eq!(control_code_param_count(0xFF00), 0);
    assert_eq!(control_code_param_count(0xFF06), 2);
    assert_eq!(control_code_param_count(0xFF0F), 1);
    assert_eq!(control_code_param_count(0xFFFF), 0);
    assert_eq!(control_code_param_count(0xFF99), 0);
}

#[test]
fn format_control_no_params() {
    assert_eq!(format_control(0xFF00, &[]), "{ctrl:FF00}");
}

#[test]
fn format_control_with_params() {
    assert_eq!(
        format_control(0xFF06, &[0x0012, 0x0034]),
        "{ctrl:FF06:0012:0034}"
    );
    assert_eq!(format_control(0xFF0F, &[0x0001]), "{ctrl:FF0F:0001}");
}

#[test]
fn bytes_to_hex_formatting() {
    assert_eq!(bytes_to_hex(&[0x83, 0x41, 0xFF, 0x00]), "83 41 FF 00");
    assert_eq!(bytes_to_hex(&[]), "");
}

#[test]
fn script_entry_serializes_to_json() {
    let entry = ScriptEntry {
        id: "COMMON_0000".into(),
        offset: "0x0AEA8".into(),
        raw_hex: "01 B6 FF 00".into(),
        text: "あ{ctrl:FF00}".into(),
        ko: None,
        status: TranslationStatus::default(),
        pad_to_original: false,
        notes: String::new(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("COMMON_0000"));
    assert!(json.contains(r#""ko":null"#));
}

#[test]
fn parse_glyph_with_control() {
    let table = test_table();
    // あ (tile 438 = 0x01B6) + FF00 (newline) + い (tile 442 = 0x01BA) + FFFF
    let data = vec![
        0x01, 0xB6, // あ (glyph 0, tile 438)
        0xFF, 0x00, // newline
        0x01, 0xBA, // い (glyph 1, tile 442)
        0xFF, 0xFF, // end
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.source, "TEST.SEQ");
    assert_eq!(dump.entries.len(), 1);
    assert_eq!(dump.entries[0].id, "TEST_0000");
    assert_eq!(dump.entries[0].text, "あ{ctrl:FF00}い");
}

#[test]
fn parse_boundary_splits_entries() {
    let table = test_table();
    // あ + FF02 + FFFF + い + FF02 + FFFF → 2 entries (with ctrl codes)
    let data = vec![
        0x01, 0xB6, // あ
        0xFF, 0x02, // FF02 (line break)
        0xFF, 0xFF, // FFFF (boundary)
        0x01, 0xBA, // い
        0xFF, 0x02, // FF02 (line break)
        0xFF, 0xFF, // FFFF (boundary)
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.entries.len(), 2);
    assert!(dump.entries[0].text.contains("あ"));
    assert!(dump.entries[1].text.contains("い"));
}

#[test]
fn parse_skips_non_text_regions() {
    let table = test_table();
    // Non-text words + glyph text + non-text words
    let data = vec![
        0x00, 0x01, // non-text word (tile 1, not a glyph)
        0x00, 0x03, // non-text word
        0x01, 0xB6, // あ (glyph 0)
        0xFF, 0x00, // newline
        0x01, 0xBA, // い (glyph 1)
        0x00, 0x01, // non-text word
    ];
    let dump = parse_script(&data, "FOO.SEQ", &table);

    assert_eq!(dump.entries.len(), 1);
    assert_eq!(dump.entries[0].id, "FOO_0000");
    assert_eq!(dump.entries[0].offset, "0x00004");
    assert_eq!(dump.entries[0].text, "あ{ctrl:FF00}い");
}

#[test]
fn parse_control_only_region_not_emitted() {
    let table = test_table();
    // Only control codes, no text glyphs
    let data = vec![0xFF, 0x0F, 0x00, 0x01, 0xFF, 0xFF];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.entries.len(), 0);
}

#[test]
fn parse_control_with_params_preserved() {
    let table = test_table();
    // あ + FF06 with 2 params + FFFF
    let data = vec![
        0x01, 0xB6, // あ (glyph 0)
        0xFF, 0x06, 0x00, 0x12, 0x00, 0x34, // FF06:0012:0034
        0xFF, 0xFF, // end
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.entries.len(), 1);
    assert!(dump.entries[0].text.contains("{ctrl:FF06:0012:0034}"));
}

#[test]
fn parse_raw_hex_matches_input() {
    let table = test_table();
    let data = vec![0x01, 0xB6, 0xFF, 0x00];
    let dump = parse_script(&data, "T.SEQ", &table);

    assert_eq!(dump.entries.len(), 1);
    assert_eq!(dump.entries[0].raw_hex, "01 B6 FF 00");
}

#[test]
fn parse_md5_computed() {
    let table = test_table();
    let data = vec![0x01, 0xB6, 0x01, 0xBA];
    let dump = parse_script(&data, "T.SEQ", &table);
    assert_eq!(dump.source_md5.len(), 32);
    assert!(dump.source_md5.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn script_dump_round_trip() {
    let dump = ScriptDump {
        source: "TEST.SEQ".into(),
        source_md5: "abc123".into(),
        entries: vec![ScriptEntry {
            id: "TEST_0000".into(),
            offset: "0x00010".into(),
            raw_hex: "01 B6".into(),
            text: "あ".into(),
            ko: Some("아".into()),
            status: TranslationStatus::Done,
            pad_to_original: false,
            notes: "test".into(),
        }],
        filtered: Vec::new(),
    };
    let json = serde_json::to_string_pretty(&dump).unwrap();
    let parsed: ScriptDump = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].ko.as_deref(), Some("아"));
}

#[test]
fn parse_with_space_wide_char() {
    let table = test_table();
    // あ + space (wide 25, tile 178=0x00B2) + い + FFFF
    let data = vec![
        0x01, 0xB6, // あ
        0x00, 0xB2, // space (wide 25)
        0x01, 0xBA, // い
        0xFF, 0xFF, // end
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.entries.len(), 1);
    assert_eq!(dump.entries[0].text, "あ い");
}

#[test]
fn parse_diary_like_sequence() {
    let table = test_table();
    // Simulate: FF3D + param + metadata + FF02 + FF02 + text + FF39
    let data = vec![
        0xFF, 0x3D, 0x00, 0x00, // FF3D with param 0
        0x00, 0x27, // metadata word 1 (non-text, skipped)
        0x06, 0x20, // metadata word 2 (non-text, skipped)
        0xFF, 0x02, // FF02 (line display)
        0xFF, 0x02, // FF02 (line display)
        0x02, 0x1A, // は (glyph 25, tile 538)
        0x00, 0xB2, // space (wide 25)
        0x01, 0xF2, // た (glyph 15, tile 498)
        0x01, 0xF6, // ち (glyph 16, tile 502)
        0x02, 0x0A, // に (glyph 21, tile 522)
        0xFF, 0x39, // FF39 (section end)
    ];
    let dump = parse_script(&data, "DIARY.SEQ", &table);

    // FF3D is a boundary → starts new entry
    // Metadata words 0x0027 and 0x0620 are not text glyphs → flush (no text, skip)
    // FF02 are non-boundary controls → accumulate
    // Then glyphs → accumulate with has_text
    // FF39 → boundary, flush
    assert!(!dump.entries.is_empty());

    // Find the entry that contains the text
    let text_entry = dump
        .entries
        .iter()
        .find(|e| e.text.contains("は"))
        .expect("Should have an entry with は");
    assert!(text_entry.text.contains("は"));
    assert!(text_entry.text.contains("た"));
    assert!(text_entry.text.contains("ち"));
    assert!(text_entry.text.contains("に"));
}

#[test]
fn parse_ff3d_text_block_structure() {
    let table = test_table();
    // FF3D + param + glyphs + FF39
    let data = vec![
        0xFF, 0x3D, 0x01, 0x00, // FF3D with param 0x0100
        0xFF, 0x02, // FF02
        0x01, 0xB6, // あ
        0x01, 0xBA, // い
        0x01, 0xBE, // う
        0xFF, 0x39, // FF39
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    let text_entry = dump
        .entries
        .iter()
        .find(|e| e.text.contains("あ"))
        .expect("Should find text entry");
    assert_eq!(text_entry.text, "{ctrl:FF3D:0100}{ctrl:FF02}あいう");
}

// -- find_text_start tests ------------------------------------------------

#[test]
fn text_start_mp_uses_pointer_targets() {
    // MP file: code region with 00 24 XX XX 00 00 00 05 pointers
    let mut data = vec![0x00; 0x100];
    // Pointer at offset 0x10 → target 0x0080
    data[0x10] = 0x00;
    data[0x11] = 0x24;
    data[0x12] = 0x00;
    data[0x13] = 0x80;
    data[0x14..0x18].copy_from_slice(&[0x00, 0x00, 0x00, 0x05]);
    // Pointer at offset 0x20 → target 0x00C0
    data[0x20] = 0x00;
    data[0x21] = 0x24;
    data[0x22] = 0x00;
    data[0x23] = 0xC0;
    data[0x24..0x28].copy_from_slice(&[0x00, 0x00, 0x00, 0x05]);

    assert_eq!(find_text_start(&data, "MP0001.SEQ", None), 0x80);
}

#[test]
fn text_start_pt_uses_reversed_suffix() {
    let mut data = vec![0x00; 0x100];
    // PT pattern: 00 25 XX XX 05 00 00 00
    data[0x10] = 0x00;
    data[0x11] = 0x25;
    data[0x12] = 0x00;
    data[0x13] = 0x90;
    data[0x14..0x18].copy_from_slice(&[0x05, 0x00, 0x00, 0x00]);

    assert_eq!(find_text_start(&data, "PT0101.SEQ", None), 0x90);
}

#[test]
fn text_start_pt_fallback_to_ff30() {
    // PT without 0025 pointers → uses FF30
    let mut data = vec![0x00; 0x200];
    data[0x100] = 0xFF;
    data[0x101] = 0x30;

    assert_eq!(find_text_start(&data, "PT0401.SEQ", None), 0x100);
}

#[test]
fn text_start_diary_uses_ff3d() {
    let mut data = vec![0x00; 0x100];
    data[0x40] = 0xFF;
    data[0x41] = 0x3D;

    assert_eq!(find_text_start(&data, "DIARY.SEQ", None), 0x40);
}

#[test]
fn text_start_common_fallback_to_text_break() {
    // COMMON: no FF30, uses FF02-with-nearby-glyphs
    let mut data = vec![0x00; 0x200];
    // Place some glyphs + FF02 at offset 0x100
    data[0xFA] = 0x01;
    data[0xFB] = 0xB6; // あ (glyph 0, tile 438)
    data[0xFC] = 0x01;
    data[0xFD] = 0xBA; // い (glyph 1, tile 442)
    data[0xFE] = 0x01;
    data[0xFF] = 0xBE; // う (glyph 2, tile 446)
    data[0x100] = 0xFF;
    data[0x101] = 0x02; // FF02

    // text_start = FF02 pos (0x100) - 32 = 0xE0
    assert_eq!(find_text_start(&data, "COMMON.SEQ", None), 0x100 - 32);
}

#[test]
fn text_start_unknown_file_skips_data_only() {
    // 실제 SEQ 크기(256+)에서 텍스트 마커가 없으면 data.len() 반환
    let data = vec![0x00; 0x400];
    assert_eq!(find_text_start(&data, "BTLMAP.SEQ", None), data.len());
}

#[test]
fn text_start_mp_skips_code_region() {
    // Verify parse_script skips data before text_start
    let table = test_table();

    let mut data = vec![0x00; 0x100];
    // Code region: fake glyph at 0x04 (would be false positive without text_start)
    data[0x04] = 0x01;
    data[0x05] = 0xB6; // あ — in code region, should be skipped

    // Pointer at 0x10 → target 0x80
    data[0x10] = 0x00;
    data[0x11] = 0x24;
    data[0x12] = 0x00;
    data[0x13] = 0x80;
    data[0x14..0x18].copy_from_slice(&[0x00, 0x00, 0x00, 0x05]);

    // Real text at 0x80 (with control code to pass noise filter)
    data[0x80] = 0x01;
    data[0x81] = 0xBA; // い
    data[0x82] = 0xFF;
    data[0x83] = 0x02; // FF02 (line break)
    data[0x84] = 0xFF;
    data[0x85] = 0xFF; // FFFF

    let dump = parse_script(&data, "MP0001.SEQ", &table);

    // Should only find "い", not "あ" from code region
    assert_eq!(dump.entries.len(), 1);
    assert!(dump.entries[0].text.contains("い"));
}

#[test]
fn bare_glyphs_go_to_filtered() {
    let table = test_table();
    // 제어코드/와이드 없는 순수 글리프 → filtered
    // 제어코드 있는 글리프 → entries
    let data = vec![
        0x01, 0xB6, // あ (bare glyph, no ctrl/wide)
        0xFF, 0xFF, // FFFF (boundary)
        0x01, 0xBA, // い
        0xFF, 0x02, // FF02 (has control)
        0xFF, 0xFF, // FFFF (boundary)
    ];
    let dump = parse_script(&data, "TEST.SEQ", &table);

    assert_eq!(dump.entries.len(), 1, "entries should have 1 (with ctrl)");
    assert!(dump.entries[0].text.contains("い"));
    assert_eq!(dump.filtered.len(), 1, "filtered should have 1 (bare glyph)");
    assert!(dump.filtered[0].text.contains("あ"));
    assert!(dump.filtered[0].id.contains("_F"), "filtered ID should have _F prefix");
}

// -- make_entry / make_skill_entry factory function tests -------------------

#[test]
fn make_entry_produces_correct_fields() {
    let raw = &[0x01, 0xB6, 0xFF, 0x00];
    let entry = make_entry("TEST", 42, 0x1234, raw, "あ{ctrl:FF00}".into());

    assert_eq!(entry.id, "TEST_0042");
    assert_eq!(entry.offset, "0x01234");
    assert_eq!(entry.raw_hex, "01 B6 FF 00");
    assert_eq!(entry.text, "あ{ctrl:FF00}");
    assert!(entry.ko.is_none());
    assert_eq!(entry.status, TranslationStatus::Untranslated);
    assert!(!entry.pad_to_original);
    assert!(entry.notes.is_empty());
}

#[test]
fn make_skill_entry_produces_correct_fields() {
    let raw = &[0x02, 0xC6, 0xFF, 0x00];
    let entry = make_skill_entry("PARTY_01", 7, 0xE548, raw, "ぷ{ctrl:FF00}".into());

    assert_eq!(entry.id, "PARTY_01_S0007");
    assert_eq!(entry.offset, "0x0E548");
    assert_eq!(entry.raw_hex, "02 C6 FF 00");
    assert_eq!(entry.text, "ぷ{ctrl:FF00}");
    assert!(entry.ko.is_none());
    assert!(entry.pad_to_original, "skill entries must have pad_to_original=true");
}

// -- scan_skill_table hardcoded table tests --------------------------------

#[test]
fn scan_skill_table_returns_empty_for_unknown_file() {
    let table = test_table();
    let data = vec![0x00; 0x100];
    let entries = scan_skill_table(&data, "UNKNOWN.SEQ", &table);
    assert!(entries.is_empty());
}

#[test]
fn scan_skill_table_returns_empty_for_common_seq() {
    let table = test_table();
    let data = vec![0x00; 0x10000];
    let entries = scan_skill_table(&data, "COMMON.SEQ", &table);
    assert!(entries.is_empty());
}

#[test]
fn scan_skill_table_extracts_entries_from_synthetic_data() {
    let table = test_table();

    // Build synthetic data sized to cover PT0401.SEQ skill range (0x017E0..0x0181C)
    let mut data = vec![0x00u8; 0x01900];

    // Place 3 skill entries at the known PT0401.SEQ range:
    // Entry 1: あい (tiles 438, 442) + FF00 at offset 0x017E0
    let off = 0x017E0;
    data[off]     = 0x01; data[off + 1] = 0xB6; // あ (tile 438)
    data[off + 2] = 0x01; data[off + 3] = 0xBA; // い (tile 442)
    data[off + 4] = 0xFF; data[off + 5] = 0x00; // FF00

    // Entry 2: うあ + FF00 at offset 0x017E6
    let off2 = 0x017E6;
    data[off2]     = 0x01; data[off2 + 1] = 0xBE; // う (tile 446)
    data[off2 + 2] = 0x01; data[off2 + 3] = 0xB6; // あ (tile 438)
    data[off2 + 4] = 0xFF; data[off2 + 5] = 0x00; // FF00

    // Entry 3: いう + FF00 at offset 0x017EC
    let off3 = 0x017EC;
    data[off3]     = 0x01; data[off3 + 1] = 0xBA; // い (tile 442)
    data[off3 + 2] = 0x01; data[off3 + 3] = 0xBE; // う (tile 446)
    data[off3 + 4] = 0xFF; data[off3 + 5] = 0x00; // FF00

    let entries = scan_skill_table(&data, "PT0401.SEQ", &table);

    assert_eq!(entries.len(), 3, "should extract 3 skill entries");

    // Verify first entry
    assert_eq!(entries[0].id, "PT0401_S0000");
    assert_eq!(entries[0].offset, "0x017E0");
    assert!(entries[0].text.contains("あ"));
    assert!(entries[0].text.contains("い"));
    assert!(entries[0].text.ends_with("{ctrl:FF00}"));
    assert!(entries[0].pad_to_original);

    // Verify second entry
    assert_eq!(entries[1].id, "PT0401_S0001");
    assert_eq!(entries[1].offset, "0x017E6");

    // Verify third entry
    assert_eq!(entries[2].id, "PT0401_S0002");
    assert_eq!(entries[2].offset, "0x017EC");
}

#[test]
fn scan_skill_table_skips_0000_separator() {
    let table = test_table();

    // Build data for PT0401.SEQ with a 0000 separator between entries
    let mut data = vec![0x00u8; 0x01900];

    // Entry 1: あい + FF00
    let off = 0x017E0;
    data[off]     = 0x01; data[off + 1] = 0xB6;
    data[off + 2] = 0x01; data[off + 3] = 0xBA;
    data[off + 4] = 0xFF; data[off + 5] = 0x00;

    // 0000 separator at 0x017E6 (already zero-filled)

    // Entry 2: うあ + FF00 at 0x017E8
    let off2 = 0x017E8;
    data[off2]     = 0x01; data[off2 + 1] = 0xBE;
    data[off2 + 2] = 0x01; data[off2 + 3] = 0xB6;
    data[off2 + 4] = 0xFF; data[off2 + 5] = 0x00;

    let entries = scan_skill_table(&data, "PT0401.SEQ", &table);

    assert_eq!(entries.len(), 2, "should extract 2 entries, skipping 0000 separator");
}

#[test]
fn scan_skill_table_handles_case_insensitive_lookup() {
    let table = test_table();

    // Ensure the lookup works for upper-case filenames
    let mut data = vec![0x00u8; 0x01900];
    let off = 0x017E0;
    data[off]     = 0x01; data[off + 1] = 0xB6;
    data[off + 2] = 0x01; data[off + 3] = 0xBA;
    data[off + 4] = 0xFF; data[off + 5] = 0x00;

    // lowercase filename
    let entries = scan_skill_table(&data, "pt0401.seq", &table);
    // Depending on filename normalization this may or may not find entries;
    // the function normalizes to uppercase, so it should work
    assert!(!entries.is_empty(), "should handle lowercase filenames");
}

#[test]
fn skill_table_ranges_are_valid() {
    // Sanity check: all ranges have start < end
    for (name, ranges) in SKILL_TABLE_RANGES {
        for &(start, end) in *ranges {
            assert!(
                start < end,
                "Invalid range for {}: 0x{:05X}..0x{:05X}",
                name, start, end
            );
            // Ranges should be word-aligned
            assert_eq!(start % 2, 0, "Range start not aligned for {}", name);
            assert_eq!(end % 2, 0, "Range end not aligned for {}", name);
        }
    }
}

// -- ScriptDump backwards compatibility with legacy pad_to_original --------

#[test]
fn script_dump_deserializes_legacy_pad_to_original() {
    // Old JSON files have "pad_to_original" at the top level.
    // After removing the field from ScriptDump, deserialization should
    // still succeed (serde ignores unknown fields by default).
    let json = r#"{
        "source": "TEST.SEQ",
        "source_md5": "abc123",
        "pad_to_original": true,
        "entries": [],
        "filtered": []
    }"#;
    let dump: ScriptDump = serde_json::from_str(json).unwrap();
    assert_eq!(dump.source, "TEST.SEQ");
    assert!(dump.entries.is_empty());
}
