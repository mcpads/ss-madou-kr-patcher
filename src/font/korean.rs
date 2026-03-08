//! Korean glyph generation using fontdue.
//!
//! Renders Korean characters from a TTF font onto a 16×16 canvas,
//! then encodes them as Saturn-format 4bpp 8×8 tiles in a 2×2 arrangement
//! (TL, TR, BL, BR — matching FONT.CEL layout).

use anyhow::ensure;
use fontdue::{Font, FontSettings};

/// Canvas size for a single 16×16 glyph (4 tiles of 8×8).
const CANVAS_SIZE: usize = 16;
/// Bytes per 8×8 4bpp tile.
const TILE_BYTES: usize = 32;
/// Tile offset where 16×16 JP glyphs start in FONT.CEL.
pub const GLYPH_TILE_START: usize = 438;
/// Number of 8×8 tiles per 16×16 glyph.
pub const TILES_PER_GLYPH: usize = 4;

/// Load a TTF/OTF font for glyph rendering.
pub fn load_font(ttf_data: &[u8]) -> Result<Font, String> {
    Font::from_bytes(ttf_data, FontSettings {
        scale: CANVAS_SIZE as f32,
        ..FontSettings::default()
    })
    .map_err(|e| format!("Failed to load font: {}", e))
}

/// Render a single character onto a 16×16 canvas.
///
/// Returns 256 coverage bytes (0–255), row-major order.
pub fn render_glyph(font: &Font, ch: char, font_size: f32) -> [u8; 256] {
    let (metrics, raster) = font.rasterize(ch, font_size);

    let mut canvas = [0u8; CANVAS_SIZE * CANVAS_SIZE];

    if metrics.width == 0 || metrics.height == 0 {
        return canvas;
    }

    // Baseline-relative vertical positioning (same approach as SNES project).
    let (y_offset, x_offset) = if let Some(lm) = font.horizontal_line_metrics(font_size) {
        let ascent = lm.ascent as i32;
        let baseline = (CANVAS_SIZE as i32 + ascent) / 2;
        let y = (baseline - metrics.ymin - metrics.height as i32).max(0) as usize;
        let x = ((CANVAS_SIZE as i32 - metrics.width as i32) / 2).max(0) as usize;
        (y, x)
    } else {
        // Fallback: simple centering.
        (
            CANVAS_SIZE.saturating_sub(metrics.height) / 2,
            CANVAS_SIZE.saturating_sub(metrics.width) / 2,
        )
    };

    for row in 0..metrics.height {
        for col in 0..metrics.width {
            let cy = y_offset + row;
            let cx = x_offset + col;
            if cy < CANVAS_SIZE && cx < CANVAS_SIZE {
                canvas[cy * CANVAS_SIZE + cx] = raster[row * metrics.width + col];
            }
        }
    }

    canvas
}

/// Rendering mode for Korean glyph generation.
///
/// Original FONT.CEL uses palette indices 0-6:
/// - 0 = transparent (background)
/// - 1 = darkest visible (outline/shadow)
/// - 6 = brightest (white fill)
#[derive(Clone, Copy, Debug)]
pub enum RenderMode {
    /// Grayscale anti-aliased (coverage / 17 → indices 0–15). Legacy mode.
    Grayscale,
    /// Outline mode matching original FONT.CEL palette.
    /// Binary threshold → 1px dilation → fill=6, outline=1, AA=2–5.
    Outline,
    /// Binary fill only (threshold → index 1, no outline).
    BinaryFill,
}

/// Convert a 16×16 coverage bitmap into 4 Saturn 8×8 4bpp tiles (128 bytes).
///
/// Tile order: TL, TR, BL, BR (matching FONT.CEL 2×2 arrangement).
/// Each 8×8 tile = 32 bytes, 2 pixels packed per byte (high nibble first).
pub fn coverage_to_4bpp_tiles(coverage: &[u8; 256]) -> [u8; 128] {
    coverage_to_4bpp_tiles_with_mode(coverage, RenderMode::Grayscale)
}

