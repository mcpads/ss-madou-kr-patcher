//! SEQ text patching for Korean translation.
//!
//! Encodes Korean text as FONT.CEL tile codes and patches them into
//! decompressed SEQ file buffers.  Supports multiple SEQ file types
//! (MP, PT, COMMON, DIARY) with their distinct pointer formats.

use std::collections::BTreeSet;
use std::collections::HashMap;

use anyhow::ensure;

use crate::font::korean::{GLYPH_TILE_START, TILES_PER_GLYPH};

// ---------------------------------------------------------------------------
// Patch options (diagnostic flags)
// ---------------------------------------------------------------------------

/// Diagnostic options for `apply_patches`.  All default to `false`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PatchOptions {
    /// Skip COMMON.SEQ pointer fixing.
    pub skip_common_ptrs: bool,
    /// Skip MP/PT script pointer fixing.
    pub skip_script_ptrs: bool,
    /// Dump patched COMMON.SEQ to `out/patched_COMMON.SEQ`.
    pub dump_seq: bool,
    /// Dump pointer fix details to `out/common_ptr_dump.txt`.
    pub dump_ptrs: bool,
}

impl PatchOptions {
    /// Build from environment variables (backward compatibility).
    pub fn from_env() -> Self {
        Self {
            skip_common_ptrs: std::env::var("SKIP_COMMON_PTRS").is_ok(),
            skip_script_ptrs: std::env::var("SKIP_SCRIPT_PTRS").is_ok(),
            dump_seq: std::env::var("DUMP_SEQ").is_ok(),
            dump_ptrs: std::env::var("DUMP_PTRS").is_ok(),
        }
    }
}

// ---------------------------------------------------------------------------
// SEQ type detection and pointer patterns
// ---------------------------------------------------------------------------

/// SEQ file type, determined by RAM load segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqType {
    /// MP*.SEQ — scene dialogue. RAM 0x0024xxxx.
    Mp,
    /// PT*.SEQ / PARTY*.SEQ — party event dialogue. RAM 0x0025xxxx.
    Pt,
    /// COMMON.SEQ — game data table. RAM 0x0020xxxx.
    Common,
    /// DIARY.SEQ — diary pages. RAM 0x0027xxxx.
    Diary,
    /// Other file types (BTLMAP, DG*, etc.) — no pointer fix needed.
    Other,
}

impl SeqType {
    /// Detect SEQ type from filename (case-insensitive prefix match).
    pub fn from_filename(name: &str) -> Self {
        let upper = name.to_ascii_uppercase();
        let basename = upper.rsplit('/').next().unwrap_or(&upper);
        let basename = basename.rsplit('\\').next().unwrap_or(basename);

        if basename.starts_with("MP") {
            SeqType::Mp
        } else if basename.starts_with("PT") || basename.starts_with("PARTY") {
            SeqType::Pt
        } else if basename.starts_with("COMMON") {
            SeqType::Common
        } else if basename.starts_with("DIARY") {
            SeqType::Diary
        } else {
            SeqType::Other
        }
    }

    /// Return the pointer pattern for this SEQ type, if applicable.
    ///
    /// MP, PT, and Other use pointer instructions with a 2-byte prefix.
    /// COMMON and DIARY use fundamentally different pointer structures
    /// and return `None` here.
    pub fn pointer_pattern(&self) -> Option<PointerPattern> {
        match self {
            SeqType::Mp | SeqType::Other => Some(PointerPattern {
                prefix: [0x00, 0x24],
                suffix: [0x00, 0x00, 0x00, 0x05],
                base_offset: 0,
            }),
            SeqType::Pt => Some(PointerPattern {
                prefix: [0x00, 0x25],
                suffix: [0x05, 0x00, 0x00, 0x00],
                base_offset: 0,
            }),
            _ => None,
        }
    }

    /// Return all pointer patterns for this SEQ type, considering file size.
    ///
    /// MP files >64KB use both `00 24` (first 64KB) and `00 25`
    /// (overflow bank, file offsets 0x10000+) pointers.
    /// Other SEQ types (MG_, DG*, DM_END, DUNG*, WORLD*) also use `00 24`.
    pub fn pointer_patterns(&self, file_len: usize) -> Vec<PointerPattern> {
        match self {
            SeqType::Mp | SeqType::Other => {
                let mut pats = vec![
                    PointerPattern { prefix: [0x00, 0x24], suffix: [0x00, 0x00, 0x00, 0x05], base_offset: 0 },
                ];
                if file_len > 0x10000 {
                    pats.push(PointerPattern {
                        prefix: [0x00, 0x25],
                        suffix: [0x00, 0x00, 0x00, 0x05],
                        base_offset: 0x10000,
                    });
                }
                pats
            }
            SeqType::Pt => vec![
                PointerPattern { prefix: [0x00, 0x25], suffix: [0x05, 0x00, 0x00, 0x00], base_offset: 0 },
            ],
            _ => vec![],
        }
    }
}

/// An 8-byte pointer instruction pattern in SEQ script code.
///
/// Format: `[prefix_0] [prefix_1] [offset_hi] [offset_lo] [suffix_0..3]`
#[derive(Clone, Debug)]
pub struct PointerPattern {
    /// First two bytes (opcode). `[0x00, 0x24]` for MP, `[0x00, 0x25]` for PT.
    pub prefix: [u8; 2],
    /// Last four bytes. `[0x00,0x00,0x00,0x05]` for MP, `[0x05,0x00,0x00,0x00]` for PT.
    pub suffix: [u8; 4],
    /// Base file offset for this prefix bank.
    /// `0x0000` for `00 24` (first 64KB), `0x10000` for `00 25` in MP files (overflow bank).
    /// PT files use `00 25` with base `0x0000` since they load at RAM 0x00250000.
    pub base_offset: usize,
}

// ---------------------------------------------------------------------------
// Preserved glyph slots
// ---------------------------------------------------------------------------

/// Return the set of glyph slot indices that must not be overwritten
/// with Korean characters.
///
/// - 161–175: symbols (。×％＆＊☆○◎◇■△▽▼→)
/// - 832–834: icons ([연기]=cloud, [하트]=♥, [Zzz]=sleep)
/// - 914: VDP1 bank boundary (tiles 4094–4097 straddle bank 0/1 at >>12)
pub fn preserved_glyph_slots() -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    for i in 161..=175 {
        set.insert(i);
    }
    for i in 832..=834 {
        set.insert(i);
    }
    set.insert(914); // bank boundary: glyph 914 = tiles 4094-4097
    set
}

