use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use ss_madou::compression;
use ss_madou::disc::{DiscImage, Iso9660, SaturnHeader};

pub(crate) fn cmd_info(rom: &Path) -> Result<()> {
    let disc =
        DiscImage::from_bin_file(rom).context("Failed to open disc image")?;

    let ip_data = disc
        .read_user_data(0)
        .context("Failed to read sector 0")?;
    let header =
        SaturnHeader::parse(ip_data).context("Failed to parse Saturn header")?;

    println!("{header}");
    println!("\nDisc: {} sectors", disc.sector_count());

    Ok(())
}

pub(crate) fn cmd_files(rom: &Path) -> Result<()> {
    let disc =
        DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso =
        Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;

    let entries = iso
        .list_root(&disc)
        .context("Failed to list root directory")?;
    println!("ISO 9660 root directory ({} entries):", entries.len());
    for entry in &entries {
        println!(
            "  {:12}  sector {:6}  size {:10}  {}",
            entry.name,
            entry.lba,
            entry.size,
            if entry.is_directory { "<DIR>" } else { "" }
        );
    }

    Ok(())
}

pub(crate) fn cmd_extract(rom: &Path, file_name: &str, output: &Path) -> Result<()> {
    let disc =
        DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso =
        Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;

    let entry = iso
        .find_file(&disc, file_name)
        .context("Failed searching ISO 9660")?
        .context(format!("File '{file_name}' not found in ISO 9660"))?;

    let data = iso
        .extract_file(&disc, &entry)
        .context("Failed to extract file data")?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }
    fs::write(output, &data).context("Failed to write output file")?;

    println!(
        "Extracted '{}' -> {} ({} bytes)",
        file_name,
        output.display(),
        data.len()
    );

    Ok(())
}

pub(crate) fn cmd_decompress(rom: &Path, file_name: &str, output: &Path) -> Result<()> {
    let disc = DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso = Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;

    let entry = iso
        .find_file(&disc, file_name)
        .context("Failed searching ISO 9660")?
        .context(format!("File '{file_name}' not found in ISO 9660"))?;

    let data = iso
        .extract_file(&disc, &entry)
        .context("Failed to extract file data")?;

    if !compression::is_cnx(&data) {
        anyhow::bail!("File '{file_name}' is not CNX-compressed");
    }

    let header = compression::parse_header(&data).context("Failed to parse CNX header")?;
    let decompressed = compression::decompress(&data).context("Failed to decompress")?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }
    fs::write(output, &decompressed).context("Failed to write output file")?;

    println!(
        "Decompressed '{}' -> {} ({} -> {} bytes)",
        file_name,
        output.display(),
        header.compressed_size,
        decompressed.len()
    );

    Ok(())
}

pub(crate) fn cmd_decompress_all(rom: &Path, output_dir: &Path) -> Result<()> {
    let disc = DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso = Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;

    let entries = iso
        .list_root(&disc)
        .context("Failed to list root directory")?;

    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    let mut count = 0;
    for entry in &entries {
        if entry.is_directory {
            continue;
        }

        let data = iso
            .extract_file(&disc, entry)
            .context(format!("Failed to extract '{}'", entry.name))?;

        if !compression::is_cnx(&data) {
            continue;
        }

        let decompressed = compression::decompress(&data)
            .context(format!("Failed to decompress '{}'", entry.name))?;

        let out_path = output_dir.join(&entry.name);
        fs::write(&out_path, &decompressed)
            .context(format!("Failed to write '{}'", out_path.display()))?;

        println!(
            "  {} ({} -> {} bytes)",
            entry.name,
            data.len(),
            decompressed.len()
        );
        count += 1;
    }

    println!("\nDecompressed {count} files to {}", output_dir.display());
    Ok(())
}