/// Convert coverage bitmap to 4bpp tiles using the specified rendering mode.
pub fn coverage_to_4bpp_tiles_with_mode(coverage: &[u8; 256], mode: RenderMode) -> [u8; 128] {
    let palette = match mode {
        RenderMode::Grayscale => coverage_to_palette_grayscale(coverage),
        RenderMode::Outline => coverage_to_palette_outline(coverage),
        RenderMode::BinaryFill => coverage_to_palette_binary(coverage),
    };
    palette_to_tiles(&palette)
}

/// Legacy grayscale: coverage / 17 → indices 0–15.
fn coverage_to_palette_grayscale(coverage: &[u8; 256]) -> [u8; 256] {
    let mut pal = [0u8; 256];
    for (i, &c) in coverage.iter().enumerate() {
        pal[i] = c / 17;
    }
    pal
}

/// Binary fill: threshold → index 6 (brightest), else 0.
fn coverage_to_palette_binary(coverage: &[u8; 256]) -> [u8; 256] {
    let mut pal = [0u8; 256];
    for (i, &c) in coverage.iter().enumerate() {
        pal[i] = if c >= 96 { 6 } else { 0 };
    }
    pal
}

/// Outline mode matching original FONT.CEL palette (indices 0-6).
///
/// Game palette confirmed by emulator test:
/// - Index 0 = transparent
/// - Index 1 = darkest (outline/shadow)
/// - Index 6 = brightest (white fill)
///
/// 1. Threshold coverage to get binary glyph interior.
/// 2. Dilate by 1px in all 8 directions for outline region.
/// 3. Map: interior → 6 (bright fill), outline → 1 (dark outline),
///    anti-aliased edges → 2-5.
fn coverage_to_palette_outline(coverage: &[u8; 256]) -> [u8; 256] {
    // Step 1: threshold to binary interior.
    let mut interior = [false; 256];
    for (i, &c) in coverage.iter().enumerate() {
        interior[i] = c >= 96;
    }

    // Step 2: dilate interior by 1px to create outline region.
    let mut dilated = [false; 256];
    for y in 0..CANVAS_SIZE {
        for x in 0..CANVAS_SIZE {
            if interior[y * CANVAS_SIZE + x] {
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let ny = y as i32 + dy;
                        let nx = x as i32 + dx;
                        if (0..CANVAS_SIZE as i32).contains(&ny)
                            && (0..CANVAS_SIZE as i32).contains(&nx)
                        {
                            dilated[ny as usize * CANVAS_SIZE + nx as usize] = true;
                        }
                    }
                }
            }
        }
    }

    // Step 3: assign palette indices.
    // interior → 6 (brightest/white fill), outline-only → 1 (darkest/black),
    // AA edges → 2-5 (dark→bright gradient).
    let mut pal = [0u8; 256];
    for i in 0..256 {
        if interior[i] {
            pal[i] = 6; // fill (brightest/white)
        } else if dilated[i] {
            // Outline pixel. Use coverage to blend if available.
            let c = coverage[i];
            if c > 64 {
                pal[i] = 4; // semi-bright (closer to fill)
            } else if c > 0 {
                pal[i] = 2; // semi-dark (closer to outline)
            } else {
                pal[i] = 1; // darkest outline (black)
            }
        }
    }
    pal
}