// ---------------------------------------------------------------------------
// Padding insertion helper
// ---------------------------------------------------------------------------

/// Find the correct insertion position for padding space tiles.
///
/// Scans backwards over all trailing `FFxx` control code pairs so that
/// padding is always inserted BEFORE the entire ctrl suffix.  This prevents
/// the game engine from interpreting padding bytes as control code parameters
/// (which causes hardlocks — e.g. MP0401 "パノッティのフエ" ending in
/// `FF02 FF3A FF3C` had padding appended after FF3C).
fn find_padding_insert_pos(bytes: &[u8]) -> usize {
    let mut pos = bytes.len();
    // Walk backwards in 2-byte steps while we see FFxx control codes.
    while pos >= 2 && bytes[pos - 2] == 0xFF {
        pos -= 2;
    }
    pos
}

/// Truncate encoded bytes to fit `target_len` by removing text tokens
/// before trailing FFxx control codes.
///
// ---------------------------------------------------------------------------
// Tile constants and tokens
// ---------------------------------------------------------------------------

/// Tile code for wide-char space (16x8).
pub const SPACE_TILE: u16 = 0x00B2;
/// Tile code for wide-char ellipsis (16x8).
/// wide_idx 129 = tiles 386-387 (three dots at bottom of cell).
/// NOTE: 0x00E6 (wide_idx 51) was previously used but is a vertical bar, not ellipsis.
pub const ELLIPSIS_TILE: u16 = 0x0182;
/// Left bracket 「
pub const LBRACKET_TILE: u16 = 0x00FE;
/// Right bracket 」
pub const RBRACKET_TILE: u16 = 0x0102;
/// Full-width space (wide:064)
pub const FULLWIDTH_SPACE_TILE: u16 = 0x0100;

/// A token in a translated text entry.
#[derive(Clone, Debug)]
pub enum TextToken {
    /// Korean text string (may contain spaces, ellipsis, brackets).
    Text(String),
    /// Raw tile code (for wide characters, special symbols, etc.).
    Tile(u16),
    /// Raw control code (FF02 line break, FF05 end, etc.).
    Ctrl(u16),
}

/// A translated entry ready for patching.
#[derive(Clone)]
pub struct TranslationEntry {
    /// Byte offset in the decompressed SEQ file.
    pub offset: usize,
    /// Original byte count at this offset.
    pub orig_len: usize,
    /// Translation tokens (text, tiles, and control codes — fully self-contained).
    pub tokens: Vec<TextToken>,
    /// Entry ID for diagnostics (e.g., "MP0101_0018").
    pub entry_id: String,
    /// Expected raw bytes at this offset (from JSON raw_hex).  Used for
    /// pre-patch validation to detect entry boundary mismatches.
    /// `None` skips byte-level validation.
    pub expected_bytes: Option<Vec<u8>>,
    /// Per-entry fixed-length padding.  When `true`, this entry is
    /// padded/trimmed to its original byte length (no pointer fix needed).
    pub pad_to_original: bool,
}

/// Characters handled as dedicated wide tile codes (not glyph slots).
fn is_wide_tile_char(ch: char) -> bool {
    matches!(ch, ' ' | '\u{2026}' | '\u{300C}' | '\u{300D}' | '\u{3000}')
}

/// Collect all unique characters that need glyph slots from translation tokens.
///
/// Includes Korean syllables, ASCII punctuation, Japanese kana/kanji, and
/// special symbols. Excludes characters with dedicated wide tile codes
/// (space, ellipsis, brackets, full-width space).
pub fn collect_text_chars(entries: &[TranslationEntry]) -> Vec<char> {
    let mut chars = BTreeSet::new();
    for entry in entries {
        for token in &entry.tokens {
            if let TextToken::Text(s) = token {
                for ch in s.chars() {
                    if !is_wide_tile_char(ch) {
                        chars.insert(ch);
                    }
                }
            }
        }
    }
    chars.into_iter().collect()
}

/// Collect only Korean (Hangul) characters from translation tokens.
pub fn collect_korean_chars(entries: &[TranslationEntry]) -> Vec<char> {
    let mut chars = BTreeSet::new();
    for entry in entries {
        for token in &entry.tokens {
            if let TextToken::Text(s) = token {
                for ch in s.chars() {
                    if ('\u{AC00}'..='\u{D7A3}').contains(&ch) {
                        chars.insert(ch);
                    }
                }
            }
        }
    }
    chars.into_iter().collect()
}

/// Build a Korean character → glyph tile code mapping.
///
/// Characters are assigned glyph slots starting from `start_slot`,
/// skipping slots in the `preserve` set (decorative/symbol glyphs).
pub fn build_char_table(
    chars: &[char],
    start_slot: usize,
    preserve: &BTreeSet<usize>,
) -> HashMap<char, u16> {
    let mut table = HashMap::new();
    let mut slot = start_slot;
    for &ch in chars {
        while preserve.contains(&slot) {
            slot += 1;
        }
        let tile_code = (GLYPH_TILE_START + slot * TILES_PER_GLYPH) as u16;
        table.insert(ch, tile_code);
        slot += 1;
    }
    table
}

/// Maximum VDP2-safe glyph index: (4095 - 438) / 4 = 914, but slot 914
/// straddles the VDP1 bank boundary (tiles 4094-4097), so max usable = 913.
/// All game text (menus + dialogue) uses VDP2 NBG3, confirmed by emulator.
pub const MAX_VDP2_GLYPH_INDEX: usize = 913;

