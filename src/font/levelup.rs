//! Level-up sprite renderer (SYSTEM.SPR partial patch).
//!
//! Replaces the Japanese "レベルアップ" burst+text sprite (56×32 4bpp) at
//! offset 0x64C0 with Korean "레벨업!" while preserving the star burst effect.
//!
//! Pipeline:
//!   1. Decode original 4bpp → indexed pixels
//!   2. Extract burst layer (morphological closing → flood fill → erosion)
//!   3. Fill original JP text positions (indices 1,2,3) with burst inner (9)
//!   4. Render Korean text per-character with letter spacing (fontdue)
//!   5. Binarize → dilate for outline (D) → gradient body (2 top / 3 bottom)
//!   6. Composite burst + text → encode 4bpp → write back

use std::collections::VecDeque;

use fontdue::Font;

/// Sprite location in decompressed SYSTEM.SPR.
const OFFSET: usize = 0x64C0;
const WIDTH: usize = 56;
const HEIGHT: usize = 32;
/// 4bpp = 2 pixels per byte → 56/2 * 32 = 896 bytes.
const NBYTES: usize = WIDTH * HEIGHT / 2;

/// Korean replacement text.
const TEXT: &str = "레벨업!";
/// Default font size (can be overridden via CLI --levelup-font-size).
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
/// Extra pixels between each character pair.
const LETTER_SPACING: usize = 3;
/// Vertical offset (pixels down from visual center) to match burst center.
const VERTICAL_OFFSET: i32 = 2;

/// Binarization threshold for text mask (higher = thinner strokes).
const BINARIZE_THRESHOLD: u8 = 128;

// Palette indices
const IDX_OUTLINE: u8 = 0xD; // text outline (= burst outer, blends naturally)
const IDX_GRADIENT_TOP: u8 = 2; // text top 60% (dark)
const IDX_GRADIENT_BOT: u8 = 3; // text bottom 40% (mid)
const IDX_BURST_INNER: u8 = 0x9;
const IDX_BURST_TIP: u8 = 0xA;

/// JP text indices that should be replaced with burst inner color.
const JP_TEXT_INDICES: [u8; 3] = [1, 2, 3];

/// Erosion radius for separating burst outer ring (D) from interior (9).
const EDGE_RADIUS: i32 = 3;

// ---------------------------------------------------------------------------
// 4bpp decode / encode
// ---------------------------------------------------------------------------

fn decode_4bpp(data: &[u8]) -> Vec<Vec<u8>> {
    let mut pixels = vec![vec![0u8; WIDTH]; HEIGHT];
    let mut idx = 0;
    for y in 0..HEIGHT {
        for x in (0..WIDTH).step_by(2) {
            if idx < data.len() {
                let byte = data[idx];
                pixels[y][x] = (byte >> 4) & 0xF;
                if x + 1 < WIDTH {
                    pixels[y][x + 1] = byte & 0xF;
                }
                idx += 1;
            }
        }
    }
    pixels
}

fn encode_4bpp(pixels: &[Vec<u8>]) -> Vec<u8> {
    let mut data = vec![0u8; NBYTES];
    for y in 0..HEIGHT {
        for xb in 0..(WIDTH / 2) {
            let x = xb * 2;
            data[y * (WIDTH / 2) + xb] = (pixels[y][x] << 4) | pixels[y][x + 1];
        }
    }
    data
}

// ---------------------------------------------------------------------------
// Burst layer extraction
// ---------------------------------------------------------------------------

