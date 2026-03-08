use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

use ss_madou::compression;
use ss_madou::disc::{DiscImage, Iso9660};

pub(crate) fn cmd_rom_diff(
    rom_a: &Path,
    rom_b: &Path,
    file: Option<&str>,
) -> Result<()> {
    let disc_a =
        DiscImage::from_bin_file(rom_a).context("Failed to open ROM A")?;
    let disc_b =
        DiscImage::from_bin_file(rom_b).context("Failed to open ROM B")?;

    let iso_a = Iso9660::parse(&disc_a).context("Failed to parse ISO 9660 from ROM A")?;
    let iso_b = Iso9660::parse(&disc_b).context("Failed to parse ISO 9660 from ROM B")?;

    if let Some(filename) = file {
        cmd_file_diff(&disc_a, &iso_a, &disc_b, &iso_b, filename)
    } else {
        cmd_sector_diff(rom_a, &disc_a, &iso_a, rom_b, &disc_b, &iso_b)
    }
}

/// Compare two ROMs sector by sector, grouped by file.
fn cmd_sector_diff(
    path_a: &Path,
    disc_a: &DiscImage,
    iso_a: &Iso9660,
    path_b: &Path,
    disc_b: &DiscImage,
    iso_b: &Iso9660,
) -> Result<()> {
    let sectors_a = disc_a.sector_count();
    let sectors_b = disc_b.sector_count();
    println!("=== ROM Diff ===\n");
    println!("ROM A: {} ({} sectors)", path_a.display(), sectors_a);
    println!("ROM B: {} ({} sectors)", path_b.display(), sectors_b);

    let sectors = sectors_a.min(sectors_b);
    if sectors_a != sectors_b {
        println!(
            "WARNING: Different sector counts ({} vs {})",
            sectors_a, sectors_b
        );
    }

    // Build sector→file reverse map from ROM A's ISO.
    let entries_a = iso_a.list_root(disc_a).context("Failed to list ISO A")?;
    let entries_b = iso_b.list_root(disc_b).context("Failed to list ISO B")?;

    let mut sector_to_file: BTreeMap<usize, String> = BTreeMap::new();
    // ISO 9660 root directory sectors
    let root_lba = iso_a.pvd.root_directory_lba as usize;
    let root_sectors =
        (iso_a.pvd.root_directory_size as usize + 2047) / 2048;
    for s in 0..root_sectors {
        sector_to_file
            .insert(root_lba + s, "<ISO 9660 rootdir>".to_string());
    }
    // System area (sectors 0-15)
    for s in 0..16 {
        sector_to_file.insert(s, "<system area>".to_string());
    }
    // PVD sector 16
    sector_to_file.insert(16, "<PVD>".to_string());
    // File data sectors
    for entry in &entries_a {
        if !entry.is_directory {
            let num_sectors = (entry.size as usize + 2047) / 2048;
            for s in 0..num_sectors {
                sector_to_file
                    .insert(entry.lba as usize + s, entry.name.clone());
            }
        }
    }

    // Compare ISO directory entries.
    println!("\n=== ISO 9660 Directory Changes ===");
    let entries_b_map: BTreeMap<String, _> =
        entries_b.iter().map(|e| (e.name.clone(), e)).collect();
    let mut iso_changes = 0;
    for ea in &entries_a {
        if ea.is_directory {
            continue;
        }
        if let Some(eb) = entries_b_map.get(&ea.name) {
            if ea.size != eb.size || ea.lba != eb.lba {
                iso_changes += 1;
                print!("  {:14}", ea.name);
                if ea.size != eb.size {
                    print!(
                        "  size: {} → {} ({:+})",
                        ea.size,
                        eb.size,
                        eb.size as i64 - ea.size as i64
                    );
                }
                if ea.lba != eb.lba {
                    print!("  LBA: {} → {}", ea.lba, eb.lba);
                }
                println!();
            }
        }
    }
    if iso_changes == 0 {
        println!("  (no changes)");
    }

    // Sector-by-sector comparison.
    let mut changed: Vec<usize> = Vec::new();
    for i in 0..sectors {
        let data_a = disc_a.read_user_data(i)?;
        let data_b = disc_b.read_user_data(i)?;
        if data_a != data_b {
            changed.push(i);
        }
    }

    println!(
        "\nChanged sectors: {} of {} ({:.1}%)",
        changed.len(),
        sectors,
        changed.len() as f64 / sectors as f64 * 100.0
    );

    if changed.is_empty() {
        println!("ROMs are identical.");
        return Ok(());
    }

    // Group consecutive changed sectors into regions.
    let regions = group_consecutive(&changed);

    println!("\n=== Changed Regions ({}) ===", regions.len());
    for (start, end) in &regions {
        let count = end - start + 1;
        // Find which file this region belongs to.
        let file_name = sector_to_file
            .get(start)
            .cloned()
            .unwrap_or_else(|| format!("unmapped"));

        println!(
            "\n  LBA 0x{:04X}..0x{:04X} ({} sector{}) [{}]",
            start,
            end,
            count,
            if count == 1 { "" } else { "s" },
            file_name
        );

        // Count byte-level changes.
        let mut byte_changes = 0usize;
        let mut first_diffs: Vec<(usize, u8, u8)> = Vec::new();
        for s in *start..=*end {
            let data_a = disc_a.read_user_data(s)?;
            let data_b = disc_b.read_user_data(s)?;
            for (j, (&a, &b)) in data_a.iter().zip(data_b.iter()).enumerate()
            {
                if a != b {
                    byte_changes += 1;
                    if first_diffs.len() < 16 {
                        let abs_offset = (s - start) * 2048 + j;
                        first_diffs.push((abs_offset, a, b));
                    }
                }
            }
        }

        println!(
            "    {} bytes differ (of {} total)",
            byte_changes,
            count * 2048
        );

        // Show byte-level diffs for small changes.
        if byte_changes <= 64 {
            for &(off, a, b) in &first_diffs {
                println!("    +0x{:06X}: {:02X} → {:02X}", off, a, b);
            }
        } else if !first_diffs.is_empty() {
            for &(off, a, b) in &first_diffs {
                println!("    +0x{:06X}: {:02X} → {:02X}", off, a, b);
            }
            println!(
                "    ... ({} more changes)",
                byte_changes - first_diffs.len()
            );
        }
    }

    Ok(())
}