/// Build a character → tile code mapping that stays within the VDP2-safe
/// glyph range (slots 0–913).
///
/// Characters in `chars` should be sorted by priority (highest frequency
/// first). Slots in `unavailable` (preserved + used-by-existing-JP) are
/// skipped. Characters that don't fit are returned as `unassigned`.
///
/// Returns `(char_table, unassigned_chars)`.
pub fn build_char_table_safe(
    chars: &[char],
    unavailable: &BTreeSet<usize>,
    max_glyph_index: usize,
) -> (HashMap<char, u16>, Vec<char>) {
    let mut table = HashMap::new();
    let mut unassigned = Vec::new();
    let mut slot = 0usize;
    for &ch in chars {
        // Find next available slot.
        while slot <= max_glyph_index && unavailable.contains(&slot) {
            slot += 1;
        }
        if slot > max_glyph_index {
            // No more room — all remaining chars are unassigned.
            unassigned.push(ch);
            continue;
        }
        let tile_code = (GLYPH_TILE_START + slot * TILES_PER_GLYPH) as u16;
        table.insert(ch, tile_code);
        slot += 1;
    }
    (table, unassigned)
}

/// Tile code used as fallback when a character has no assigned glyph slot.
/// Uses the wide-char space tile so missing chars appear as blank spaces
/// rather than crashing the game.
pub const FALLBACK_TILE: u16 = SPACE_TILE;

/// Encode a text string as tile code bytes.
///
/// Characters with dedicated wide tile codes (space, ellipsis, brackets,
/// full-width space) are mapped directly. All other characters are looked
/// up in `char_table` (Korean syllables, ASCII, Japanese, symbols).
///
/// Characters not found in `char_table` use [`FALLBACK_TILE`] instead of
/// panicking — this allows graceful degradation when glyph slots are
/// exhausted.
fn encode_text_str(text: &str, char_table: &HashMap<char, u16>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for ch in text.chars() {
        let code = match ch {
            ' ' => SPACE_TILE,
            '\u{2026}' => ELLIPSIS_TILE,          // …
            '\u{300C}' => LBRACKET_TILE,           // 「
            '\u{300D}' => RBRACKET_TILE,           // 」
            '\u{3000}' => FULLWIDTH_SPACE_TILE,    // 全角スペース
            c => *char_table
                .get(&c)
                .unwrap_or(&FALLBACK_TILE),
        };
        bytes.push((code >> 8) as u8);
        bytes.push((code & 0xFF) as u8);
    }
    bytes
}

/// Encode a translation entry's tokens into raw bytes.
pub fn encode_entry(entry: &TranslationEntry, char_table: &HashMap<char, u16>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for token in &entry.tokens {
        match token {
            TextToken::Text(s) => bytes.extend_from_slice(&encode_text_str(s, char_table)),
            TextToken::Tile(code) | TextToken::Ctrl(code) => {
                bytes.push((code >> 8) as u8);
                bytes.push((code & 0xFF) as u8);
            }
        }
    }
    bytes
}

// ---------------------------------------------------------------------------
// Patch application
// ---------------------------------------------------------------------------

/// A text patch with computed sizes (offset, original length, new bytes).
struct PatchInfo {
    offset: usize,
    orig_len: usize,
    new_bytes: Vec<u8>,
}

/// Pre-computed cumulative shift table for O(log n) pointer offset lookups.
///
/// Built once from sorted patches, replaces O(n) linear `compute_shift` with
/// O(log n) binary search via `partition_point`.
struct ShiftTable {
    /// `(patch_end_offset, cumulative_shift)` sorted ascending by offset.
    entries: Vec<(usize, isize)>,
}

impl ShiftTable {
    fn from_patches(patches: &[PatchInfo]) -> Self {
        let mut entries = Vec::with_capacity(patches.len());
        let mut cum: isize = 0;
        for p in patches {
            cum += p.new_bytes.len() as isize - p.orig_len as isize;
            entries.push((p.offset + p.orig_len, cum));
        }
        Self { entries }
    }

    /// Return the cumulative byte shift at `orig_offset`.
    fn shift_at(&self, orig_offset: usize) -> isize {
        // partition_point returns the first index where patch_end > orig_offset.
        match self.entries.partition_point(|&(end, _)| end <= orig_offset) {
            0 => 0,
            n => self.entries[n - 1].1,
        }
    }
}

/// Split byte slice on `FF 00` separator boundaries (2-byte aligned).
///
/// Each returned slice includes its trailing delimiter (except the last
/// slice, which typically ends with `FF 05` or at the entry boundary).
///
/// COMMON.SEQ uses two sub-item delimiter types:
///   - `FF 00`: item/skill name tables (COMMON_0048-0051)
///   - `FF 09`: battle result messages (COMMON_0046), character names (COMMON_0047)
fn split_on_sub_item_delimiters(data: &[u8]) -> Vec<&[u8]> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == 0xFF && (data[i + 1] == 0x00 || data[i + 1] == 0x09) {
            items.push(&data[start..i + 2]);
            start = i + 2;
            i += 2;
        } else {
            i += 2;
        }
    }
    if start < data.len() {
        items.push(&data[start..]);
    }
    items
}

/// Split multi-item entries (FF00/FF09-delimited) into individual sub-entries.
///
/// COMMON.SEQ contains entries where record-table pointers target individual
/// sub-items within a single translation entry.  Treating the whole entry
/// as one `PatchInfo` makes `compute_shift` return the same shift for all
/// intra-entry targets, even though individual items change size differently.
/// Splitting into sub-entries lets `compute_shift` track per-sub-item shifts.
///
/// Delimiter types:
///   - `FF 00`: item/skill name tables (COMMON_0048-0051, 331+ items)
///   - `FF 09`: battle results (COMMON_0046, 18 items), char names (COMMON_0047, 11 items)
fn split_common_sub_item_patches(seq_data: &[u8], patches: Vec<PatchInfo>) -> Vec<PatchInfo> {
    let mut result = Vec::new();
    for p in patches {
        let orig = &seq_data[p.offset..p.offset + p.orig_len];
        let orig_items = split_on_sub_item_delimiters(orig);
        let new_items = split_on_sub_item_delimiters(&p.new_bytes);

        if orig_items.len() > 1 && orig_items.len() == new_items.len() {
            let mut sub_offset = p.offset;
            for (orig_item, new_item) in orig_items.iter().zip(new_items.iter()) {
                result.push(PatchInfo {
                    offset: sub_offset,
                    orig_len: orig_item.len(),
                    new_bytes: new_item.to_vec(),
                });
                sub_offset += orig_item.len();
            }
        } else {
            result.push(p);
        }
    }
    result
}