fn extract_burst_layer(orig: &[Vec<u8>]) -> Vec<Vec<u8>> {
    // Step 1: full silhouette — all non-zero pixels
    let sil: Vec<Vec<bool>> = orig
        .iter()
        .map(|row| row.iter().map(|&p| p != 0).collect())
        .collect();

    // Step 2: morphological closing (dilate then erode, radius=1)
    let dilated = dilate_bool(&sil, 1);
    let closed = erode_bool(&dilated, 1);

    // Step 3: flood-fill from outside on padded grid to find exterior
    let pw = WIDTH + 2;
    let ph = HEIGHT + 2;
    let mut padded = vec![vec![false; pw]; ph];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            padded[y + 1][x + 1] = closed[y][x];
        }
    }

    let mut exterior = vec![vec![false; pw]; ph];
    let mut queue = VecDeque::new();
    queue.push_back((0usize, 0usize));
    exterior[0][0] = true;
    while let Some((cx, cy)) = queue.pop_front() {
        for (dx, dy) in &[(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx >= 0 && nx < pw as i32 && ny >= 0 && ny < ph as i32 {
                let (nx, ny) = (nx as usize, ny as usize);
                if !exterior[ny][nx] && !padded[ny][nx] {
                    exterior[ny][nx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    // star_body = closed shape + any interior holes
    let mut star_body = vec![vec![false; WIDTH]; HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            star_body[y][x] = closed[y][x] || !exterior[y + 1][x + 1];
        }
    }

    // Step 4: erode star_body to find interior
    let interior = erode_bool(&star_body, EDGE_RADIUS);

    // Build result: D outer ring, 9 interior
    let mut result = vec![vec![0u8; WIDTH]; HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if star_body[y][x] {
                result[y][x] = if interior[y][x] { IDX_BURST_INNER } else { IDX_OUTLINE };
            }
        }
    }

    // Overlay original 9 pixels (inner ring detail)
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if orig[y][x] == IDX_BURST_INNER {
                result[y][x] = IDX_BURST_INNER;
            }
        }
    }

    // Restore A at original positions (burst tips)
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if orig[y][x] == IDX_BURST_TIP {
                result[y][x] = IDX_BURST_TIP;
            }
        }
    }

    result
}

fn dilate_bool(grid: &[Vec<bool>], radius: i32) -> Vec<Vec<bool>> {
    let h = grid.len();
    let w = grid[0].len();
    let mut out = vec![vec![false; w]; h];
    for y in 0..h {
        for x in 0..w {
            'search: for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let ny = y as i32 + dy;
                    let nx = x as i32 + dx;
                    if ny >= 0 && ny < h as i32 && nx >= 0 && nx < w as i32 {
                        if grid[ny as usize][nx as usize] {
                            out[y][x] = true;
                            break 'search;
                        }
                    }
                }
            }
        }
    }
    out
}

