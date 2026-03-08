use super::tile::DecodedTile;
use std::io::BufWriter;

/// 3x5 pixel bitmaps for digits 0-9 (each digit is 3 columns x 5 rows).
/// Stored as 5 rows of 3-bit patterns (MSB = leftmost pixel).
const DIGIT_BITMAPS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// Draw a single digit at the given position in a grayscale image buffer.
/// `digit_scale` controls how many output pixels per bitmap pixel.
fn draw_digit(
    img: &mut [u8],
    img_w: usize,
    x: usize,
    y: usize,
    digit: u8,
    digit_scale: usize,
    color: u8,
) {
    if digit > 9 {
        return;
    }
    let bitmap = &DIGIT_BITMAPS[digit as usize];
    for row in 0..5 {
        for col in 0..3 {
            let bit = (bitmap[row] >> (2 - col)) & 1;
            if bit == 1 {
                for sy in 0..digit_scale {
                    for sx in 0..digit_scale {
                        let px = x + col * digit_scale + sx;
                        let py = y + row * digit_scale + sy;
                        if px < img_w && py * img_w + px < img.len() {
                            img[py * img_w + px] = color;
                        }
                    }
                }
            }
        }
    }
}

/// Draw a multi-digit number at the given position.
/// Returns the width consumed (in pixels).
fn draw_number(
    img: &mut [u8],
    img_w: usize,
    x: usize,
    y: usize,
    number: usize,
    digit_scale: usize,
    color: u8,
) -> usize {
    let s = number.to_string();
    let char_w = 3 * digit_scale + digit_scale; // 3px digit + 1px gap, all scaled
    for (i, ch) in s.chars().enumerate() {
        let d = ch as u8 - b'0';
        draw_digit(img, img_w, x + i * char_w, y, d, digit_scale, color);
    }
    s.len() * char_w
}

/// Grid rendering configuration.
pub struct GridConfig {
    /// Number of columns in the grid.
    pub cols: usize,
    /// Scale factor for each pixel.
    pub scale: usize,
    /// Padding between tiles (in output pixels).
    pub padding: usize,
    /// Background color (0=black, 255=white).
    pub bg_color: u8,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            cols: 16,
            scale: 2,
            padding: 1,
            bg_color: 0,
        }
    }
}

/// Render tiles into a PNG grid image.
pub fn render_grid_png(tiles: &[DecodedTile], config: &GridConfig) -> Vec<u8> {
    if tiles.is_empty() {
        return Vec::new();
    }

    let tw = tiles[0].width;
    let th = tiles[0].height;
    let stw = tw * config.scale;
    let sth = th * config.scale;

    let cols = config.cols;
    let rows = (tiles.len() + cols - 1) / cols;

    let img_w = cols * stw + (cols + 1) * config.padding;
    let img_h = rows * sth + (rows + 1) * config.padding;

    // Create grayscale image buffer
    let mut img = vec![config.bg_color; img_w * img_h];

    for (idx, tile) in tiles.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let base_x = config.padding + col * (stw + config.padding);
        let base_y = config.padding + row * (sth + config.padding);

        for ty in 0..th {
            for tx in 0..tw {
                let pixel = tile.pixels[ty * tw + tx];
                for sy in 0..config.scale {
                    for sx in 0..config.scale {
                        let ox = base_x + tx * config.scale + sx;
                        let oy = base_y + ty * config.scale + sy;
                        img[oy * img_w + ox] = pixel;
                    }
                }
            }
        }
    }

    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let w = BufWriter::new(&mut png_data);
        let mut encoder = png::Encoder::new(w, img_w as u32, img_h as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer.write_image_data(&img).expect("PNG data write failed");
    }

    png_data
}

/// Render tiles into a PNG grid image with row index labels in a left margin.
///
/// `start_index` is the glyph index of the first tile in `tiles`, used for
/// computing the label shown at the start of each row.
pub fn render_grid_png_with_labels(
    tiles: &[DecodedTile],
    config: &GridConfig,
    start_index: usize,
) -> Vec<u8> {
    render_grid_png_labeled(tiles, config, start_index, false, 1)
}

/// Render tiles into a PNG grid with per-tile index labels above each cell.
///
/// Each tile gets a small number showing its absolute index (`start_index + i * stride`).
/// Use `stride = 4` for 2x2 combined tiles to show original tile indices.
pub fn render_grid_png_indexed(
    tiles: &[DecodedTile],
    config: &GridConfig,
    start_index: usize,
    stride: usize,
) -> Vec<u8> {
    render_grid_png_labeled(tiles, config, start_index, true, stride)
}