/// Pack a 16×16 palette-index array into 4 Saturn 8×8 4bpp tiles.
fn palette_to_tiles(palette: &[u8; 256]) -> [u8; 128] {
    let mut tiles = [0u8; TILES_PER_GLYPH * TILE_BYTES];
    let quadrants: [(usize, usize); 4] = [(0, 0), (0, 8), (8, 0), (8, 8)];

    for (qi, &(row_off, col_off)) in quadrants.iter().enumerate() {
        let tile_base = qi * TILE_BYTES;
        for row in 0..8 {
            for col_pair in 0..4 {
                let col = col_pair * 2;
                let hi_nib = palette[(row_off + row) * CANVAS_SIZE + col_off + col];
                let lo_nib = palette[(row_off + row) * CANVAS_SIZE + col_off + col + 1];
                tiles[tile_base + row * 4 + col_pair] = (hi_nib << 4) | lo_nib;
            }
        }
    }
    tiles
}

/// Render coverage as a display-ready grayscale bitmap (for preview images).
///
/// Maps palette indices to display brightness using the game's inverted palette:
/// index 0 → black (transparent), index 1 → white, index 6 → dark gray.
pub fn palette_to_display(palette: &[u8; 256]) -> [u8; 256] {
    let mut display = [0u8; 256];
    // Game palette: 0=transparent(black), 1=darkest, 6=brightest(white)
    let lut: [u8; 16] = [
        0, 42, 85, 128, 170, 212, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    for (i, &idx) in palette.iter().enumerate() {
        display[i] = lut[idx as usize & 0xF];
    }
    display
}

/// Generate 4bpp tile data for a list of characters using [`RenderMode::Outline`].
///
/// Returns a Vec of `(char, [u8; 128])` pairs.
pub fn generate_tiles(
    font: &Font,
    chars: &[char],
    font_size: f32,
) -> Vec<(char, [u8; 128])> {
    generate_tiles_with_mode(font, chars, font_size, RenderMode::Outline)
}

/// Generate 4bpp tile data with a specific rendering mode.
pub fn generate_tiles_with_mode(
    font: &Font,
    chars: &[char],
    font_size: f32,
    mode: RenderMode,
) -> Vec<(char, [u8; 128])> {
    chars
        .iter()
        .map(|&ch| {
            let coverage = render_glyph(font, ch, font_size);
            let tiles = coverage_to_4bpp_tiles_with_mode(&coverage, mode);
            (ch, tiles)
        })
        .collect()
}

/// Draw ASCII text onto an RGB buffer using fontdue for rasterization.
fn draw_ascii_label(
    buf: &mut [u8],
    buf_w: usize,
    x: usize,
    y: usize,
    text: &str,
    font: &Font,
    label_size: f32,
    pixel_scale: usize,
    color: [u8; 3],
) {
    let mut cursor_x = x as f32;
    let line_metrics = font.horizontal_line_metrics(label_size);

    for ch in text.chars() {
        let (metrics, raster) = font.rasterize(ch, label_size);

        let y_off = line_metrics.map_or(0usize, |lm| {
            (lm.ascent as i32 - metrics.ymin - metrics.height as i32).max(0) as usize
        });

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let cov = raster[row * metrics.width + col];
                if cov > 80 {
                    let px = cursor_x as usize + col * pixel_scale;
                    let py = y + (y_off + row) * pixel_scale;
                    for sy in 0..pixel_scale {
                        for sx in 0..pixel_scale {
                            let fx = px + sx;
                            let fy = py + sy;
                            if fx < buf_w {
                                let idx = (fy * buf_w + fx) * 3;
                                if idx + 2 < buf.len() {
                                    buf[idx] = color[0];
                                    buf[idx + 1] = color[1];
                                    buf[idx + 2] = color[2];
                                }
                            }
                        }
                    }
                }
            }
        }

        cursor_x += metrics.advance_width * pixel_scale as f32;
    }
}

/// Measure the pixel width of ASCII text rendered with fontdue.
fn measure_text_width(font: &Font, text: &str, label_size: f32, pixel_scale: usize) -> usize {
    let total: f32 = text
        .chars()
        .map(|ch| font.metrics(ch, label_size).advance_width)
        .sum();
    (total * pixel_scale as f32).ceil() as usize
}

