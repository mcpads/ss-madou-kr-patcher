//! Deterministic compiler for the Sega Saturn Korean title logo.
//!
//! The decompressed `TITLELOG.SPR` payload is a headerless 296x120 linear 8bpp
//! VDP1 texture; the disc file itself is CNX-compressed. Its 256-color RGB555
//! palette is stored separately in `TITLE.SEQ` at offset 0x41B0 and loaded into
//! CRAM entries 0x100..0x1FF.

use sha2::{Digest, Sha256};

pub const SPRITE_WIDTH: usize = 296;
pub const SPRITE_HEIGHT: usize = 120;
pub const SPRITE_BYTES: usize = SPRITE_WIDTH * SPRITE_HEIGHT;
pub const TITLE_SEQ_PALETTE_OFFSET: usize = 0x41B0;
pub const PALETTE_BYTES: usize = 256 * 2;

pub const ORIGINAL_SPR_SHA256: &str =
    "e92d89782c35873ea88a751b5941a2f9ceb630e0619a858823539fb40a76830e";
pub const PALETTE_SHA256: &str = "0e383c30de0e133ab668ea44dc00ad6cddaea07b9185ae8daf8a986090418915";
pub const MASTER_SHA256: &str = "e7009dea86c80965440488d28fdff502934e5f16b92df82b60870283d8c3a958";

const MASTER_WIDTH: usize = 850;
const MASTER_HEIGHT: usize = 365;
const MASTER_BOUNDS: Bounds = Bounds {
    x: 33,
    y: 50,
    width: 783,
    height: 290,
};
const ALPHA_THRESHOLD: u8 = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bounds {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

/// Compile the admitted RGBA master to raw `TITLELOG.SPR` bytes.
pub fn compile(master_png: &[u8], palette_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_hash(master_png, MASTER_SHA256, "title-logo master")?;
    verify_palette(palette_bytes)?;

    let rgba = decode_master(master_png)?;
    let bounds = detect_alpha_bounds(&rgba, MASTER_WIDTH, MASTER_HEIGHT)?;
    if bounds != MASTER_BOUNDS {
        return Err(format!(
            "title-logo master alpha bounds drifted from {MASTER_BOUNDS:?} to {bounds:?}"
        ));
    }

    let palette = decode_palette(palette_bytes)?;
    reduce_to_indexed(&rgba, bounds, &palette)
}

/// Verify the supported JP source texture before replacing it.
pub fn verify_original_sprite(original: &[u8]) -> Result<(), String> {
    if original.len() != SPRITE_BYTES {
        return Err(format!(
            "TITLELOG.SPR is {} bytes, expected {SPRITE_BYTES}",
            original.len()
        ));
    }
    verify_hash(original, ORIGINAL_SPR_SHA256, "original TITLELOG.SPR")
}

/// Render indexed sprite bytes with the runtime RGB555 palette for previews.
pub fn render_rgba(indexed: &[u8], palette_bytes: &[u8]) -> Result<Vec<u8>, String> {
    if indexed.len() != SPRITE_BYTES {
        return Err(format!(
            "indexed title logo is {} bytes, expected {SPRITE_BYTES}",
            indexed.len()
        ));
    }
    let palette = decode_palette(palette_bytes)?;
    let mut rgba = Vec::with_capacity(SPRITE_BYTES * 4);
    for &index in indexed {
        let color = palette[index as usize];
        rgba.extend_from_slice(&[color.r, color.g, color.b, if index == 0 { 0 } else { 0xFF }]);
    }
    Ok(rgba)
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

fn verify_palette(palette_bytes: &[u8]) -> Result<(), String> {
    if palette_bytes.len() != PALETTE_BYTES {
        return Err(format!(
            "title palette is {} bytes, expected {PALETTE_BYTES}",
            palette_bytes.len()
        ));
    }
    verify_hash(palette_bytes, PALETTE_SHA256, "TITLE.SEQ title palette")
}

fn decode_master(master_png: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(master_png));
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to read title-logo PNG header: {error}"))?;
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "title-logo PNG output is too large".to_string())?;
    let mut output = vec![0u8; output_size];
    let info = reader
        .next_frame(&mut output)
        .map_err(|error| format!("failed to decode title-logo PNG: {error}"))?;
    if info.width as usize != MASTER_WIDTH
        || info.height as usize != MASTER_HEIGHT
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err(format!(
            "title-logo master must be {MASTER_WIDTH}x{MASTER_HEIGHT} RGBA8"
        ));
    }
    output.truncate(info.buffer_size());
    Ok(output)
}

