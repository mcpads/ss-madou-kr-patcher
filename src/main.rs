use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "ss_madou")]
#[command(about = "Sega Saturn Madou Monogatari ROM hacking tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display BIN/CUE disc and header information.
    Info {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
    },
    /// List ISO 9660 root directory files.
    Files {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
    },
    /// Extract a file from the disc image via ISO 9660.
    Extract {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// File to extract (ISO 9660 filename).
        #[arg(short, long, default_value = "0")]
        file: String,
        /// Output path.
        #[arg(short, long, default_value = "out/1st_read.bin")]
        output: PathBuf,
    },
    /// Decompress a single CNX-compressed file from the disc.
    Decompress {
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// ISO 9660 filename to decompress.
        #[arg(short, long)]
        file: String,
        /// Output path for decompressed data.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Decompress all CNX-compressed files from the disc.
    DecompressAll {
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// Output directory.
        #[arg(short, long, default_value = "out/dec")]
        output: PathBuf,
    },
    /// Dump font tiles from a decompressed file as a PNG grid image.
    FontDump {
        /// Input file (decompressed font data).
        #[arg(short, long)]
        input: PathBuf,
        /// Output PNG file path.
        #[arg(short, long, default_value = "out/font_grid.png")]
        output: PathBuf,
        /// Tile width in pixels.
        #[arg(long, default_value = "8")]
        tile_width: usize,
        /// Tile height in pixels.
        #[arg(long, default_value = "8")]
        tile_height: usize,
        /// Bits per pixel (1, 4, or 8).
        #[arg(long, default_value = "4")]
        bpp: usize,
        /// Number of columns in the grid.
        #[arg(long, default_value = "32")]
        cols: usize,
        /// Scale factor.
        #[arg(long, default_value = "3")]
        scale: usize,
        /// Skip N tiles from the start.
        #[arg(long, default_value = "0")]
        skip: usize,
        /// Maximum number of tiles to render (0 = all).
        #[arg(long, default_value = "0")]
        count: usize,
        /// Combine every 4 tiles into 16x16 (2x2 arrangement).
        #[arg(long, default_value = "false")]
        combine_2x2: bool,
        /// Combine every 2 tiles into 16x8 (1x2 horizontal pair, for wide chars).
        #[arg(long, default_value = "false")]
        combine_1x2: bool,
        /// Combine every 2 tiles into 8x16 (2x1 vertical pair, top-bottom).
        #[arg(long, default_value = "false")]
        combine_2x1: bool,
        /// Combine tiles in WxH grid (e.g. "4x1", "3x3"). Overrides other combine flags.
        #[arg(long)]
        combine: Option<String>,
        /// Split output into batches of N glyphs each (0 = single file).
        #[arg(short = 'b', long, default_value = "0")]
        batch_size: usize,
        /// Show tile index number above each cell.
        #[arg(long, default_value = "false")]
        indexed: bool,
        /// Override the starting label number for indexed mode.
        #[arg(long)]
        label_start: Option<usize>,
    },
    /// Extract text and control codes from a decompressed SEQ file to JSON.
    DumpScript {
        /// Input SEQ file (decompressed).
        #[arg(short, long)]
        input: Option<PathBuf>,
        /// Process all SEQ files in input directory.
        #[arg(long, default_value = "false")]
        all: bool,
        /// Input directory for --all mode.
        #[arg(long, default_value = "out/dec")]
        input_dir: PathBuf,
        /// Output JSON file (single file mode).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output directory for --all mode.
        #[arg(long, default_value = "out/scripts")]
        output_dir: PathBuf,
        /// Path to glyph_mapping.csv for tile-index decoding.
        #[arg(long, default_value = "assets/glyph_mapping.csv")]
        glyph_map: PathBuf,
        /// Max entries per JSON file (0 = no split).
        #[arg(long, default_value = "0")]
        max_entries: usize,
    },
    /// Build a fully translated Korean ROM from translation JSONs.
    BuildRom {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// Path to the Korean TTF font.
        #[arg(short, long, default_value = "assets/fonts/Galmuri11.ttf")]
        font: PathBuf,
        /// Output patched BIN file name.
        #[arg(short, long, default_value = "out/Madou_Monogatari_KO.bin")]
        output: PathBuf,
        /// Output directory (overrides directory portion of --output).
        #[arg(short = 'O', long)]
        output_dir: Option<PathBuf>,
        /// Directory containing translation JSON files.
        #[arg(short, long, default_value = "assets/translations/scripts/needs_review")]
        translations_dir: PathBuf,
        /// Font rendering size in pixels.
        #[arg(long, default_value = "12.0")]
        font_size: f32,
        /// Only patch SEQ files matching these patterns (case-insensitive substring, repeatable).
        #[arg(long)]
        only_seq: Vec<String>,
        /// Skip SEQ files matching these patterns (case-insensitive substring, repeatable).
        #[arg(long)]
        except_seq: Vec<String>,
        /// Skip all SEQ patches (font-only build).
        #[arg(long)]
        skip_seq: bool,
        /// Dump patched SEQ files to output directory.
        #[arg(long)]
        dump_seq: bool,
        /// Dump COMMON.SEQ pointer fix details.
        #[arg(long)]
        dump_ptrs: bool,
        /// Skip COMMON.SEQ pointer fixing.
        #[arg(long)]
        skip_common_ptrs: bool,
        /// Skip script pointer fixing (text patches only, no pointer adjustment).
        #[arg(long)]
        skip_script_ptrs: bool,
        /// Path to TTF font for prologue sprite (OP_SP02.SPR). Use --no-prologue to skip.
        #[arg(long, default_value = "assets/fonts/MaplestoryBold.ttf")]
        prologue_font: PathBuf,
        /// Skip prologue sprite patching.
        #[arg(long)]
        no_prologue: bool,
        /// Font size for prologue sprite rendering.
        #[arg(long, default_value = "14.0")]
        prologue_font_size: f32,
        /// Path to TTF font for battle UI sprite (SYSTEM.SPR). Use --no-battle-ui to skip.
        #[arg(long, default_value = "assets/fonts/dalmoori.ttf")]
        battle_ui_font: PathBuf,
        /// Skip battle UI sprite patching.
        #[arg(long)]
        no_battle_ui: bool,
        /// Font size for battle UI sprite rendering.
        #[arg(long, default_value = "8.0")]
        battle_ui_font_size: f32,
    },
    /// Recompress SEQ file(s) without text changes (CNX compressor isolation test).
    TestRecompress {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// SEQ filename to recompress (e.g. MP0101.SEQ). Omit for --all.
        #[arg(short, long)]
        seq: Option<String>,
        /// Recompress ALL CNX files on the disc.
        #[arg(long)]
        all: bool,
        /// Output patched BIN file.
        #[arg(short, long, default_value = "out/test_recompress/Madou_Monogatari_KO.bin")]
        output: PathBuf,
    },
    /// Compare two ROM images (sector-level diff with file mapping).
    RomDiff {
        /// First ROM (reference).
        #[arg(short = 'a', long)]
        rom_a: PathBuf,
        /// Second ROM (modified).
        #[arg(short = 'b', long)]
        rom_b: PathBuf,
        /// Compare a specific file in detail (e.g. MP0101.SEQ).
        #[arg(short, long)]
        file: Option<String>,
    },
    /// Decode tile codes ↔ Korean/Japanese text (debug garbled text).
    DecodeText {
        /// Korean (or Japanese) text to decode.
        #[arg(short, long)]
        query: String,
        /// Translation JSON directory.
        #[arg(short, long, default_value = "assets/translations/scripts/needs_review")]
        translations_dir: PathBuf,
    },
    /// Check glyph slot allocation (dry-run, no ROM needed).
    CheckGlyphs {
        /// Translation JSON directory.
        #[arg(short, long, default_value = "assets/translations/scripts")]
        translations_dir: PathBuf,
        /// Show unassigned char usage examples.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Check translation texts for overflow (line length / line count).
    CheckOverflow {
        /// Translation JSON directory.
        #[arg(short, long, default_value = "assets/translations/scripts")]
        translations_dir: PathBuf,
        /// Show statistics (distribution histogram, summary).
        #[arg(short, long)]
        verbose: bool,
    },
    /// Run SH-2 disassembly on the 1st Read binary.
    Disasm {
        /// Path to the BIN file.
        #[arg(short, long, default_value = "roms/Madou_Monogatari_JAP.bin")]
        rom: PathBuf,
        /// Disassembly mode.
        #[arg(short, long, default_value_t = commands::disasm::DisasmMode::Recursive, value_enum)]
        mode: commands::disasm::DisasmMode,
        /// Override start address (hex, e.g. 0x06004000).
        #[arg(long)]
        start: Option<String>,
        /// Override end address for linear mode (hex).
        #[arg(long)]
        end: Option<String>,
        /// Query argument: hex value for find-value/xrefs/func, text for find-string.
        #[arg(short, long)]
        query: Option<String>,
        /// Output file path.
        #[arg(short, long, default_value = "out/disasm.txt")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Info { rom } => commands::disc::cmd_info(&rom),
        Commands::Files { rom } => commands::disc::cmd_files(&rom),
        Commands::Extract { rom, file, output } => commands::disc::cmd_extract(&rom, &file, &output),
        Commands::Decompress { rom, file, output } => commands::disc::cmd_decompress(&rom, &file, &output),
        Commands::DecompressAll { rom, output } => commands::disc::cmd_decompress_all(&rom, &output),
        Commands::FontDump {
            input,
            output,
            tile_width,
            tile_height,
            bpp,
            cols,
            scale,
            skip,
            count,
            combine_2x2,
            combine_1x2,
            combine_2x1,
            combine,
            batch_size,
            indexed,
            label_start,
        } => commands::font::cmd_font_dump(&input, &output, tile_width, tile_height, bpp, cols, scale, skip, count, combine_2x2, combine_1x2, combine_2x1, combine.as_deref(), batch_size, indexed, label_start),
        Commands::DumpScript {
            input,
            all,
            input_dir,
            output,
            output_dir,
            glyph_map,
            max_entries,
        } => commands::text::cmd_dump_script(input.as_deref(), all, &input_dir, output.as_deref(), &output_dir, &glyph_map, max_entries),
        Commands::BuildRom {
            rom,
            font,
            output,
            output_dir,
            translations_dir,
            font_size,
            only_seq,
            except_seq,
            skip_seq,
            dump_seq,
            dump_ptrs,
            skip_common_ptrs,
            skip_script_ptrs,
            prologue_font,
            no_prologue,
            prologue_font_size,
            battle_ui_font,
            no_battle_ui,
            battle_ui_font_size,
        } => {
            let final_output = match output_dir {
                Some(dir) => dir.join(output.file_name().unwrap_or(std::ffi::OsStr::new("Madou_Monogatari_KO.bin"))),
                None => output,
            };
            let pf = if no_prologue { None } else { Some(prologue_font.as_path()) };
            let bf = if no_battle_ui { None } else { Some(battle_ui_font.as_path()) };
            commands::build::cmd_build_rom(
                &rom, &font, &final_output, &translations_dir, font_size,
                &only_seq, &except_seq, skip_seq,
                dump_seq, dump_ptrs, skip_common_ptrs, skip_script_ptrs,
                pf, prologue_font_size,
                bf, battle_ui_font_size,
            )
        }
        Commands::TestRecompress {
            rom,
            seq,
            all,
            output,
        } => {
            if all {
                commands::build::cmd_test_recompress_all(&rom, &output)
            } else {
                let seq = seq.as_deref().unwrap_or("PT0402.SEQ");
                commands::build::cmd_test_recompress(&rom, seq, &output)
            }
        }
        Commands::RomDiff {
            rom_a,
            rom_b,
            file,
        } => commands::diff::cmd_rom_diff(&rom_a, &rom_b, file.as_deref()),
        Commands::DecodeText {
            query,
            translations_dir,
        } => commands::decode::cmd_decode_text(&query, &translations_dir),
        Commands::CheckGlyphs {
            translations_dir,
            verbose,
        } => commands::build::cmd_check_glyphs(&translations_dir, verbose),
        Commands::CheckOverflow {
            translations_dir,
            verbose,
        } => commands::text::cmd_check_overflow(&translations_dir, verbose),
        Commands::Disasm {
            rom,
            mode,
            start,
            end,
            query,
            output,
        } => commands::disasm::cmd_disasm(&rom, mode, start.as_deref(), end.as_deref(), query.as_deref(), &output),
    }
}
