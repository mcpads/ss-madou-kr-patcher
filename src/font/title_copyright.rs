//! Deterministic Korean patch for the title-screen character copyright line.
//!
//! `COMPILE.SPR` decompresses to a 32-byte VDP1 color lookup table followed by
//! a headerless 200x32 linear 4bpp surface. The Japanese `キャラクター`
//! region, including the final long-vowel pixels at x=48..55, ends before the
//! company copyright allocation at x=56. Replacing six Japanese characters
//! with three Korean syllables shortens the line, so the patch moves the whole
//! `캐릭터 + ©SEGA...` group nine pixels left. Its visible center becomes
//! x=100.5, within half a pixel of the original line's x=100.0 center, while
//! retaining the three-pixel gap between `터` and `©`. Each native 9x9
//! Galmuri9 body receives the original LUT's one-pixel black outline in an
//! 11x11 cell.

use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

pub const SPRITE_WIDTH: usize = 200;
pub const SPRITE_HEIGHT: usize = 32;
pub const PALETTE_BYTES: usize = 32;
pub const PIXEL_BYTES: usize = SPRITE_WIDTH * SPRITE_HEIGHT / 2;
pub const SPRITE_BYTES: usize = PALETTE_BYTES + PIXEL_BYTES;

pub const SOURCE_TEXT: &str = "キャラクター";
pub const KOREAN_TEXT: &str = "캐릭터";
pub const COMPANY_SOURCE_X: usize = 56;
pub const COMPANY_SOURCE_FIRST_PIXEL_X: usize = 59;
pub const LINE_SHIFT_LEFT: usize = 9;
pub const COMPANY_DESTINATION_X: usize = COMPANY_SOURCE_X - LINE_SHIFT_LEFT;
pub const COMPANY_FIRST_PIXEL_X: usize = COMPANY_SOURCE_FIRST_PIXEL_X - LINE_SHIFT_LEFT;
pub const CELL_WIDTH: usize = 11;
pub const KOREAN_END_X: usize = COMPANY_FIRST_PIXEL_X - 1;
pub const KOREAN_START_X: usize = KOREAN_END_X - CELL_WIDTH * 3;
pub const LOWER_LINE_Y: usize = 16;
pub const LOWER_LINE_HEIGHT: usize = 16;

const FONT_SIZE: f32 = 9.0;
const BODY_WIDTH: usize = 9;
const BODY_ROWS: usize = 9;
const CELL_ROWS: usize = 11;
const BODY_INSET: usize = 1;
const CELL_Y_OFFSET: usize = (LOWER_LINE_HEIGHT - CELL_ROWS) / 2;
const PIXEL_THRESHOLD: u8 = 64;
const OUTLINE_INDEX: u8 = 1;
const WHITE_INDEX: u8 = 9;

const ORIGINAL_SPR_SHA256: &str =
    "95ac5c55c9a161f0a60a513251935e19bd9ca3494d19b3f8672e9848fde790b9";
