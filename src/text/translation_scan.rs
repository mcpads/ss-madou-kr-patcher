//! Shared translation JSON scanning for build-rom and check-glyphs.
//!
//! Both `cmd_build_rom` and `cmd_check_glyphs` need to discover JSON files
//! across the standard subdirectories, parse them as `ScriptDump`, and build
//! `TranslationEntry` lists.  This module extracts that common logic.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::text::patcher::{self, TranslationEntry};
use crate::text::script::{ScriptDump, TranslationStatus};

/// Result of scanning translation JSON files.
pub struct ScanResult {
    /// Sorted list of JSON file paths found.
    pub json_paths: Vec<PathBuf>,
    /// Script dumps grouped by source SEQ filename.
    pub seq_groups: HashMap<String, Vec<ScriptDump>>,
    /// Patch entries grouped by source SEQ filename.
    pub all_patch_entries: HashMap<String, Vec<TranslationEntry>>,
    /// Total entry count across all JSONs (including untranslated).
    pub total_entries: usize,
    /// Count of entries with valid Korean translation text.
    pub total_translated: usize,
}

/// Standard subdirectories scanned for translation JSONs.
const SCAN_SUBDIRS: &[&str] = &["needs_review", "needs_human_review", "complete"];

/// Check whether a directory directly contains any `.json` files.
fn has_json_files(dir: &Path) -> bool {
    fs::read_dir(dir).map_or(false, |mut d| {
        d.any(|e| {
            e.ok()
                .map_or(false, |e| e.path().extension().is_some_and(|ext| ext == "json"))
        })
    })
}

/// Determine which directories to scan for translation JSONs.
///
/// If `translations_dir` itself contains JSON files, it is included directly
/// and its sibling subdirectories (needs_review, etc.) are also scanned.
/// Otherwise, the standard subdirectories are looked up under `translations_dir`.
fn resolve_scan_dirs(translations_dir: &Path) -> Vec<PathBuf> {
    let base_dir = translations_dir.parent().unwrap_or(translations_dir);

    if has_json_files(translations_dir) {
        // translations_dir has JSONs directly -- scan it plus siblings.
        let mut dirs = vec![translations_dir.to_path_buf()];
        for sub in SCAN_SUBDIRS {
            let sibling = base_dir.join(sub);
            if sibling != translations_dir && sibling.is_dir() {
                dirs.push(sibling);
            }
        }
        dirs
    } else {
        SCAN_SUBDIRS
            .iter()
            .map(|s| translations_dir.join(s))
            .filter(|p| p.is_dir())
            .collect()
    }
}

/// Returns `true` for translation statuses accepted by the patch pipeline.
fn is_translatable_status(status: &TranslationStatus) -> bool {
    matches!(
        status,
        TranslationStatus::NeedsReview
            | TranslationStatus::NeedsHumanReview
            | TranslationStatus::Done
    )
}

/// Scan translation JSON files from `translations_dir` and its standard subdirectories.
///
/// Discovers JSON files, parses each as [`ScriptDump`], groups them by source SEQ,
/// and builds [`TranslationEntry`] lists ready for patching or analysis.
pub fn scan_translation_jsons(translations_dir: &Path) -> Result<ScanResult> {
    let dirs_to_scan = resolve_scan_dirs(translations_dir);

    // 1. Collect all JSON file paths.
    let mut json_paths: Vec<PathBuf> = Vec::new();
    for dir in &dirs_to_scan {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    json_paths.push(path);
                }
            }
        }
    }
    json_paths.sort();

    // 2. Parse all JSONs and group by source SEQ.
    let mut seq_groups: HashMap<String, Vec<ScriptDump>> = HashMap::new();
    let mut total_entries = 0usize;
    let mut total_translated = 0usize;

    for path in &json_paths {
        let json_str =
            fs::read_to_string(path).context(format!("Failed to read {}", path.display()))?;
        let script: ScriptDump = serde_json::from_str(&json_str)
            .context(format!("Failed to parse {}", path.display()))?;

        let translated_count = script
            .entries
            .iter()
            .filter(|e| {
                e.ko.as_ref().is_some_and(|k| !k.is_empty()) && is_translatable_status(&e.status)
            })
            .count();

        total_entries += script.entries.len();
        total_translated += translated_count;

        seq_groups
            .entry(script.source.clone())
            .or_default()
            .push(script);
    }

    // 3. Build unified patch entries for all SEQs.
    let mut all_patch_entries: HashMap<String, Vec<TranslationEntry>> = HashMap::new();

    for (source, dumps) in &seq_groups {
        let mut patch_entries = Vec::new();
        for dump in dumps {
            for entry in &dump.entries {
                if let Some(ko) = &entry.ko {
                    if ko.is_empty() {
                        continue;
                    }
                    if !is_translatable_status(&entry.status) {
                        continue;
                    }

                    let offset = usize::from_str_radix(
                        entry
                            .offset
                            .trim_start_matches("0x")
                            .trim_start_matches("0X"),
                        16,
                    )
                    .context(format!("Invalid offset {} in {}", entry.offset, source))?;

                    let orig_len = entry.raw_hex.split_whitespace().count();
                    let tokens = patcher::parse_ko_tokens(ko);
                    let expected_bytes: Vec<u8> = entry
                        .raw_hex
                        .split_whitespace()
                        .filter_map(|h| u8::from_str_radix(h, 16).ok())
                        .collect();

                    patch_entries.push(TranslationEntry {
                        offset,
                        orig_len,
                        tokens,
                        entry_id: entry.id.clone(),
                        expected_bytes: Some(expected_bytes),
                        pad_to_original: entry.pad_to_original,
                    });
                }
            }
        }
        if !patch_entries.is_empty() {
            all_patch_entries.insert(source.clone(), patch_entries);
        }
    }

    Ok(ScanResult {
        json_paths,
        seq_groups,
        all_patch_entries,
        total_entries,
        total_translated,
    })
}
