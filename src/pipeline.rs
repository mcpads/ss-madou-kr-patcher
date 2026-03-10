//! Shared build pipeline stages for ROM patching commands.
//!
//! Extracts the common logic from `cmd_poc_korean`, `cmd_poc_prologue`, and
//! `cmd_build_rom` into reusable functions that operate on [`TrackedDisc`].

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::compression;
use crate::disc::sector::USER_DATA_SIZE;
use crate::disc::{Iso9660, TrackedDisc};
use crate::font::korean;
use crate::font::prologue;
use crate::text::patcher;

// ---------------------------------------------------------------------------
// Context structs
// ---------------------------------------------------------------------------

/// Disc context: the tracked disc image + ISO 9660 filesystem.
pub struct DiscCtx {
    pub disc: TrackedDisc,
    pub iso: Iso9660,
}

/// Font context: decompressed FONT.CEL + original disc entry metadata.
pub struct FontCtx {
    pub font_cel: Vec<u8>,
    pub original_compressed_len: usize,
    pub original_lba: u32,
    pub cnx_subtype: [u8; 3],
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write compressed data back to disc, relocating if necessary.
///
/// - If the compressed data fits within the original size, pad and write in place.
/// - If it exceeds the original sector count, relocate via ISO 9660.
/// - Otherwise (grew but same sector count), write in place and update ISO file size.
///
/// Returns `true` if the file was relocated, `false` if written in place.
fn write_compressed_file(
    ctx: &mut DiscCtx,
    filename: &str,
    lba: u32,
    original_compressed_len: usize,
    compressed: &[u8],
    label: &str,
) -> Result<bool> {
    let original_sectors = (original_compressed_len + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let new_sectors = (compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

    if compressed.len() <= original_compressed_len {
        let mut padded = compressed.to_vec();
        padded.resize(original_compressed_len, 0x00);
        ctx.disc
            .write_file_at(lba, &padded, &format!("{}:inplace", label))
            .context(format!("Failed to write {}", filename))?;
        Ok(false)
    } else if new_sectors > original_sectors {
        ctx.iso
            .relocate_file_tracked(&mut ctx.disc, filename, compressed)
            .context(format!("Failed to relocate {}", filename))?;
        Ok(true)
    } else {
        ctx.disc
            .write_file_at(lba, compressed, &format!("{}:inplace", label))
            .context(format!("Failed to write {}", filename))?;
        ctx.iso
            .patch_file_size_tracked(&mut ctx.disc, filename, compressed.len() as u32)
            .context(format!("Failed to update {} size", filename))?;
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Pipeline stages
// ---------------------------------------------------------------------------

/// Load a ROM disc image and parse ISO 9660.
pub fn load_disc(rom: &Path) -> Result<DiscCtx> {
    println!("Loading ROM: {}", rom.display());
    let disc =
        crate::disc::DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso = Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;
    let tracked = TrackedDisc::new(disc);
    Ok(DiscCtx {
        disc: tracked,
        iso,
    })
}

/// Extract and decompress FONT.CEL from the disc.
pub fn extract_font(ctx: &DiscCtx) -> Result<FontCtx> {
    let font_entry = ctx
        .iso
        .find_file(ctx.disc.disc(), "FONT.CEL")
        .context("Failed searching ISO 9660")?
        .context("FONT.CEL not found in disc image")?;
    let original_compressed = ctx
        .iso
        .extract_file(ctx.disc.disc(), &font_entry)
        .context("Failed to extract FONT.CEL")?;
    let header =
        compression::parse_header(&original_compressed).context("Failed to parse CNX header")?;
    let font_cel =
        compression::decompress(&original_compressed).context("Failed to decompress FONT.CEL")?;

    println!(
        "FONT.CEL: LBA {}, {} bytes compressed, {} bytes decompressed",
        font_entry.lba,
        original_compressed.len(),
        font_cel.len()
    );

    Ok(FontCtx {
        font_cel,
        original_compressed_len: original_compressed.len(),
        original_lba: font_entry.lba,
        cnx_subtype: header.subtype,
    })
}

/// Load a TTF font and generate 4bpp tile data for the given characters.
///
/// Returns `(Font, Vec<(char, tile_data)>)`.
pub fn generate_korean_glyphs(
    font_path: &Path,
    chars: &[char],
    font_size: f32,
) -> Result<Vec<(char, [u8; 128])>> {
    let ttf_data = std::fs::read(font_path)
        .context(format!("Failed to read font: {}", font_path.display()))?;
    let ko_font = korean::load_font(&ttf_data).map_err(|e| anyhow::anyhow!(e))?;
    println!("Font: {} (size {}px)", font_path.display(), font_size);

    let glyph_tiles = korean::generate_tiles(&ko_font, chars, font_size);
    Ok(glyph_tiles)
}

/// Sec6 16×16 characters to re-render with the Korean font.
///
/// Each entry: `(character, first_tile_index)`.
/// Only includes characters actually referenced in translations (ko_count > 0)
/// or hardcoded in the engine (! = hardcoded in glyph.rs, … = ELLIPSIS_TILE).
/// Excludes reclaimable slots (、, 。, ", D, Q, U, V, W, Y, Z, h, ヴ) and
/// bottom-half-only refs (I, J, X).
const SEC6_RENDER_CHARS: &[(char, usize)] = &[
    ('0', 182), ('1', 186), ('2', 190), ('3', 194),
    ('4', 198), ('5', 202), ('6', 206), ('7', 210),
    ('8', 214), ('9', 218),
    ('-', 222), ('\u{00B7}', 226), // · Middle Dot (원본 tile = ・)
    ('!', 230), ('?', 234),
    ('\u{2190}', 250), // ← Arrow Left
    ('\u{300C}', 254), // 「 Corner Bracket Left
    ('\u{300D}', 258), // 」 Corner Bracket Right
    ('A', 262), ('B', 266), ('C', 270),
    ('E', 278), ('F', 282), ('G', 286),
    ('H', 290), ('K', 302), ('L', 306),
    ('M', 310), ('N', 314), ('O', 318),
    ('P', 322), ('R', 330), ('S', 334),
    ('T', 338),
    ('~', 370),
    ('\u{2605}', 382), // ★ Star
    ('\u{2026}', 386), // … Ellipsis
    ('(', 390), (')', 394),
    ('&', 398), ('/', 402), ('%', 406),
    ('\u{2192}', 410), // → Arrow Right
    ('\u{2191}', 414), // ↑ Arrow Up
    ('\u{266A}', 418), // ♪ Music Note
    ('+', 434),
];

/// Sec6 tile positions reclaimable for Korean glyphs.
///
/// These are 16×16 wide characters with 0 references in both top and bottom
/// halves across all translations. First tile index of each 2×2 glyph.
pub const SEC6_RECLAIM_TILES: &[usize] = &[
    238, // 、 Ideographic Comma (ko 미사용)
    242, // 。 Ideographic Period (ko 미사용)
    246, // " Double Quote
    274, // D
    294, // I (ko 미사용)
    298, // J (ko 미사용)
    326, // Q
    342, // U
    346, // V
    350, // W
    358, // Y
    362, // Z
    366, // h
    422, // ヴ Katakana Vu (ko 미사용)
    426, // " Left DQ (ko에서 「로 교체)
    430, // " Right DQ (ko에서 」로 교체)
    354, // X (ko 미사용)
    374, // 『 White Corner Bracket Left (ko에서 삭제)
    378, // 』 White Corner Bracket Right (ko에서 삭제)
];

/// SEC6_RENDER_CHARS에서 char → sec6 tile code 매핑 빌드.
/// `is_wide_tile_char()`로 이미 처리되는 문자(…, 「, 」)는 제외.
pub fn sec6_direct_tile_map() -> HashMap<char, u16> {
    SEC6_RENDER_CHARS
        .iter()
        .filter(|&&(ch, _)| !matches!(ch, '\u{2026}' | '\u{300C}' | '\u{300D}'))
        .map(|&(ch, tile)| (ch, tile as u16))
        .collect()
}

/// Re-render sec6 16×16 characters with the specified font.
///
/// Overwrites the original JP glyphs in FONT.CEL tiles 178–437 so that
/// digits, punctuation, and Latin letters match the Korean font visually.
pub fn render_sec6_glyphs(
    font_ctx: &mut FontCtx,
    font_path: &Path,
    font_size: f32,
) -> Result<()> {
    let ttf_data = std::fs::read(font_path)
        .context(format!("Failed to read font for sec6: {}", font_path.display()))?;
    let font = korean::load_font(&ttf_data).map_err(|e| anyhow::anyhow!(e))?;

    let mut rendered = 0usize;
    for &(ch, first_tile) in SEC6_RENDER_CHARS {
        let coverage = korean::render_glyph(&font, ch, font_size);
        // Skip if font doesn't have this glyph (all-zero coverage).
        if coverage.iter().all(|&b| b == 0) {
            continue;
        }
        let tiles = korean::coverage_to_4bpp_tiles_with_mode(&coverage, korean::RenderMode::Outline);
        korean::patch_font_cel_at_tile(&mut font_ctx.font_cel, first_tile, &tiles)?;
        rendered += 1;
    }
    println!("  Sec6 re-rendered: {}/{} characters", rendered, SEC6_RENDER_CHARS.len());

    Ok(())
}

/// Patch FONT.CEL with generated glyph tiles, compress, and write to disc.
///
/// If the compressed FONT.CEL exceeds its original sector allocation, it is
/// relocated to a free region at the end of the disc. Otherwise it is written
/// in place.
///
/// Handles both sec7 glyph slots (tile >= GLYPH_TILE_START) and sec6
/// reclaimed slots (tile < GLYPH_TILE_START) transparently.
pub fn patch_font(
    ctx: &mut DiscCtx,
    font_ctx: &mut FontCtx,
    glyph_tiles: &[(char, [u8; 128])],
    char_table: &std::collections::HashMap<char, u16>,
) -> Result<()> {
    // Patch glyph tile data into FONT.CEL.
    for (ch, tile_data) in glyph_tiles {
        let slot = char_table[ch];
        if (slot as usize) < korean::GLYPH_TILE_START {
            // Sec6 reclaimed slot — write directly at tile position.
            korean::patch_font_cel_at_tile(&mut font_ctx.font_cel, slot as usize, tile_data)?;
        } else {
            // Sec7 glyph slot.
            let glyph_idx =
                ((slot as usize) - korean::GLYPH_TILE_START) / korean::TILES_PER_GLYPH;
            korean::patch_font_cel(&mut font_ctx.font_cel, glyph_idx, tile_data)?;
        }
    }

    // Compress.
    println!("Compressing FONT.CEL...");
    let compressed = compression::compress(&font_ctx.font_cel, &font_ctx.cnx_subtype);
    println!(
        "FONT.CEL: {} → {} bytes",
        font_ctx.original_compressed_len,
        compressed.len()
    );

    let original_sectors =
        (font_ctx.original_compressed_len + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let new_sectors = (compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

    let relocated = write_compressed_file(
        ctx,
        "FONT.CEL",
        font_ctx.original_lba,
        font_ctx.original_compressed_len,
        &compressed,
        "FONT.CEL",
    )?;
    if relocated {
        println!(
            "  Relocated FONT.CEL ({} → {} sectors)",
            original_sectors, new_sectors
        );
    } else if compressed.len() <= font_ctx.original_compressed_len {
        println!(
            "  Wrote FONT.CEL in place at LBA {} (padded {} → {})",
            font_ctx.original_lba,
            compressed.len(),
            font_ctx.original_compressed_len
        );
    } else {
        println!(
            "  Wrote FONT.CEL in place at LBA {} (ISO size updated: {} → {})",
            font_ctx.original_lba, font_ctx.original_compressed_len, compressed.len()
        );
    }

    Ok(())
}

/// Extract a SEQ file, apply translation patches, compress, and write back.
///
/// Returns `(ptrs_fixed, was_relocated, new_decompressed_size)`.
pub fn patch_seq(
    ctx: &mut DiscCtx,
    source: &str,
    entries: &[patcher::TranslationEntry],
    char_table: &std::collections::HashMap<char, u16>,
    opts: &patcher::PatchOptions,
) -> Result<(usize, bool, usize)> {
    let seq_entry = match ctx.iso.find_file(ctx.disc.disc(), source) {
        Ok(Some(e)) => e,
        Ok(None) => {
            eprintln!("  WARNING: {} not found on disc, skipping", source);
            return Ok((0, false, 0));
        }
        Err(e) => {
            eprintln!("  WARNING: Error finding {}: {}, skipping", source, e);
            return Ok((0, false, 0));
        }
    };

    let original_compressed = ctx
        .iso
        .extract_file(ctx.disc.disc(), &seq_entry)
        .context(format!("Failed to extract {}", source))?;
    let original_header = compression::parse_header(&original_compressed)
        .context(format!("Failed to parse {} CNX header", source))?;
    let seq_data = compression::decompress(&original_compressed)
        .context(format!("Failed to decompress {}", source))?;

    let seq_type = patcher::SeqType::from_filename(source);
    let (patched_seq, ptrs_fixed) =
        patcher::apply_patches(&seq_data, entries, char_table, seq_type, opts)?;

    let compressed_seq = compression::compress(&patched_seq, &original_header.subtype);

    // Write compressed output back to disc.
    // The CNX header's compressed_size field tells the decompressor where the
    // real data ends; trailing padding bytes are never read.
    let relocated = write_compressed_file(
        ctx,
        source,
        seq_entry.lba,
        original_compressed.len(),
        &compressed_seq,
        source,
    )?;
    if relocated {
        println!(
            "  {} → relocated ({} entries, {:+} bytes, {} → {} compressed, {} ptrs)",
            source,
            entries.len(),
            patched_seq.len() as isize - seq_data.len() as isize,
            original_compressed.len(),
            compressed_seq.len(),
            ptrs_fixed
        );
    } else {
        println!(
            "  {} → in place ({} entries, {:+} bytes decomp, {} compressed → {} padded to {}, {} ptrs)",
            source,
            entries.len(),
            patched_seq.len() as isize - seq_data.len() as isize,
            compressed_seq.len(),
            original_compressed.len(),
            original_compressed.len(),
            ptrs_fixed
        );
    }

    Ok((ptrs_fixed, relocated, patched_seq.len()))
}

/// Apply all 1ST_READ.BIN patches in a single read-modify-write cycle.
///
/// Combines decompression buffer relocation, FONT.CEL buffer size update,
/// and SEQ decompressed size table updates to avoid sector write collisions
/// (TrackedDisc only allows one write per LBA range).
///
/// `seq_sizes` maps SEQ filename (e.g. "COMMON.SEQ") to new decompressed size.
pub fn patch_first_read_combined(
    ctx: &mut DiscCtx,
    new_decompressed_size: usize,
    seq_sizes: &[(&str, usize)],
) -> Result<()> {
    const FIRST_READ_NAME: &str = "0";
    const FONT_BUFSIZE_OFFSET: usize = 0x035DF4;
    const OLD_BUFFER_ADDR: [u8; 4] = [0x06, 0x07, 0xCC, 0x60];
    const NEW_BUFFER_ADDR: [u8; 4] = [0x00, 0x2C, 0x00, 0x00];
    const LITERAL_POOL_OFFSETS: [usize; 2] = [0x023BA8, 0x024BD8];
    /// SEQ decompressed size table: 112 entries × 28 bytes.
    /// Format: [20 zero bytes][4-byte BE size][4-byte BE name string pointer]
    const SEQ_SIZE_TABLE_START: usize = 0x0427F0;
    const SEQ_SIZE_TABLE_ENTRIES: usize = 112;
    const SEQ_SIZE_TABLE_ENTRY_SIZE: usize = 28;
    /// Base RAM address of 1ST_READ.BIN (for resolving name string pointers).
    const RAM_BASE: u32 = 0x06004000;

    let first_read_entry = ctx
        .iso
        .find_file(ctx.disc.disc(), FIRST_READ_NAME)
        .context("Failed searching for 1ST_READ.BIN")?
        .context("1ST_READ.BIN ('0') not found on disc")?;
    let mut data = ctx
        .iso
        .extract_file(ctx.disc.disc(), &first_read_entry)
        .context("Failed to extract 1ST_READ.BIN")?;

    // --- Patch 1: Decompression buffer relocation ---
    for &offset in &LITERAL_POOL_OFFSETS {
        anyhow::ensure!(
            offset + 4 <= data.len(),
            "Literal pool offset 0x{:06X} out of range",
            offset
        );
        anyhow::ensure!(
            data[offset..offset + 4] == OLD_BUFFER_ADDR,
            "Expected 0x0607CC60 at 1ST_READ.BIN+0x{:06X}, found {:02X}{:02X}{:02X}{:02X}",
            offset,
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        );
        data[offset..offset + 4].copy_from_slice(&NEW_BUFFER_ADDR);
    }
    println!(
        "Patched 1ST_READ.BIN: decompression buffer 0x0607CC60 → 0x002C0000 (Work RAM Low)"
    );

    // --- Patch 2: FONT.CEL descriptor buffer size ---
    let original_bufsize = u32::from_be_bytes([
        data[FONT_BUFSIZE_OFFSET],
        data[FONT_BUFSIZE_OFFSET + 1],
        data[FONT_BUFSIZE_OFFSET + 2],
        data[FONT_BUFSIZE_OFFSET + 3],
    ]);
    anyhow::ensure!(
        original_bufsize == 122_048,
        "Expected FONT.CEL buffer size 122048 at 1ST_READ.BIN+0x{:06X}, found {}",
        FONT_BUFSIZE_OFFSET,
        original_bufsize
    );
    let new_bufsize = new_decompressed_size as u32;
    data[FONT_BUFSIZE_OFFSET..FONT_BUFSIZE_OFFSET + 4]
        .copy_from_slice(&new_bufsize.to_be_bytes());
    println!(
        "Patched 1ST_READ.BIN: FONT.CEL buffer size {} → {} bytes",
        original_bufsize, new_bufsize
    );

    // --- Patch 3: SEQ decompressed size table ---
    if !seq_sizes.is_empty() {
        let mut sizes_updated = 0usize;
        for i in 0..SEQ_SIZE_TABLE_ENTRIES {
            let entry_off = SEQ_SIZE_TABLE_START + i * SEQ_SIZE_TABLE_ENTRY_SIZE;
            if entry_off + SEQ_SIZE_TABLE_ENTRY_SIZE > data.len() {
                break;
            }
            // Read name string pointer (bytes 24..28 of entry)
            let name_ptr_off = entry_off + 24;
            let name_ptr = u32::from_be_bytes([
                data[name_ptr_off],
                data[name_ptr_off + 1],
                data[name_ptr_off + 2],
                data[name_ptr_off + 3],
            ]);
            // Resolve name string from file offset
            let name_file_off = (name_ptr.wrapping_sub(RAM_BASE)) as usize;
            if name_file_off >= data.len() {
                continue;
            }
            let name_end = data[name_file_off..]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(20)
                + name_file_off;
            let entry_name = std::str::from_utf8(&data[name_file_off..name_end])
                .unwrap_or("")
                .to_owned();

            // Check if this entry matches any patched SEQ
            for &(seq_name, new_size) in seq_sizes {
                if entry_name.eq_ignore_ascii_case(seq_name) {
                    let size_off = entry_off + 20;
                    let old_size = u32::from_be_bytes([
                        data[size_off],
                        data[size_off + 1],
                        data[size_off + 2],
                        data[size_off + 3],
                    ]);
                    let new_size_u32 = new_size as u32;
                    if old_size != new_size_u32 {
                        data[size_off..size_off + 4]
                            .copy_from_slice(&new_size_u32.to_be_bytes());
                        println!(
                            "  SEQ size table: {} {} → {} bytes ({:+})",
                            entry_name,
                            old_size,
                            new_size_u32,
                            new_size as i64 - old_size as i64
                        );
                        sizes_updated += 1;
                    }
                    break;
                }
            }
        }
        println!(
            "Patched 1ST_READ.BIN: {} SEQ decompressed sizes updated",
            sizes_updated
        );
    }

    // --- Write once ---
    ctx.disc
        .write_file_at(first_read_entry.lba, &data, "1ST_READ.BIN:combined")
        .context("Failed to write patched 1ST_READ.BIN")?;

    Ok(())
}

/// Check for sector collisions and save the disc image + CUE file.
pub fn save_disc(ctx: &mut DiscCtx, output: &Path) -> Result<()> {
    // Abort on write collisions — a colliding ROM is corrupted and will crash.
    ctx.disc
        .check()
        .map_err(|report| anyhow::anyhow!("{}", report))?;

    if ctx.disc.region_count() > 0 {
        ctx.disc.dump_regions();
    }

    // Regenerate EDC/ECC for all Mode 1 data track sectors.
    // Data track = Track 01, 57,318 sectors (see CUE: Track 02 starts at 12:44:18).
    const DATA_SECTOR_COUNT: usize = 57_318;
    let edc_count = ctx.disc.regenerate_edc_ecc(DATA_SECTOR_COUNT);
    println!("EDC/ECC regenerated: {} sectors", edc_count);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }
    ctx.disc
        .save(output)
        .context("Failed to save disc image")?;
    println!("\nPatched ROM saved to: {}", output.display());

    // Generate CUE file.
    let cue_path = output.with_extension("cue");
    let bin_filename = output
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let cue_content = format!(
        "FILE \"{}\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 12:44:18\n",
        bin_filename
    );
    std::fs::write(&cue_path, &cue_content).context("Failed to write CUE file")?;
    println!("CUE file saved to: {}", cue_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// Prologue sprite patch (OP_SP02.SPR)
// ---------------------------------------------------------------------------

/// Render Korean prologue text and patch OP_SP02.SPR on disc.
///
/// The sprite is a 288×208 4bpp bitmap with a 32-byte Saturn 15-bit RGB
/// palette header. It is stored CNX-compressed on disc.
pub fn patch_prologue_sprite(
    ctx: &mut DiscCtx,
    prologue_font_path: &Path,
    prologue_font_size: f32,
) -> Result<()> {
    const SPR_NAME: &str = "OP_SP02.SPR";

    println!("\nPatching prologue sprite ({})...", SPR_NAME);

    // Load prologue font
    let ttf_data = std::fs::read(prologue_font_path)
        .context(format!("Failed to read prologue font: {}", prologue_font_path.display()))?;
    let font = korean::load_font(&ttf_data).map_err(|e| anyhow::anyhow!(e))?;
    println!(
        "  Prologue font: {} ({}px)",
        prologue_font_path.display(),
        prologue_font_size
    );

    // Render Korean text to SPR format
    let ko_spr_data = prologue::render_prologue_sprite(&font, prologue_font_size);
    println!(
        "  Rendered: {}x{} -> {} bytes (palette + 4bpp)",
        prologue::SPRITE_WIDTH,
        prologue::SPRITE_HEIGHT,
        ko_spr_data.len()
    );

    // Extract original to get CNX subtype for recompression
    let spr_entry = ctx
        .iso
        .find_file(ctx.disc.disc(), SPR_NAME)?
        .context(format!("{} not found on disc", SPR_NAME))?;

    let original_compressed = ctx
        .iso
        .extract_file(ctx.disc.disc(), &spr_entry)?;
    let original_header = compression::parse_header(&original_compressed)?;

    println!(
        "  Original: {} bytes compressed, {} bytes decompressed",
        original_compressed.len(),
        original_header.decompressed_size
    );

    // Verify original round-trip first
    let original_decompressed = compression::decompress(&original_compressed)?;
    println!(
        "  Original decompressed: {} bytes",
        original_decompressed.len()
    );

    let spr_data = ko_spr_data;

    // Compress the new sprite data
    let compressed = compression::compress(&spr_data, &original_header.subtype);
    println!(
        "  Compressed: {} -> {} bytes",
        spr_data.len(),
        compressed.len()
    );

    // Round-trip verify: decompress our compressed data
    let verify = compression::decompress(&compressed)?;
    if verify == spr_data {
        println!("  CNX round-trip: PASS");
    } else {
        println!(
            "  CNX round-trip: FAIL ({} bytes vs {} bytes, {} differ)",
            verify.len(),
            spr_data.len(),
            verify.iter().zip(&spr_data).filter(|(a, b)| a != b).count()
        );
    }

    // Write back to disc (relocate if larger)
    let original_sectors = (original_compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let new_sectors = (compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

    let relocated = write_compressed_file(
        ctx,
        SPR_NAME,
        spr_entry.lba,
        original_compressed.len(),
        &compressed,
        SPR_NAME,
    )?;
    if relocated {
        println!(
            "  Relocated {} (grew: {} -> {} sectors)",
            SPR_NAME, original_sectors, new_sectors
        );
    } else if compressed.len() <= original_compressed.len() {
        println!(
            "  Wrote {} in place at LBA {} (padded {} → {})",
            SPR_NAME,
            spr_entry.lba,
            compressed.len(),
            original_compressed.len()
        );
    } else {
        println!(
            "  Wrote {} in place at LBA {} (ISO size updated)",
            SPR_NAME, spr_entry.lba
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Battle UI sprite patch (SYSTEM.SPR)
// ---------------------------------------------------------------------------

/// Patch SYSTEM.SPR: battle UI kanji tiles + menu tab sprites in a single pass.
///
/// Decompresses SYSTEM.SPR once, applies both battle UI tile patches (16×16
/// 4bpp at 0x7800–0x7B00) and menu tab sprite patches (40×20 sel + 40×14
/// unsel × 6), then recompresses and writes back to disc once.
pub fn patch_system_sprite(
    ctx: &mut DiscCtx,
    battle_ui_font_path: Option<&Path>,
    battle_ui_font_size: f32,
    menu_tab_font_path: Option<&Path>,
    menu_tab_font_size: f32,
) -> Result<()> {
    use crate::font::battle_menu;
    use crate::font::battle_ui;

    const SPR_NAME: &str = "SYSTEM.SPR";

    // Nothing to do if both are disabled.
    if battle_ui_font_path.is_none() && menu_tab_font_path.is_none() {
        return Ok(());
    }

    println!("\nPatching SYSTEM.SPR...");

    // Extract and decompress once.
    let spr_entry = ctx
        .iso
        .find_file(ctx.disc.disc(), SPR_NAME)?
        .context(format!("{} not found on disc", SPR_NAME))?;

    let original_compressed = ctx
        .iso
        .extract_file(ctx.disc.disc(), &spr_entry)?;
    let original_header = compression::parse_header(&original_compressed)?;

    println!(
        "  Original: {} bytes compressed, {} bytes decompressed",
        original_compressed.len(),
        original_header.decompressed_size
    );

    let mut spr_data = compression::decompress(&original_compressed)?;
    println!("  Decompressed: {} bytes", spr_data.len());

    // Battle UI tiles (攻防命動運全 → 공격/방어/명중/동작/행운/전체).
    if let Some(bf) = battle_ui_font_path {
        let ttf_data = std::fs::read(bf)
            .context(format!("Failed to read battle UI font: {}", bf.display()))?;
        let font = korean::load_font(&ttf_data).map_err(|e| anyhow::anyhow!(e))?;
        println!("  Battle UI font: {} ({}px)", bf.display(), battle_ui_font_size);

        let count = battle_ui::patch_battle_tiles(&mut spr_data, &font, battle_ui_font_size);
        println!("  Patched {} battle UI tiles ({} bytes each)", count, battle_ui::TILE_BYTES);
    }

    // Menu tab sprites (アイテム…にげる → 아이템…도망).
    if let Some(mf) = menu_tab_font_path {
        let ttf_data = std::fs::read(mf)
            .context(format!("Failed to read menu tab font: {}", mf.display()))?;
        let font = korean::load_font(&ttf_data).map_err(|e| anyhow::anyhow!(e))?;
        println!("  Menu tab font: {} ({}px)", mf.display(), menu_tab_font_size);

        let count = battle_menu::patch_menu_tabs(&mut spr_data, &font, menu_tab_font_size);
        println!("  Patched {} menu tab pairs (selected + unselected)", count);
    }

    // Compress once and write once.
    let compressed = compression::compress(&spr_data, &original_header.subtype);
    println!("  Compressed: {} -> {} bytes", spr_data.len(), compressed.len());

    let verify = compression::decompress(&compressed)?;
    if verify == spr_data {
        println!("  CNX round-trip: PASS");
    } else {
        println!(
            "  CNX round-trip: FAIL ({} bytes vs {} bytes, {} differ)",
            verify.len(),
            spr_data.len(),
            verify.iter().zip(&spr_data).filter(|(a, b)| a != b).count()
        );
    }

    let relocated = write_compressed_file(
        ctx,
        SPR_NAME,
        spr_entry.lba,
        original_compressed.len(),
        &compressed,
        SPR_NAME,
    )?;
    let original_sectors = (original_compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let new_sectors = (compressed.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    if relocated {
        println!(
            "  Relocated {} (grew: {} -> {} sectors)",
            SPR_NAME, original_sectors, new_sectors
        );
    } else if compressed.len() <= original_compressed.len() {
        println!(
            "  Wrote {} in place at LBA {} (padded {} → {})",
            SPR_NAME, spr_entry.lba, compressed.len(), original_compressed.len()
        );
    } else {
        println!(
            "  Wrote {} in place at LBA {} (ISO size updated)",
            SPR_NAME, spr_entry.lba
        );
    }

    Ok(())
}