const GALMURI9_SHA256: &str = "5cb68052ee0a15571747e91c20f145e24b51bb459c6cd58226fafee78d9c0b16";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn verify_hash(bytes: &[u8], expected: &str, label: &str) -> Result<(), String> {
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "{label} SHA-256 mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

pub fn verify_original_sprite(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != SPRITE_BYTES {
        return Err(format!(
            "COMPILE.SPR is {} bytes, expected {SPRITE_BYTES}",
            bytes.len()
        ));
    }
    verify_hash(bytes, ORIGINAL_SPR_SHA256, "original COMPILE.SPR")
}

fn set_pixel(surface: &mut [u8], x: usize, y: usize, value: u8) {
    let offset = PALETTE_BYTES + (y * SPRITE_WIDTH + x) / 2;
    if x & 1 == 0 {
        surface[offset] = (surface[offset] & 0x0f) | ((value & 0x0f) << 4);
    } else {
        surface[offset] = (surface[offset] & 0xf0) | (value & 0x0f);
    }
}

fn get_pixel(surface: &[u8], x: usize, y: usize) -> u8 {
    let value = surface[PALETTE_BYTES + (y * SPRITE_WIDTH + x) / 2];
    if x & 1 == 0 { value >> 4 } else { value & 0x0f }
}

fn render_cell(font: &Font, ch: char) -> [[u8; CELL_WIDTH]; CELL_ROWS] {
    let mut body = [[false; BODY_WIDTH]; BODY_ROWS];
    let (metrics, raster) = font.rasterize(ch, FONT_SIZE);
    if metrics.width == 0 || metrics.height == 0 {
        return [[0u8; CELL_WIDTH]; CELL_ROWS];
    }

    let x_offset = ((BODY_WIDTH as isize - metrics.width as isize) / 2).max(0) as usize;
    let y_offset = ((BODY_ROWS as isize - metrics.height as isize) / 2).max(0) as usize;
    for src_y in 0..metrics.height {
        let dst_y = y_offset + src_y;
        if dst_y >= BODY_ROWS {
            break;
        }
        for src_x in 0..metrics.width {
            let dst_x = x_offset + src_x;
            if dst_x >= BODY_WIDTH {
                break;
            }
            if raster[src_y * metrics.width + src_x] >= PIXEL_THRESHOLD {
                body[dst_y][dst_x] = true;
            }
        }
    }

    let mut cell = [[0u8; CELL_WIDTH]; CELL_ROWS];
    for y in 0..BODY_ROWS {
        for x in 0..BODY_WIDTH {
            if !body[y][x] {
                continue;
            }
            let center_x = x + BODY_INSET;
            let center_y = y + BODY_INSET;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let outline_x = (center_x as isize + dx) as usize;
                    let outline_y = (center_y as isize + dy) as usize;
                    cell[outline_y][outline_x] = OUTLINE_INDEX;
                }
            }
        }
    }
    for y in 0..BODY_ROWS {
        for x in 0..BODY_WIDTH {
            if body[y][x] {
                cell[y + BODY_INSET][x + BODY_INSET] = WHITE_INDEX;
            }
        }
    }

    cell
}

fn patch_surface(surface: &mut [u8], font: &Font) {
    let mut company = vec![vec![0u8; SPRITE_WIDTH - COMPANY_SOURCE_X]; LOWER_LINE_HEIGHT];
    for (row, y) in (LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT).enumerate() {
        for x in COMPANY_SOURCE_X..SPRITE_WIDTH {
            company[row][x - COMPANY_SOURCE_X] = get_pixel(surface, x, y);
        }
    }

    for y in LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT {
        for x in 0..SPRITE_WIDTH {
            set_pixel(surface, x, y, 0);
        }
    }

    for (row, pixels) in company.iter().enumerate() {
        for (column, &value) in pixels.iter().enumerate() {
            set_pixel(
                surface,
                COMPANY_DESTINATION_X + column,
                LOWER_LINE_Y + row,
                value,
            );
        }
    }

    for (index, ch) in KOREAN_TEXT.chars().enumerate() {
        let cell = render_cell(font, ch);
        let x_origin = KOREAN_START_X + index * CELL_WIDTH;
        for (cell_y, row) in cell.iter().enumerate() {
            for (cell_x, &value) in row.iter().enumerate() {
                if value != 0 {
                    set_pixel(
                        surface,
                        x_origin + cell_x,
                        LOWER_LINE_Y + CELL_Y_OFFSET + cell_y,
                        value,
                    );
                }
            }
        }
    }
}

