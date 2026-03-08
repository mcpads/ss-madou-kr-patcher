//! `decode-text` — decode tile codes ↔ Korean/Japanese text for debugging.
//!
//! Given Korean text seen on screen (garbled or normal), shows what tile codes
//! the game engine is reading and what those tile codes meant in the original
//! Japanese ROM.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use ss_madou::font::korean::{GLYPH_TILE_START, TILES_PER_GLYPH};
use ss_madou::pipeline;
use ss_madou::text::{glyph, patcher};

/// Build the full Korean char_table (same logic as build-rom step 4).
/// Returns (ko_char → tile_code, tile_code → ko_char).
fn build_ko_tables(
    translations_dir: &Path,
) -> Result<(HashMap<char, u16>, HashMap<u16, char>)> {
    let extra_dir = translations_dir
        .parent()
        .unwrap_or(translations_dir)
        .join("needs_human_review");

    let mut all_entries: Vec<patcher::TranslationEntry> = Vec::new();

    for dir in [translations_dir, &extra_dir] {
        if !dir.is_dir() {
            continue;
        }
        let mut paths: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .map(|e| e.path())
            .collect();
        paths.sort();

        for path in paths {
            let content = std::fs::read_to_string(&path)?;
            let data: serde_json::Value = serde_json::from_str(&content)?;
            let entries = data["entries"].as_array().unwrap_or(&Vec::new()).clone();
            for entry in entries {
                let ko = match entry["ko"].as_str() {
                    Some(s) if !s.is_empty() => s,
                    _ => continue,
                };
                let raw_hex = entry["raw_hex"].as_str().unwrap_or("");
                let offset_str = entry["offset"].as_str().unwrap_or("0x0");
                let offset =
                    usize::from_str_radix(offset_str.trim_start_matches("0x"), 16).unwrap_or(0);
                let orig_len = raw_hex.split_whitespace().count();
                let tokens = patcher::parse_ko_tokens(ko);
                let expected_bytes: Vec<u8> = raw_hex
                    .split_whitespace()
                    .filter_map(|h| u8::from_str_radix(h, 16).ok())
                    .collect();

                all_entries.push(patcher::TranslationEntry {
                    offset,
                    orig_len,
                    tokens,
                    entry_id: entry["id"].as_str().unwrap_or("").to_string(),
                    expected_bytes: Some(expected_bytes),
                    pad_to_original: false,
                });
            }
        }
    }

    let text_chars = patcher::collect_text_chars(&all_entries);

    let glyph_csv = std::fs::read_to_string("assets/glyph_mapping.csv")
        .context("Failed to read glyph_mapping.csv")?;
    let glyph_table = glyph::GlyphTable::from_csv(&glyph_csv)
        .map_err(|e| anyhow::anyhow!("Failed to parse glyph mapping: {}", e))?;

    let preserve = patcher::preserved_glyph_slots();
    let sec6_map = pipeline::sec6_direct_tile_map();
    let mut new_glyph_chars: Vec<char> = Vec::new();
    let mut original_tile_map: HashMap<char, u16> = HashMap::new();
    for &ch in &text_chars {
        if let Some(tile_code) = glyph_table.encode(ch) {
            let glyph_idx = ((tile_code as usize) - GLYPH_TILE_START) / TILES_PER_GLYPH;
            if tile_code >= GLYPH_TILE_START as u16 && preserve.contains(&glyph_idx) {
                original_tile_map.insert(ch, tile_code);
                continue;
            }
        }
        if let Some(&sec6_tile) = sec6_map.get(&ch) {
            original_tile_map.insert(ch, sec6_tile);
            continue;
        }
        new_glyph_chars.push(ch);
    }

    // Build frequency map for priority ordering.
    let mut char_freq: HashMap<char, usize> = HashMap::new();
    for entry in &all_entries {
        for token in &entry.tokens {
            if let patcher::TextToken::Text(s) = token {
                for ch in s.chars() {
                    if !matches!(ch, ' ' | '\u{2026}' | '\u{300C}' | '\u{300D}' | '\u{3000}') {
                        *char_freq.entry(ch).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    new_glyph_chars.sort_by(|a, b| {
        char_freq.get(b).unwrap_or(&0).cmp(&char_freq.get(a).unwrap_or(&0))
    });

    let unavailable = preserve.clone();
    let (mut char_table, _unassigned) =
        patcher::build_char_table_safe(&new_glyph_chars, &unavailable, patcher::MAX_VDP2_GLYPH_INDEX);
    char_table.extend(original_tile_map.iter().map(|(&ch, &tc)| (ch, tc)));

    // Build reverse: tile_code → ko_char
    let reverse: HashMap<u16, char> = char_table.iter().map(|(&ch, &tc)| (tc, ch)).collect();

    Ok((char_table, reverse))
}

/// Build original Japanese tile_code → char mapping from glyph_mapping.csv.
fn build_jp_table() -> Result<(HashMap<u16, char>, HashMap<char, u16>)> {
    let glyph_csv = std::fs::read_to_string("assets/glyph_mapping.csv")
        .context("Failed to read glyph_mapping.csv")?;
    let glyph_table = glyph::GlyphTable::from_csv(&glyph_csv)
        .map_err(|e| anyhow::anyhow!("Failed to parse glyph mapping: {}", e))?;

    let mut tile_to_jp: HashMap<u16, char> = HashMap::new();
    let mut jp_to_tile: HashMap<char, u16> = HashMap::new();
    for (ch, tc) in glyph_table.all_mappings() {
        tile_to_jp.insert(tc, ch);
        jp_to_tile.insert(ch, tc);
    }
    Ok((tile_to_jp, jp_to_tile))
}

pub fn cmd_decode_text(
    query: &str,
    translations_dir: &Path,
) -> Result<()> {
    let (ko_to_tile, tile_to_ko) = build_ko_tables(translations_dir)?;
    let (tile_to_jp, _jp_to_tile) = build_jp_table()?;

    // Also include sec6 wide chars and special tiles in display.
    let sec6_map = pipeline::sec6_direct_tile_map();
    // Reverse sec6: tile → char (original)
    // sec6 chars are the same in both JP and KO (re-rendered with Galmuri11)

    println!("=== Decode Text ===");
    println!("Input: {}", query);
    println!();

    // Decode each character.
    println!("{:<4}  {:<6}  {:<8}  {:<4}  {}", "Pos", "KO", "Tile", "JP", "Note");
    println!("{}", "─".repeat(60));

    for (i, ch) in query.chars().enumerate() {
        // Find tile code for this Korean char.
        if let Some(&tile_code) = ko_to_tile.get(&ch) {
            let glyph_idx = if tile_code >= GLYPH_TILE_START as u16 {
                Some(((tile_code as usize) - GLYPH_TILE_START) / TILES_PER_GLYPH)
            } else {
                None
            };

            // What was the original JP char at this tile code?
            let jp_ch = tile_to_jp.get(&tile_code).copied();

            let note = if sec6_map.values().any(|&t| t == tile_code) {
                "sec6 direct"
            } else if let Some(idx) = glyph_idx {
                if idx >= 832 && idx <= 834 {
                    "preserved icon"
                } else {
                    "sec7 glyph"
                }
            } else {
                "special"
            };

            println!(
                "{:<4}  {:<6}  0x{:04X} {:>4}  {:<4}  {}",
                i,
                ch,
                tile_code,
                glyph_idx.map(|g| format!("g{}", g)).unwrap_or_default(),
                jp_ch.map(|c| c.to_string()).unwrap_or("?".into()),
                note,
            );
        } else if ch == ' ' {
            println!(
                "{:<4}  {:<6}  0x00B2       {:<4}  space tile",
                i, "SP", "SP",
            );
        } else {
            println!(
                "{:<4}  {:<6}  {:<8}  {:<4}  NOT IN CHAR TABLE",
                i, ch, "???", "?",
            );
        }
    }

    // Also show the reverse: if this were original JP tile codes, what would they decode to?
    println!();
    println!("Korean → Japanese reverse:");
    let jp_str: String = query
        .chars()
        .map(|ch| {
            ko_to_tile
                .get(&ch)
                .and_then(|tc| tile_to_jp.get(tc))
                .copied()
                .unwrap_or('?')
        })
        .collect();
    println!("  {} → {}", query, jp_str);

    // If input looks like Japanese, also try JP → KO direction.
    let has_jp = query.chars().any(|c| {
        ('\u{3040}'..='\u{309F}').contains(&c)  // hiragana
        || ('\u{30A0}'..='\u{30FF}').contains(&c)  // katakana
        || ('\u{4E00}'..='\u{9FFF}').contains(&c)  // CJK
    });
    if has_jp {
        println!();
        println!("Japanese → Korean forward:");
        let ko_str: String = query
            .chars()
            .map(|ch| {
                _jp_to_tile
                    .get(&ch)
                    .and_then(|tc| tile_to_ko.get(tc))
                    .copied()
                    .unwrap_or('?')
            })
            .collect();
        println!("  {} → {}", query, ko_str);
    }

    Ok(())
}