/// Build the patched binary buffer from translation entries.
///
/// Returns `(patched_buffer, patch_infos)` where `patch_infos` records
/// each patch's original offset, original length, and new byte length
/// for use by pointer fixers.
fn build_patched_buffer(
    seq_data: &[u8],
    entries: &[TranslationEntry],
    char_table: &HashMap<char, u16>,
    seq_type: SeqType,
) -> anyhow::Result<(Vec<u8>, Vec<PatchInfo>)> {
    // -----------------------------------------------------------------------
    // Pre-patch validation: verify entry bytes match SEQ data.
    // -----------------------------------------------------------------------
    for entry in entries {
        let end = entry.offset + entry.orig_len;
        ensure!(
            end <= seq_data.len(),
            "{}: offset 0x{:X} + orig_len {} = 0x{:X} exceeds SEQ size ({})",
            entry.entry_id, entry.offset, entry.orig_len, end, seq_data.len()
        );
        if let Some(ref expected) = entry.expected_bytes {
            ensure!(
                expected.len() == entry.orig_len,
                "{}: expected_bytes length {} != orig_len {}",
                entry.entry_id, expected.len(), entry.orig_len
            );
            let actual = &seq_data[entry.offset..end];
            if actual != expected.as_slice() {
                let diff_pos = actual.iter().zip(expected.iter())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                anyhow::bail!(
                    "{}: raw_hex mismatch at offset 0x{:X}+{} — \
                     expected 0x{:02X} but SEQ has 0x{:02X} \
                     (JSON raw_hex may belong to a different entry)",
                    entry.entry_id, entry.offset, diff_pos,
                    expected[diff_pos], actual[diff_pos]
                );
            }
        }
    }

    let mut patches: Vec<PatchInfo> = entries
        .iter()
        .map(|e| PatchInfo {
            offset: e.offset,
            orig_len: e.orig_len,
            new_bytes: encode_entry(e, char_table),
        })
        .collect();

    // Per-entry alignment padding: keep each entry's delta ≡ 0 mod 4.
    // All text tokens are 2 bytes, so deltas are always even (Δ%4 ∈ {0,2}).
    // When Δ%4=2, insert a space tile (0x00B2) as padding.
    //
    // Applies to MP/PT AND COMMON: all three have 4-byte-aligned pointer
    // tables and structured data interleaved between text entries.
    //  - MP/PT: jump tables, event descriptors in text gaps
    //  - COMMON: 0020XXXX pointer tables between name/description entries
    //
    // For MP/PT: padding goes BEFORE the FF05 end marker (inside text)
    // so the script engine never sees stray bytes between entries.
    // For COMMON: padding goes BEFORE the last FF00/FF09/FF05 delimiter
    // Fixed-length in-place patching (e.g. TITLE.SEQ, skill tables).
    // Entries with pad_to_original have no pointer table, so text
    // length MUST NOT change.  Pad shorter translations with space tiles.
    {
        let space = [(SPACE_TILE >> 8) as u8, (SPACE_TILE & 0xFF) as u8];
        for (i, p) in patches.iter_mut().enumerate() {
            let entry_pad = entries.get(i).map(|e| e.pad_to_original).unwrap_or(false);
            if !entry_pad {
                continue;
            }
            let new_len = p.new_bytes.len();
            if new_len > p.orig_len {
                anyhow::bail!(
                    "Entry {} (offset 0x{:X}): Korean text is {} bytes longer than original ({} vs {}). \
                     Shorten the translation to fit.",
                    entries.get(i).map(|e| e.entry_id.as_str()).unwrap_or("?"),
                    p.offset, new_len - p.orig_len, new_len, p.orig_len
                );
            }

            // Split by FF00 to pad each sub-item individually.
            // Skill name tables are FF00-delimited; each sub-item must keep
            // its original byte length so the game's sequential FF00 scan
            // finds each name at the correct offset.
            let orig_bytes = &seq_data[p.offset..p.offset + p.orig_len];
            let orig_items = split_on_sub_item_delimiters(orig_bytes);
            let new_items = split_on_sub_item_delimiters(&p.new_bytes);

            if orig_items.len() > 1 && orig_items.len() == new_items.len() {
                let mut padded = Vec::new();
                for (j, (orig_sub, new_sub)) in orig_items.iter().zip(new_items.iter()).enumerate() {
                    let mut sub = new_sub.to_vec();
                    if sub.len() > orig_sub.len() {
                        anyhow::bail!(
                            "Entry {} (offset 0x{:X}): sub-item {} is {} bytes longer than original ({} vs {}). \
                             Shorten the skill name.",
                            entries.get(i).map(|e| e.entry_id.as_str()).unwrap_or("?"),
                            p.offset, j, sub.len() - orig_sub.len(), sub.len(), orig_sub.len()
                        );
                    }
                    while sub.len() + 2 <= orig_sub.len() {
                        let pos = find_padding_insert_pos(&sub);
                        sub.insert(pos, space[1]);
                        sub.insert(pos, space[0]);
                    }
                    if sub.len() < orig_sub.len() {
                        sub.push(0x00);
                    }
                    padded.extend_from_slice(&sub);
                }
                p.new_bytes = padded;
            } else {
                // Single item or mismatched count: pad entire entry.
                while p.new_bytes.len() + 2 <= p.orig_len {
                    let pos = find_padding_insert_pos(&p.new_bytes);
                    p.new_bytes.insert(pos, space[1]);
                    p.new_bytes.insert(pos, space[0]);
                }
                if p.new_bytes.len() < p.orig_len {
                    p.new_bytes.push(0x00);
                }
            }
        }
    }

    // so that split_common_sub_item_patches sees the same sub-item count
    // in original and new data.  If padding were appended AFTER the last
    // delimiter, it would create an extra sub-item, preventing the split
    // from happening — which breaks per-sub-item pointer computation.
    if seq_type.pointer_pattern().is_some() || seq_type == SeqType::Common || seq_type == SeqType::Diary {
        let space = [(SPACE_TILE >> 8) as u8, (SPACE_TILE & 0xFF) as u8];
        for p in &mut patches {
            let delta = p.new_bytes.len() as isize - p.orig_len as isize;
            let rem = ((delta % 4) + 4) % 4;
            if rem != 0 {
                let pad_tiles = (4 - rem as usize) % 4 / 2; // always 1 tile (2 bytes)
                let nb = &mut p.new_bytes;
                // Find insertion point: skip backwards over ALL trailing FFxx
                // control code pairs so padding never lands after a ctrl sequence
                // (which would cause the game engine to misparse the extra bytes
                // as control code parameters → hardlock).
                let insert_pos = find_padding_insert_pos(nb);
                for _ in 0..pad_tiles {
                    nb.insert(insert_pos, space[1]);
                    nb.insert(insert_pos, space[0]);
                }
            }
        }
    }

    // Split multi-item entries (FF00-delimited) into individual sub-entries
    // so that intra-entry pointer targets get per-sub-item shift computation.
    // Required for COMMON (item/skill name tables), PT/PARTY (monster skill
    // tables with per-skill pointers — without this, pointers to individual
    // skill names within an entry all get the same shift, causing misalignment).
    if seq_type == SeqType::Common || seq_type == SeqType::Pt {
        patches = split_common_sub_item_patches(seq_data, patches);

        // COMMON has a pointer chain at file offset 0xA9B0 containing 318
        // 4-byte RAM pointers.  SH-2 requires longword-aligned access, so
        // the chain's new position must stay ≡ 0 mod 4.  Compute cumulative
        // shift of all patches before the chain; if misaligned, pad the last
        // patch before the boundary.
        const COMMON_PTR_CHAIN: usize = 0xA9B0;
        let cum_shift: isize = if seq_data.len() <= COMMON_PTR_CHAIN { 0 } else {
            patches.iter()
            .filter(|p| p.offset + p.orig_len <= COMMON_PTR_CHAIN)
            .map(|p| p.new_bytes.len() as isize - p.orig_len as isize)
            .sum()
        };
        let rem = ((cum_shift % 4) + 4) % 4;
        if rem != 0 {
            let pad = (4 - rem as usize) % 4;
            if let Some(last) = patches.iter_mut().rev()
                .find(|p| p.offset + p.orig_len <= COMMON_PTR_CHAIN)
            {
                last.new_bytes.extend(std::iter::repeat(0u8).take(pad));
            }
        }
    }

    // Sort ascending for shift computation.
    patches.sort_by_key(|p| p.offset);

    // Validate: no overlapping patches.
    for w in patches.windows(2) {
        let end = w[0].offset + w[0].orig_len;
        ensure!(
            end <= w[1].offset,
            "Overlapping patches at 0x{:X} (len {}) and 0x{:X}: {} byte overlap",
            w[0].offset, w[0].orig_len, w[1].offset, end - w[1].offset
        );
    }

    // Apply splices from end to start (descending offset order).
    let mut result = seq_data.to_vec();
    for p in patches.iter().rev() {
        let end = p.offset + p.orig_len;
        ensure!(
            end <= result.len(),
            "Patch at 0x{:X} (len {}) extends beyond buffer ({})",
            p.offset,
            p.orig_len,
            result.len()
        );
        result.splice(p.offset..end, p.new_bytes.iter().copied());
    }

    // Ensure the final size stays a multiple of 4 bytes.
    // All 112 original SEQ files have sizes ≡ 0 mod 4.  For MP/PT files
    // (where per-entry padding is not applied), this file-end padding keeps
    // the total delta aligned; for COMMON the per-entry padding above
    // already guarantees this, so this is a no-op safety net.
    if result.len() % 4 != 0 {
        let pad = 4 - (result.len() % 4);
        result.extend(std::iter::repeat(0u8).take(pad));
    }

    Ok((result, patches))
}

