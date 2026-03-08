use super::*;

#[test]
fn render_empty_tiles() {
    let tiles: Vec<DecodedTile> = Vec::new();
    let config = GridConfig::default();
    let data = render_grid_png(&tiles, &config);
    assert!(data.is_empty());
}

#[test]
fn render_single_tile_produces_png() {
    let tile = DecodedTile {
        width: 8,
        height: 8,
        pixels: vec![128; 64],
    };
    let config = GridConfig {
        cols: 1,
        scale: 1,
        padding: 0,
        bg_color: 0,
    };
    let data = render_grid_png(&[tile], &config);
    // Check PNG magic bytes
    assert_eq!(&data[..4], b"\x89PNG");
}

#[test]
fn render_grid_with_labels_produces_png() {
    let tiles: Vec<DecodedTile> = (0..8)
        .map(|_| DecodedTile {
            width: 16,
            height: 16,
            pixels: vec![128; 256],
        })
        .collect();
    let config = GridConfig {
        cols: 4,
        scale: 3,
        padding: 1,
        bg_color: 32,
    };
    let data = render_grid_png_with_labels(&tiles, &config, 64);
    assert!(!data.is_empty());
    assert_eq!(&data[..4], b"\x89PNG");
}

#[test]
fn render_grid_with_labels_empty() {
    let tiles: Vec<DecodedTile> = Vec::new();
    let config = GridConfig::default();
    let data = render_grid_png_with_labels(&tiles, &config, 0);
    assert!(data.is_empty());
}

#[test]
fn draw_number_renders_digits() {
    // Simple smoke test: draw "42" into a small buffer and verify some pixels are set
    let mut img = vec![0u8; 100 * 20];
    let width = draw_number(&mut img, 100, 0, 0, 42, 1, 255);
    // "42" = 2 digits, each 3+1=4 px wide at scale 1 => 8px total
    assert_eq!(width, 8);
    // At least some pixels should be non-zero
    assert!(img.iter().any(|&p| p == 255));
}

#[test]
fn render_grid_dimensions() {
    let tiles: Vec<DecodedTile> = (0..4)
        .map(|_| DecodedTile {
            width: 8,
            height: 8,
            pixels: vec![0; 64],
        })
        .collect();
    let config = GridConfig {
        cols: 2,
        scale: 1,
        padding: 0,
        bg_color: 0,
    };
    let data = render_grid_png(&tiles, &config);
    assert!(!data.is_empty());
    // PNG should be valid (starts with magic)
    assert_eq!(&data[..4], b"\x89PNG");
}
