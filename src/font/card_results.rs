//! Puyo Card result letters. Preserve cells, palettes and SEQ animation.
use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

pub const SOURCE_SHA256: &str = "5a3e52f0a330466bf95836b6e95814cff2cd0b454ae4da132ec63a9fd1954f52";
const FONT_SHA256: &str = "eebf8c20fea14a927e74216f972d6484d8a2398efdea356f6d0c5adcae531743";
/// Offset, width, Korean letter, original palette fill index. All are 48 high.
pub const CELLS: [(usize, usize, char, u8); 4] = [
    (0x31a0, 56, '승', 10),
    (0x36e0, 48, '리', 10),
    (0x8020, 48, '패', 4),
    (0x84a0, 48, '배', 4),
];

fn letter(font: &Font, ch: char, width: usize, fill: u8) -> Result<Vec<u8>, String> {
    if font.lookup_glyph_index(ch) == 0 {
        return Err(format!("missing card glyph {ch}"));
    }
    let (m, raster) = font.rasterize(ch, 40.0);
    let points: Vec<_> = (0..m.height)
        .flat_map(|y| (0..m.width).map(move |x| (x, y)))
        .filter(|&(x, y)| raster[y * m.width + x] >= 110)
        .collect();
    let min_x = points.iter().map(|p| p.0).min().ok_or("empty card glyph")?;
    let max_x = points.iter().map(|p| p.0).max().unwrap();
    let min_y = points.iter().map(|p| p.1).min().unwrap();
    let max_y = points.iter().map(|p| p.1).max().unwrap();
    let mut mask = vec![false; width * 48];
    for y in 0..40 {
        for x in 0..width - 12 {
            let sx = min_x + x * (max_x - min_x + 1) / (width - 12);
            let sy = min_y + y * (max_y - min_y + 1) / 40;
            mask[(y + 4) * width + x + 6] = raster[sy * m.width + sx] >= 110;
        }
    }
    let at = |x: isize, y: isize| {
        x >= 0 && y >= 0 && (x as usize) < width && y < 48 && mask[y as usize * width + x as usize]
    };
    let mut pixels = vec![0u8; width * 48];
    for y in 0..48isize {
        for x in 0..width as isize {
            pixels[y as usize * width + x as usize] = if at(x, y) {
                if !at(x + 1, y) || !at(x, y + 1) {
                    3
                } else {
                    fill
                }
            } else if (-2..=2)
                .any(|dy| (-2..=2).any(|dx| dx * dx + dy * dy <= 5 && at(x + dx, y + dy)))
            {
                1
            } else {
                0
            };
        }
    }
    Ok(pixels.chunks_exact(2).map(|p| p[0] << 4 | p[1]).collect())
}

pub fn compile(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    for (data, expected, name) in [
        (original, SOURCE_SHA256, "P_CARD01.SPR"),
        (font_bytes, FONT_SHA256, "DNFBitBitv2"),
    ] {
        if format!("{:x}", Sha256::digest(data)) != expected {
            return Err(format!("{name} source hash mismatch"));
        }
    }
    let font = Font::from_bytes(font_bytes, FontSettings::default()).map_err(|e| e.to_string())?;
    let mut out = original.to_vec();
    for (offset, width, ch, fill) in CELLS {
        let cell = letter(&font, ch, width, fill)?;
        out[offset..offset + cell.len()].copy_from_slice(&cell);
    }
    Ok(out)
}