/// Apply translation patches to a decompressed SEQ buffer, then fix
/// any absolute-offset pointers embedded in the script code.
///
/// Returns the patched buffer and the number of pointers fixed.
///
/// `seq_type` controls which pointer pattern is scanned.  For MP and
/// PT files the 8-byte "call scene" instruction is fixed automatically.
/// For COMMON/DIARY/Other, no pointer fix is attempted (returns 0).
pub fn apply_patches(
    seq_data: &[u8],
    entries: &[TranslationEntry],
    char_table: &HashMap<char, u16>,
    seq_type: SeqType,
    opts: &PatchOptions,
) -> anyhow::Result<(Vec<u8>, usize)> {
    let (mut result, patches) = build_patched_buffer(seq_data, entries, char_table, seq_type)?;

    // Fix absolute-offset pointers in script code.
    let ptrs_fixed = match seq_type {
        _ if !seq_type.pointer_patterns(seq_data.len()).is_empty() && !opts.skip_script_ptrs => {
            let pats = seq_type.pointer_patterns(seq_data.len());
            fix_script_pointers(&mut result, seq_data, &patches, &pats, opts.dump_ptrs)?
        }
        SeqType::Common if !opts.skip_common_ptrs => {
            fix_common_pointers(&mut result, seq_data, &patches, opts.dump_ptrs)?
        }
        SeqType::Diary => {
            fix_diary_pointers(&mut result, seq_data, &patches)?
        }
        _ => 0,
    };

    if opts.dump_seq {
        let type_str = match seq_type {
            SeqType::Common => "COMMON",
            SeqType::Diary => "DIARY",
            _ => "SEQ",
        };
        let path = format!("out/patched_{}.bin", type_str);
        std::fs::write(&path, &result).ok();
        eprintln!("  Dumped patched SEQ: {} bytes → {}", result.len(), path);
    }

    Ok((result, ptrs_fixed))
}

/// Record table regions where only field-0 (first 4 bytes) of each record
/// contains a pointer.  Non-zero field offsets hold numeric data that can
/// coincidentally match the RAM address range, causing false positives.
///
/// Format: `(start_offset, end_offset_exclusive, record_size_bytes)`.
const RECORD_REGIONS: &[(usize, usize, usize)] = &[
    (0x0280, 0x0B54, 20),  // skill_rec:   113 records × 20B
    (0x0B7C, 0x19A4, 24),  // equip_a:     151 records × 24B
    (0x19BC, 0x3024, 24),  // equip_b:     239 records × 24B
    (0x3030, 0x32B8, 12),  // consumable:   54 records × 12B
    (0x0804C, 0x08094, 12), // text_gap_rec:  6 records × 12B
];

/// Return true if offset `i` falls inside a record table at a non-zero
/// field offset (i.e. it's data, not a pointer).
fn is_record_data_field(i: usize) -> bool {
    RECORD_REGIONS.iter().any(|&(start, end, rec_size)| {
        i >= start && i < end && (i - start) % rec_size != 0
    })
}