pub fn compile(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_original_sprite(original)?;
    verify_hash(font_bytes, GALMURI9_SHA256, "Galmuri9 font")?;

    let font = Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| format!("failed to load Galmuri9: {error}"))?;
    let mut compiled = original.to_vec();
    patch_surface(&mut compiled, &font);
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires assets/fonts/Galmuri9.ttf"]
    fn korean_is_outlined_and_right_aligned_to_copyright() {
        let font_bytes = crate::test_input::read("assets/fonts/Galmuri9.ttf");
        assert_eq!(sha256_hex(&font_bytes), GALMURI9_SHA256);
        let font = Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).unwrap();
        let mut surface = vec![0x55; SPRITE_BYTES];
        patch_surface(&mut surface, &font);

        for y in LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT {
            for x in 0..KOREAN_START_X {
                assert_eq!(get_pixel(&surface, x, y), 0, "front pixel {x},{y}");
            }
        }

        let outline_pixels = (LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT)
            .flat_map(|y| (KOREAN_START_X..KOREAN_END_X).map(move |x| (x, y)))
            .filter(|&(x, y)| get_pixel(&surface, x, y) == OUTLINE_INDEX)
            .count();
        let body_pixels = (LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT)
            .flat_map(|y| (KOREAN_START_X..KOREAN_END_X).map(move |x| (x, y)))
            .filter(|&(x, y)| get_pixel(&surface, x, y) == WHITE_INDEX)
            .count();
        assert!(outline_pixels > 30, "black outline must be present");
        assert!(body_pixels > 30, "white Galmuri9 body must be present");
    }

    #[test]
    #[ignore = "requires assets/fonts/Galmuri9.ttf"]
    fn shortened_lower_line_retains_original_visual_center() {
        let font_bytes = crate::test_input::read("assets/fonts/Galmuri9.ttf");
        let font = Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).unwrap();
        let mut surface = vec![0u8; SPRITE_BYTES];
        set_pixel(
            &mut surface,
            COMPANY_SOURCE_FIRST_PIXEL_X,
            LOWER_LINE_Y + 6,
            WHITE_INDEX,
        );
        set_pixel(&mut surface, 194, LOWER_LINE_Y + 6, WHITE_INDEX);
        patch_surface(&mut surface, &font);

        let occupied_x: Vec<usize> = (0..SPRITE_WIDTH)
            .filter(|&x| {
                (LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT)
                    .any(|y| get_pixel(&surface, x, y) != 0)
            })
            .collect();
        assert_eq!(occupied_x.first(), Some(&16));
        assert_eq!(occupied_x.last(), Some(&185));
        assert_eq!(
            occupied_x.first().unwrap() + occupied_x.last().unwrap(),
            201
        );
        for x in 47..COMPANY_FIRST_PIXEL_X {
            assert!(
                (LOWER_LINE_Y..LOWER_LINE_Y + LOWER_LINE_HEIGHT)
                    .all(|y| get_pixel(&surface, x, y) == 0),
                "gap pixel {x}"
            );
        }
    }

    #[test]
    #[ignore = "requires assets/fonts/Galmuri9.ttf"]
    fn palette_first_line_and_company_are_preserved_while_company_moves_left() {
        let font_bytes = crate::test_input::read("assets/fonts/Galmuri9.ttf");
        let font = Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).unwrap();
        let mut original: Vec<u8> = (0..SPRITE_BYTES).map(|index| index as u8).collect();
        for y in LOWER_LINE_Y..SPRITE_HEIGHT {
            for x in COMPANY_SOURCE_X..COMPANY_SOURCE_FIRST_PIXEL_X {
                set_pixel(&mut original, x, y, 0);
            }
        }
        let mut patched = original.clone();
        patch_surface(&mut patched, &font);

        assert_eq!(&patched[..PALETTE_BYTES], &original[..PALETTE_BYTES]);
        let row_bytes = SPRITE_WIDTH / 2;
        assert_eq!(
            &patched[PALETTE_BYTES..PALETTE_BYTES + LOWER_LINE_Y * row_bytes],
            &original[PALETTE_BYTES..PALETTE_BYTES + LOWER_LINE_Y * row_bytes]
        );
        for y in LOWER_LINE_Y..SPRITE_HEIGHT {
            for source_x in COMPANY_SOURCE_FIRST_PIXEL_X..SPRITE_WIDTH {
                let destination_x = source_x - LINE_SHIFT_LEFT;
                assert_eq!(
                    get_pixel(&patched, destination_x, y),
                    get_pixel(&original, source_x, y)
                );
            }
            for x in SPRITE_WIDTH - LINE_SHIFT_LEFT..SPRITE_WIDTH {
                assert_eq!(get_pixel(&patched, x, y), 0);
            }
        }
    }
}
