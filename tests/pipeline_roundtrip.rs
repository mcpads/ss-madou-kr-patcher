//! Integration test: verify patch_seq round-trip produces valid CNX data.

use std::collections::HashMap;
use std::path::Path;

#[test]
#[ignore] // Requires ROM file at roms/Madou_Monogatari_JAP.bin
fn patch_seq_cnx_roundtrip() {
    use ss_madou::compression;
    use ss_madou::disc::{DiscImage, Iso9660};
    use ss_madou::text::patcher::{self, PatchOptions, SeqType, TextToken, TranslationEntry};

    let rom = Path::new("roms/Madou_Monogatari_JAP.bin");
    if !rom.exists() {
        eprintln!("Skipping: ROM not found at {}", rom.display());
        return;
    }

    let disc = DiscImage::from_bin_file(rom).unwrap();
    let iso = Iso9660::parse(&disc).unwrap();

    // Test with a few different SEQ files
    for seq_name in &["MP0101.SEQ", "MP0803.SEQ", "COMMON.SEQ"] {
        let entry = match iso.find_file(&disc, seq_name).unwrap() {
            Some(e) => e,
            None => {
                eprintln!("  {} not found, skipping", seq_name);
                continue;
            }
        };

        let compressed = iso.extract_file(&disc, &entry).unwrap();
        let header = compression::parse_header(&compressed).unwrap();
        let original = compression::decompress(&compressed).unwrap();

        // Build a minimal char table
        let mut ct: HashMap<char, u16> = HashMap::new();
        ct.insert('\u{AC00}', 0x01B6); // '가'
        ct.insert(' ', 0x00B2);

        // Find the first plausible text offset (2-byte aligned, after header region)
        // Use a safe dummy entry with pad_to_original to avoid size changes
        let test_offset = find_first_text_offset(&original, seq_name);
        if test_offset == 0 {
            eprintln!("  {} no suitable text offset found, skipping", seq_name);
            continue;
        }

        let entries = vec![TranslationEntry {
            offset: test_offset,
            orig_len: 2,
            tokens: vec![TextToken::Text("\u{AC00}".into())],
            entry_id: format!("{}_TEST", seq_name),
            expected_bytes: None, // skip validation for this test
            pad_to_original: true, // pad to original to keep size stable
        }];

        let seq_type = SeqType::from_filename(seq_name);
        let opts = PatchOptions::default();

        let (patched, _ptrs) =
            patcher::apply_patches(&original, &entries, &ct, seq_type, &opts).unwrap();

        // Verify patched size is 4-byte aligned
        assert_eq!(
            patched.len() % 4,
            0,
            "{}: patched size {} not 4-byte aligned",
            seq_name,
            patched.len()
        );

        // CNX compress -> decompress round-trip
        let recompressed = compression::compress(&patched, &header.subtype);
        let decompressed = compression::decompress(&recompressed).unwrap();

        assert_eq!(
            decompressed, patched,
            "{}: CNX round-trip mismatch ({} bytes patched, {} compressed, {} decompressed)",
            seq_name,
            patched.len(),
            recompressed.len(),
            decompressed.len()
        );

        eprintln!(
            "  {} PASS: {} -> {} bytes patched, {} compressed, round-trip OK",
            seq_name,
            original.len(),
            patched.len(),
            recompressed.len()
        );
    }
}

/// Find the first 2-byte-aligned offset with a non-zero value in the text region.
/// For MP files, text typically starts after offset 0x10.
/// For COMMON, text starts after offset 0x100.
fn find_first_text_offset(data: &[u8], seq_name: &str) -> usize {
    let start = if seq_name.starts_with("COMMON") {
        0x200
    } else {
        0x20
    };
    let end = data.len().min(start + 0x100);
    for i in (start..end).step_by(2) {
        if i + 1 < data.len() && data[i] != 0 && data[i + 1] != 0 {
            return i;
        }
    }
    0
}