/// Scan the **entire** original COMMON.SEQ data for `00 20 XX XX` pointers
/// and update them in the patched buffer.
///
/// `00 20` is the RAM segment prefix for COMMON.SEQ data.  Pointers exist
/// in three regions:
///   1. Record tables (0x0280–0x32B0) — before any text patches
///   2. Menu/UI gap pointers — interleaved between text patches
///   3. Chain pointers (0xA9B0–0xAEA8) — after the name table patch
///
/// v4 adds record-field filtering: within known record tables, only
/// field-0 positions are treated as pointers.  This eliminates false
/// positives from data fields that coincidentally match the RAM range
/// (3 confirmed cases: equip_a field 8, equip_b fields 8 and 22).
///
/// Scanning uses 2-byte alignment because COMMON.SEQ data structures
/// are uniformly 2-byte aligned.
fn fix_common_pointers(
    patched: &mut [u8],
    original: &[u8],
    patches: &[PatchInfo],
    dump: bool,
) -> anyhow::Result<usize> {
    let file_len = original.len();
    let mut fixed = 0;
    let mut dump_lines: Vec<String> = Vec::new();

    // PTR_SCAN_END=0xHEX limits pointer scanning to offsets < value.
    // For binary-search diagnostics of which region causes issues.
    let scan_limit: Option<usize> = std::env::var("PTR_SCAN_END")
        .ok()
        .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok());

    if patches.is_empty() {
        return Ok(0);
    }

    // COMMON.SEQ RAM base is 0x00200000.  Pointers are stored as full
    // 32-bit RAM addresses: 0x00200000 + file_offset.  For a 110KB file
    // this means both `00 20 XX XX` (offset < 0x10000) and `00 21 XX XX`
    // (offset >= 0x10000) prefixes occur.
    let ram_base: u32 = 0x00200000;

    let first_patch_end = patches[0].offset + patches[0].orig_len;
    let shift_table = ShiftTable::from_patches(patches);

    let scan_end = match scan_limit {
        Some(limit) => limit.min(original.len().saturating_sub(3)),
        None => original.len().saturating_sub(3),
    };

    // Track which patch we might be inside (patches sorted ascending).
    let mut patch_idx = 0;

    let mut i = 0;
    while i < scan_end {
        // Advance past patches that end before current position.
        while patch_idx < patches.len()
            && patches[patch_idx].offset + patches[patch_idx].orig_len <= i
        {
            patch_idx += 1;
        }

        // Skip positions within a patched entry (text data, not pointers).
        if patch_idx < patches.len() && patches[patch_idx].offset <= i {
            let patch_end = patches[patch_idx].offset + patches[patch_idx].orig_len;
            // Jump past this patch, maintaining 2-byte alignment.
            i = patch_end + (patch_end % 2);
            continue;
        }

        // Skip non-pointer data fields inside record tables.
        if is_record_data_field(i) {
            i += 2;
            continue;
        }

        // Read full 32-bit BE value and check if it's a valid RAM pointer.
        let val = u32::from_be_bytes([
            original[i],
            original[i + 1],
            original[i + 2],
            original[i + 3],
        ]);

        if val < ram_base || val >= ram_base + file_len as u32 {
            i += 2;
            continue;
        }

        let ptr_val = (val - ram_base) as usize;

        // Only fix pointers that target the shifted text region.
        if ptr_val < first_patch_end {
            i += 2;
            continue;
        }

        let target_shift = shift_table.shift_at(ptr_val);
        if target_shift == 0 {
            i += 2;
            continue;
        }

        // Compute where position `i` maps to in the patched buffer.
        let pos_shift = shift_table.shift_at(i);
        let patched_i_signed = i as isize + pos_shift;
        ensure!(
            patched_i_signed >= 0 && (patched_i_signed as usize) + 3 < patched.len(),
            "COMMON pointer at 0x{:04X}: patched position {} out of bounds (buf len {})",
            i, patched_i_signed, patched.len()
        );
        let patched_i = patched_i_signed as usize;

        let new_ram = val as i64 + target_shift as i64;
        ensure!(
            new_ram >= ram_base as i64
                && new_ram < ram_base as i64 + patched.len() as i64,
            "COMMON pointer at 0x{:04X} overflows: 0x{:08X} + ({:+}) = 0x{:X}",
            i,
            val,
            target_shift,
            new_ram
        );
        let new_val = new_ram as u32;

        if dump {
            let region = if i < 0x008C { "master_ptr" }
                else if i < 0x0280 { "skill_grp_ptr" }
                else if i < 0x0B54 { "skill_rec" }
                else if i < 0x1990 { "equip_a" }
                else if i < 0x3010 { "equip_b" }
                else if i < 0x32B0 { "consumable" }
                else if i < 0x7736 { "undoc_data" }
                else if i < 0xA9AA { "text_gap" }
                else if i < 0xAEA8 { "ptr_chain" }
                else { "desc_text" };
            dump_lines.push(format!(
                "0x{:05X}  {:14}  0x{:08X} → 0x{:08X}  (target 0x{:05X}, shift {:+}, pos_shift {:+})",
                i, region, val, new_val, ptr_val, target_shift, pos_shift
            ));
        }

        patched[patched_i] = (new_val >> 24) as u8;
        patched[patched_i + 1] = (new_val >> 16) as u8;
        patched[patched_i + 2] = (new_val >> 8) as u8;
        patched[patched_i + 3] = (new_val & 0xFF) as u8;
        fixed += 1;
        i += 2;
    }

    if dump && !dump_lines.is_empty() {
        let path = "out/common_ptr_dump.txt";
        let content = format!("COMMON.SEQ pointer dump ({} ptrs fixed)\n{}\n{}\n",
            dump_lines.len(),
            "offset     region          old_ram    → new_ram      (target, shift, pos_shift)",
            dump_lines.join("\n"));
        std::fs::write(path, content).ok();
        eprintln!("  Pointer dump: {}", path);
    }

    Ok(fixed)
}

