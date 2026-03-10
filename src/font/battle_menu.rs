//! Battle menu tab sprite renderer (SYSTEM.SPR partial patch).
//!
//! Replaces 6 Japanese katakana menu tab sprites in SYSTEM.SPR with
//! Korean labels rendered using fontdue. Each tab has a selected (40×20)
//! and unselected (40×14) variant. The renderer erases original text
//! pixels and paints new Korean text at the correct palette index.

use fontdue::Font;

const TAB_WIDTH: usize = 40;
const BYTES_PER_ROW: usize = 20; // 40 pixels / 2 (4bpp)
const SEL_ROWS: usize = 20;
const UNSEL_ROWS: usize = 14;

/// Palette index used for text in the selected tab state.
const SEL_TEXT_IDX: u8 = 1;
/// Palette index used for text in the unselected tab state.
const UNSEL_TEXT_IDX: u8 = 8;

/// Coverage threshold (0–255) for text pixels.
const TEXT_THRESHOLD: u8 = 60;

/// Text rendering zone (x1, y1, x2, y2 inclusive) within selected tab.
const SEL_TEXT_ZONE: (usize, usize, usize, usize) = (4, 5, 35, 15);
/// Text rendering zone within unselected tab.
const UNSEL_TEXT_ZONE: (usize, usize, usize, usize) = (4, 3, 35, 11);

/// Menu tab definitions: (sel_offset, unsel_offset, jp_label, ko_label).
/// Each tab pair is 800 bytes apart (400 sel + 280 unsel + 120 gap).
pub const MENU_TABS: &[(usize, usize, &str, &str)] = &[
    (0x3840, 0x39F8, "アイテム", "아이템"),
    (0x3B60, 0x3D18, "イベント", "이벤트"),
    (0x3E80, 0x4038, "サポート", "보조"),
    (0x41A0, 0x4358, "アタック", "공격"),
    (0x44C0, 0x4678, "ガード", "방어"),
    (0x47E0, 0x4998, "にげる", "도망"),
];

// ---- 4bpp read/write helpers ----

/// Read a 4bpp sprite region into a 2D grid of palette indices.
fn read_4bpp(data: &[u8], offset: usize, rows: usize) -> Vec<Vec<u8>> {
    let mut grid = vec![vec![0u8; TAB_WIDTH]; rows];
    for y in 0..rows {
        for xb in 0..BYTES_PER_ROW {
            let b = data[offset + y * BYTES_PER_ROW + xb];
            grid[y][xb * 2] = (b >> 4) & 0xF;
            grid[y][xb * 2 + 1] = b & 0xF;
        }
    }
    grid
}

/// Write a 2D palette-index grid back to 4bpp data.
fn write_4bpp(data: &mut [u8], offset: usize, grid: &[Vec<u8>]) {
    for (y, row) in grid.iter().enumerate() {
        for xb in 0..BYTES_PER_ROW {
            data[offset + y * BYTES_PER_ROW + xb] =
                (row[xb * 2] << 4) | row[xb * 2 + 1];
        }
    }
}

// ---- text erasure ----

/// Find the most common non-text background index near (x, y).
fn find_bg_neighbor(grid: &[Vec<u8>], x: usize, y: usize, text_idx: u8) -> u8 {
    let rows = grid.len();
    for radius in 1..5i32 {
        let mut counts = [0u32; 16];
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let ny = y as i32 + dy;
                let nx = x as i32 + dx;
                if ny >= 0 && (ny as usize) < rows && nx >= 0 && (nx as usize) < TAB_WIDTH {
                    let v = grid[ny as usize][nx as usize];
                    // Exclude text index, white border (0xF), and low border indices (0-3)
                    if v != text_idx && v != 0xF && v > 3 {
                        counts[v as usize] += 1;
                    }
                }
            }
        }
        if let Some((idx, _)) = counts
            .iter()
            .enumerate()
            .filter(|&(_, &c)| c > 0)
            .max_by_key(|&(_, &c)| c)
        {
            return idx as u8;
        }
    }
    0xB // fallback: medium gradient value
}

