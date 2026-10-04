//! Korean flea location markers and battle-effect captions.
//!
//! The flea event draws the marker from three assets:
//!
//! - `B_NOMIA0.SPR` (CNX) starts with a 32-byte VDP1 color lookup table,
//!   two 8x8 arrow/dot cells, and a 48x32 4bpp 「ここに / いるっ!」 at 0x60.
//!   Battle draws it over the flea's position.
//! - `FNOMI01.SPR` (raw, 12,800 bytes) is an 80x80 RGB555 red 「ここに /
//!   いるっ!」 with an arrow and dot below. The battle message window shows it.
//! - `FNOMI02.SPR` has the same layout in yellow without the exclamation. The
//!   field shows it after the flea is defeated.
//!
//! Both 80x80 surfaces are stored mirrored; VDP1 draws them with horizontal
//! flip (CMDCTRL Dir=1). The Korean text is laid out in display orientation
//! and mirrored back. Arrow and dot pixels below `FIELD_TEXT_ROWS` are kept.
//!
//! Strokes use the original shaded-tube look: one-pixel dark outline, darker
//! bottom and right edges, brighter top and left edges, and a mid tone inside.
//! The audited captions in B_NOMIA0 and B_NOMIA1 are also translated;
//! their individual sprite dimensions and animation positions are unchanged.

use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

pub const BATTLE_OFFSET: usize = 0x60;
pub const BATTLE_WIDTH: usize = 48;
pub const BATTLE_HEIGHT: usize = 32;
const BATTLE_BYTES: usize = BATTLE_WIDTH * BATTLE_HEIGHT / 2;
const BATTLE_FONT_SIZE: f32 = 15.0;

pub const FIELD_SIZE: usize = 80;
pub const FIELD_BYTES: usize = FIELD_SIZE * FIELD_SIZE * 2;
/// Rows 0..53 hold the Japanese text; the arrow starts at row 53.
const FIELD_TEXT_ROWS: usize = 53;
const FIELD_FONT_SIZE: f32 = 20.0;
const FIELD_LINE_TOPS: [usize; 2] = [9, 31];

pub const TOP_LINE: &str = "여기";
pub const ALERT_BOTTOM_LINE: &str = "있어!";
pub const FIELD_BOTTOM_LINE: &str = "있어";

const PIXEL_THRESHOLD: u8 = 110;

const ORIGINAL_B_NOMIA0_SHA256: &str =
    "a4ad293c887a5c89700ad52c93156c1e7a503382f623b16a53151fd9097cdddd";
const ORIGINAL_FNOMI01_SHA256: &str =
    "d132fd79a8d88dc5640f1e072c5d5391e993f8994fbaab969e6ec839768e2a09";
const ORIGINAL_FNOMI02_SHA256: &str =
    "1aff056bde75f0180a6ad707f2a299cb53b9bd042964960c0f0576388c65b074";
const DNF_BITBIT_SHA256: &str =
    "eebf8c20fea14a927e74216f972d6484d8a2398efdea356f6d0c5adcae531743";

/// Stroke shading class for each pixel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shade {
    Empty,
    Outline,
    BottomEdge,
    RightEdge,
    TopLeftEdge,
    Interior,
}

/// LUT indices of the original 4bpp battle marker.
fn battle_index(shade: Shade) -> u8 {
    match shade {
        Shade::Empty => 0,
        Shade::Outline => 0x6,
        Shade::BottomEdge => 0x7,
        Shade::RightEdge => 0x8,
        Shade::TopLeftEdge => 0xA,
        Shade::Interior => 0x9,
    }
}

/// RGB555 colors taken from the original red FNOMI01 text.
fn red_color(shade: Shade) -> u16 {
    match shade {
        Shade::Empty => 0,
        Shade::Outline => 0x8001,
        Shade::BottomEdge => 0x842C,
        Shade::RightEdge => 0x8431,
        Shade::TopLeftEdge => 0x885A,
        Shade::Interior => 0x8437,
    }
}