/// Scan the original DIARY.SEQ data for `00 27 XX XX` pointers and update
/// them in the patched buffer.
///
/// DIARY.SEQ uses RAM base 0x00270000.  The structure is simpler than
/// COMMON — no record tables, just a pointer table and page descriptors
/// with embedded text pointers.  All `0027xxxx` values in non-text
/// regions are scanned and adjusted.
fn fix_diary_pointers(
    patched: &mut [u8],
    original: &[u8],
    patches: &[PatchInfo],
) -> anyhow::Result<usize> {
    if patches.is_empty() {
        return Ok(0);
    }

    let ram_base: u32 = 0x00270000;
    let file_len = original.len();
    let first_patch_end = patches[0].offset + patches[0].orig_len;
    let shift_table = ShiftTable::from_patches(patches);
    let mut fixed = 0;

    let mut patch_idx = 0;
    let mut i = 0;
    let scan_end = original.len().saturating_sub(3);

    while i < scan_end {
        // Advance past patches that end before current position.
        while patch_idx < patches.len()
            && patches[patch_idx].offset + patches[patch_idx].orig_len <= i
        {
            patch_idx += 1;
        }

        // Skip positions within a patched entry (text data, not pointers).
        if patch_idx < patches.len() && patches[patch_idx].offset <= i {
            let patch_end = patches[patch_idx].offset + patches[patch_idx].orig_len;
            i = patch_end + (patch_end % 2);
            continue;
        }

        let val = u32::from_be_bytes([
            original[i],
            original[i + 1],
            original[i + 2],
            original[i + 3],
        ]);

        if val < ram_base || val >= ram_base + file_len as u32 {
            i += 2;
            continue;
        }

        let ptr_val = (val - ram_base) as usize;

        // Only fix pointers that target the shifted text region.
        if ptr_val < first_patch_end {
            i += 2;
            continue;
        }

        let target_shift = shift_table.shift_at(ptr_val);
        if target_shift == 0 {
            i += 2;
            continue;
        }

        let pos_shift = shift_table.shift_at(i);
        let patched_i_signed = i as isize + pos_shift;
        ensure!(
            patched_i_signed >= 0 && (patched_i_signed as usize) + 3 < patched.len(),
            "DIARY pointer at 0x{:04X}: patched position {} out of bounds (buf len {})",
            i, patched_i_signed, patched.len()
        );
        let patched_i = patched_i_signed as usize;

        let new_ram = val as i64 + target_shift as i64;
        ensure!(
            new_ram >= ram_base as i64
                && new_ram < ram_base as i64 + patched.len() as i64,
            "DIARY pointer at 0x{:04X} overflows: 0x{:08X} + ({:+}) = 0x{:X}",
            i, val, target_shift, new_ram
        );
        let new_val = new_ram as u32;

        patched[patched_i] = (new_val >> 24) as u8;
        patched[patched_i + 1] = (new_val >> 16) as u8;
        patched[patched_i + 2] = (new_val >> 8) as u8;
        patched[patched_i + 3] = (new_val & 0xFF) as u8;
        fixed += 1;
        i += 2;
    }

    Ok(fixed)
}