fn erode_bool(grid: &[Vec<bool>], radius: i32) -> Vec<Vec<bool>> {
    let h = grid.len();
    let w = grid[0].len();
    let mut out = vec![vec![false; w]; h];
    for y in 0..h {
        for x in 0..w {
            if !grid[y][x] {
                continue;
            }
            let mut all = true;
            'check: for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let ny = y as i32 + dy;
                    let nx = x as i32 + dx;
                    if !(ny >= 0 && ny < h as i32 && nx >= 0 && nx < w as i32
                        && grid[ny as usize][nx as usize])
                    {
                        all = false;
                        break 'check;
                    }
                }
            }
            out[y][x] = all;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Text rendering with letter spacing
// ---------------------------------------------------------------------------

/// Render each character individually and place with spacing, centered on canvas.
/// Returns (body mask as bool grid, text_top, text_bot) or None.
fn render_text_spaced(
    font: &Font,
    font_size: f32,
) -> Option<(Vec<Vec<bool>>, usize, usize)> {
    let chars: Vec<char> = TEXT.chars().collect();

    // Rasterize each character and find its visible bounds
    struct CharInfo {
        bitmap: Vec<Vec<u8>>,
        vis_w: usize,
        vis_h: usize,
        /// Y offset relative to a common reference (metrics.ymin)
        y_offset: i32,
    }

    let mut char_infos = Vec::new();
    let mut min_ymin = i32::MAX;
    let mut max_y_bottom = i32::MIN;

    for &ch in &chars {
        let (metrics, raster) = font.rasterize(ch, font_size);
        if metrics.width == 0 || metrics.height == 0 {
            return None;
        }

        // Find visible pixels in raster
        let mut vis_top: Option<usize> = None;
        let mut vis_bot: usize = 0;
        let mut vis_left: usize = metrics.width;
        let mut vis_right: usize = 0;

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                if raster[row * metrics.width + col] > 128 {
                    if vis_top.is_none() {
                        vis_top = Some(row);
                    }
                    vis_bot = row;
                    vis_left = vis_left.min(col);
                    vis_right = vis_right.max(col);
                }
            }
        }

        let vis_top = vis_top?;
        let vis_w = vis_right - vis_left + 1;
        let vis_h = vis_bot - vis_top + 1;

        // Crop visible region
        let mut bitmap = vec![vec![0u8; vis_w]; vis_h];
        for row in 0..vis_h {
            for col in 0..vis_w {
                bitmap[row][col] = raster[(vis_top + row) * metrics.width + (vis_left + col)];
            }
        }

        // ymin from fontdue is the distance from baseline to top of glyph (positive = above)
        // We use (metrics.ymin + vis_top as i32) as the baseline-relative top
        let y_ref = -(metrics.ymin as i32) + vis_top as i32;
        min_ymin = min_ymin.min(y_ref);
        max_y_bottom = max_y_bottom.max(y_ref + vis_h as i32);

        char_infos.push(CharInfo {
            bitmap,
            vis_w,
            vis_h,
            y_offset: y_ref,
        });
    }

    if char_infos.is_empty() {
        return None;
    }

    let text_height = (max_y_bottom - min_ymin) as usize;
    let total_w: usize =
        char_infos.iter().map(|c| c.vis_w).sum::<usize>()
        + LETTER_SPACING * (char_infos.len().saturating_sub(1));

    if total_w > WIDTH || text_height > HEIGHT {
        return None;
    }

    // Place on WIDTH×HEIGHT canvas, centered + vertical offset
    let paste_x = ((WIDTH as i32 - total_w as i32) / 2).max(0);
    let paste_y = ((HEIGHT as i32 - text_height as i32) / 2) + VERTICAL_OFFSET;

    let mut mask = vec![vec![false; WIDTH]; HEIGHT];
    let mut cur_x = paste_x;

    for info in &char_infos {
        let char_y = paste_y + (info.y_offset - min_ymin) as i32;

        for row in 0..info.vis_h {
            for col in 0..info.vis_w {
                let px = cur_x + col as i32;
                let py = char_y + row as i32;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    if info.bitmap[row][col] >= BINARIZE_THRESHOLD {
                        mask[py as usize][px as usize] = true;
                    }
                }
            }
        }

        cur_x += info.vis_w as i32 + LETTER_SPACING as i32;
    }

    // Find text bounding rows
    let mut text_top = None;
    let mut text_bot = 0;
    for y in 0..HEIGHT {
        if mask[y].iter().any(|&v| v) {
            if text_top.is_none() {
                text_top = Some(y);
            }
            text_bot = y;
        }
    }

    let text_top = text_top?;
    Some((mask, text_top, text_bot))
}

// ---------------------------------------------------------------------------
// Composite: burst + Korean text
// ---------------------------------------------------------------------------

