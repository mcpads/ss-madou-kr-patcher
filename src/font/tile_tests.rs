use super::*;

#[test]
fn decode_4bpp_8x8() {
    let format = TileFormat {
        width: 8,
        height: 8,
        bpp: 4,
    };
    assert_eq!(format.tile_bytes(), 32);

    // All zeros → all black pixels
    let data = vec![0u8; 32];
    let tile = decode_tile(&data, format);
    assert_eq!(tile.pixels.len(), 64);
    assert!(tile.pixels.iter().all(|&p| p == 0));
}

#[test]
fn decode_4bpp_max_value() {
    let format = TileFormat {
        width: 8,
        height: 8,
        bpp: 4,
    };
    // All 0xFF → all pixels max (both nibbles = 0xF → 255)
    let data = vec![0xFF; 32];
    let tile = decode_tile(&data, format);
    assert!(tile.pixels.iter().all(|&p| p == 255));
}

#[test]
fn decode_1bpp_8x8() {
    let format = TileFormat {
        width: 8,
        height: 8,
        bpp: 1,
    };
    assert_eq!(format.tile_bytes(), 8);

    let mut data = vec![0u8; 8];
    data[0] = 0b10000001; // first and last pixel on
    let tile = decode_tile(&data, format);
    assert_eq!(tile.pixels[0], 255);
    assert_eq!(tile.pixels[1], 0);
    assert_eq!(tile.pixels[7], 255);
}

#[test]
fn decode_tiles_count() {
    let format = TileFormat {
        width: 8,
        height: 8,
        bpp: 4,
    };
    let data = vec![0u8; 32 * 10];
    let tiles = decode_tiles(&data, format, 10);
    assert_eq!(tiles.len(), 10);
}