/// RGB555 colors taken from the original yellow FNOMI02 text.
fn yellow_color(shade: Shade) -> u16 {
    match shade {
        Shade::Empty => 0,
        Shade::Outline => 0x8C41,
        Shade::BottomEdge => 0x9138,
        Shade::RightEdge => 0x91DC,
        Shade::TopLeftEdge => 0xBBBF,
        Shade::Interior => 0x929F,
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

/// Tight binary mask of one text line.
struct LineMask {
    width: usize,
    height: usize,
    pixels: Vec<bool>,
}

fn rasterize_line(font: &Font, text: &str, size: f32) -> LineMask {
    let ascent = font
        .horizontal_line_metrics(size)
        .map(|metrics| metrics.ascent)
        .unwrap_or(size)
        .ceil() as i32;
    let mut placed = Vec::new();
    let mut pen = 0.0f32;
    for ch in text.chars() {
        let (metrics, raster) = font.rasterize(ch, size);
        let left = pen.round() as i32 + metrics.xmin;
        let top = ascent - (metrics.ymin + metrics.height as i32);
        placed.push((left, top, metrics.width, metrics.height, raster));
        pen += metrics.advance_width;
    }

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (left, top, width, height, raster) in &placed {
        for y in 0..*height {
            for x in 0..*width {
                if raster[y * width + x] >= PIXEL_THRESHOLD {
                    min_x = min_x.min(left + x as i32);
                    max_x = max_x.max(left + x as i32);
                    min_y = min_y.min(top + y as i32);
                    max_y = max_y.max(top + y as i32);
                }
            }
        }
    }
    let width = (max_x - min_x + 1) as usize;
    let height = (max_y - min_y + 1) as usize;
    let mut pixels = vec![false; width * height];
    for (left, top, glyph_width, glyph_height, raster) in &placed {
        for y in 0..*glyph_height {
            for x in 0..*glyph_width {
                if raster[y * glyph_width + x] >= PIXEL_THRESHOLD {
                    let dx = (left + x as i32 - min_x) as usize;
                    let dy = (top + y as i32 - min_y) as usize;
                    pixels[dy * width + dx] = true;
                }
            }
        }
    }
    LineMask {
        width,
        height,
        pixels,
    }
}

/// Place each line horizontally centered at its top row.
fn compose(
    width: usize,
    height: usize,
    lines: &[(&LineMask, usize)],
) -> Result<Vec<bool>, String> {
    let mut body = vec![false; width * height];
    for (line, top) in lines {
        if line.width + 2 > width || top + line.height + 1 > height || *top == 0 {
            return Err(format!(
                "{}x{} line at row {top} does not fit a {width}x{height} surface with its outline",
                line.width, line.height
            ));
        }
        let left = (width - line.width) / 2;
        for y in 0..line.height {
            for x in 0..line.width {
                if line.pixels[y * line.width + x] {
                    body[(top + y) * width + left + x] = true;
                }
            }
        }
    }
    Ok(body)
}

fn shade(body: &[bool], width: usize, height: usize) -> Vec<Shade> {
    let at = |x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < width
            && (y as usize) < height
            && body[y as usize * width + x as usize]
    };
    let mut out = vec![Shade::Empty; width * height];
    for y in 0..height as isize {
        for x in 0..width as isize {
            let index = y as usize * width + x as usize;
            out[index] = if at(x, y) {
                if !at(x, y + 1) {
                    Shade::BottomEdge
                } else if !at(x + 1, y) {
                    Shade::RightEdge
                } else if !at(x, y - 1) || !at(x - 1, y) {
                    Shade::TopLeftEdge
                } else {
                    Shade::Interior
                }
            } else if (-1..=1).any(|dy| (-1..=1).any(|dx| at(x + dx, y + dy))) {
                Shade::Outline
            } else {
                Shade::Empty
            };
        }
    }
    out
}

fn load_font(font_bytes: &[u8]) -> Result<Font, String> {
    verify_hash(font_bytes, DNF_BITBIT_SHA256, "DNFBitBitv2 font")?;
    Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| format!("failed to load DNFBitBitv2: {error}"))
}

