use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use ss_madou::font;

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_font_dump(
    input: &Path,
    output: &Path,
    tile_width: usize,
    tile_height: usize,
    bpp: usize,
    cols: usize,
    scale: usize,
    skip: usize,
    count: usize,
    do_combine_2x2: bool,
    do_combine_1x2: bool,
    do_combine_2x1: bool,
    combine_wxh: Option<&str>,
    batch_size: usize,
    indexed: bool,
    label_start: Option<usize>,
) -> Result<()> {
    let data = fs::read(input).context("Failed to read input file")?;

    let format = font::TileFormat {
        width: tile_width,
        height: tile_height,
        bpp,
    };

    let tile_bytes = format.tile_bytes();
    let max_tiles = data.len() / tile_bytes;

    let start = skip.min(max_tiles);
    let num_tiles = if count == 0 {
        max_tiles - start
    } else {
        count.min(max_tiles - start)
    };

    let tile_data = &data[start * tile_bytes..];
    let tiles = font::decode_tiles(tile_data, format, num_tiles);

    // Parse --combine WxH (e.g. "4x1", "3x3")
    let parsed_combine: Option<(usize, usize)> = combine_wxh.and_then(|s| {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?))
        } else {
            None
        }
    });

    let render_tiles;
    let effective_tiles: &[font::DecodedTile];
    let tiles_per_glyph: usize;

    if let Some((w, h)) = parsed_combine {
        render_tiles = font::combine_nxm(&tiles, w, h);
        effective_tiles = &render_tiles;
        tiles_per_glyph = w * h;
    } else if do_combine_2x2 {
        render_tiles = font::combine_2x2(&tiles);
        effective_tiles = &render_tiles;
        tiles_per_glyph = 4;
    } else if do_combine_1x2 {
        render_tiles = font::combine_1x2(&tiles);
        effective_tiles = &render_tiles;
        tiles_per_glyph = 2;
    } else if do_combine_2x1 {
        render_tiles = font::combine_2x1(&tiles);
        effective_tiles = &render_tiles;
        tiles_per_glyph = 2;
    } else {
        effective_tiles = &tiles;
        tiles_per_glyph = 1;
    }

    let config = font::GridConfig {
        cols,
        scale,
        padding: 1,
        bg_color: 32,
    };

    if batch_size == 0 {
        // Single file mode (original behavior)
        let png_data = if indexed {
            let (start_idx, stride) = match label_start {
                Some(ls) => (ls, 1),
                None => (skip, tiles_per_glyph),
            };
            font::render_grid_png_indexed(effective_tiles, &config, start_idx, stride)
        } else {
            font::render_grid_png(effective_tiles, &config)
        };

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).context("Failed to create output directory")?;
        }
        fs::write(output, &png_data).context("Failed to write PNG")?;

        println!(
            "Rendered {} glyphs from {} tiles ({}x{} {}bpp, skip={}{}) -> {} ({} bytes)",
            effective_tiles.len(),
            tiles.len(),
            tile_width,
            tile_height,
            bpp,
            skip,
            if do_combine_2x2 { ", 2x2 combined" } else { "" },
            output.display(),
            png_data.len()
        );
    } else {
        // Batch mode: split into multiple files with index labels
        let prefix = output
            .to_string_lossy()
            .strip_suffix(".png")
            .unwrap_or(&output.to_string_lossy())
            .to_string();

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).context("Failed to create output directory")?;
        }

        let total = effective_tiles.len();
        let mut batch_start = 0;
        let mut file_count = 0;

        while batch_start < total {
            let batch_end = (batch_start + batch_size).min(total);
            let batch_tiles = &effective_tiles[batch_start..batch_end];

            let png_data =
                font::render_grid_png_with_labels(batch_tiles, &config, batch_start);

            let filename = format!(
                "{}_{:03}-{:03}.png",
                prefix,
                batch_start,
                batch_end - 1
            );
            fs::write(&filename, &png_data)
                .with_context(|| format!("Failed to write {}", filename))?;

            println!(
                "  Batch: glyphs {:03}-{:03} -> {} ({} bytes)",
                batch_start,
                batch_end - 1,
                filename,
                png_data.len()
            );

            batch_start = batch_end;
            file_count += 1;
        }

        println!(
            "Rendered {} glyphs in {} files ({}x{} {}bpp, skip={}{}, batch={})",
            effective_tiles.len(),
            file_count,
            tile_width,
            tile_height,
            bpp,
            skip,
            if do_combine_2x2 { ", 2x2 combined" } else { "" },
            batch_size
        );
    }

    Ok(())
}