/// Encode an RGB buffer as PNG.
fn encode_rgb_png(buf: &[u8], width: usize, height: usize) -> Vec<u8> {
    use std::io::BufWriter;
    let mut png_data = Vec::new();
    {
        let w = BufWriter::new(&mut png_data);
        let mut encoder = png::Encoder::new(w, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer.write_image_data(buf).expect("PNG data write failed");
    }
    png_data
}

/// Blit a 16×16 glyph (display brightness) onto an RGB buffer at the given position.
fn blit_glyph(
    buf: &mut [u8],
    buf_w: usize,
    base_x: usize,
    base_y: usize,
    display: &[u8; 256],
    scale: usize,
) {
    for py in 0..CANVAS_SIZE {
        for px in 0..CANVAS_SIZE {
            let brightness = display[py * CANVAS_SIZE + px];
            if brightness == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    let ox = base_x + px * scale + sx;
                    let oy = base_y + py * scale + sy;
                    if ox < buf_w {
                        let idx = (oy * buf_w + ox) * 3;
                        if idx + 2 < buf.len() {
                            buf[idx] = brightness;
                            buf[idx + 1] = brightness;
                            buf[idx + 2] = brightness;
                        }
                    }
                }
            }
        }
    }
}

/// Generate a comparison PNG image showing multiple fonts × rendering modes.
///
/// Each row is a font, each column group shows the sample characters in one mode.
/// Returns the PNG data.
pub fn generate_comparison_png(
    font_entries: &[(&str, &Font)],
    chars: &[char],
    font_size: f32,
    scale: usize,
) -> Vec<u8> {
    let modes: &[(&str, RenderMode)] = &[
        ("Outline", RenderMode::Outline),
        ("BinaryFill", RenderMode::BinaryFill),
        ("Grayscale", RenderMode::Grayscale),
    ];

    // Use first font for ASCII labels.
    let label_font = font_entries[0].1;
    let label_size: f32 = 10.0;
    let label_scale = scale.max(1);

    // Calculate label column width (font names).
    let max_name_w = font_entries
        .iter()
        .map(|(name, _)| measure_text_width(label_font, name, label_size, label_scale))
        .max()
        .unwrap_or(0);
    let label_col_w = max_name_w + 4 * scale;

    // Calculate header row height (mode names).
    let header_h = (label_size as usize) * label_scale + 4 * scale;

    let glyphs_per_mode = chars.len();
    let cell = CANVAS_SIZE * scale;
    let gap = 4 * scale;
    let row_gap = 2 * scale;
    let margin = 2 * scale;

    let content_w = modes.len() * glyphs_per_mode * cell + (modes.len() - 1) * gap;
    let img_w = margin + label_col_w + content_w + margin;
    let img_h = margin + header_h + font_entries.len() * (cell + row_gap) - row_gap + margin;

    let mut img = vec![40u8; img_w * img_h * 3];

    // Draw mode header labels.
    let header_color = [120u8, 180, 220]; // light blue
    for (mi, (mode_name, _)) in modes.iter().enumerate() {
        let hx = margin + label_col_w + mi * (glyphs_per_mode * cell + gap);
        draw_ascii_label(
            &mut img, img_w, hx, margin, mode_name, label_font, label_size, label_scale, header_color,
        );
    }

    // Draw font rows.
    let label_color = [200u8, 200, 200]; // light gray
    for (fi, (font_name, font)) in font_entries.iter().enumerate() {
        let row_y = margin + header_h + fi * (cell + row_gap);

        // Draw font name label (vertically centered in row).
        let label_y = row_y + (cell.saturating_sub((label_size as usize) * label_scale)) / 2;
        draw_ascii_label(
            &mut img, img_w, margin, label_y, font_name, label_font, label_size, label_scale, label_color,
        );

        // Draw glyphs for each mode.
        for (mi, (_, mode)) in modes.iter().enumerate() {
            for (ci, &ch) in chars.iter().enumerate() {
                let coverage = render_glyph(font, ch, font_size);
                let palette = match mode {
                    RenderMode::Grayscale => coverage_to_palette_grayscale(&coverage),
                    RenderMode::Outline => coverage_to_palette_outline(&coverage),
                    RenderMode::BinaryFill => coverage_to_palette_binary(&coverage),
                };
                let display = palette_to_display(&palette);

                let base_x = margin + label_col_w + mi * (glyphs_per_mode * cell + gap) + ci * cell;
                blit_glyph(&mut img, img_w, base_x, row_y, &display, scale);
            }
        }
    }

    encode_rgb_png(&img, img_w, img_h)
}

