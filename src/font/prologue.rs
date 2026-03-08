//! Prologue sprite renderer (OP_SP02.SPR replacement).
//!
//! Renders Korean prologue scroll text as a 288×208 4bpp bitmap with
//! Saturn 15-bit RGB palette header, matching the original OP_SP02.SPR format.

use fontdue::Font;

/// Sprite dimensions (matching original OP_SP02.SPR).
pub const SPRITE_WIDTH: usize = 288;
pub const SPRITE_HEIGHT: usize = 208;

/// Saturn 15-bit RGB palette (from original OP_SP02.SPR).
/// Index 0 = VDP1 transparent key color (magenta).
const PALETTE: [(u8, u8, u8); 16] = [
    (184, 88, 184), // 0: transparent
    (0, 0, 0),      // 1: black
    (16, 16, 16),   // 2
    (32, 32, 32),   // 3
    (64, 64, 64),   // 4
    (80, 80, 80),   // 5
    (96, 96, 96),   // 6
    (112, 112, 112), // 7
    (120, 128, 128), // 8
    (152, 152, 152), // 9
    (168, 168, 168), // 10
    (184, 184, 184), // 11
    (200, 200, 200), // 12
    (216, 216, 216), // 13
    (232, 232, 232), // 14
    (248, 248, 248), // 15: brightest white
];

/// Prologue text — two sets separated by a blank line.
const SET1: &[&str] = &[
    "한 명의 용사가 있었다",
    "그는 금색 투의를 두르고",
    "수수께끼의 대괴구와 싸웠다",
    "아득한 차원의 저편에서",
    "무한히 쏟아지는 그 괴구에",
    "수많은 궁지에 몰리면서도",
    "불굴의 투지로 이를 물리쳤다",
    "격전 끝에···",
    "하나로 집속한 그 사악한 존재를",
    "신이 남긴 옛 유산",
    "「대봉인탑」에 봉인하였다",
];

const SET2: &[&str] = &[
    "살아 있는 모든 것의 희망을 엮어",
    "용사는 「대봉인탑」으로 향했다",
];

/// Measure the pixel width of a string using fontdue metrics.
fn measure_text(font: &Font, text: &str, font_size: f32) -> f32 {
    let mut width = 0.0f32;
    for ch in text.chars() {
        let metrics = font.metrics(ch, font_size);
        width += metrics.advance_width;
    }
    width
}

/// Render a line of text onto a grayscale canvas at (x, y).
fn render_line(
    canvas: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    font: &Font,
    text: &str,
    font_size: f32,
    x_start: f32,
    y_baseline: f32,
) {
    let mut cursor_x = x_start;

    for ch in text.chars() {
        let (metrics, raster) = font.rasterize(ch, font_size);

        // Position relative to baseline
        let gx = (cursor_x + metrics.xmin as f32).round() as i32;
        let gy = (y_baseline - metrics.height as f32 - metrics.ymin as f32).round() as i32;

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px >= 0 && (px as usize) < canvas_w && py >= 0 && (py as usize) < canvas_h {
                    let coverage = raster[row * metrics.width + col];
                    if coverage > 0 {
                        let idx = py as usize * canvas_w + px as usize;
                        // Alpha-composite (max blend for overlapping glyphs)
                        canvas[idx] = canvas[idx].max(coverage);
                    }
                }
            }
        }

        cursor_x += metrics.advance_width;
    }
}

/// Render the full prologue text onto a 288×208 grayscale canvas.
///
/// The original sprite packs all 13 text lines at 16px each (13×16=208).
/// There is NO blank separator line in the sprite data — the game engine
/// handles paragraph grouping during scroll display.
fn render_canvas(font: &Font, font_size: f32) -> Vec<u8> {
    let mut canvas = vec![0u8; SPRITE_WIDTH * SPRITE_HEIGHT];

    // Match original: 13 lines × 16px = 208px (exact fit)
    let total_lines = (SET1.len() + SET2.len()) as i32; // 13
    let line_h = SPRITE_HEIGHT as i32 / total_lines;    // 16

    let ascent = font
        .horizontal_line_metrics(font_size)
        .map(|lm| lm.ascent)
        .unwrap_or(font_size * 0.8);

    let mut y = 0i32;

    for &line in SET1.iter().chain(SET2.iter()) {
        let text_w = measure_text(font, line, font_size);
        let x = ((SPRITE_WIDTH as f32 - text_w) / 2.0).max(0.0);
        let baseline = y as f32 + ascent;
        render_line(&mut canvas, SPRITE_WIDTH, SPRITE_HEIGHT, font, line, font_size, x, baseline);
        y += line_h;
    }

    canvas
}