/// Internal: render grid with either row labels or per-tile labels.
/// `stride` controls how much the index increments per tile (e.g., 4 for 2x2 combined).
fn render_grid_png_labeled(
    tiles: &[DecodedTile],
    config: &GridConfig,
    start_index: usize,
    per_tile: bool,
    stride: usize,
) -> Vec<u8> {
    if tiles.is_empty() {
        return Vec::new();
    }

    let tw = tiles[0].width;
    let th = tiles[0].height;
    let stw = tw * config.scale;
    let sth = th * config.scale;

    let cols = config.cols;
    let rows = (tiles.len() + cols - 1) / cols;

    let digit_scale = config.scale.max(1);
    // Label area height for per-tile mode (5 rows of pixels * scale + gap)
    let label_h = if per_tile { 5 * digit_scale + digit_scale } else { 0 };
    // Left margin for row-label mode
    let left_margin = if per_tile { 0 } else { 4 * 4 * digit_scale + 4 * digit_scale };

    let cell_h = sth + label_h;
    let grid_w = cols * stw + (cols + 1) * config.padding;
    let grid_h = rows * cell_h + (rows + 1) * config.padding;

    let img_w = left_margin + grid_w;
    let img_h = grid_h;

    let mut img = vec![config.bg_color; img_w * img_h];

    let label_color = if config.bg_color > 127 { 0u8 } else { 255u8 };

    if !per_tile {
        // Row labels in left margin
        for row in 0..rows {
            let glyph_idx = start_index + row * cols * stride;
            let label_y = config.padding + row * (sth + config.padding);
            let label_y_centered = label_y + (sth.saturating_sub(5 * digit_scale)) / 2;
            draw_number(
                &mut img, img_w,
                digit_scale, label_y_centered,
                glyph_idx, digit_scale, label_color,
            );
        }
    }

    for (idx, tile) in tiles.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let base_x = left_margin + config.padding + col * (stw + config.padding);
        let base_y = config.padding + row * (cell_h + config.padding);

        if per_tile {
            // Draw tile index above the tile
            let tile_idx = start_index + idx * stride;
            draw_number(
                &mut img, img_w,
                base_x, base_y,
                tile_idx, digit_scale.min(2), label_color,
            );
        }

        let tile_y = base_y + label_h;

        for ty in 0..th {
            for tx in 0..tw {
                let pixel = tile.pixels[ty * tw + tx];
                for sy in 0..config.scale {
                    for sx in 0..config.scale {
                        let ox = base_x + tx * config.scale + sx;
                        let oy = tile_y + ty * config.scale + sy;
                        if ox < img_w && oy < img_h {
                            img[oy * img_w + ox] = pixel;
                        }
                    }
                }
            }
        }
    }

    let mut png_data = Vec::new();
    {
        let w = BufWriter::new(&mut png_data);
        let mut encoder = png::Encoder::new(w, img_w as u32, img_h as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer.write_image_data(&img).expect("PNG data write failed");
    }

    png_data
}

/// Configuration for individual tile PNG export.
pub struct TileExportConfig {
    pub scale: usize,
    /// Invert colors (bright-on-dark → dark-on-bright).
    pub invert: bool,
    /// Padding around the glyph in scaled pixels.
    pub padding: usize,
}

impl Default for TileExportConfig {
    fn default() -> Self {
        Self {
            scale: 2,
            invert: false,
            padding: 0,
        }
    }
}

