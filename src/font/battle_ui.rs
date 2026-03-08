//! Battle UI sprite renderer (SYSTEM.SPR partial patch).
//!
//! Replaces 6 Japanese kanji tiles (16×16 4bpp) in SYSTEM.SPR with
//! Korean 2-character labels rendered at 8px using Dalmoori font.
//! Each 16×16 tile is composed of two 8×8 Korean characters placed
//! side by side, vertically centered (y_offset=4, rows 4–11).

use fontdue::Font;

/// Palette indices matching the original SYSTEM.SPR kanji style:
/// - 0 = transparent (background)
/// - BODY_COLOR (2) = dark interior fill
/// - EDGE_COLOR (3) = body edge / transition
/// - OUTLINE_COLOR (15/F) = bright white outline border
const BODY_COLOR: u8 = 2;
const EDGE_COLOR: u8 = 3;
const OUTLINE_COLOR: u8 = 0xF;

/// Bytes per 16×16 4bpp tile (8 bytes/row × 16 rows).
pub const TILE_BYTES: usize = 128;

/// Battle UI tile mappings: (offset in decompressed SYSTEM.SPR, original kanji, Korean label).
pub const BATTLE_TILES: &[(usize, &str, &str)] = &[
    (0x7800, "攻", "공격"),
    (0x7880, "防", "방어"),
    (0x7900, "命", "명중"),
    (0x7980, "動", "동작"),
    (0x7A00, "運", "행운"),
    (0x7A80, "全", "전체"),
];

/// Render a single Korean character onto an 8×8 bitmap, centered.
fn render_char_8x8(font: &Font, ch: char, font_size: f32) -> [[bool; 8]; 8] {
    let mut grid = [[false; 8]; 8];

    let (metrics, raster) = font.rasterize(ch, font_size);
    if metrics.width == 0 || metrics.height == 0 {
        return grid;
    }

    // Center within 8×8 cell
    let gx = ((8i32 - metrics.width as i32) / 2).max(0);
    let gy = ((8i32 - metrics.height as i32) / 2).max(0);

    for row in 0..metrics.height {
        for col in 0..metrics.width {
            let px = gx + col as i32;
            let py = gy + row as i32;
            if px >= 0 && px < 8 && py >= 0 && py < 8 {
                let coverage = raster[row * metrics.width + col];
                if coverage >= 96 {
                    grid[py as usize][px as usize] = true;
                }
            }
        }
    }

    grid
}

/// Render a 2-character Korean label as a 16×16 4bpp tile (128 bytes).
///
/// Matches the original kanji style: dark body (index 2-3) with white
/// outline (index F). Two 8×8 characters are placed side by side,
/// vertically centered with `y_offset` rows of blank above.
pub fn render_battle_tile(font: &Font, text: &str, font_size: f32, y_offset: usize) -> [u8; 128] {
    let mut body = [[false; 16]; 16];
    let chars: Vec<char> = text.chars().collect();

    for (ci, &ch) in chars.iter().take(2).enumerate() {
        let grid = render_char_8x8(font, ch, font_size);
        for y in 0..8 {
            for x in 0..8 {
                if grid[y][x] {
                    body[y_offset + y][ci * 8 + x] = true;
                }
            }
        }
    }

    // Dilate body by 1px to create outline region
    let mut dilated = [[false; 16]; 16];
    for y in 0..16 {
        for x in 0..16 {
            if body[y][x] {
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let ny = y as i32 + dy;
                        let nx = x as i32 + dx;
                        if (0..16).contains(&ny) && (0..16).contains(&nx) {
                            dilated[ny as usize][nx as usize] = true;
                        }
                    }
                }
            }
        }
    }

    // Assign palette: body=BODY_COLOR, body edge=EDGE_COLOR, outline=OUTLINE_COLOR
    let mut palette = [[0u8; 16]; 16];
    for y in 0..16 {
        for x in 0..16 {
            if body[y][x] {
                // Body pixel — check if it borders a non-body pixel
                let on_edge = (|| {
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dy == 0 && dx == 0 {
                                continue;
                            }
                            let ny = y as i32 + dy;
                            let nx = x as i32 + dx;
                            if ny < 0 || ny >= 16 || nx < 0 || nx >= 16 {
                                return true;
                            }
                            if !body[ny as usize][nx as usize] {
                                return true;
                            }
                        }
                    }
                    false
                })();
                palette[y][x] = if on_edge { EDGE_COLOR } else { BODY_COLOR };
            } else if dilated[y][x] {
                palette[y][x] = OUTLINE_COLOR;
            }
        }
    }

    palette_to_4bpp(&palette)
}

