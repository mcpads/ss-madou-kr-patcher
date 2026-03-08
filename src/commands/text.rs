use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use ss_madou::text;

pub(crate) fn cmd_dump_script(
    input: Option<&Path>,
    all: bool,
    input_dir: &Path,
    output: Option<&Path>,
    output_dir: &Path,
    glyph_map: &Path,
    max_entries: usize,
) -> Result<()> {
    let csv_data = fs::read_to_string(glyph_map)
        .context(format!("Failed to read glyph mapping: {}", glyph_map.display()))?;
    let table = text::GlyphTable::from_csv(&csv_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse glyph mapping: {}", e))?;
    println!("Loaded {} glyphs from {}", table.glyph_count(), glyph_map.display());

    if all {
        fs::create_dir_all(output_dir).context("Failed to create output directory")?;

        let mut count = 0;
        let mut dir_entries: Vec<_> = fs::read_dir(input_dir)
            .context("Failed to read input directory")?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("seq"))
                    .unwrap_or(false)
            })
            .collect();
        dir_entries.sort_by_key(|e| e.file_name());

        for entry in dir_entries {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            let data =
                fs::read(&path).context(format!("Failed to read {}", path.display()))?;

            let dump = text::parse_script(&data, &file_name, &table);
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let files = write_script_dump(&dump, output_dir, &stem, max_entries)?;

            let filtered_msg = if dump.filtered.is_empty() {
                String::new()
            } else {
                format!(", {} filtered", dump.filtered.len())
            };
            println!(
                "  {} → {} ({} entries{}, {} file{})",
                file_name,
                files[0].display(),
                dump.entries.len(),
                filtered_msg,
                files.len(),
                if files.len() > 1 { "s" } else { "" },
            );
            count += 1;
        }

        println!("\nProcessed {} SEQ files to {}", count, output_dir.display());
    } else {
        let input_path = input.context("--input is required (or use --all)")?;
        let data = fs::read(input_path)
            .context(format!("Failed to read {}", input_path.display()))?;

        let file_name = input_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        let dump = text::parse_script(&data, &file_name, &table);

        if let Some(out_path) = output {
            let parent = out_path.parent().unwrap_or(Path::new("."));
            fs::create_dir_all(parent)
                .context("Failed to create output directory")?;
            let stem = out_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let files = write_script_dump(&dump, parent, &stem, max_entries)?;
            let filtered_msg = if dump.filtered.is_empty() {
                String::new()
            } else {
                format!(", {} filtered", dump.filtered.len())
            };
            println!(
                "Dumped {} entries{} from {} → {} file{}",
                dump.entries.len(),
                filtered_msg,
                file_name,
                files.len(),
                if files.len() > 1 { "s" } else { "" },
            );
            for f in &files {
                println!("  {}", f.display());
            }
        } else {
            let json =
                serde_json::to_string_pretty(&dump).context("Failed to serialize JSON")?;
            println!("{}", json);
        }
    }

    Ok(())
}

/// Write a ScriptDump to one or more JSON files, splitting by max_entries.
/// Returns the list of file paths written.
fn write_script_dump(
    dump: &text::ScriptDump,
    dir: &Path,
    stem: &str,
    max_entries: usize,
) -> Result<Vec<PathBuf>> {
    use text::ScriptDump;

    if max_entries == 0 || dump.entries.len() <= max_entries {
        // Single file
        let path = dir.join(format!("{}.json", stem));
        let json = serde_json::to_string_pretty(dump).context("Failed to serialize JSON")?;
        fs::write(&path, &json).context(format!("Failed to write {}", path.display()))?;
        return Ok(vec![path]);
    }

    // Split into chunks
    let chunks: Vec<_> = dump.entries.chunks(max_entries).collect();
    let mut paths = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let part = ScriptDump {
            source: dump.source.clone(),
            source_md5: dump.source_md5.clone(),
            entries: chunk.to_vec(),
            // filtered는 첫 번째 분할 파일에만 포함
            filtered: if i == 0 {
                dump.filtered.clone()
            } else {
                Vec::new()
            },
        };
        let path = dir.join(format!("{}_{:02}.json", stem, i + 1));
        let json = serde_json::to_string_pretty(&part).context("Failed to serialize JSON")?;
        fs::write(&path, &json).context(format!("Failed to write {}", path.display()))?;
        paths.push(path);
    }

    Ok(paths)
}