/// Erase text pixels in the given zone by replacing them with background.
fn erase_text_zone(
    grid: &mut Vec<Vec<u8>>,
    text_idx: u8,
    zone: (usize, usize, usize, usize),
) {
    let (x1, y1, x2, y2) = zone;
    let rows = grid.len();
    let snapshot = grid.clone();

    for y in y1..=y2.min(rows - 1) {
        for x in x1..=x2.min(TAB_WIDTH - 1) {
            if grid[y][x] == text_idx {
                grid[y][x] = find_bg_neighbor(&snapshot, x, y, text_idx);
            }
        }
    }
}

// ---- fontdue text rendering ----

/// Render multi-character Korean text as a coverage mask using fontdue.
/// Returns a 2D vec of coverage values (0–255) sized (height × width).
fn render_text_mask(
    font: &Font,
    text: &str,
    font_size: f32,
    target_w: usize,
    target_h: usize,
) -> Vec<Vec<u8>> {
    let mut mask = vec![vec![0u8; target_w]; target_h];
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return mask;
    }

    // Collect glyph data
    let glyphs: Vec<_> = chars
        .iter()
        .map(|&ch| font.rasterize(ch, font_size))
        .collect();

    // Total advance width for horizontal centering
    let total_advance: f32 = chars
        .iter()
        .map(|&ch| font.metrics(ch, font_size).advance_width)
        .sum();

    // Max glyph height for vertical centering
    let max_height = glyphs
        .iter()
        .map(|(m, _)| m.height)
        .max()
        .unwrap_or(0);

    let x_start = ((target_w as f32 - total_advance) / 2.0).max(0.0);
    let y_base = ((target_h as i32 - max_height as i32) / 2).max(0);

    let mut cursor_x = x_start;
    for (i, (metrics, raster)) in glyphs.iter().enumerate() {
        if metrics.width == 0 || metrics.height == 0 {
            cursor_x += font.metrics(chars[i], font_size).advance_width;
            continue;
        }

        let gx = cursor_x as i32 + metrics.xmin as i32;
        // Center this glyph relative to max_height
        let gy = y_base + ((max_height as i32 - metrics.height as i32) / 2);

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px >= 0
                    && (px as usize) < target_w
                    && py >= 0
                    && (py as usize) < target_h
                {
                    let coverage = raster[row * metrics.width + col];
                    let cur = &mut mask[py as usize][px as usize];
                    *cur = (*cur).max(coverage);
                }
            }
        }

        cursor_x += font.metrics(chars[i], font_size).advance_width;
    }

    mask
}

/// Patch a single tab sprite (selected or unselected) in-place.
fn patch_tab_variant(
    spr_data: &mut [u8],
    font: &Font,
    font_size: f32,
    offset: usize,
    rows: usize,
    text_idx: u8,
    zone: (usize, usize, usize, usize),
    ko_text: &str,
) {
    let sprite_bytes = BYTES_PER_ROW * rows;
    if offset + sprite_bytes > spr_data.len() {
        return;
    }

    // Read original sprite
    let mut grid = read_4bpp(spr_data, offset, rows);

    // Erase old Japanese text
    erase_text_zone(&mut grid, text_idx, zone);

    // Render Korean text mask
    let (x1, y1, x2, y2) = zone;
    let tw = x2 - x1 + 1;
    let th = y2 - y1 + 1;
    let mask = render_text_mask(font, ko_text, font_size, tw, th);

    // Paint text pixels
    for y in 0..th {
        for x in 0..tw {
            if mask[y][x] >= TEXT_THRESHOLD {
                let ny = y + y1;
                let nx = x + x1;
                if ny < rows && nx < TAB_WIDTH {
                    grid[ny][nx] = text_idx;
                }
            }
        }
    }

    // Write back
    write_4bpp(spr_data, offset, &grid);
}