/// Render a single tile as a PNG image.
pub fn render_tile_png(tile: &DecodedTile, config: &TileExportConfig) -> Vec<u8> {
    let stw = tile.width * config.scale;
    let sth = tile.height * config.scale;
    let w = stw + config.padding * 2;
    let h = sth + config.padding * 2;
    let bg = if config.invert { 255u8 } else { 0u8 };
    let mut img = vec![bg; w * h];

    for ty in 0..tile.height {
        for tx in 0..tile.width {
            let mut pixel = tile.pixels[ty * tile.width + tx];
            if config.invert {
                pixel = 255 - pixel;
            }
            for sy in 0..config.scale {
                for sx in 0..config.scale {
                    let ox = config.padding + tx * config.scale + sx;
                    let oy = config.padding + ty * config.scale + sy;
                    img[oy * w + ox] = pixel;
                }
            }
        }
    }

    let mut png_data = Vec::new();
    {
        let bw = BufWriter::new(&mut png_data);
        let mut encoder = png::Encoder::new(bw, w as u32, h as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer.write_image_data(&img).expect("PNG data write failed");
    }

    png_data
}

/// Combine consecutive 8×8 tiles into larger glyphs with arbitrary WxH layout.
///
/// `cols` = tiles across, `rows` = tiles down. Tiles are arranged row-major
/// (left-to-right, top-to-bottom). E.g. 2×2 = TL,TR,BL,BR; 4×1 = L1,L2,L3,L4.
pub fn combine_nxm(tiles: &[DecodedTile], cols: usize, rows: usize) -> Vec<DecodedTile> {
    let per = cols * rows;
    let mut combined = Vec::new();
    for chunk in tiles.chunks(per) {
        if chunk.len() < per {
            break;
        }
        let tw = chunk[0].width;
        let th = chunk[0].height;
        let cw = tw * cols;
        let ch = th * rows;
        let mut pixels = vec![0u8; cw * ch];
        for (i, tile) in chunk.iter().enumerate() {
            let tc = i % cols;
            let tr = i / cols;
            for y in 0..th {
                for x in 0..tw {
                    pixels[(tr * th + y) * cw + (tc * tw + x)] = tile.pixels[y * tw + x];
                }
            }
        }
        combined.push(DecodedTile {
            width: cw,
            height: ch,
            pixels,
        });
    }
    combined
}

/// Combine groups of 4 consecutive 8x8 tiles into 16x16 tiles (2x2 arrangement).
///
/// Layout per glyph: tile[0]=top-left, tile[1]=top-right, tile[2]=bottom-left, tile[3]=bottom-right.
/// Combine pairs of 8×8 tiles into 8×16 tiles (top-bottom).
///
/// Used for tall characters in FONT.CEL: each char is two
/// consecutive 8×8 tiles arranged vertically (top, bottom).
pub fn combine_2x1(tiles: &[DecodedTile]) -> Vec<DecodedTile> {
    let mut combined = Vec::new();
    for chunk in tiles.chunks(2) {
        if chunk.len() < 2 {
            break;
        }
        let tw = chunk[0].width;
        let th = chunk[0].height;
        let ch = th * 2;
        let mut pixels = vec![0u8; tw * ch];
        // Top
        for y in 0..th {
            for x in 0..tw {
                pixels[y * tw + x] = chunk[0].pixels[y * tw + x];
            }
        }
        // Bottom
        for y in 0..th {
            for x in 0..tw {
                pixels[(y + th) * tw + x] = chunk[1].pixels[y * tw + x];
            }
        }
        combined.push(DecodedTile {
            width: tw,
            height: ch,
            pixels,
        });
    }
    combined
}

/// Combine pairs of 8×8 tiles into 16×8 tiles (left-right).
///
/// Used for wide characters in FONT.CEL: each wide char is two
/// consecutive 8×8 tiles arranged horizontally.
pub fn combine_1x2(tiles: &[DecodedTile]) -> Vec<DecodedTile> {
    let mut combined = Vec::new();
    for chunk in tiles.chunks(2) {
        if chunk.len() < 2 {
            break;
        }
        let tw = chunk[0].width;
        let th = chunk[0].height;
        let cw = tw * 2;
        let mut pixels = vec![0u8; cw * th];
        // Left
        for y in 0..th {
            for x in 0..tw {
                pixels[y * cw + x] = chunk[0].pixels[y * tw + x];
            }
        }
        // Right
        for y in 0..th {
            for x in 0..tw {
                pixels[y * cw + (x + tw)] = chunk[1].pixels[y * tw + x];
            }
        }
        combined.push(DecodedTile {
            width: cw,
            height: th,
            pixels,
        });
    }
    combined
}

pub fn combine_2x2(tiles: &[DecodedTile]) -> Vec<DecodedTile> {
    let mut combined = Vec::new();

    for chunk in tiles.chunks(4) {
        if chunk.len() < 4 {
            break;
        }

        let tw = chunk[0].width;
        let th = chunk[0].height;
        let cw = tw * 2;
        let ch = th * 2;
        let mut pixels = vec![0u8; cw * ch];

        // Top-left
        for y in 0..th {
            for x in 0..tw {
                pixels[y * cw + x] = chunk[0].pixels[y * tw + x];
            }
        }
        // Top-right
        for y in 0..th {
            for x in 0..tw {
                pixels[y * cw + (x + tw)] = chunk[1].pixels[y * tw + x];
            }
        }
        // Bottom-left
        for y in 0..th {
            for x in 0..tw {
                pixels[(y + th) * cw + x] = chunk[2].pixels[y * tw + x];
            }
        }
        // Bottom-right
        for y in 0..th {
            for x in 0..tw {
                pixels[(y + th) * cw + (x + tw)] = chunk[3].pixels[y * tw + x];
            }
        }

        combined.push(DecodedTile {
            width: cw,
            height: ch,
            pixels,
        });
    }

    combined
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