/// Font entry with native size information for multi-size comparison.
pub struct FontCompareEntry<'a> {
    pub name: &'a str,
    pub font: &'a Font,
    pub native_px: u32,
}

/// Generate a multi-size comparison PNG (Outline mode only).
///
/// Layout: rows = fonts, column groups = sizes.
/// Each font name is labeled with its native size. Native-size cells get a marker.
pub fn generate_multi_size_comparison_png(
    entries: &[FontCompareEntry<'_>],
    chars: &[char],
    sizes: &[f32],
    scale: usize,
) -> Vec<u8> {
    let label_font = entries[0].font;
    let label_size: f32 = 10.0;
    let label_scale = scale.max(1);

    // Calculate label column width.
    let max_name_w = entries
        .iter()
        .map(|e| {
            let label = format!("{} [{}px]", e.name, e.native_px);
            measure_text_width(label_font, &label, label_size, label_scale)
        })
        .max()
        .unwrap_or(0);
    let label_col_w = max_name_w + 4 * scale;

    let header_h = (label_size as usize) * label_scale + 4 * scale;

    let cell = CANVAS_SIZE * scale;
    let gap = 6 * scale;
    let row_gap = 2 * scale;
    let margin = 4 * scale;

    let chars_per_group = chars.len();
    let group_w = chars_per_group * cell;
    let content_w = sizes.len() * group_w + (sizes.len() - 1) * gap;

    let img_w = margin + label_col_w + content_w + margin;
    let img_h = margin + header_h + entries.len() * (cell + row_gap) - row_gap + margin;

    let mut img = vec![40u8; img_w * img_h * 3];

    // Draw size headers.
    let header_color = [100u8, 200, 100]; // green
    for (si, &sz) in sizes.iter().enumerate() {
        let label = format!("{}px", sz as u32);
        let hx = margin + label_col_w + si * (group_w + gap);
        draw_ascii_label(
            &mut img, img_w, hx, margin, &label, label_font, label_size, label_scale, header_color,
        );
    }

    // Draw font rows.
    let label_color = [200u8, 200, 200];
    let native_color = [255u8, 220, 80]; // gold for native size label
    for (fi, entry) in entries.iter().enumerate() {
        let row_y = margin + header_h + fi * (cell + row_gap);

        // Font name label with native size.
        let label = format!("{} [{}px]", entry.name, entry.native_px);
        let label_y = row_y + (cell.saturating_sub((label_size as usize) * label_scale)) / 2;
        draw_ascii_label(
            &mut img, img_w, margin, label_y, &label, label_font, label_size, label_scale, label_color,
        );

        // Render glyphs at each size.
        for (si, &sz) in sizes.iter().enumerate() {
            let is_native = sz as u32 == entry.native_px;

            // Draw native indicator (gold dot) above native-size group.
            if is_native {
                let header_label = format!("{}px *", sz as u32);
                let hx = margin + label_col_w + si * (group_w + gap);
                // Overdraw header with gold star.
                draw_ascii_label(
                    &mut img, img_w, hx, margin, &header_label, label_font, label_size, label_scale, native_color,
                );
            }

            for (ci, &ch) in chars.iter().enumerate() {
                let coverage = render_glyph(entry.font, ch, sz);
                let palette = coverage_to_palette_outline(&coverage);
                let display = palette_to_display(&palette);

                let base_x = margin + label_col_w + si * (group_w + gap) + ci * cell;
                blit_glyph(&mut img, img_w, base_x, row_y, &display, scale);
            }

            // Draw a thin border around native-size cells.
            if is_native {
                let bx = margin + label_col_w + si * (group_w + gap);
                let by = row_y;
                let bw = group_w;
                let bh = cell;
                let border_color = [255u8, 220, 80];
                // Top and bottom lines.
                for x in bx..bx + bw {
                    if x < img_w {
                        for t in 0..scale.min(2) {
                            let top_idx = ((by + t) * img_w + x) * 3;
                            let bot_idx = ((by + bh - 1 - t) * img_w + x) * 3;
                            if top_idx + 2 < img.len() {
                                img[top_idx] = border_color[0];
                                img[top_idx + 1] = border_color[1];
                                img[top_idx + 2] = border_color[2];
                            }
                            if bot_idx + 2 < img.len() {
                                img[bot_idx] = border_color[0];
                                img[bot_idx + 1] = border_color[1];
                                img[bot_idx + 2] = border_color[2];
                            }
                        }
                    }
                }
                // Left and right lines.
                for y in by..by + bh {
                    for t in 0..scale.min(2) {
                        let left_idx = (y * img_w + bx + t) * 3;
                        let right_idx = (y * img_w + bx + bw - 1 - t) * 3;
                        if left_idx + 2 < img.len() {
                            img[left_idx] = border_color[0];
                            img[left_idx + 1] = border_color[1];
                            img[left_idx + 2] = border_color[2];
                        }
                        if right_idx + 2 < img.len() {
                            img[right_idx] = border_color[0];
                            img[right_idx + 1] = border_color[1];
                            img[right_idx + 2] = border_color[2];
                        }
                    }
                }
            }
        }
    }

    encode_rgb_png(&img, img_w, img_h)
}

/// Bytes per 8×8 4bpp tile (re-exported for buffer size calculations).
pub const TILE_BYTES_PUB: usize = TILE_BYTES;

/// Patch tile data directly at a specific tile position in FONT.CEL.
///
/// Used for sec6 glyphs (tiles 178–437) that are outside the glyph region.
/// `first_tile` is the starting 8×8 tile index (e.g. 182 for digit '0').
pub fn patch_font_cel_at_tile(
    font_cel: &mut [u8],
    first_tile: usize,
    tile_data: &[u8; 128],
) -> anyhow::Result<()> {
    let offset = first_tile * TILE_BYTES;
    let end = offset + 128;
    ensure!(
        end <= font_cel.len(),
        "patch_font_cel_at_tile: first_tile {} (offset 0x{:X}) exceeds buffer size 0x{:X}",
        first_tile,
        offset,
        font_cel.len()
    );
    font_cel[offset..end].copy_from_slice(tile_data);
    Ok(())
}

/// Patch glyph tile data into a decompressed FONT.CEL buffer.
///
/// `glyph_index` is the 0-based index into the 16×16 glyph region
/// (glyph 0 = tiles 438–441 in FONT.CEL).
pub fn patch_font_cel(
    font_cel: &mut [u8],
    glyph_index: usize,
    tile_data: &[u8; 128],
) -> anyhow::Result<()> {
    let tile_offset = (GLYPH_TILE_START + glyph_index * TILES_PER_GLYPH) * TILE_BYTES;
    let end = tile_offset + 128;
    ensure!(
        end <= font_cel.len(),
        "patch_font_cel: glyph_index {} (tile_offset 0x{:X}) exceeds buffer size 0x{:X}",
        glyph_index,
        tile_offset,
        font_cel.len()
    );
    font_cel[tile_offset..end].copy_from_slice(tile_data);
    Ok(())
}

#[cfg(test)]
#[path = "korean_tests.rs"]
mod tests;