fn composite(
    burst: &[Vec<u8>],
    orig: &[Vec<u8>],
    text_mask: &[Vec<bool>],
    text_top: usize,
    text_bot: usize,
) -> Vec<Vec<u8>> {
    let mut result: Vec<Vec<u8>> = burst.iter().map(|row| row.clone()).collect();

    // Fill original JP text positions (1,2,3) with burst inner (9)
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if JP_TEXT_INDICES.contains(&orig[y][x]) {
                result[y][x] = IDX_BURST_INNER;
            }
        }
    }

    // Dilate text mask for outline
    let dilated = dilate_bool(text_mask, 1);

    let glyph_height = if text_bot > text_top { text_bot - text_top + 1 } else { 1 };
    let bot_start = text_top + glyph_height * 60 / 100;

    // Pass 1: outline pixels (D)
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let is_body = text_mask[y][x];
            let is_outline = dilated[y][x] && !is_body;
            if is_outline {
                result[y][x] = IDX_OUTLINE;
            }
        }
    }

    // Pass 2: body pixels (gradient)
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if text_mask[y][x] {
                result[y][x] = if y >= bot_start {
                    IDX_GRADIENT_BOT
                } else {
                    IDX_GRADIENT_TOP
                };
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render the Korean level-up sprite and return 896 bytes of 4bpp data.
///
/// Reads original sprite data from `spr_data[0x64C0..0x6840]`, extracts the
/// star burst, overlays "레벨업!" in Korean, and returns the replacement bytes.
pub fn render_levelup_sprite(spr_data: &[u8], font: &Font, font_size: f32) -> Option<Vec<u8>> {
    if spr_data.len() < OFFSET + NBYTES {
        return None;
    }

    let orig_data = &spr_data[OFFSET..OFFSET + NBYTES];
    let orig_pixels = decode_4bpp(orig_data);

    let burst = extract_burst_layer(&orig_pixels);

    let (text_mask, text_top, text_bot) = render_text_spaced(font, font_size)?;

    let result = composite(&burst, &orig_pixels, &text_mask, text_top, text_bot);

    Some(encode_4bpp(&result))
}

/// Patch the level-up sprite in-place in the decompressed SYSTEM.SPR buffer.
///
/// Returns 1 if patched, 0 if skipped (font can't render or buffer too small).
pub fn patch_levelup_sprite(spr_data: &mut [u8], font: &Font, font_size: f32) -> usize {
    match render_levelup_sprite(spr_data, font, font_size) {
        Some(data) => {
            spr_data[OFFSET..OFFSET + NBYTES].copy_from_slice(&data);
            1
        }
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_4bpp_roundtrip() {
        // Create a test pattern
        let mut pixels = vec![vec![0u8; WIDTH]; HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                pixels[y][x] = ((x + y) % 16) as u8;
            }
        }
        let encoded = encode_4bpp(&pixels);
        assert_eq!(encoded.len(), NBYTES);
        let decoded = decode_4bpp(&encoded);
        assert_eq!(decoded, pixels);
    }

    #[test]
    fn test_encode_4bpp_nibble_order() {
        let mut pixels = vec![vec![0u8; WIDTH]; HEIGHT];
        pixels[0][0] = 0xA;
        pixels[0][1] = 0xB;
        let encoded = encode_4bpp(&pixels);
        assert_eq!(encoded[0], 0xAB); // high nibble = left, low nibble = right
    }

    #[test]
    fn test_burst_extraction_preserves_shape() {
        // Read real SYSTEM.SPR if available
        let spr_data = match std::fs::read("out/dec/SYSTEM.SPR") {
            Ok(d) => d,
            Err(_) => return,
        };
        if spr_data.len() < OFFSET + NBYTES {
            return;
        }

        let orig = decode_4bpp(&spr_data[OFFSET..OFFSET + NBYTES]);
        let burst = extract_burst_layer(&orig);

        // Burst should have D, 9, A indices and 0 (transparent)
        let mut has_d = false;
        let mut has_9 = false;
        let mut has_a = false;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                match burst[y][x] {
                    0xD => has_d = true,
                    0x9 => has_9 = true,
                    0xA => has_a = true,
                    0 => {}
                    other => panic!("Unexpected index {} at ({}, {})", other, x, y),
                }
            }
        }
        assert!(has_d, "Should have D (outer)");
        assert!(has_9, "Should have 9 (inner)");
        assert!(has_a, "Should have A (tips)");
    }

    #[test]
    fn test_render_levelup_produces_output() {
        let font_data = match std::fs::read("assets/fonts/MaplestoryBold.ttf") {
            Ok(d) => d,
            Err(_) => return,
        };
        let font = fontdue::Font::from_bytes(
            font_data.as_slice(),
            fontdue::FontSettings::default(),
        )
        .unwrap();

        let spr_data = match std::fs::read("out/dec/SYSTEM.SPR") {
            Ok(d) => d,
            Err(_) => return,
        };

        let result = render_levelup_sprite(&spr_data, &font, 13.0);
        assert!(result.is_some(), "Should produce output");
        let data = result.unwrap();
        assert_eq!(data.len(), NBYTES);

        // Should have non-zero content
        assert!(data.iter().any(|&b| b != 0), "Should have non-zero pixels");
    }
}