fn battle_shades(font: &Font) -> Result<Vec<Shade>, String> {
    let top = rasterize_line(font, TOP_LINE, BATTLE_FONT_SIZE);
    let bottom = rasterize_line(font, ALERT_BOTTOM_LINE, BATTLE_FONT_SIZE);
    let bottom_top = BATTLE_HEIGHT - 1 - bottom.height;
    let body = compose(
        BATTLE_WIDTH,
        BATTLE_HEIGHT,
        &[(&top, 1), (&bottom, bottom_top)],
    )?;
    Ok(shade(&body, BATTLE_WIDTH, BATTLE_HEIGHT))
}

// Geometry read from the game's loaded sprite table at WorkRAMH 0xa3714.
// Source/size pairs were also observed in meal, punch, dodge and rush commands.
#[derive(Clone, Copy, PartialEq)]
enum CaptionPalette { Red, Pink, Gold }
use CaptionPalette::{Red, Pink, Gold};

const REACTION_CELLS: &[(usize, usize, usize, &str, CaptionPalette)] = &[
    (0x0360, 88, 32, "키~킥!", Red),
    (0x08e0, 88, 32, "물기!", Red),
    (0x0e60, 80, 24, "펀치!", Red),
    (0x1220, 80, 24, "막았다!", Gold),
    (0x15e0, 48, 24, "아야!", Red),
    (0x1820, 40, 32, "쭈", Red),
    // おいしそうだな: seven independently animated cells, not just おいしそ.
    (0x1ca0, 24, 24, "맛", Pink),
    (0x1dc0, 24, 24, "있", Pink),
    (0x1ee0, 16, 24, "어", Pink),
    (0x1fa0, 16, 24, "보", Pink),
    (0x2060, 16, 24, "이", Pink),
    (0x2120, 24, 24, "네", Pink),
    (0x2240, 24, 24, "~", Pink),
    (0x2380, 80, 24, "맛있어!", Gold),
    (0x2740, 64, 24, "별로네", Gold),
];

fn caption_index(shade: Shade, palette: CaptionPalette) -> u8 {
    match palette {
        Red => battle_index(shade),
        Pink => match shade {
            Shade::Empty => 0, Shade::Outline => 1, Shade::BottomEdge => 2,
            Shade::RightEdge => 3, Shade::TopLeftEdge => 5, Shade::Interior => 4,
        },
        Gold => match shade {
            Shade::Empty => 0, Shade::Outline => 6, Shade::BottomEdge => 9,
            Shade::RightEdge => 10, Shade::TopLeftEdge => 14, Shade::Interior => 12,
        },
    }
}

fn reaction_pixels(font: &Font, width: usize, height: usize, text: &str,
    palette: CaptionPalette) -> Result<Vec<u8>, String> {
    let size = if palette == Pink { 13.0 } else if height == 32 { 20.0 } else { 16.0 };
    let line = rasterize_line(font, text, size);
    let body = compose(width, height, &[(&line, (height - line.height) / 2)])?;
    Ok(shade(&body, width, height).chunks_exact(2).map(|pair|
        (caption_index(pair[0], palette) << 4) | caption_index(pair[1], palette)
    ).collect())
}

