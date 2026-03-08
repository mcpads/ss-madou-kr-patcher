/// Tile format configuration.
#[derive(Debug, Clone, Copy)]
pub struct TileFormat {
    pub width: usize,
    pub height: usize,
    /// Bits per pixel (1, 4, or 8).
    pub bpp: usize,
}

impl TileFormat {
    /// Byte size of a single tile.
    pub fn tile_bytes(&self) -> usize {
        self.width * self.height * self.bpp / 8
    }
}

/// A decoded tile as a grayscale pixel array.
pub struct DecodedTile {
    pub width: usize,
    pub height: usize,
    /// Grayscale pixel values (0-255), row-major order.
    pub pixels: Vec<u8>,
}

/// Decode a single tile from raw bytes.
pub fn decode_tile(data: &[u8], format: TileFormat) -> DecodedTile {
    let pixel_count = format.width * format.height;
    let mut pixels = Vec::with_capacity(pixel_count);
    let max_val = (1u16 << format.bpp) - 1;

    match format.bpp {
        1 => {
            for &byte in data.iter().take(format.tile_bytes()) {
                for bit in (0..8).rev() {
                    if pixels.len() >= pixel_count {
                        break;
                    }
                    let val = (byte >> bit) & 1;
                    pixels.push(if val != 0 { 255 } else { 0 });
                }
            }
        }
        4 => {
            for &byte in data.iter().take(format.tile_bytes()) {
                if pixels.len() >= pixel_count {
                    break;
                }
                let hi = (byte >> 4) & 0x0F;
                let lo = byte & 0x0F;
                pixels.push((hi as u16 * 255 / max_val) as u8);
                if pixels.len() < pixel_count {
                    pixels.push((lo as u16 * 255 / max_val) as u8);
                }
            }
        }
        8 => {
            for &byte in data.iter().take(format.tile_bytes()) {
                if pixels.len() >= pixel_count {
                    break;
                }
                pixels.push(byte);
            }
        }
        _ => {
            // Unsupported bpp, fill with zeros
            pixels.resize(pixel_count, 0);
        }
    }

    // Pad if data was short
    pixels.resize(pixel_count, 0);

    DecodedTile {
        width: format.width,
        height: format.height,
        pixels,
    }
}

/// Decode multiple tiles from sequential data.
pub fn decode_tiles(data: &[u8], format: TileFormat, count: usize) -> Vec<DecodedTile> {
    let tile_size = format.tile_bytes();
    let mut tiles = Vec::with_capacity(count);

    for i in 0..count {
        let offset = i * tile_size;
        if offset + tile_size > data.len() {
            break;
        }
        tiles.push(decode_tile(&data[offset..offset + tile_size], format));
    }

    tiles
}

#[cfg(test)]
#[path = "tile_tests.rs"]
mod tests;