pub(crate) fn cmd_check_overflow(
    translations_dir: &Path,
    verbose: bool,
) -> Result<()> {
    use ss_madou::text::overflow;
    use ss_madou::text::patcher::SeqType;
    use ss_madou::text::script::ScriptDump;

    let scan_dirs = ["needs_review", "needs_human_review", "complete", "raw"];
    let mut json_paths: Vec<PathBuf> = Vec::new();

    for sub in &scan_dirs {
        let dir = translations_dir.join(sub);
        if dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "json") {
                        json_paths.push(path);
                    }
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir(translations_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") && !json_paths.contains(&path) {
                json_paths.push(path);
            }
        }
    }
    json_paths.sort();

    if json_paths.is_empty() {
        anyhow::bail!("No JSON files found in {}", translations_dir.display());
    }

    println!("=== Text Overflow Check ===\n");
    println!("Scanning {} JSON files...\n", json_paths.len());

    let mut all_dumps: Vec<ScriptDump> = Vec::new();
    for path in &json_paths {
        let json_str = fs::read_to_string(path)
            .context(format!("Failed to read {}", path.display()))?;
        let script: ScriptDump = serde_json::from_str(&json_str)
            .context(format!("Failed to parse {}", path.display()))?;
        all_dumps.push(script);
    }

    let mut all_violations = Vec::new();
    for dump in &all_dumps {
        let seq_type = SeqType::from_filename(&dump.source);
        let limit = overflow::limit_for_seq_type(seq_type);
        let violations = overflow::check_script(dump, &limit, seq_type);
        all_violations.extend(violations);
    }

    if all_violations.is_empty() {
        println!("No overflow violations found!");
    } else {
        println!("Found {} violation(s):\n", all_violations.len());
        for v in &all_violations {
            match &v.kind {
                overflow::ViolationKind::LineOverflow {
                    line_index,
                    char_count,
                    limit,
                    line_text,
                } => {
                    println!(
                        "  [LINE] {} ({}): line {} -- {} chars (limit {})",
                        v.entry_id, v.source, line_index + 1, char_count, limit
                    );
                    println!("         \"{}\"", line_text);
                }
                overflow::ViolationKind::TooManyLines { line_count, limit } => {
                    println!(
                        "  [LINES] {} ({}): {} lines (limit {})",
                        v.entry_id, v.source, line_count, limit
                    );
                }
            }
        }
    }

    if verbose {
        let dump_refs: Vec<&ScriptDump> = all_dumps.iter().collect();
        for seq_type in [SeqType::Mp, SeqType::Pt, SeqType::Diary, SeqType::Common] {
            let limit = overflow::limit_for_seq_type(seq_type);
            let type_dumps: Vec<&ScriptDump> = dump_refs
                .iter()
                .filter(|d| SeqType::from_filename(&d.source) == seq_type)
                .copied()
                .collect();
            if type_dumps.is_empty() {
                continue;
            }
            let stats = overflow::compute_stats(&type_dumps, &limit, seq_type);
            println!("\n--- {} ({:?}) ---", limit.label, seq_type);
            println!(
                "  Entries: {}, Lines: {}, Max chars/line: {}, Max lines/entry: {}",
                stats.total_entries, stats.total_lines,
                stats.max_char_count, stats.max_line_count,
            );
            println!(
                "  Overflow: {} lines, {} entries",
                stats.overflow_lines, stats.overflow_entries,
            );
            println!("  Distribution:");
            for (chars, count) in &stats.char_distribution {
                let bar_len = (*count * 40) / stats.total_lines.max(1);
                let bar: String = "#".repeat(bar_len);
                println!("    {:>3} chars: {:>5} lines  {}", chars, count, bar);
            }
        }
    }

    // 재배치 분석: LINE overflow 엔트리 중 줄 재배치로 해결 가능한 것 분류
    {
        use std::collections::HashSet;
        // 위반 엔트리 ID 수집
        let violation_ids: HashSet<&str> = all_violations
            .iter()
            .filter(|v| matches!(v.kind, overflow::ViolationKind::LineOverflow { .. }))
            .map(|v| v.entry_id.as_str())
            .collect();

        let mut fixable_by_redistribute = 0usize;
        let mut needs_rewrite = 0usize;
        let mut fixable_no_line_limit = 0usize;

        for dump in &all_dumps {
            let seq_type = SeqType::from_filename(&dump.source);
            let limit = overflow::limit_for_seq_type(seq_type);
            for entry in &dump.entries {
                if !violation_ids.contains(entry.id.as_str()) {
                    continue;
                }
                let ko = match &entry.ko {
                    Some(k) if !k.is_empty() => k,
                    _ => continue,
                };
                let ko_lines = overflow::measure_lines_for(ko, Some(seq_type));
                let jp_lines = if entry.text.is_empty() {
                    None
                } else {
                    Some(overflow::measure_lines_for(&entry.text, Some(seq_type)))
                };
                let total_chars: usize = ko_lines.iter().map(|l| l.char_count).sum();
                let ko_line_count = ko_lines.len();

                // effective max lines (JP baseline 고려)
                let max_lines = match limit.max_lines {
                    Some(max) => {
                        let jp_lc = jp_lines.as_ref().map(|jl| jl.len()).unwrap_or(0);
                        Some(max.max(jp_lc))
                    }
                    None => None,
                };

                match max_lines {
                    None => {
                        // 줄 수 제한 없음 → 줄 분할로 항상 해결 가능
                        fixable_no_line_limit += 1;
                    }
                    Some(max) => {
                        // effective limit per line (JP baseline 고려)
                        let jp_max_chars = jp_lines
                            .as_ref()
                            .map(|jl| jl.iter().map(|l| l.char_count).max().unwrap_or(0))
                            .unwrap_or(0);
                        let effective_chars = limit.max_chars_per_line.max(jp_max_chars);
                        let capacity = max * effective_chars;
                        // 현재 줄 수 이내에서 재배치 가능한지 확인
                        let available_lines = max.max(ko_line_count);
                        let capacity_with_current = available_lines * effective_chars;

                        if total_chars <= capacity {
                            fixable_by_redistribute += 1;
                        } else if total_chars <= capacity_with_current {
                            fixable_by_redistribute += 1;
                        } else {
                            needs_rewrite += 1;
                            if verbose {
                                eprintln!(
                                    "  REWRITE: {} ({}) — {} total chars, capacity {} ({} lines × {} chars)",
                                    entry.id, dump.source, total_chars, capacity, max, effective_chars
                                );
                            }
                        }
                    }
                }
            }
        }

        let total_line_violations = violation_ids.len();
        println!("\n--- Redistribution Analysis ---");
        println!(
            "  LINE overflow entries: {} unique",
            total_line_violations
        );
        println!(
            "  Fixable by line split (no line limit):  {}",
            fixable_no_line_limit
        );
        println!(
            "  Fixable by redistribute (within limit): {}",
            fixable_by_redistribute
        );
        println!(
            "  Needs rewrite (total chars > capacity):  {}",
            needs_rewrite
        );
    }

    // Byte-level delta analysis
    {
        println!("\n--- Byte Delta Analysis ---");
        let mut file_summaries: Vec<overflow::ByteDeltaSummary> = Vec::new();
        for dump in &all_dumps {
            let summary = overflow::compute_byte_deltas(dump);
            if summary.total_entries > 0 {
                file_summaries.push(summary);
            }
        }

        // Sort by total_delta descending (biggest growers first)
        file_summaries.sort_by(|a, b| b.total_delta.cmp(&a.total_delta));

        let total_grow_files = file_summaries.iter().filter(|s| s.total_delta > 0).count();
        let total_shrink_files = file_summaries.iter().filter(|s| s.total_delta < 0).count();
        let grand_delta: isize = file_summaries.iter().map(|s| s.total_delta).sum();

        println!(
            "  Files analyzed: {}  (grow: {}, shrink: {}, net: {:+} bytes)",
            file_summaries.len(),
            total_grow_files,
            total_shrink_files,
            grand_delta
        );

        // Show files with largest growth
        let growers: Vec<&overflow::ByteDeltaSummary> = file_summaries
            .iter()
            .filter(|s| s.total_delta > 0)
            .collect();

        if !growers.is_empty() {
            println!(
                "\n  Top growing files ({} files with byte expansion):",
                growers.len()
            );
            for s in growers.iter().take(20) {
                println!(
                    "    {:20} {:+5} bytes  ({} entries: {} grow, {} shrink, max grow {:+})",
                    s.source, s.total_delta, s.total_entries,
                    s.grow_entries, s.shrink_entries, s.max_grow
                );
            }
        }

        if verbose {
            // Show per-entry top growers across all files
            let mut all_growers: Vec<overflow::ByteDeltaEntry> = Vec::new();
            for dump in &all_dumps {
                let summary = overflow::compute_byte_deltas(dump);
                all_growers.extend(summary.top_growers);
            }
            all_growers.sort_by(|a, b| b.delta.cmp(&a.delta));

            if !all_growers.is_empty() {
                println!("\n  Top growing entries (across all files):");
                for e in all_growers.iter().take(20) {
                    let pad_mark = if e.pad_to_original { " [PAD]" } else { "" };
                    println!(
                        "    {:20} {:15} {:+5} bytes  (orig {} → ko {}){}",
                        e.source, e.entry_id, e.delta, e.raw_bytes, e.ko_bytes, pad_mark
                    );
                }
            }

            // Show ALL files sorted by delta
            println!("\n  All files by byte delta:");
            for s in &file_summaries {
                let marker = if s.total_delta > 0 { "▲" }
                    else if s.total_delta < 0 { "▼" }
                    else { "=" };
                println!(
                    "    {} {:20} {:+5} bytes  ({} entries)",
                    marker, s.source, s.total_delta, s.total_entries
                );
            }
        }
    }

    println!(
        "\nTotal: {} violation(s) across {} files",
        all_violations.len(),
        all_dumps.len()
    );

    Ok(())
}