/// Convert a 16×16 palette-index map to Saturn 4bpp format (128 bytes).
///
/// Saturn 4bpp: high nibble = left pixel, low nibble = right pixel.
fn palette_to_4bpp(palette: &[[u8; 16]; 16]) -> [u8; 128] {
    let mut data = [0u8; 128];
    for y in 0..16 {
        for xb in 0..8 {
            let x = xb * 2;
            data[y * 8 + xb] = (palette[y][x] << 4) | palette[y][x + 1];
        }
    }
    data
}

/// Patch all 6 battle UI tiles in the decompressed SYSTEM.SPR buffer.
///
/// Returns the number of tiles patched.
pub fn patch_battle_tiles(spr_data: &mut [u8], font: &Font, font_size: f32) -> usize {
    let mut count = 0;
    for &(offset, _kanji, ko) in BATTLE_TILES {
        if offset + TILE_BYTES <= spr_data.len() {
            let tile = render_battle_tile(font, ko, font_size, 4);
            spr_data[offset..offset + TILE_BYTES].copy_from_slice(&tile);
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_to_4bpp_empty() {
        let palette = [[0u8; 16]; 16];
        let data = palette_to_4bpp(&palette);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_palette_to_4bpp_full() {
        let palette = [[0xFu8; 16]; 16];
        let data = palette_to_4bpp(&palette);
        assert!(data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_palette_to_4bpp_values() {
        let mut palette = [[0u8; 16]; 16];
        // Set even columns to BODY_COLOR, odd to OUTLINE_COLOR
        for y in 0..16 {
            for x in (0..16).step_by(2) {
                palette[y][x] = BODY_COLOR;
                palette[y][x + 1] = OUTLINE_COLOR;
            }
        }
        let data = palette_to_4bpp(&palette);
        let expected = (BODY_COLOR << 4) | OUTLINE_COLOR;
        assert!(data.iter().all(|&b| b == expected));
    }

    #[test]
    fn test_render_battle_tile_dimensions() {
        let font_data = match std::fs::read("assets/fonts/dalmoori.ttf") {
            Ok(d) => d,
            Err(_) => return, // skip when font not available
        };
        let font = fontdue::Font::from_bytes(
            font_data.as_slice(),
            fontdue::FontSettings::default(),
        )
        .unwrap();

        let tile = render_battle_tile(&font, "공격", 8.0, 4);
        assert_eq!(tile.len(), 128);

        // Top 3 rows should be empty (y_offset=4 minus 1px outline)
        for y in 0..3 {
            for xb in 0..8 {
                assert_eq!(tile[y * 8 + xb], 0, "row {} should be empty", y);
            }
        }
        // Bottom 3 rows should be empty (outline extends 1px below row 11)
        for y in 13..16 {
            for xb in 0..8 {
                assert_eq!(tile[y * 8 + xb], 0, "row {} should be empty", y);
            }
        }
        // Middle rows should have some content
        let middle_bytes: u32 = (4..12)
            .flat_map(|y| (0..8).map(move |xb| tile[y * 8 + xb] as u32))
            .sum();
        assert!(middle_bytes > 0, "middle rows should have rendered content");
    }
}