/// Compare a specific file extracted from both ROMs.
fn cmd_file_diff(
    disc_a: &DiscImage,
    iso_a: &Iso9660,
    disc_b: &DiscImage,
    iso_b: &Iso9660,
    filename: &str,
) -> Result<()> {
    println!("=== File Diff: {} ===\n", filename);

    let entry_a = iso_a
        .find_file(disc_a, filename)?
        .context(format!("'{}' not found in ROM A", filename))?;
    let entry_b = iso_b
        .find_file(disc_b, filename)?
        .context(format!("'{}' not found in ROM B", filename))?;

    println!("ROM A: LBA {}, {} bytes", entry_a.lba, entry_a.size);
    println!("ROM B: LBA {}, {} bytes", entry_b.lba, entry_b.size);

    let data_a = iso_a.extract_file(disc_a, &entry_a)?;
    let data_b = iso_b.extract_file(disc_b, &entry_b)?;

    // Raw (compressed) comparison.
    println!("\n--- Compressed data ---");
    println!("  Size A: {} bytes", data_a.len());
    println!("  Size B: {} bytes", data_b.len());
    if data_a.len() != data_b.len() {
        println!(
            "  Delta: {:+} bytes",
            data_b.len() as i64 - data_a.len() as i64
        );
    }

    let min_len = data_a.len().min(data_b.len());
    let raw_diffs: usize = data_a[..min_len]
        .iter()
        .zip(&data_b[..min_len])
        .filter(|(a, b)| a != b)
        .count()
        + data_a.len().abs_diff(data_b.len());
    println!("  Bytes differ: {}", raw_diffs);

    // If CNX compressed, also compare decompressed.
    if compression::is_cnx(&data_a) && compression::is_cnx(&data_b) {
        let hdr_a = compression::parse_header(&data_a)?;
        let hdr_b = compression::parse_header(&data_b)?;

        println!("\n--- CNX header ---");
        println!(
            "  Subtype A: {:?}, compressed: {}, decompressed: {}",
            hdr_a.subtype, hdr_a.compressed_size, hdr_a.decompressed_size
        );
        println!(
            "  Subtype B: {:?}, compressed: {}, decompressed: {}",
            hdr_b.subtype, hdr_b.compressed_size, hdr_b.decompressed_size
        );

        let dec_a = compression::decompress(&data_a)
            .context("Failed to decompress ROM A file")?;
        let dec_b = compression::decompress(&data_b)
            .context("Failed to decompress ROM B file")?;

        println!("\n--- Decompressed data ---");
        println!("  Size A: {} bytes", dec_a.len());
        println!("  Size B: {} bytes", dec_b.len());
        if dec_a.len() != dec_b.len() {
            println!(
                "  Delta: {:+} bytes",
                dec_b.len() as i64 - dec_a.len() as i64
            );
        }

        let dec_min = dec_a.len().min(dec_b.len());
        let dec_diffs: usize = dec_a[..dec_min]
            .iter()
            .zip(&dec_b[..dec_min])
            .filter(|(a, b)| a != b)
            .count();
        let total_dec_diffs = dec_diffs + dec_a.len().abs_diff(dec_b.len());
        println!("  Bytes differ: {}", total_dec_diffs);

        // Show first few decompressed diffs.
        if total_dec_diffs > 0 && total_dec_diffs <= 200 {
            println!("\n  Decompressed byte diffs:");
            let mut shown = 0;
            for (i, (&a, &b)) in
                dec_a[..dec_min].iter().zip(&dec_b[..dec_min]).enumerate()
            {
                if a != b {
                    println!("    0x{:06X}: {:02X} → {:02X}", i, a, b);
                    shown += 1;
                    if shown >= 32 {
                        println!(
                            "    ... ({} more)",
                            total_dec_diffs - shown
                        );
                        break;
                    }
                }
            }
        }

        // For SEQ files, show text region analysis.
        if filename.to_ascii_uppercase().ends_with(".SEQ")
            && dec_a.len() >= 4
            && dec_b.len() >= 4
        {
            println!("\n--- SEQ structure ---");
            // First 4 bytes = text region size (BE)
            let text_size_a = u32::from_be_bytes([
                dec_a[0], dec_a[1], dec_a[2], dec_a[3],
            ]);
            let text_size_b = u32::from_be_bytes([
                dec_b[0], dec_b[1], dec_b[2], dec_b[3],
            ]);
            println!(
                "  Text region size: {} → {} ({:+})",
                text_size_a,
                text_size_b,
                text_size_b as i64 - text_size_a as i64
            );

            // Count diffs in text region vs binary region.
            let text_end_a = (text_size_a as usize).min(dec_a.len());
            let text_end_b = (text_size_b as usize).min(dec_b.len());

            // Compare pointer tables (bytes 4..text_end).
            if text_end_a >= 4 && text_end_b >= 4 {
                let text_min = text_end_a.min(text_end_b);
                let text_diffs: usize = dec_a[4..text_min]
                    .iter()
                    .zip(&dec_b[4..text_min])
                    .filter(|(a, b)| a != b)
                    .count();
                println!("  Text region diffs: {} bytes", text_diffs);

                // Binary region (after text).
                let bin_start_a = text_end_a;
                let bin_start_b = text_end_b;
                if bin_start_a < dec_a.len() && bin_start_b < dec_b.len() {
                    let bin_a = &dec_a[bin_start_a..];
                    let bin_b = &dec_b[bin_start_b..];
                    let bin_min = bin_a.len().min(bin_b.len());
                    let bin_diffs: usize = bin_a[..bin_min]
                        .iter()
                        .zip(&bin_b[..bin_min])
                        .filter(|(a, b)| a != b)
                        .count();
                    println!(
                        "  Binary region diffs: {} bytes (A: {}..{}, B: {}..{})",
                        bin_diffs, bin_start_a, dec_a.len(), bin_start_b, dec_b.len()
                    );
                }
            }
        }
    } else if data_a == data_b {
        println!("\n  Files are identical.");
    }

    Ok(())
}

fn group_consecutive(indices: &[usize]) -> Vec<(usize, usize)> {
    if indices.is_empty() {
        return Vec::new();
    }
    let mut regions = Vec::new();
    let mut start = indices[0];
    let mut prev = indices[0];
    for &i in &indices[1..] {
        if i == prev + 1 {
            prev = i;
        } else {
            regions.push((start, prev));
            start = i;
            prev = i;
        }
    }
    regions.push((start, prev));
    regions
}