/// Replace the marker and audited reaction cells in decompressed B_NOMIA0.SPR.
/// Palette, arrow cells, sprite geometry, and all other animation data are preserved.
pub fn compile_battle(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_hash(original, ORIGINAL_B_NOMIA0_SHA256, "original B_NOMIA0.SPR")?;
    let font = load_font(font_bytes)?;
    let shades = battle_shades(&font)?;
    let mut compiled = original.to_vec();
    for (byte_index, pair) in shades.chunks(2).enumerate() {
        compiled[BATTLE_OFFSET + byte_index] =
            (battle_index(pair[0]) << 4) | battle_index(pair[1]);
    }
    debug_assert_eq!(shades.len() / 2, BATTLE_BYTES);
    for &(offset, width, height, text, palette) in REACTION_CELLS {
        let pixels = reaction_pixels(&font, width, height, text, palette)?;
        compiled[offset..offset + pixels.len()].copy_from_slice(&pixels);
    }
    // The special-attack caption is split into 64x56 and 56x56 sprites.
    // Their observed x positions differ by 60, leaving four shared columns.
    let line = rasterize_line(&font, "필살!", 34.0);
    let body = compose(116, 56, &[(&line, (56 - line.height) / 2)])?;
    let pixels = shade(&body, 116, 56);
    for (offset, width, left) in [(0x2a40, 64, 0), (0x3140, 56, 60)] {
        for y in 0..56 {
            for x in (0..width).step_by(2) {
                compiled[offset + y * width / 2 + x / 2] =
                    (caption_index(pixels[y * 116 + left + x], Gold) << 4)
                    | caption_index(pixels[y * 116 + left + x + 1], Gold);
            }
        }
    }
    Ok(compiled)
}

/// Blue dodge and purple taste captions in B_NOMIA1; both observed in VDP1.
pub fn compile_battle_effects(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_hash(original, "345a5834f10b46217ee5acd68aa3697889320d8afb20bcf16ad4600ce66c23db", "original B_NOMIA1.SPR")?;
    let font = load_font(font_bytes)?;
    let mut compiled = original.to_vec();
    for (offset, width, height, text, purple) in [
        (0x5a0, 64, 24, "어이쿠~", false),
        (0x8a0, 56, 32, "맛없어", true),
    ] {
        let line = rasterize_line(&font, text, if purple { 16.0 } else { 15.0 });
        let body = compose(width, height, &[(&line, (height - line.height) / 2)])?;
        let index = |s| if purple { match s {
            Shade::Empty => 0, Shade::Outline => 11, Shade::BottomEdge => 12,
            Shade::RightEdge => 13, Shade::TopLeftEdge => 15, Shade::Interior => 14,
        }} else { match s {
            Shade::Empty => 0, Shade::Outline => 1, Shade::BottomEdge => 3,
            Shade::RightEdge => 5, Shade::TopLeftEdge => 10, Shade::Interior => 8,
        }};
        for (i, pair) in shade(&body, width, height).chunks_exact(2).enumerate() {
            compiled[offset + i] = (index(pair[0]) << 4) | index(pair[1]);
        }
    }
    Ok(compiled)
}

fn field_shades(font: &Font, bottom_line: &str) -> Result<Vec<Shade>, String> {
    let top = rasterize_line(font, TOP_LINE, FIELD_FONT_SIZE);
    let bottom = rasterize_line(font, bottom_line, FIELD_FONT_SIZE);
    let body = compose(
        FIELD_SIZE,
        FIELD_TEXT_ROWS,
        &[(&top, FIELD_LINE_TOPS[0]), (&bottom, FIELD_LINE_TOPS[1])],
    )?;
    Ok(shade(&body, FIELD_SIZE, FIELD_TEXT_ROWS))
}