/// Convert grayscale coverage to 4-bit palette index.
/// Maps 0 → 0 (transparent), 1..255 → 1..15 (grayscale).
fn coverage_to_palette(coverage: u8) -> u8 {
    if coverage < 8 {
        0 // transparent
    } else {
        // Map 8..255 to 1..15
        let idx = (coverage as u16 * 14 / 255 + 1).min(15) as u8;
        idx
    }
}

/// Convert 8-bit RGB to Saturn 15-bit with MSB set (VDP1 CLUT format).
///
/// Saturn VDP1 color table entries use bit 15 as a control bit.
/// The original OP_SP02.SPR has MSB=1 on all palette entries.
fn rgb_to_saturn15(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g5 = (g >> 3) as u16;
    let b5 = (b >> 3) as u16;
    0x8000 | r5 | (g5 << 5) | (b5 << 10)
}

/// Apply 1px outline (3×3 max dilation) to the grayscale canvas.
///
/// Returns a new canvas where:
/// - Text pixels keep their original brightness
/// - Outline pixels (dilated but not original) get a dark value
fn apply_outline(canvas: &[u8], w: usize, h: usize, outline_value: u8) -> Vec<u8> {
    let mut dilated = vec![0u8; w * h];

    // 3×3 max filter (dilation)
    for y in 0..h {
        for x in 0..w {
            let mut max_val = 0u8;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ny = y as i32 + dy;
                    let nx = x as i32 + dx;
                    if ny >= 0 && (ny as usize) < h && nx >= 0 && (nx as usize) < w {
                        max_val = max_val.max(canvas[ny as usize * w + nx as usize]);
                    }
                }
            }
            dilated[y * w + x] = max_val;
        }
    }

    // Composite: text pixels stay bright, outline-only pixels get dark value
    let mut result = vec![0u8; w * h];
    for i in 0..w * h {
        if canvas[i] > 30 {
            result[i] = canvas[i]; // original text
        } else if dilated[i] > 30 {
            result[i] = outline_value; // outline
        }
    }
    result
}

/// Render Korean prologue and return raw SPR data (32-byte palette + 4bpp pixels).
///
/// The output is ready to be CNX-compressed and written to OP_SP02.SPR on disc.
pub fn render_prologue_sprite(font: &Font, font_size: f32) -> Vec<u8> {
    let text_canvas = render_canvas(font, font_size);

    // Apply 1px dark outline around text (value 80 ≈ palette index 4-5)
    let canvas = apply_outline(&text_canvas, SPRITE_WIDTH, SPRITE_HEIGHT, 80);

    // Build Saturn palette header (32 bytes = 16 × 2-byte BE words)
    let mut data = Vec::with_capacity(32 + SPRITE_WIDTH * SPRITE_HEIGHT / 2);
    for &(r, g, b) in &PALETTE {
        let val = rgb_to_saturn15(r, g, b);
        data.push((val >> 8) as u8);
        data.push((val & 0xFF) as u8);
    }

    // Pack grayscale canvas to 4bpp (2 pixels per byte, high nibble first)
    for y in 0..SPRITE_HEIGHT {
        for x in (0..SPRITE_WIDTH).step_by(2) {
            let hi = coverage_to_palette(canvas[y * SPRITE_WIDTH + x]);
            let lo = if x + 1 < SPRITE_WIDTH {
                coverage_to_palette(canvas[y * SPRITE_WIDTH + x + 1])
            } else {
                0
            };
            data.push((hi << 4) | lo);
        }
    }

    data
}
