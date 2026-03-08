//! ISO 9660 filesystem parser for CD-ROM disc images.
//!
//! Parses the Primary Volume Descriptor (PVD) at sector 16 and provides
//! directory listing and file lookup over the disc image.
//!
//! ## Key ISO 9660 details
//!
//! - PVD is located at LBA 16 and starts with `0x01 "CD001"`.
//! - Multi-byte integers are stored in *both-endian* format: little-endian u32
//!   followed immediately by big-endian u32 (8 bytes total for one logical value).
//!   We always read the little-endian half.
//! - Directory records are variable-length, packed contiguously within the
//!   extent sectors of a directory. A zero-length record signals padding to the
//!   next sector boundary.

use super::sector::{DiscError, DiscImage, USER_DATA_SIZE};
use super::tracked::TrackedDisc;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// LBA of the Primary Volume Descriptor.
const PVD_SECTOR: usize = 16;

/// ISO 9660 standard identifier.
const ISO_SIGNATURE: &[u8; 5] = b"CD001";

/// Offset of the root directory record within the PVD user data.
const PVD_ROOT_DIR_RECORD_OFFSET: usize = 0x9C;

// ---------------------------------------------------------------------------
// Both-endian helpers
// ---------------------------------------------------------------------------

/// Read a both-endian u32 (8 bytes: LE u32 then BE u32). Returns the LE value.
fn read_both_u32(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

/// Read a both-endian u16 (4 bytes: LE u16 then BE u16). Returns the LE value.
fn read_both_u16(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

// ---------------------------------------------------------------------------
// PrimaryVolumeDescriptor
// ---------------------------------------------------------------------------

/// Parsed ISO 9660 Primary Volume Descriptor (from sector 16).
#[derive(Debug, Clone)]
pub struct PrimaryVolumeDescriptor {
    /// System identifier (32 bytes, space-padded, trimmed).
    pub system_id: String,
    /// Volume identifier (32 bytes, space-padded, trimmed).
    pub volume_id: String,
    /// Total number of logical blocks in the volume (both-endian u32 at PVD+0x50).
    pub volume_space_size: u32,
    /// Logical block size in bytes (both-endian u16 at PVD+0x80, typically 2048).
    pub logical_block_size: u16,
    /// LBA of the root directory extent (from the embedded root directory record).
    pub root_directory_lba: u32,
    /// Size in bytes of the root directory extent.
    pub root_directory_size: u32,
}

// ---------------------------------------------------------------------------
// DirectoryEntry
// ---------------------------------------------------------------------------

/// A single parsed ISO 9660 directory record.
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// Filename (with `;1` version suffix stripped).
    pub name: String,
    /// Starting LBA of the file or directory extent.
    pub lba: u32,
    /// Size of the file or directory in bytes.
    pub size: u32,
    /// `true` if this entry represents a directory.
    pub is_directory: bool,
}

// ---------------------------------------------------------------------------
// Iso9660
// ---------------------------------------------------------------------------

/// High-level ISO 9660 filesystem handle.
///
/// Holds the parsed PVD and provides methods to list directories and find/extract
/// files via a borrowed [`DiscImage`].
#[derive(Debug, Clone)]
pub struct Iso9660 {
    /// The parsed Primary Volume Descriptor.
    pub pvd: PrimaryVolumeDescriptor,
}

impl Iso9660 {
    /// Parse the ISO 9660 filesystem from `disc`.
    ///
    /// Reads sector 16, validates the `"CD001"` signature, and extracts the PVD.
    pub fn parse(disc: &DiscImage) -> Result<Self, DiscError> {
        let pvd_data = disc.read_user_data(PVD_SECTOR)?;

        // Byte 0: type code (0x01 for PVD).
        // Bytes 1..6: "CD001".
        if pvd_data.len() < 256 {
            return Err(DiscError::IsoParse(
                "PVD sector user data too short".into(),
            ));
        }

        if pvd_data[0] != 0x01 || &pvd_data[1..6] != ISO_SIGNATURE {
            return Err(DiscError::InvalidIsoSignature);
        }

        let read_str = |offset: usize, len: usize| -> String {
            String::from_utf8_lossy(&pvd_data[offset..offset + len])
                .trim()
                .to_string()
        };

        let system_id = read_str(0x08, 32);
        let volume_id = read_str(0x28, 32);
        let volume_space_size = read_both_u32(&pvd_data[0x50..]);
        let logical_block_size = read_both_u16(&pvd_data[0x80..]);

        // Root directory record is a 34-byte directory record at PVD+0x9C.
        let root_rec = &pvd_data[PVD_ROOT_DIR_RECORD_OFFSET..PVD_ROOT_DIR_RECORD_OFFSET + 34];
        let root_directory_lba = read_both_u32(&root_rec[2..]);
        let root_directory_size = read_both_u32(&root_rec[10..]);

        Ok(Iso9660 {
            pvd: PrimaryVolumeDescriptor {
                system_id,
                volume_id,
                volume_space_size,
                logical_block_size,
                root_directory_lba,
                root_directory_size,
            },
        })
    }

    /// List the entries in the root directory.
    pub fn list_root(&self, disc: &DiscImage) -> Result<Vec<DirectoryEntry>, DiscError> {
        self.list_directory(disc, self.pvd.root_directory_lba, self.pvd.root_directory_size)
    }

    /// List the entries within a directory at `dir_lba` with `dir_size` bytes.
    ///
    /// Handles variable-length records and zero-padding at sector boundaries.
    pub fn list_directory(
        &self,
        disc: &DiscImage,
        dir_lba: u32,
        dir_size: u32,
    ) -> Result<Vec<DirectoryEntry>, DiscError> {
        let dir_data = disc.extract_file(dir_lba, dir_size)?;
        let mut entries = Vec::new();
        let mut offset = 0usize;

        while offset < dir_size as usize {
            // A record length of 0 means we must skip to the next sector boundary.
            let record_len = dir_data[offset] as usize;
            if record_len == 0 {
                let next_sector = (offset / USER_DATA_SIZE + 1) * USER_DATA_SIZE;
                if next_sector >= dir_size as usize {
                    break;
                }
                offset = next_sector;
                continue;
            }

            if offset + record_len > dir_data.len() {
                break;
            }

            let record = &dir_data[offset..offset + record_len];

            let lba = read_both_u32(&record[2..]);
            let size = read_both_u32(&record[10..]);
            let flags = record[25];
            let name_len = record[32] as usize;

            let name = if name_len == 0 {
                String::new()
            } else {
                let name_bytes = &record[33..33 + name_len];
                if name_len == 1 && name_bytes[0] == 0x00 {
                    ".".to_string()
                } else if name_len == 1 && name_bytes[0] == 0x01 {
                    "..".to_string()
                } else {
                    let raw = String::from_utf8_lossy(name_bytes).to_string();
                    // Strip the ";1" version suffix if present.
                    raw.split(';').next().unwrap_or(&raw).to_string()
                }
            };

            let is_directory = flags & 0x02 != 0;

            entries.push(DirectoryEntry {
                name,
                lba,
                size,
                is_directory,
            });

            offset += record_len;
        }

        Ok(entries)
    }

    /// Search for a file by path (e.g. `"0.BIN"` or `"SUBDIR/FILE.DAT"`).
    ///
    /// Path components are compared case-insensitively and the `;1` version
    /// suffix is already stripped from directory entries.
    pub fn find_file(
        &self,
        disc: &DiscImage,
        path: &str,
    ) -> Result<Option<DirectoryEntry>, DiscError> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if components.is_empty() {
            return Ok(None);
        }

        let mut current_lba = self.pvd.root_directory_lba;
        let mut current_size = self.pvd.root_directory_size;

        for (i, component) in components.iter().enumerate() {
            let entries = self.list_directory(disc, current_lba, current_size)?;
            let target = component.to_uppercase();

            let found = entries.into_iter().find(|e| {
                let entry_name = e.name.to_uppercase();
                // Also try stripping a trailing '.' that ISO 9660 may leave.
                entry_name == target || entry_name.trim_end_matches('.') == target
            });

            match found {
                Some(entry) => {
                    if i == components.len() - 1 {
                        // Last component -- this is the target.
                        return Ok(Some(entry));
                    }
                    // Intermediate component must be a directory.
                    if !entry.is_directory {
                        return Ok(None);
                    }
                    current_lba = entry.lba;
                    current_size = entry.size;
                }
                None => return Ok(None),
            }
        }

        Ok(None)
    }

    /// Extract the raw file content of the given directory entry from `disc`.
    pub fn extract_file(
        &self,
        disc: &DiscImage,
        entry: &DirectoryEntry,
    ) -> Result<Vec<u8>, DiscError> {
        disc.extract_file(entry.lba, entry.size)
    }

    /// Patch the Data Length field of a file's directory record on disc.
    ///
    /// Finds the named file in the root directory and updates its both-endian
    /// size field. This is needed when a file's compressed size changes.
    #[deprecated(note = "use patch_file_size_tracked for tracked writes")]
    #[allow(deprecated)]
    pub fn patch_file_size(
        &self,
        disc: &mut DiscImage,
        file_name: &str,
        new_size: u32,
    ) -> Result<(), DiscError> {
        self.patch_file_entry(disc, file_name, None, Some(new_size))
    }

    /// Patch the LBA and/or Data Length fields of a file's directory record.
    ///
    /// Finds the named file in the root directory and updates its both-endian
    /// LBA (offset+2) and/or size (offset+10) fields.
    #[deprecated(note = "use patch_file_entry_tracked for tracked writes")]
    #[allow(deprecated)]
    pub fn patch_file_entry(
        &self,
        disc: &mut DiscImage,
        file_name: &str,
        new_lba: Option<u32>,
        new_size: Option<u32>,
    ) -> Result<(), DiscError> {
        if new_lba.is_none() && new_size.is_none() {
            return Ok(());
        }

        let dir_lba = self.pvd.root_directory_lba;
        let dir_size = self.pvd.root_directory_size;
        let dir_data = disc.extract_file(dir_lba, dir_size)?;

        let mut offset = 0usize;
        while offset < dir_size as usize {
            let record_len = dir_data[offset] as usize;
            if record_len == 0 {
                let next_sector = (offset / USER_DATA_SIZE + 1) * USER_DATA_SIZE;
                if next_sector >= dir_size as usize {
                    break;
                }
                offset = next_sector;
                continue;
            }
            if offset + record_len > dir_data.len() {
                break;
            }

            let name_len = dir_data[offset + 32] as usize;
            if name_len > 1 {
                let name_bytes = &dir_data[offset + 33..offset + 33 + name_len];
                let raw = String::from_utf8_lossy(name_bytes).to_string();
                let clean = raw.split(';').next().unwrap_or(&raw);

                if clean.eq_ignore_ascii_case(file_name) {
                    let sector_idx = offset / USER_DATA_SIZE;
                    let offset_in_sector = offset % USER_DATA_SIZE;
                    let sector_lba = dir_lba as usize + sector_idx;

                    let mut sector = disc.read_user_data(sector_lba)?.to_vec();

                    // Patch LBA (both-endian u32 at record offset+2).
                    if let Some(lba) = new_lba {
                        let lba_off = offset_in_sector + 2;
                        sector[lba_off..lba_off + 4]
                            .copy_from_slice(&lba.to_le_bytes());
                        sector[lba_off + 4..lba_off + 8]
                            .copy_from_slice(&lba.to_be_bytes());
                    }

                    // Patch size (both-endian u32 at record offset+10).
                    if let Some(size) = new_size {
                        let size_off = offset_in_sector + 10;
                        sector[size_off..size_off + 4]
                            .copy_from_slice(&size.to_le_bytes());
                        sector[size_off + 4..size_off + 8]
                            .copy_from_slice(&size.to_be_bytes());
                    }

                    disc.write_user_data_at(sector_lba, &sector)?;
                    return Ok(());
                }
            }

            offset += record_len;
        }

        Err(DiscError::IsoParse(format!(
            "File '{}' not found in root directory",
            file_name
        )))
    }

    /// Find a contiguous free region at the end of used disc space.
    ///
    /// Scans all files in the root directory, finds the sector after the
    /// last used file, and returns it as the start LBA for relocation.
    /// Returns an error if there isn't enough space.
    pub fn find_free_region(
        &self,
        disc: &DiscImage,
        sectors_needed: u32,
    ) -> Result<u32, DiscError> {
        let entries = self.list_root(disc)?;

        // Collect (start_lba, end_lba) for all files, sorted by LBA.
        let mut file_spans: Vec<(u32, u32)> = entries
            .iter()
            .filter(|e| !e.is_directory && e.name != "." && e.name != "..")
            .map(|e| {
                let sectors =
                    (e.size + USER_DATA_SIZE as u32 - 1) / USER_DATA_SIZE as u32;
                (e.lba, e.lba + sectors)
            })
            .collect();
        file_spans.sort_by_key(|&(lba, _)| lba);

        // Detect CD-DA boundary: find a gap > 1000 sectors between consecutive
        // files. Files beyond that gap are audio track data (e.g., CDALERT.DA)
        // and should not be considered when looking for free space.
        let mut audio_boundary = self.pvd.volume_space_size;
        for pair in file_spans.windows(2) {
            let prev_end = pair[0].1;
            let next_start = pair[1].0;
            if next_start > prev_end + 1000 {
                audio_boundary = next_start;
                break;
            }
        }

        // Last used sector in the data region (before audio boundary).
        let last_used = file_spans
            .iter()
            .filter(|&&(_, end)| end <= audio_boundary)
            .map(|&(_, end)| end)
            .max()
            .unwrap_or(0);

        let free_start = last_used;
        if free_start + sectors_needed > audio_boundary {
            return Err(DiscError::IsoParse(format!(
                "Not enough disc space: need {} sectors at LBA {}, but audio boundary at {}",
                sectors_needed, free_start, audio_boundary
            )));
        }

        Ok(free_start)
    }

    /// Relocate a file to the free region at the end of the disc.
    ///
    /// Writes `data` at the first available free LBA, then updates the
    /// ISO 9660 directory record (LBA + size) for `file_name`.
    /// Returns the new LBA where the file was written.
    #[deprecated(note = "use relocate_file_tracked for tracked writes")]
    #[allow(deprecated)]
    pub fn relocate_file(
        &self,
        disc: &mut DiscImage,
        file_name: &str,
        data: &[u8],
    ) -> Result<u32, DiscError> {
        let sectors_needed =
            ((data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE) as u32;
        let new_lba = self.find_free_region(disc, sectors_needed)?;

        disc.write_file_at(new_lba, data)?;
        self.patch_file_entry(disc, file_name, Some(new_lba), Some(data.len() as u32))?;

        Ok(new_lba)
    }
}

// ---------------------------------------------------------------------------
// TrackedDisc write methods
// ---------------------------------------------------------------------------

impl Iso9660 {
    /// Patch the LBA and/or Data Length fields of a file's directory record,
    /// using a [`TrackedDisc`] for write tracking.
    pub fn patch_file_entry_tracked(
        &self,
        disc: &mut TrackedDisc,
        file_name: &str,
        new_lba: Option<u32>,
        new_size: Option<u32>,
    ) -> Result<(), DiscError> {
        if new_lba.is_none() && new_size.is_none() {
            return Ok(());
        }

        let dir_lba = self.pvd.root_directory_lba;
        let dir_size = self.pvd.root_directory_size;
        let dir_data = disc.extract_file(dir_lba, dir_size)?;

        let mut offset = 0usize;
        while offset < dir_size as usize {
            let record_len = dir_data[offset] as usize;
            if record_len == 0 {
                let next_sector = (offset / USER_DATA_SIZE + 1) * USER_DATA_SIZE;
                if next_sector >= dir_size as usize {
                    break;
                }
                offset = next_sector;
                continue;
            }
            if offset + record_len > dir_data.len() {
                break;
            }

            let name_len = dir_data[offset + 32] as usize;
            if name_len > 1 {
                let name_bytes = &dir_data[offset + 33..offset + 33 + name_len];
                let raw = String::from_utf8_lossy(name_bytes).to_string();
                let clean = raw.split(';').next().unwrap_or(&raw);

                if clean.eq_ignore_ascii_case(file_name) {
                    let sector_idx = offset / USER_DATA_SIZE;
                    let offset_in_sector = offset % USER_DATA_SIZE;
                    let sector_lba = dir_lba as usize + sector_idx;

                    let mut sector = disc.read_user_data(sector_lba)?.to_vec();

                    if let Some(lba) = new_lba {
                        let lba_off = offset_in_sector + 2;
                        sector[lba_off..lba_off + 4]
                            .copy_from_slice(&lba.to_le_bytes());
                        sector[lba_off + 4..lba_off + 8]
                            .copy_from_slice(&lba.to_be_bytes());
                    }

                    if let Some(size) = new_size {
                        let size_off = offset_in_sector + 10;
                        sector[size_off..size_off + 4]
                            .copy_from_slice(&size.to_le_bytes());
                        sector[size_off + 4..size_off + 8]
                            .copy_from_slice(&size.to_be_bytes());
                    }

                    disc.write_user_data_at(sector_lba, &sector, "ISO9660:rootdir")?;
                    return Ok(());
                }
            }

            offset += record_len;
        }

        Err(DiscError::IsoParse(format!(
            "File '{}' not found in root directory",
            file_name
        )))
    }

    /// Patch the Data Length field using a [`TrackedDisc`].
    pub fn patch_file_size_tracked(
        &self,
        disc: &mut TrackedDisc,
        file_name: &str,
        new_size: u32,
    ) -> Result<(), DiscError> {
        self.patch_file_entry_tracked(disc, file_name, None, Some(new_size))
    }

    /// Relocate a file to the free region at the end of the disc,
    /// using a [`TrackedDisc`] for write tracking.
    ///
    /// Returns the new LBA where the file was written.
    pub fn relocate_file_tracked(
        &self,
        disc: &mut TrackedDisc,
        file_name: &str,
        data: &[u8],
    ) -> Result<u32, DiscError> {
        let sectors_needed =
            ((data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE) as u32;
        let new_lba = self.find_free_region(disc.disc(), sectors_needed)?;

        let label = format!("{}:relocate", file_name);
        disc.write_file_at(new_lba, data, &label)?;
        self.patch_file_entry_tracked(disc, file_name, Some(new_lba), Some(data.len() as u32))?;

        Ok(new_lba)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "iso9660_tests.rs"]
mod tests;