fn compile_field(
    original: &[u8],
    expected_sha256: &str,
    label: &str,
    font_bytes: &[u8],
    bottom_line: &str,
    color: fn(Shade) -> u16,
) -> Result<Vec<u8>, String> {
    verify_hash(original, expected_sha256, label)?;
    let font = load_font(font_bytes)?;
    let shades = field_shades(&font, bottom_line)?;
    let mut compiled = original.to_vec();
    for y in 0..FIELD_TEXT_ROWS {
        for display_x in 0..FIELD_SIZE {
            let stored_x = FIELD_SIZE - 1 - display_x;
            let offset = (y * FIELD_SIZE + stored_x) * 2;
            let value = color(shades[y * FIELD_SIZE + display_x]);
            compiled[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
    }
    Ok(compiled)
}

/// Red battle-window marker `FNOMI01.SPR`: 「여기 / 있어!」.
pub fn compile_alert_window(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    compile_field(
        original,
        ORIGINAL_FNOMI01_SHA256,
        "original FNOMI01.SPR",
        font_bytes,
        ALERT_BOTTOM_LINE,
        red_color,
    )
}

/// Yellow field marker `FNOMI02.SPR`: 「여기 / 있어」.
pub fn compile_field_marker(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    compile_field(
        original,
        ORIGINAL_FNOMI02_SHA256,
        "original FNOMI02.SPR",
        font_bytes,
        FIELD_BOTTOM_LINE,
        yellow_color,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        load_font(&crate::test_input::read("assets/fonts/DNFBitBitv2.ttf")).unwrap()
    }

    #[test]
    #[ignore = "requires assets/fonts/DNFBitBitv2.ttf"]
    fn battle_text_fits_with_outline_inside_surface() {
        let shades = battle_shades(&font()).unwrap();
        let strokes = shades
            .iter()
            .filter(|&&shade| shade != Shade::Empty && shade != Shade::Outline)
            .count();
        assert!(strokes > 150, "both lines must render, got {strokes} pixels");
        for x in 0..BATTLE_WIDTH {
            assert_ne!(shades[x], Shade::BottomEdge, "top row holds only outline");
        }
    }

    #[test]
    #[ignore = "requires assets/fonts/DNFBitBitv2.ttf"]
    fn reaction_cells_preserve_geometry_and_fit_animation_advances() {
        let font = font();
        for &(offset, width, height, text, palette) in REACTION_CELLS {
            let pixels = reaction_pixels(&font, width, height, text, palette).unwrap();
            assert_eq!(pixels.len(), width * height / 2);
            assert!(offset >= BATTLE_OFFSET + BATTLE_BYTES);
            assert!(pixels.iter().any(|&b| b != 0));
            if palette == Pink {
                // Smallest observed horizontal glyph advance is 13 pixels.
                assert!(rasterize_line(&font, text, 13.0).width <= 13);
            }
        }
    }

    #[test]
    #[ignore = "requires assets/fonts/DNFBitBitv2.ttf"]
    fn field_text_leaves_arrow_rows_and_side_margins_empty() {
        for line in [ALERT_BOTTOM_LINE, FIELD_BOTTOM_LINE] {
            let shades = field_shades(&font(), line).unwrap();
            assert_eq!(shades.len(), FIELD_SIZE * FIELD_TEXT_ROWS);
            let last_row = &shades[(FIELD_TEXT_ROWS - 1) * FIELD_SIZE..];
            assert!(last_row.iter().all(|&shade| shade == Shade::Empty));
            let occupied: Vec<usize> = (0..FIELD_SIZE)
                .filter(|&x| (0..FIELD_TEXT_ROWS).any(|y| shades[y * FIELD_SIZE + x] != Shade::Empty))
                .collect();
            let (left, right) = (occupied[0], *occupied.last().unwrap());
            assert!((left as isize - (FIELD_SIZE - 1 - right) as isize).abs() <= 1, "{line}: {left}..{right}");
        }
    }

    #[test]
    #[ignore = "requires assets/fonts/DNFBitBitv2.ttf"]
    fn every_stroke_pixel_is_surrounded_by_outline_or_stroke() {
        let shades = battle_shades(&font()).unwrap();
        for y in 0..BATTLE_HEIGHT {
            for x in 0..BATTLE_WIDTH {
                if matches!(shades[y * BATTLE_WIDTH + x], Shade::Empty | Shade::Outline) {
                    continue;
                }
                for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    assert!(nx >= 0 && ny >= 0 && (nx as usize) < BATTLE_WIDTH && (ny as usize) < BATTLE_HEIGHT);
                    assert_ne!(shades[ny as usize * BATTLE_WIDTH + nx as usize], Shade::Empty);
                }
            }
        }
    }
}
