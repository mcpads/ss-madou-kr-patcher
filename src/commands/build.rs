use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use ss_madou::compression;

/// Recompress a SEQ file without any text changes and write to a new ROM.
/// This isolates CNX compressor compatibility from text patching.
pub(crate) fn cmd_test_recompress(
    rom: &Path,
    seq_name: &str,
    output: &Path,
) -> Result<()> {
    use ss_madou::pipeline;
    const USER_DATA_SIZE: usize = 2048;

    println!("=== Recompress Test: {} ===\n", seq_name);

    let mut ctx = pipeline::load_disc(rom)?;

    let seq_entry = ctx
        .iso
        .find_file(ctx.disc.disc(), seq_name)?
        .context(format!("{} not found on disc", seq_name))?;

    let original_compressed = ctx
        .iso
        .extract_file(ctx.disc.disc(), &seq_entry)?;
    let original_header = compression::parse_header(&original_compressed)?;

    println!(
        "Original: {} bytes compressed, {} bytes decompressed",
        original_compressed.len(),
        original_header.decompressed_size
    );

    // Decompress.
    let decompressed = compression::decompress(&original_compressed)?;
    println!("Decompressed: {} bytes", decompressed.len());

    // Recompress the UNCHANGED data.
    let recompressed = compression::compress(&decompressed, &original_header.subtype);
    let recomp_header = compression::parse_header(&recompressed)?;
    println!(
        "Recompressed: {} bytes (header says: compressed={}, decompressed={})",
        recompressed.len(),
        recomp_header.compressed_size,
        recomp_header.decompressed_size
    );

    // Verify round-trip: decompress the recompressed data.
    let re_decompressed = compression::decompress(&recompressed)?;
    if re_decompressed == decompressed {
        println!("Round-trip verification: PASS (identical)");
    } else {
        println!(
            "Round-trip verification: FAIL ({} bytes differ!)",
            re_decompressed
                .iter()
                .zip(&decompressed)
                .filter(|(a, b)| a != b)
                .count()
        );
        anyhow::bail!("CNX round-trip failed — compressor bug!");
    }

    // Write recompressed data to disc.
    let original_sectors =
        (original_compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let new_sectors = (recompressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

    if new_sectors > original_sectors {
        ctx.iso
            .relocate_file_tracked(&mut ctx.disc, seq_name, &recompressed)?;
        println!("Relocated to new LBA (grew from {} to {} sectors)", original_sectors, new_sectors);
    } else {
        let label = format!("{}:recompress-test", seq_name);
        ctx.disc
            .write_file_at(seq_entry.lba, &recompressed, &label)?;
        if recompressed.len() != original_compressed.len() {
            ctx.iso
                .patch_file_size_tracked(&mut ctx.disc, seq_name, recompressed.len() as u32)?;
            println!(
                "In-place write, ISO size updated: {} → {}",
                original_compressed.len(),
                recompressed.len()
            );
        } else {
            println!("In-place write, same size");
        }
    }

    // Save.
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    pipeline::save_disc(&mut ctx, output)?;
    println!(
        "\nTest ROM: {}",
        output.with_extension("cue").display()
    );

    Ok(())
}

/// Recompress ALL CNX files on the disc without text changes.
/// Tests whether the compressor output is game-compatible.
pub(crate) fn cmd_test_recompress_all(rom: &Path, output: &Path) -> Result<()> {
    use ss_madou::pipeline;
    const USER_DATA_SIZE: usize = 2048;

    println!("=== Recompress ALL CNX files ===\n");

    let mut ctx = pipeline::load_disc(rom)?;

    // Collect all files from ISO directory
    let all_files = ctx.iso.list_root(ctx.disc.disc())?;
    let mut recompressed_count = 0;
    let mut skipped = 0;
    let mut total_delta: isize = 0;

    for entry in &all_files {
        if entry.is_directory || entry.size == 0 {
            continue;
        }

        let file_data = match ctx.iso.extract_file(ctx.disc.disc(), entry) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if !compression::is_cnx(&file_data) {
            skipped += 1;
            continue;
        }

        let header = match compression::parse_header(&file_data) {
            Ok(h) => h,
            Err(_) => { skipped += 1; continue; }
        };

        let decompressed = match compression::decompress(&file_data) {
            Ok(d) => d,
            Err(_) => { skipped += 1; continue; }
        };

        let recompressed = compression::compress(&decompressed, &header.subtype);

        // Verify round-trip
        let verify = compression::decompress(&recompressed)?;
        if verify != decompressed {
            anyhow::bail!("Round-trip FAIL for {}", entry.name);
        }

        let delta = recompressed.len() as isize - file_data.len() as isize;
        total_delta += delta;

        // Pad to original size if smaller, relocate if needs more sectors
        let original_sectors = (file_data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
        let new_sectors = (recompressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

        if recompressed.len() <= file_data.len() {
            let mut padded = recompressed;
            padded.resize(file_data.len(), 0x00);
            let label = format!("{}:recomp-pad", entry.name);
            ctx.disc.write_file_at(entry.lba as u32, &padded, &label)?;
        } else if new_sectors > original_sectors {
            ctx.iso.relocate_file_tracked(&mut ctx.disc, &entry.name, &recompressed)?;
            println!("  {} relocated ({} → {} sectors)", entry.name, original_sectors, new_sectors);
        } else {
            let label = format!("{}:recomp", entry.name);
            ctx.disc.write_file_at(entry.lba as u32, &recompressed, &label)?;
            ctx.iso.patch_file_size_tracked(&mut ctx.disc, &entry.name, recompressed.len() as u32)?;
        }

        recompressed_count += 1;
    }

    println!("\nRecompressed: {} files, skipped: {}, total delta: {:+} bytes", recompressed_count, skipped, total_delta);

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    pipeline::save_disc(&mut ctx, output)?;
    println!("Test ROM: {}", output.with_extension("cue").display());

    Ok(())
}

/// Dry-run glyph allocation: show assigned/unassigned chars with frequency.
/// No ROM needed — only scans translation JSONs.
pub(crate) fn cmd_check_glyphs(
    translations_dir: &Path,
    verbose: bool,
) -> Result<()> {
    use ss_madou::font::korean::{GLYPH_TILE_START, TILES_PER_GLYPH};
    use ss_madou::pipeline;
    use ss_madou::text::patcher;
    use ss_madou::text::script::TranslationStatus;
    use std::collections::HashMap;

    println!("=== Check Glyph Allocation (dry-run) ===\n");

    // 1. Scan translation JSONs (shared with build-rom).
    let scan = ss_madou::text::translation_scan::scan_translation_jsons(translations_dir)?;
    println!("Scanned {} JSON files", scan.json_paths.len());

    // 2. Flatten patch entries.
    let all_entries_flat: Vec<patcher::TranslationEntry> = scan
        .all_patch_entries
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();

    // 3. Collect unique chars.
    let text_chars = patcher::collect_text_chars(&all_entries_flat);
    let glyph_csv = fs::read_to_string("assets/glyph_mapping.csv")
        .context("Failed to read glyph_mapping.csv")?;
    let glyph_table = ss_madou::text::glyph::GlyphTable::from_csv(&glyph_csv)
        .map_err(|e| anyhow::anyhow!("Failed to parse glyph mapping: {}", e))?;

    let preserve = patcher::preserved_glyph_slots();
    let sec6_map = pipeline::sec6_direct_tile_map();
    let mut new_glyph_chars: Vec<char> = Vec::new();
    let mut original_tile_map: HashMap<char, u16> = HashMap::new();
    let mut sec6_direct_count = 0usize;
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
            sec6_direct_count += 1;
            continue;
        }
        new_glyph_chars.push(ch);
    }

    // 4. Frequency map + per-char file/chapter tracking.
    //    Uses seq_groups from the scan result instead of re-reading JSONs.
    let mut char_freq: HashMap<char, usize> = HashMap::new();
    // char -> set of (source_filename, entry_id, ko_text_snippet)
    let mut char_sources: HashMap<char, Vec<(String, String, String)>> = HashMap::new();
    for (source, dumps) in &scan.seq_groups {
        for dump in dumps {
            for entry in &dump.entries {
                if let Some(ko) = &entry.ko {
                    if ko.is_empty() { continue; }
                    if !matches!(entry.status, TranslationStatus::NeedsReview | TranslationStatus::NeedsHumanReview | TranslationStatus::Done) {
                        continue;
                    }
                    let snippet = if ko.len() > 80 {
                        format!("{}...", &ko[..ko.char_indices().take(60).last().map(|(i,_)|i).unwrap_or(80)])
                    } else {
                        ko.clone()
                    };
                    for ch in ko.chars() {
                        if !matches!(ch, ' ' | '\u{2026}' | '\u{300C}' | '\u{300D}' | '\u{3000}') {
                            *char_freq.entry(ch).or_insert(0) += 1;
                            char_sources.entry(ch).or_default().push((source.clone(), entry.id.clone(), snippet.clone()));
                        }
                    }
                }
            }
        }
    }

    new_glyph_chars.sort_by(|a, b| {
        char_freq.get(b).unwrap_or(&0).cmp(&char_freq.get(a).unwrap_or(&0))
    });

    // 5. Slot assignment.
    let unavailable = preserve.clone();
    let (mut char_table, mut unassigned) = patcher::build_char_table_safe(
        &new_glyph_chars, &unavailable, patcher::MAX_VDP2_GLYPH_INDEX,
    );
    char_table.extend(original_tile_map.iter().map(|(&ch, &tc)| (ch, tc)));

    // Sec6 reclaim.
    let mut sec6_reclaimed = 0usize;
    if !unassigned.is_empty() {
        let reclaim_count = unassigned.len().min(pipeline::SEC6_RECLAIM_TILES.len());
        let mut still_unassigned = Vec::new();
        for (i, &ch) in unassigned.iter().enumerate() {
            if i < reclaim_count {
                let tile_code = pipeline::SEC6_RECLAIM_TILES[i] as u16;
                char_table.insert(ch, tile_code);
                sec6_reclaimed += 1;
            } else {
                still_unassigned.push(ch);
            }
        }
        unassigned = still_unassigned;
    }

    let assigned_count = new_glyph_chars.len() - unassigned.len();
    let preserved_used = original_tile_map.len() - sec6_direct_count;
    let korean_need = new_glyph_chars.iter().filter(|c| ('\u{AC00}'..='\u{D7A3}').contains(c)).count();
    let other_need = new_glyph_chars.len() - korean_need;

    // 6. Report.
    println!("\n--- Slot Summary ---");
    println!("Unique chars in translations: {}", text_chars.len());
    println!("  Sec6 direct-mapped (no slot needed): {}", sec6_direct_count);
    println!("  Preserved slots reused: {} (symbols 161-175, icons 832-834)", preserved_used);
    println!("  Glyphs needing new slots: {} ({} Korean + {} other)", new_glyph_chars.len(), korean_need, other_need);
    println!();
    println!("VDP2 limit: 914 slots (12-bit PND)");
    println!("  Preserved (blocked): {} slots", preserve.len());
    println!("  Available for Korean: {}", 914 - preserve.len());
    println!("  Sec6 reclaimed: +{}", sec6_reclaimed);
    println!("  Assigned: {}", assigned_count);
    println!("  Unassigned (blank): {}", unassigned.len());
    println!();

    // Assigned chars (sorted by freq desc).
    let mut assigned_chars: Vec<(char, usize)> = new_glyph_chars.iter()
        .filter(|c| !unassigned.contains(c))
        .map(|&c| (c, *char_freq.get(&c).unwrap_or(&0)))
        .collect();
    assigned_chars.sort_by(|a, b| b.1.cmp(&a.1));

    println!("--- Assigned: {} chars ---", assigned_chars.len());
    // Print in compact rows
    for row in assigned_chars.chunks(10) {
        let line: Vec<String> = row.iter()
            .map(|(ch, freq)| format!("{}({})", ch, freq))
            .collect();
        println!("  {}", line.join(" "));
    }

    if !unassigned.is_empty() {
        println!("\n--- Unassigned: {} chars (will render as BLANK) ---", unassigned.len());
        let mut unassigned_with_freq: Vec<(char, usize)> = unassigned.iter()
            .map(|&c| (c, *char_freq.get(&c).unwrap_or(&0)))
            .collect();
        unassigned_with_freq.sort_by(|a, b| b.1.cmp(&a.1));
        for row in unassigned_with_freq.chunks(10) {
            let line: Vec<String> = row.iter()
                .map(|(ch, freq)| format!("{}({})", ch, freq))
                .collect();
            println!("  {}", line.join(" "));
        }

        if verbose {
            println!("\n--- Unassigned char usage examples ---");
            for &(ch, freq) in &unassigned_with_freq {
                if let Some(sources) = char_sources.get(&ch) {
                    println!("  [{}] freq={}", ch, freq);
                    let mut seen = std::collections::HashSet::new();
                    let mut shown = 0usize;
                    for (fname, eid, snippet) in sources {
                        let key = (fname.as_str(), eid.as_str());
                        if !seen.insert(key) { continue; }
                        let hl = snippet.replace(ch, &format!("【{}】", ch));
                        let truncated = if hl.len() > 120 {
                            let end = hl.char_indices().take(80).last().map(|(i,_)|i).unwrap_or(120);
                            format!("{}...", &hl[..end])
                        } else {
                            hl
                        };
                        println!("    {} #{}: {}", fname, eid, truncated);
                        shown += 1;
                        if shown >= 3 {
                            let total_unique = sources.iter()
                                .map(|(f, e, _)| (f.as_str(), e.as_str()))
                                .collect::<std::collections::HashSet<_>>().len();
                            if total_unique > 3 {
                                println!("    ... 외 {}건", total_unique - 3);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    println!("\n--- Frequency Top 50 (overall) ---");
    let mut all_freq: Vec<(char, usize)> = char_freq.iter().map(|(&c, &f)| (c, f)).collect();
    all_freq.sort_by(|a, b| b.1.cmp(&a.1));
    for row in all_freq.iter().take(50).collect::<Vec<_>>().chunks(10) {
        let line: Vec<String> = row.iter()
            .map(|(ch, freq)| format!("{}({})", ch, freq))
            .collect();
        println!("  {}", line.join(" "));
    }

    // 7. Single-file / single-chapter analysis for assigned chars.
    // Extract chapter prefix from filename: MP0101_03.json -> "MP01", DIARY_01.json -> "DIARY", COMMON_02.json -> "COMMON"
    fn chapter_of(fname: &str) -> &str {
        let base = fname.strip_suffix(".json").unwrap_or(fname);
        // MP0101_03 -> MP01, PT0901 -> PT09, DIARY_01 -> DIARY, COMMON_02 -> COMMON, DM_END_05 -> DM_END, DUNG01_01 -> DUNG01, TITLE -> TITLE
        if base.starts_with("MP") && base.len() >= 4 {
            &base[..4]  // MP01, MP02, ...
        } else if base.starts_with("PT") && base.len() >= 4 {
            &base[..4]  // PT09, PT10, ...
        } else if base.starts_with("DUNG") && base.len() >= 6 {
            &base[..6]  // DUNG01, DUNG02, ...
        } else if base.starts_with("DM_END") {
            "DM_END"
        } else {
            // DIARY_01 -> DIARY, COMMON_02 -> COMMON, TITLE -> TITLE
            base.split('_').next().unwrap_or(base)
        }
    }

    use std::collections::HashSet;

    // Build per-char unique files and chapters (only for assigned chars needing new slots)
    let mut single_file_chars: Vec<(char, usize, String)> = Vec::new(); // (char, freq, filename)
    let mut single_chapter_chars: Vec<(char, usize, String)> = Vec::new(); // (char, freq, chapter)

    for &(ch, freq) in &assigned_chars {
        if let Some(sources) = char_sources.get(&ch) {
            let unique_files: HashSet<&str> = sources.iter().map(|(f, _, _)| f.as_str()).collect();
            let unique_chapters: HashSet<&str> = sources.iter().map(|(f, _, _)| chapter_of(f)).collect();

            if unique_files.len() == 1 {
                single_file_chars.push((ch, freq, unique_files.into_iter().next().unwrap().to_string()));
            }
            if unique_chapters.len() == 1 {
                single_chapter_chars.push((ch, freq, unique_chapters.into_iter().next().unwrap().to_string()));
            }
        }
    }

    // Sort by freq ascending (rarest first = best candidates for removal)
    single_file_chars.sort_by(|a, b| a.1.cmp(&b.1));
    single_chapter_chars.sort_by(|a, b| a.1.cmp(&b.1));

    println!("\n--- Assigned but single-file only: {} chars (slot reclaim candidates) ---", single_file_chars.len());
    for (ch, freq, fname) in &single_file_chars {
        // Show example usage
        let examples: Vec<String> = char_sources.get(ch).unwrap()
            .iter()
            .map(|(_, eid, snippet)| {
                let hl = snippet.replace(*ch, &format!("【{}】", ch));
                let truncated = if hl.len() > 100 {
                    let end = hl.char_indices().take(70).last().map(|(i,_)|i).unwrap_or(100);
                    format!("{}...", &hl[..end])
                } else {
                    hl
                };
                format!("#{}: {}", eid, truncated)
            })
            .collect::<HashSet<_>>() // dedup
            .into_iter()
            .take(2)
            .collect();
        println!("  {} freq={} file={} | {}", ch, freq, fname, examples.join(" / "));
    }

    println!("\n--- Assigned but single-chapter only: {} chars ---", single_chapter_chars.len());
    // Group by chapter for cleaner output
    let mut by_chapter: std::collections::BTreeMap<String, Vec<(char, usize)>> = std::collections::BTreeMap::new();
    for (ch, freq, chap) in &single_chapter_chars {
        by_chapter.entry(chap.clone()).or_default().push((*ch, *freq));
    }
    for (chap, mut chars) in by_chapter {
        chars.sort_by(|a, b| a.1.cmp(&b.1));
        let line: Vec<String> = chars.iter().map(|(ch, freq)| format!("{}({})", ch, freq)).collect();
        println!("  [{}] {} chars: {}", chap, chars.len(), line.join(" "));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// build-rom stage helpers
// ---------------------------------------------------------------------------

use ss_madou::font::korean::{GLYPH_TILE_START, TILES_PER_GLYPH, TILE_BYTES_PUB};
use ss_madou::pipeline;
use ss_madou::text::patcher;
use std::collections::HashMap;

/// Result of the glyph-allocation stage, passed to later stages.
struct AllocResult {
    /// Character → tile code mapping (Korean glyphs + preserved + sec6 direct).
    char_table: HashMap<char, u16>,
    /// Characters that need new glyph rendering (excludes preserved / sec6 direct).
    new_glyph_chars: Vec<char>,
    /// Characters that could not be assigned a slot (VDP2 12-bit overflow).
    unassigned: Vec<char>,
    /// Number of new-glyph chars that were assigned slots.
    assigned_count: usize,
    /// Number of sec6 tiles reclaimed for extra Korean glyphs.
    sec6_reclaimed: usize,
    /// Highest glyph index used in char_table (for FONT.CEL sizing).
    max_glyph_used: usize,
    /// Total unique text characters (for summary).
    text_char_count: usize,
}

/// Result of the SEQ-patching stage, passed to finalize.
struct SeqPatchResult {
    seqs_patched: usize,
    seqs_relocated: usize,
    seqs_skipped: usize,
    total_ptrs_fixed: usize,
    seq_new_sizes: Vec<(String, usize)>,
}

/// Stage 1: Overflow check — warn but don't block the build.
fn build_stage_overflow_check(seq_groups: &HashMap<String, Vec<ss_madou::text::script::ScriptDump>>) {
    use ss_madou::text::overflow;
    let mut overflow_count = 0usize;
    for (source, dumps) in seq_groups {
        let seq_type = patcher::SeqType::from_filename(source);
        let limit = overflow::limit_for_seq_type(seq_type);
        for dump in dumps {
            let violations = overflow::check_script(dump, &limit, seq_type);
            for v in &violations {
                overflow_count += 1;
                match &v.kind {
                    overflow::ViolationKind::LineOverflow {
                        line_index,
                        char_count,
                        limit,
                        line_text,
                    } => {
                        eprintln!(
                            "  WARN overflow: {} line {} -- {} chars (limit {}) \"{}\"",
                            v.entry_id, line_index + 1, char_count, limit, line_text
                        );
                    }
                    overflow::ViolationKind::TooManyLines { line_count, limit } => {
                        eprintln!(
                            "  WARN overflow: {} -- {} lines (limit {})",
                            v.entry_id, line_count, limit
                        );
                    }
                }
            }
        }
    }
    if overflow_count > 0 {
        eprintln!(
            "\n  {} text overflow warning(s) -- run `check-overflow --verbose` for details\n",
            overflow_count
        );
    }
}

/// Stage 2: Collect chars, compute frequencies, assign glyph slots, reclaim sec6.
fn build_stage_allocate(
    all_patch_entries: &HashMap<String, Vec<patcher::TranslationEntry>>,
) -> Result<AllocResult> {
    // Flatten patch entries.
    let all_entries_flat: Vec<patcher::TranslationEntry> = all_patch_entries
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();
    let text_chars = patcher::collect_text_chars(&all_entries_flat);

    let glyph_csv_path = "assets/glyph_mapping.csv";
    let glyph_csv = fs::read_to_string(glyph_csv_path)
        .context(format!("Failed to read {}", glyph_csv_path))?;
    let glyph_table = ss_madou::text::glyph::GlyphTable::from_csv(&glyph_csv)
        .map_err(|e| anyhow::anyhow!("Failed to parse glyph mapping: {}", e))?;

    let preserve = patcher::preserved_glyph_slots();
    let sec6_map = pipeline::sec6_direct_tile_map();
    let mut new_glyph_chars: Vec<char> = Vec::new();
    let mut original_tile_map: HashMap<char, u16> = HashMap::new();
    let mut sec6_direct_count = 0usize;
    for &ch in &text_chars {
        // Check 1: preserved icon slots (832-834)
        if let Some(tile_code) = glyph_table.encode(ch) {
            let glyph_idx = ((tile_code as usize) - GLYPH_TILE_START) / TILES_PER_GLYPH;
            if tile_code >= GLYPH_TILE_START as u16 && preserve.contains(&glyph_idx) {
                original_tile_map.insert(ch, tile_code);
                continue;
            }
        }
        // Check 2: sec6에 이미 렌더링된 문자 → sec6 타일코드 직접 사용
        if let Some(&sec6_tile) = sec6_map.get(&ch) {
            original_tile_map.insert(ch, sec6_tile);
            sec6_direct_count += 1;
            continue;
        }
        new_glyph_chars.push(ch);
    }

    let korean_count = new_glyph_chars.iter().filter(|c| ('\u{AC00}'..='\u{D7A3}').contains(c)).count();
    println!("\nUnique text characters: {} total", text_chars.len());
    println!(
        "  Glyphs needed: {} ({} Korean + {} other)",
        new_glyph_chars.len(), korean_count, new_glyph_chars.len() - korean_count
    );
    if sec6_direct_count > 0 {
        println!(
            "  Sec6 direct-mapped: {} (bypassed slot allocation)",
            sec6_direct_count
        );
    }
    if !original_tile_map.is_empty() {
        let icon_count = original_tile_map.len() - sec6_direct_count;
        if icon_count > 0 {
            println!(
                "  Preserved icon tiles reused: {} (slots 832-834)",
                icon_count
            );
        }
    }

    // Build character frequency map for priority-based slot assignment.
    // All game text (menus + dialogue) uses VDP2 NBG3 (confirmed by emulator),
    // so ALL chars must fit within the 12-bit PND limit (max 914 glyph slots).
    let mut char_freq: HashMap<char, usize> = HashMap::new();
    for entry in &all_entries_flat {
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

    // Sort new_glyph_chars by frequency (highest first) so the most-used
    // chars get slots and rare chars overflow gracefully.
    new_glyph_chars.sort_by(|a, b| {
        char_freq.get(b).unwrap_or(&0).cmp(&char_freq.get(a).unwrap_or(&0))
    });

    // Build unavailable-slot set: preserved slots only.
    let unavailable = preserve.clone();

    // Assign glyph slots within VDP2-safe range (slots 0-913).
    // Chars that don't fit are returned as `unassigned` and will render
    // as blank spaces (FALLBACK_TILE) in-game.
    let (mut char_table, mut unassigned) = patcher::build_char_table_safe(
        &new_glyph_chars, &unavailable, patcher::MAX_VDP2_GLYPH_INDEX,
    );
    // Merge preserved icon tiles back into char_table.
    char_table.extend(original_tile_map.iter().map(|(&ch, &tc)| (ch, tc)));

    // Reclaim unused sec6 slots for additional Korean glyphs.
    // These slots had 0 references (top+bottom) in all translations.
    let mut sec6_reclaimed = 0usize;
    if !unassigned.is_empty() {
        let reclaim_count = unassigned.len().min(pipeline::SEC6_RECLAIM_TILES.len());
        let mut still_unassigned = Vec::new();
        for (i, &ch) in unassigned.iter().enumerate() {
            if i < reclaim_count {
                let tile_code = pipeline::SEC6_RECLAIM_TILES[i] as u16;
                char_table.insert(ch, tile_code);
                sec6_reclaimed += 1;
            } else {
                still_unassigned.push(ch);
            }
        }
        unassigned = still_unassigned;
    }

    let assigned_count = new_glyph_chars.len() - unassigned.len();

    // Compute the max glyph index used (for FONT.CEL sizing).
    let max_glyph_used = char_table.values()
        .filter(|&&tc| tc >= GLYPH_TILE_START as u16)
        .map(|&tc| ((tc as usize) - GLYPH_TILE_START) / TILES_PER_GLYPH)
        .max()
        .unwrap_or(843);

    println!(
        "Glyph slots assigned: {} / {} ({} sec6 reclaimed) = {} total (max index {})",
        assigned_count, new_glyph_chars.len(), sec6_reclaimed,
        char_table.len(), max_glyph_used
    );
    if !unassigned.is_empty() {
        println!(
            "  {} chars unassigned (VDP2 12-bit limit, will render as blank):",
            unassigned.len()
        );
        // Show first 20 unassigned chars with their frequency
        for (_, &ch) in unassigned.iter().enumerate().take(20) {
            let freq = char_freq.get(&ch).unwrap_or(&0);
            println!("    '{}' (U+{:04X}, freq={})", ch, ch as u32, freq);
        }
        if unassigned.len() > 20 {
            println!("    ... and {} more", unassigned.len() - 20);
        }
    }

    Ok(AllocResult {
        char_table,
        new_glyph_chars,
        unassigned,
        assigned_count,
        sec6_reclaimed,
        max_glyph_used,
        text_char_count: text_chars.len(),
    })
}

/// Stage 3: Load ROM, extract font, extend FONT.CEL, render sec6, generate Korean glyphs, patch font.
fn build_stage_font(
    ctx: &mut pipeline::DiscCtx,
    font_ctx: &mut pipeline::FontCtx,
    alloc: &AllocResult,
    font_path: &Path,
    font_size: f32,
) -> Result<()> {
    // Extend FONT.CEL to fit all VDP2-safe glyph slots (0-913).
    // The decompression buffer is relocated from 0x0607CC60 (Work RAM High,
    // overlaps BSS) to 0x002C0000 (Work RAM Low, 256 KB free during boot).
    let required_tiles = GLYPH_TILE_START + (alloc.max_glyph_used + 1) * TILES_PER_GLYPH;
    let required_size = required_tiles * TILE_BYTES_PUB;
    if required_size > font_ctx.font_cel.len() {
        println!(
            "Extending FONT.CEL: {} → {} bytes ({} → {} glyphs)",
            font_ctx.font_cel.len(), required_size,
            844, alloc.max_glyph_used + 1
        );
        font_ctx.font_cel.resize(required_size, 0);
    }

    // Re-render sec6 characters (digits, punctuation, Latin) with Korean font.
    println!("Re-rendering sec6 characters with {}...", font_path.display());
    pipeline::render_sec6_glyphs(font_ctx, font_path, font_size)?;

    // Generate and patch Korean glyphs (sec7 + sec6 reclaimed, skip unassigned).
    let assigned_chars: Vec<char> = alloc.new_glyph_chars.iter()
        .filter(|c| alloc.char_table.contains_key(c))
        .copied()
        .collect();
    let glyph_tiles = pipeline::generate_korean_glyphs(font_path, &assigned_chars, font_size)?;
    pipeline::patch_font(ctx, font_ctx, &glyph_tiles, &alloc.char_table)?;

    println!(
        "Patched {} glyphs ({} sec6 reclaimed)",
        glyph_tiles.len(), alloc.sec6_reclaimed
    );

    Ok(())
}

/// Stage 4: Patch each SEQ file with CLI/env-var filtering.
fn build_stage_seq(
    ctx: &mut pipeline::DiscCtx,
    all_patch_entries: &HashMap<String, Vec<patcher::TranslationEntry>>,
    char_table: &HashMap<char, u16>,
    patch_opts: &patcher::PatchOptions,
    skip_seq: bool,
    only_seq: &[String],
    except_seq: &[String],
) -> Result<SeqPatchResult> {
    const SEQ_SKIP_LIST: &[&str] = &[];
    // CLI flags take priority, fall back to env vars for backward compat.
    let skip_seq = skip_seq || std::env::var("SKIP_SEQ").is_ok();
    let only_seq: Vec<String> = if only_seq.is_empty() {
        std::env::var("ONLY_SEQ").ok().into_iter().collect()
    } else {
        only_seq.to_vec()
    };
    let except_seq: Vec<String> = if except_seq.is_empty() {
        std::env::var("EXCEPT_SEQ").ok().into_iter().collect()
    } else {
        except_seq.to_vec()
    };
    print!("\nPatching SEQ files...");
    if skip_seq { print!(" (SKIPPED)"); }
    if !only_seq.is_empty() { print!(" (ONLY matching {:?})", only_seq); }
    if !except_seq.is_empty() { print!(" (EXCEPT matching {:?})", except_seq); }
    println!();

    let mut result = SeqPatchResult {
        seqs_patched: 0,
        seqs_relocated: 0,
        seqs_skipped: 0,
        total_ptrs_fixed: 0,
        seq_new_sizes: Vec::new(),
    };

    if !skip_seq {
        let mut sorted_sources: Vec<String> = all_patch_entries.keys().cloned().collect();
        sorted_sources.sort();

        for source in &sorted_sources {
            if SEQ_SKIP_LIST.iter().any(|s| source.eq_ignore_ascii_case(s)) {
                println!("  {} → SKIPPED (menu/data structure, no pointer support)", source);
                result.seqs_skipped += 1;
                continue;
            }
            if !only_seq.is_empty() {
                let src_upper = source.to_ascii_uppercase();
                if !only_seq.iter().any(|pat| src_upper.contains(&pat.to_ascii_uppercase())) {
                    result.seqs_skipped += 1;
                    continue;
                }
            }
            if !except_seq.is_empty() {
                let src_upper = source.to_ascii_uppercase();
                if except_seq.iter().any(|pat| src_upper.contains(&pat.to_ascii_uppercase())) {
                    result.seqs_skipped += 1;
                    continue;
                }
            }
            let entries = &all_patch_entries[source];
            let (ptrs_fixed, relocated, new_decomp_size) = pipeline::patch_seq(ctx, source, entries, char_table, patch_opts)?;
            result.seqs_patched += 1;
            result.total_ptrs_fixed += ptrs_fixed;
            if relocated { result.seqs_relocated += 1; }
            if new_decomp_size > 0 {
                result.seq_new_sizes.push((source.clone(), new_decomp_size));
            }
        }
    }

    Ok(result)
}

/// Stage 5: Patch 1ST_READ.BIN, prologue sprite, battle UI, save disc, generate BPS, print summary.
fn build_stage_finalize(
    ctx: &mut pipeline::DiscCtx,
    font_ctx: &pipeline::FontCtx,
    alloc: &AllocResult,
    seq_result: &SeqPatchResult,
    rom: &Path,
    output: &Path,
    prologue_font: Option<&Path>,
    prologue_font_size: f32,
    battle_ui_font: Option<&Path>,
    battle_ui_font_size: f32,
    menu_tab_font: Option<&Path>,
    menu_tab_font_size: f32,
) -> Result<()> {
    // Patch 1ST_READ.BIN: relocate decompression buffer + update descriptor size
    // + update SEQ decompressed size table.
    // All text uses VDP2 NBG3 (12-bit PND limit), so descriptor_size covers
    // only the VDP2-safe region. font_cel.len() already equals the VDP2-safe
    // size since build_char_table_safe caps at MAX_VDP2_GLYPH_INDEX (913).
    let descriptor_size = font_ctx.font_cel.len();
    let skip_seq_sizes = std::env::var("SKIP_SEQ_SIZES").is_ok();
    let seq_sizes_ref: Vec<(&str, usize)> = if skip_seq_sizes {
        println!("  (SKIP_SEQ_SIZES: skipping SEQ size table update in 1ST_READ.BIN)");
        Vec::new()
    } else {
        seq_result.seq_new_sizes
            .iter()
            .map(|(name, size)| (name.as_str(), *size))
            .collect()
    };
    pipeline::patch_first_read_combined(ctx, descriptor_size, &seq_sizes_ref)?;

    // Patch prologue sprite (OP_SP02.SPR) if font is provided.
    if let Some(pf) = prologue_font {
        pipeline::patch_prologue_sprite(ctx, pf, prologue_font_size)?;
    }

    // Patch SYSTEM.SPR (battle UI tiles + menu tab sprites) in a single pass.
    pipeline::patch_system_sprite(
        ctx,
        battle_ui_font,
        battle_ui_font_size,
        menu_tab_font,
        menu_tab_font_size,
    )?;

    // Save (includes EDC/ECC regeneration).
    pipeline::save_disc(ctx, output)?;

    // Generate BPS patch (original ROM → patched ROM).
    let bps_path = output.with_extension("bps");
    let original_bin = fs::read(rom).context("Failed to read original ROM for BPS")?;
    let patched_bin = fs::read(output).context("Failed to read patched ROM for BPS")?;
    let bps_patch = ss_madou::disc::bps::generate_bps(&original_bin, &patched_bin);
    fs::write(&bps_path, &bps_patch).context("Failed to write BPS patch")?;
    println!(
        "BPS patch: {} ({} bytes)",
        bps_path.display(),
        bps_patch.len()
    );

    // Summary.
    let cue_path = output.with_extension("cue");
    println!("\n=== Build Summary ===");
    println!("  Text characters: {} (all rendered with Korean font)", alloc.text_char_count);
    println!("  Glyph slots: {} assigned + {} unassigned (max index {})", alloc.assigned_count, alloc.unassigned.len(), alloc.max_glyph_used);
    println!("  FONT.CEL: {} bytes decompressed", font_ctx.font_cel.len());
    println!("  Decompression buffer: relocated to 0x002C0000 (Work RAM Low)");
    println!("  SEQ files patched: {}", seq_result.seqs_patched);
    println!("  SEQ files relocated: {}", seq_result.seqs_relocated);
    if seq_result.seqs_skipped > 0 {
        println!("  SEQ files skipped: {}", seq_result.seqs_skipped);
    }
    println!("  Pointers fixed: {}", seq_result.total_ptrs_fixed);
    println!("  SEQ sizes updated: {}", seq_result.seq_new_sizes.len());
    println!("  BPS patch: {} bytes", bps_patch.len());
    println!("\nTo test: load {} in a Saturn emulator", cue_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_build_rom — thin orchestrator
// ---------------------------------------------------------------------------

pub(crate) fn cmd_build_rom(
    rom: &Path,
    font_path: &Path,
    output: &Path,
    translations_dir: &Path,
    font_size: f32,
    only_seq: &[String],
    except_seq: &[String],
    skip_seq: bool,
    dump_seq: bool,
    dump_ptrs: bool,
    skip_common_ptrs: bool,
    skip_script_ptrs: bool,
    prologue_font: Option<&Path>,
    prologue_font_size: f32,
    battle_ui_font: Option<&Path>,
    battle_ui_font_size: f32,
    menu_tab_font: Option<&Path>,
    menu_tab_font_size: f32,
) -> Result<()> {
    // Build patch options from CLI flags, falling back to env vars.
    let mut patch_opts = patcher::PatchOptions::from_env();
    if dump_seq { patch_opts.dump_seq = true; }
    if dump_ptrs { patch_opts.dump_ptrs = true; }
    if skip_common_ptrs { patch_opts.skip_common_ptrs = true; }
    if skip_script_ptrs { patch_opts.skip_script_ptrs = true; }

    // Ensure output directory exists.
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
        }
    }

    println!("=== Build Korean ROM ===\n");

    // 1. Scan translation JSONs from multiple subdirectories.
    let scan = ss_madou::text::translation_scan::scan_translation_jsons(translations_dir)?;
    println!("Found {} JSON files total", scan.json_paths.len());
    if scan.json_paths.is_empty() {
        anyhow::bail!("No JSON files found in scanned directories");
    }
    println!(
        "  {} SEQ sources, {} total entries, {} translated",
        scan.seq_groups.len(), scan.total_entries, scan.total_translated
    );

    // 2. Overflow check — warn but don't block.
    build_stage_overflow_check(&scan.seq_groups);

    let all_patch_entries = scan.all_patch_entries;
    println!(
        "  {} SEQ files to patch ({} total entries)",
        all_patch_entries.len(),
        all_patch_entries.values().map(|v| v.len()).sum::<usize>()
    );

    // 3. Allocate glyph slots.
    let alloc = build_stage_allocate(&all_patch_entries)?;

    // 4. Load ROM + patch font.
    let mut ctx = pipeline::load_disc(rom)?;
    let mut font_ctx = pipeline::extract_font(&ctx)?;
    build_stage_font(&mut ctx, &mut font_ctx, &alloc, font_path, font_size)?;

    // 5. Patch SEQ files.
    let seq_result = build_stage_seq(
        &mut ctx, &all_patch_entries, &alloc.char_table, &patch_opts,
        skip_seq, only_seq, except_seq,
    )?;

    // 6. Finalize (1ST_READ, prologue, battle UI, save, BPS, summary).
    build_stage_finalize(
        &mut ctx, &font_ctx, &alloc, &seq_result,
        rom, output, prologue_font, prologue_font_size,
        battle_ui_font, battle_ui_font_size,
        menu_tab_font, menu_tab_font_size,
    )?;

    Ok(())
}