/// Scan the original SEQ data for absolute-offset pointers and update
/// them in the patched buffer.
///
/// Handles multiple pointer patterns (e.g. `00 24` + `00 25` for >64KB MP
/// files) in a single 2-byte-aligned scan.  When a text shift causes a
/// pointer target to cross the 64KB bank boundary, the prefix bytes are
/// updated accordingly (`00 24` ↔ `00 25`).
///
/// Patched entry interiors are skipped to avoid corrupting replacement
/// text bytes.
fn fix_script_pointers(
    patched: &mut [u8],
    original: &[u8],
    patches: &[PatchInfo],
    patterns: &[PointerPattern],
    dump: bool,
) -> anyhow::Result<usize> {
    let file_len = original.len();
    let mut fixed = 0;
    let mut dump_lines: Vec<String> = Vec::new();

    let first_patch_start = patches
        .first()
        .map(|p| p.offset)
        .unwrap_or(file_len);

    let first_patch_end = patches
        .first()
        .map(|p| p.offset + p.orig_len)
        .unwrap_or(file_len);
    let shift_table = ShiftTable::from_patches(patches);

    // PTR_SCAN_END=0xHEX limits script pointer scan range (for diagnostics).
    let scan_end: usize = std::env::var("PTR_SCAN_END")
        .ok()
        .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(original.len());

    // Track which patch we might be inside (patches sorted ascending).
    let mut patch_idx = 0;

    // Scan with 2-byte alignment.
    let mut i = 0;
    while i + 3 < original.len() && i < scan_end {
        // Advance past patches that end before current position.
        while patch_idx < patches.len()
            && patches[patch_idx].offset + patches[patch_idx].orig_len <= i
        {
            patch_idx += 1;
        }

        // Skip positions within a patched entry (text data, not pointers).
        if patch_idx < patches.len() && patches[patch_idx].offset <= i {
            let patch_end = patches[patch_idx].offset + patches[patch_idx].orig_len;
            // Jump past this patch, maintaining 2-byte alignment.
            i = patch_end + (patch_end % 2);
            continue;
        }

        // Check if any pattern's prefix matches at this position.
        let matched = patterns.iter().find(|p| {
            original[i] == p.prefix[0] && original[i + 1] == p.prefix[1]
        });
        let matched = match matched {
            Some(p) => p,
            None => { i += 2; continue; }
        };

        let ptr_val = ((original[i + 2] as usize) << 8) | (original[i + 3] as usize);
        let file_offset = matched.base_offset + ptr_val;

        // All valid SEQ pointers target 2-byte-aligned addresses (text tokens
        // are 2 bytes). An odd file_offset is a definitive false positive
        // (e.g. script opcode `23 01 00 25 13 81` where 00 25 is param value 37).
        if file_offset % 2 != 0 {
            i += 2;
            continue;
        }

        // (suffix blacklist removed — produced false negatives that broke NPC display)

        // Only fix pointers that target the shifted text region.
        if file_offset < first_patch_end || file_offset >= file_len {
            i += 2;
            continue;
        }

        let shift = shift_table.shift_at(file_offset);
        if shift == 0 {
            i += 2;
            continue;
        }

        let new_file_offset = file_offset as isize + shift;
        ensure!(
            new_file_offset >= 0 && (new_file_offset as usize) < file_len + 0x10000,
            "Pointer at 0x{:04X}: file_off 0x{:05X} + ({:+}) = 0x{:X} out of range",
            i, file_offset, shift, new_file_offset
        );
        let new_file_offset_u = new_file_offset as usize;

        // Determine which bank the NEW file offset belongs to.
        // Find the pattern whose bank contains the new offset.
        let new_bank = patterns.iter().find(|p| {
            new_file_offset_u >= p.base_offset
                && new_file_offset_u < p.base_offset + 0x10000
        });
        let new_bank = match new_bank {
            Some(b) => b,
            None => anyhow::bail!(
                "Pointer at 0x{:04X}: new file_off 0x{:05X} falls outside all banks",
                i, new_file_offset_u
            ),
        };

        let new_val = (new_file_offset_u - new_bank.base_offset) as u16;

        // Compute the position of this pointer in the patched buffer.
        let pos_shift = shift_table.shift_at(i);
        let patched_i_signed = i as isize + pos_shift;
        ensure!(
            patched_i_signed >= 0 && (patched_i_signed as usize) + 3 < patched.len(),
            "Script pointer at 0x{:04X}: patched position {} out of bounds (buf len {})",
            i, patched_i_signed, patched.len()
        );
        let patched_i = patched_i_signed as usize;

        let bank_crossed = matched.prefix != new_bank.prefix;

        // Bank-crossing validation: changing prefix bytes (0025→0024) is
        // destructive — only allow when we're confident this is a real pointer.
        // Legitimate bank-crossing pointers (event handlers) have suffix
        // 00 00 00 05 or 00 00 00 11 at i+4. Data values like coordinates
        // (e.g. 0x25 = 37) that coincidentally match the 0025 prefix do NOT
        // have these suffixes.
        if bank_crossed && i + 7 < original.len() {
            let s = &original[i + 4..i + 8];
            let valid_suffix = (s == [0x00, 0x00, 0x00, 0x05])
                || (s == [0x00, 0x00, 0x00, 0x11]);
            if !valid_suffix {
                if dump {
                    let ctx_start = i.saturating_sub(4);
                    let ctx_end = (i + 8).min(original.len());
                    let ctx: String = original[ctx_start..ctx_end]
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    dump_lines.push(format!(
                        "0x{:04X}  SKIP-BANK-CROSS  0x{:05X} → 0x{:05X} (no valid suffix)  ctx: {}",
                        i, file_offset, new_file_offset_u, ctx
                    ));
                }
                i += 2;
                continue;
            }
        }
        if dump {
            let region = if i < first_patch_start { "code" } else { "text_gap" };
            let ctx_start = i.saturating_sub(4);
            let ctx_end = (i + 8).min(original.len());
            let ctx: String = original[ctx_start..ctx_end]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let cross_marker = if bank_crossed { " BANK-CROSS" } else { "" };
            dump_lines.push(format!(
                "0x{:04X}  {:8}  {:02X}{:02X} 0x{:05X} → {:02X}{:02X} 0x{:05X}  (shift {:+}, pos_shift {:+}){}  ctx: {}",
                i, region,
                matched.prefix[0], matched.prefix[1], file_offset,
                new_bank.prefix[0], new_bank.prefix[1], new_file_offset_u,
                shift, pos_shift, cross_marker, ctx
            ));
        }

        // Write all 4 bytes: prefix (may change on bank cross) + value.
        patched[patched_i] = new_bank.prefix[0];
        patched[patched_i + 1] = new_bank.prefix[1];
        patched[patched_i + 2] = (new_val >> 8) as u8;
        patched[patched_i + 3] = (new_val & 0xFF) as u8;
        fixed += 1;
        i += 2;
    }

    if dump && !dump_lines.is_empty() {
        let path = "out/script_ptr_dump.txt";
        let bank_crossed_count = dump_lines.iter().filter(|l| l.contains("BANK-CROSS")).count();
        let content = format!(
            "Script pointer dump ({} ptrs fixed, {} bank-crossed, patterns: {})\n{}\n{}\n",
            dump_lines.len(),
            bank_crossed_count,
            patterns.iter().map(|p| format!("{:02X}{:02X}+0x{:X}", p.prefix[0], p.prefix[1], p.base_offset)).collect::<Vec<_>>().join(", "),
            "offset    region    old             → new              (shift, pos_shift)  context",
            dump_lines.join("\n")
        );
        std::fs::write(path, content).ok();
        eprintln!("  Script pointer dump: {}", path);
    }

    Ok(fixed)
}

// ---------------------------------------------------------------------------
// Token parsing
// ---------------------------------------------------------------------------

/// Parse Korean translation text into TextToken list.
///
/// Recognizes:
/// - Korean characters (가-힣), spaces, ellipsis (…), brackets (「」) → Text
/// - `{tile:XXXX}` → Tile(0xXXXX)
/// - `{ctrl:XXXX}` → Ctrl(0xXXXX) (each colon-separated part becomes a Ctrl token)
pub fn parse_ko_tokens(text: &str) -> Vec<TextToken> {
    let mut tokens = Vec::new();
    let mut current_text = String::new();
    let mut chars = text.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '{' {
            // Flush accumulated text.
            if !current_text.is_empty() {
                tokens.push(TextToken::Text(current_text.clone()));
                current_text.clear();
            }
            // Parse tag.
            let mut tag = String::new();
            chars.next(); // consume '{'
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    break;
                }
                tag.push(c);
                chars.next();
            }
            if let Some(rest) = tag.strip_prefix("tile:") {
                if let Ok(code) = u16::from_str_radix(rest, 16) {
                    tokens.push(TextToken::Tile(code));
                }
            } else if let Some(rest) = tag.strip_prefix("ctrl:") {
                for part in rest.split(':') {
                    if let Ok(code) = u16::from_str_radix(part, 16) {
                        tokens.push(TextToken::Ctrl(code));
                    }
                }
            }
            else if let Some(rest) = tag.strip_prefix("wide:") {
                // {wide:NNN} → tile code 128 + NNN * 2
                if let Ok(idx) = rest.parse::<u16>() {
                    let tile_code = 128 + idx * 2;
                    tokens.push(TextToken::Tile(tile_code));
                }
            }
        } else {
            current_text.push(ch);
            chars.next();
        }
    }

    if !current_text.is_empty() {
        tokens.push(TextToken::Text(current_text));
    }

    tokens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "patcher_tests.rs"]
mod tests;