/// Patch all 6 menu tab sprites (selected + unselected) in SYSTEM.SPR.
///
/// Returns the number of tab pairs patched.
pub fn patch_menu_tabs(spr_data: &mut [u8], font: &Font, font_size: f32) -> usize {
    let mut count = 0;
    for &(sel_off, unsel_off, _jp, ko) in MENU_TABS {
        patch_tab_variant(
            spr_data,
            font,
            font_size,
            sel_off,
            SEL_ROWS,
            SEL_TEXT_IDX,
            SEL_TEXT_ZONE,
            ko,
        );
        patch_tab_variant(
            spr_data,
            font,
            font_size,
            unsel_off,
            UNSEL_ROWS,
            UNSEL_TEXT_IDX,
            UNSEL_TEXT_ZONE,
            ko,
        );
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_4bpp_roundtrip() {
        // Create test data: 2 rows of 40px = 40 bytes
        let mut data = vec![0u8; 100];
        // Set some test values at offset 10
        data[10] = 0xAB; // pixel 0=A, pixel 1=B
        data[11] = 0xCD; // pixel 2=C, pixel 3=D

        let grid = read_4bpp(&data, 10, 2);
        assert_eq!(grid[0][0], 0xA);
        assert_eq!(grid[0][1], 0xB);
        assert_eq!(grid[0][2], 0xC);
        assert_eq!(grid[0][3], 0xD);

        // Write back and verify
        let mut out = vec![0u8; 100];
        write_4bpp(&mut out, 10, &grid);
        assert_eq!(out[10], 0xAB);
        assert_eq!(out[11], 0xCD);
    }

    #[test]
    fn test_render_text_mask_nonempty() {
        let font_data = match std::fs::read("assets/fonts/Galmuri9.ttf") {
            Ok(d) => d,
            Err(_) => return,
        };
        let font = fontdue::Font::from_bytes(
            font_data.as_slice(),
            fontdue::FontSettings::default(),
        )
        .unwrap();

        let mask = render_text_mask(&font, "공격", 10.0, 32, 11);
        assert_eq!(mask.len(), 11);
        assert_eq!(mask[0].len(), 32);

        let total: u32 = mask.iter().flat_map(|r| r.iter()).map(|&v| v as u32).sum();
        assert!(total > 0, "text mask should have rendered content");

        // Check pixels above threshold
        let above_threshold: usize = mask.iter()
            .flat_map(|r| r.iter())
            .filter(|&&v| v >= TEXT_THRESHOLD)
            .count();
        assert!(above_threshold > 10, "should have at least 10 pixels above threshold, got {above_threshold}");

        // Print for debug
        for y in 0..11 {
            let row: String = mask[y].iter()
                .map(|&v| if v >= TEXT_THRESHOLD { '#' } else if v > 0 { '.' } else { ' ' })
                .collect();
            eprintln!("  {row}");
        }
    }

    #[test]
    fn test_patch_modifies_data() {
        let spr_data_result = std::fs::read("out/dec/SYSTEM.SPR");
        let font_data_result = std::fs::read("assets/fonts/Galmuri9.ttf");
        let (mut spr_data, font_data) = match (spr_data_result, font_data_result) {
            (Ok(s), Ok(f)) => (s, f),
            _ => return,
        };
        let font = fontdue::Font::from_bytes(
            font_data.as_slice(),
            fontdue::FontSettings::default(),
        )
        .unwrap();

        let original = spr_data.clone();
        let count = patch_menu_tabs(&mut spr_data, &font, 10.0);
        assert_eq!(count, 6);

        // Check that data actually changed at tab offsets
        let mut total_diff = 0;
        for &(sel_off, unsel_off, _jp, _ko) in MENU_TABS {
            let sel_diff: usize = (0..BYTES_PER_ROW * SEL_ROWS)
                .filter(|&i| spr_data[sel_off + i] != original[sel_off + i])
                .count();
            let unsel_diff: usize = (0..BYTES_PER_ROW * UNSEL_ROWS)
                .filter(|&i| spr_data[unsel_off + i] != original[unsel_off + i])
                .count();
            eprintln!("  sel 0x{sel_off:04X}: {sel_diff} bytes differ, unsel 0x{unsel_off:04X}: {unsel_diff} bytes differ");
            total_diff += sel_diff + unsel_diff;
        }
        assert!(total_diff > 0, "patch should modify at least some bytes");
    }

    #[test]
    fn test_erase_text_zone() {
        // Create a simple grid with text index 1 and background 0xB
        let mut grid = vec![vec![0xBu8; 10]; 10];
        grid[3][4] = 1;
        grid[3][5] = 1;
        grid[4][4] = 1;

        erase_text_zone(&mut grid, 1, (3, 2, 6, 5));

        // Text pixels should be replaced with background
        assert_ne!(grid[3][4], 1);
        assert_ne!(grid[3][5], 1);
        assert_ne!(grid[4][4], 1);
    }
}