fn detect_alpha_bounds(rgba: &[u8], width: usize, height: usize) -> Result<Bounds, String> {
    if rgba.len() != width * height * 4 {
        return Err("title-logo RGBA buffer length is invalid".to_string());
    }
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            if rgba[(y * width + x) * 4 + 3] == 0 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !found {
        return Err("title-logo master has no non-transparent pixels".to_string());
    }
    Ok(Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}

fn decode_palette(bytes: &[u8]) -> Result<[Rgb; 256], String> {
    if bytes.len() != PALETTE_BYTES {
        return Err(format!(
            "title palette is {} bytes, expected {PALETTE_BYTES}",
            bytes.len()
        ));
    }
    let mut palette = [Rgb::default(); 256];
    for (index, color) in palette.iter_mut().enumerate() {
        let offset = index * 2;
        let word = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        *color = Rgb {
            r: ((word & 0x1F) as u32 * 255 / 31) as u8,
            g: (((word >> 5) & 0x1F) as u32 * 255 / 31) as u8,
            b: (((word >> 10) & 0x1F) as u32 * 255 / 31) as u8,
        };
    }
    Ok(palette)
}

fn fit_bounds(source: Bounds) -> (usize, usize) {
    if source.width * SPRITE_HEIGHT <= source.height * SPRITE_WIDTH {
        let height = SPRITE_HEIGHT;
        let width = (source.width * height + source.height / 2) / source.height;
        (width, height)
    } else {
        let width = SPRITE_WIDTH;
        let height = (source.height * width + source.width / 2) / source.width;
        (width, height)
    }
}

fn reduce_to_indexed(rgba: &[u8], bounds: Bounds, palette: &[Rgb; 256]) -> Result<Vec<u8>, String> {
    let (fit_width, fit_height) = fit_bounds(bounds);
    if fit_width == 0 || fit_height == 0 || fit_width > SPRITE_WIDTH || fit_height > SPRITE_HEIGHT {
        return Err("title-logo master does not fit the Saturn surface".to_string());
    }
    let target_x = (SPRITE_WIDTH - fit_width) / 2;
    let target_y = (SPRITE_HEIGHT - fit_height) / 2;
    let mut output = vec![0u8; SPRITE_BYTES];

    for output_y in 0..fit_height {
        for output_x in 0..fit_width {
            let source_x0 = bounds.x + output_x * bounds.width / fit_width;
            let source_x1 = bounds.x + (output_x + 1) * bounds.width / fit_width;
            let source_y0 = bounds.y + output_y * bounds.height / fit_height;
            let source_y1 = bounds.y + (output_y + 1) * bounds.height / fit_height;
            if source_x0 >= source_x1 || source_y0 >= source_y1 {
                return Err("title-logo area reduction produced an empty sample".to_string());
            }

            let mut weighted_rgb = [0u64; 3];
            let mut alpha_sum = 0u64;
            let mut samples = 0u64;
            for source_y in source_y0..source_y1 {
                for source_x in source_x0..source_x1 {
                    let offset = (source_y * MASTER_WIDTH + source_x) * 4;
                    let alpha = rgba[offset + 3] as u64;
                    alpha_sum += alpha;
                    weighted_rgb[0] += rgba[offset] as u64 * alpha;
                    weighted_rgb[1] += rgba[offset + 1] as u64 * alpha;
                    weighted_rgb[2] += rgba[offset + 2] as u64 * alpha;
                    samples += 1;
                }
            }
            if alpha_sum < samples * ALPHA_THRESHOLD as u64 {
                continue;
            }
            let averaged = Rgb {
                r: ((weighted_rgb[0] + alpha_sum / 2) / alpha_sum) as u8,
                g: ((weighted_rgb[1] + alpha_sum / 2) / alpha_sum) as u8,
                b: ((weighted_rgb[2] + alpha_sum / 2) / alpha_sum) as u8,
            };
            let palette_index = nearest_opaque_palette_index(averaged, palette);
            output[(target_y + output_y) * SPRITE_WIDTH + target_x + output_x] =
                palette_index as u8;
        }
    }
    Ok(output)
}

fn nearest_opaque_palette_index(color: Rgb, palette: &[Rgb; 256]) -> usize {
    (1..256)
        .min_by_key(|&index| {
            let candidate = palette[index];
            let dr = color.r as i32 - candidate.r as i32;
            let dg = color.g as i32 - candidate.g as i32;
            let db = color.b as i32 - candidate.b as i32;
            (dr * dr + dg * dg + db * db) as u32
        })
        .expect("the 256-color palette always has opaque entries")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_preview_uses_alpha_only_for_transparent_index() {
        let mut palette = vec![0u8; PALETTE_BYTES];
        palette[2..4].copy_from_slice(&0x7FFFu16.to_be_bytes());
        let mut sprite = vec![0u8; SPRITE_BYTES];
        sprite[1] = 1;
        let rgba = render_rgba(&sprite, &palette).unwrap();
        assert_eq!(rgba[3], 0);
        assert_eq!(rgba[7], 0xFF);
    }

    #[test]
    fn source_contracts_fail_closed() {
        assert!(verify_original_sprite(&vec![0; SPRITE_BYTES]).is_err());
        assert!(verify_palette(&[0; PALETTE_BYTES - 1]).is_err());
        assert!(verify_palette(&[0; PALETTE_BYTES]).is_err());
        assert!(compile(&[], &[0; PALETTE_BYTES]).is_err());
    }
}
