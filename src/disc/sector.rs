//! Raw 2352-byte CD-ROM sector parsing for BIN/CUE disc images.
//!
//! A CD-ROM MODE1/2352 sector is laid out as:
//!
//! | Offset | Size  | Field          |
//! |--------|-------|----------------|
//! | 0x000  | 12    | Sync pattern   |
//! | 0x00C  | 4     | Header (MSF+Mode) |
//! | 0x010  | 2048  | User data      |
//! | 0x810  | 4     | EDC            |
//! | 0x814  | 8     | Reserved       |
//! | 0x81C  | 276   | ECC            |
//!
//! Total: 2352 bytes per sector.

use std::path::Path;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Total size of one raw CD-ROM sector (MODE1/2352).
pub const RAW_SECTOR_SIZE: usize = 2352;

/// Size of the sync pattern at the start of every Mode 1 sector.
pub const SYNC_SIZE: usize = 12;

/// Size of the sector header (minutes, seconds, frames, mode).
pub const HEADER_SIZE: usize = 4;

/// Byte offset where user data begins within a sector.
pub const USER_DATA_OFFSET: usize = SYNC_SIZE + HEADER_SIZE; // 16

/// Size of the user data payload in a Mode 1 sector.
pub const USER_DATA_SIZE: usize = 2048;

/// The fixed 12-byte sync pattern that marks the start of every Mode 1 sector.
///
/// `[0x00, 0xFF x10, 0x00]`
pub const SYNC_PATTERN: [u8; 12] = [
    0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
];

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while working with disc images.
#[derive(Debug, Error)]
pub enum DiscError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("file size {size} is not a multiple of the raw sector size ({RAW_SECTOR_SIZE})")]
    InvalidFileSize { size: u64 },

    #[error("sector index {index} is out of range (disc has {count} sectors)")]
    SectorOutOfRange { index: usize, count: usize },

    #[error("invalid sync pattern at sector {sector}")]
    InvalidSync { sector: usize },

    #[error("invalid ISO 9660 signature at sector 16")]
    InvalidIsoSignature,

    #[error("ISO 9660 parse error: {0}")]
    IsoParse(String),

    #[error("Saturn header parse error: {0}")]
    HeaderParse(String),
}

// ---------------------------------------------------------------------------
// Sector
// ---------------------------------------------------------------------------

/// A parsed Mode 1 sector, borrowing its data from the underlying disc image.
#[derive(Debug)]
pub struct Sector<'a> {
    /// Zero-based sector index within the BIN file.
    pub index: usize,
    /// BCD-encoded minutes field from the sector header.
    pub minutes: u8,
    /// BCD-encoded seconds field from the sector header.
    pub seconds: u8,
    /// BCD-encoded frames field from the sector header.
    pub frames: u8,
    /// Mode byte (0x01 for Mode 1).
    pub mode: u8,
    /// The 2048-byte user data payload.
    pub user_data: &'a [u8],
}

// ---------------------------------------------------------------------------
// DiscImage
// ---------------------------------------------------------------------------

/// Wrapper around raw BIN file data providing sector-level access.
pub struct DiscImage {
    data: Vec<u8>,
    sector_count: usize,
}

impl DiscImage {
    /// Load a raw BIN disc image from `path`.
    ///
    /// The file size must be a multiple of [`RAW_SECTOR_SIZE`] (2352).
    pub fn from_bin_file(path: &Path) -> Result<Self, DiscError> {
        let data = std::fs::read(path)?;
        let size = data.len() as u64;
        if size % RAW_SECTOR_SIZE as u64 != 0 {
            return Err(DiscError::InvalidFileSize { size });
        }
        let sector_count = data.len() / RAW_SECTOR_SIZE;
        Ok(Self { data, sector_count })
    }

    /// Create a `DiscImage` directly from an in-memory buffer.
    ///
    /// The buffer length must be a multiple of [`RAW_SECTOR_SIZE`].
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DiscError> {
        let size = data.len() as u64;
        if size % RAW_SECTOR_SIZE as u64 != 0 {
            return Err(DiscError::InvalidFileSize { size });
        }
        let sector_count = data.len() / RAW_SECTOR_SIZE;
        Ok(Self { data, sector_count })
    }

    /// Number of raw sectors contained in the image.
    pub fn sector_count(&self) -> usize {
        self.sector_count
    }

    /// Parse and return the sector at the given zero-based `index`.
    ///
    /// Validates the sync pattern and extracts the BCD header fields.
    pub fn read_sector(&self, index: usize) -> Result<Sector<'_>, DiscError> {
        if index >= self.sector_count {
            return Err(DiscError::SectorOutOfRange {
                index,
                count: self.sector_count,
            });
        }
        let offset = index * RAW_SECTOR_SIZE;
        let raw = &self.data[offset..offset + RAW_SECTOR_SIZE];

        // Validate sync pattern.
        if raw[..SYNC_SIZE] != SYNC_PATTERN {
            return Err(DiscError::InvalidSync { sector: index });
        }

        Ok(Sector {
            index,
            minutes: raw[SYNC_SIZE],
            seconds: raw[SYNC_SIZE + 1],
            frames: raw[SYNC_SIZE + 2],
            mode: raw[SYNC_SIZE + 3],
            user_data: &raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE],
        })
    }

    /// Return a slice over the 2048-byte user data region of sector `lba`.
    pub fn read_user_data(&self, lba: usize) -> Result<&[u8], DiscError> {
        if lba >= self.sector_count {
            return Err(DiscError::SectorOutOfRange {
                index: lba,
                count: self.sector_count,
            });
        }
        let start = lba * RAW_SECTOR_SIZE + USER_DATA_OFFSET;
        Ok(&self.data[start..start + USER_DATA_SIZE])
    }

    /// Extract a contiguous run of user data spanning multiple sectors.
    ///
    /// Reads `size` bytes starting at sector `lba`, concatenating the 2048-byte
    /// user-data payloads from consecutive sectors.
    pub fn extract_file(&self, lba: u32, size: u32) -> Result<Vec<u8>, DiscError> {
        let mut result = Vec::with_capacity(size as usize);
        let sector_count = (size as usize + USER_DATA_SIZE - 1) / USER_DATA_SIZE;

        for i in 0..sector_count {
            let sector_lba = lba as usize + i;
            let user_data = self.read_user_data(sector_lba)?;
            let remaining = size as usize - result.len();
            let to_read = remaining.min(USER_DATA_SIZE);
            result.extend_from_slice(&user_data[..to_read]);
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Disc writing
// ---------------------------------------------------------------------------

impl DiscImage {
    /// Write data to the user-data region of a single sector.
    ///
    /// `data` must be at most [`USER_DATA_SIZE`] (2048) bytes.
    /// If shorter, the remainder of the user-data region is zero-padded.
    #[deprecated(note = "use TrackedDisc::write_user_data_at for tracked writes")]
    pub fn write_user_data_at(&mut self, lba: usize, data: &[u8]) -> Result<(), DiscError> {
        if lba >= self.sector_count {
            return Err(DiscError::SectorOutOfRange {
                index: lba,
                count: self.sector_count,
            });
        }
        let start = lba * RAW_SECTOR_SIZE + USER_DATA_OFFSET;
        let len = data.len().min(USER_DATA_SIZE);
        self.data[start..start + len].copy_from_slice(&data[..len]);
        // Zero-pad if data is shorter than one sector.
        for b in &mut self.data[start + len..start + USER_DATA_SIZE] {
            *b = 0;
        }
        Ok(())
    }

    /// Write a file spanning multiple consecutive sectors.
    ///
    /// Returns the number of sectors written.
    #[deprecated(note = "use TrackedDisc::write_file_at for tracked writes")]
    #[allow(deprecated)]
    pub fn write_file_at(&mut self, lba: u32, data: &[u8]) -> Result<usize, DiscError> {
        let sectors_needed = (data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
        for i in 0..sectors_needed {
            let sector_lba = lba as usize + i;
            let chunk_start = i * USER_DATA_SIZE;
            let chunk_end = (chunk_start + USER_DATA_SIZE).min(data.len());
            self.write_user_data_at(sector_lba, &data[chunk_start..chunk_end])?;
        }
        Ok(sectors_needed)
    }

    /// Save the (possibly modified) disc image to a file.
    pub fn save(&self, path: &Path) -> Result<(), DiscError> {
        std::fs::write(path, &self.data)?;
        Ok(())
    }

    /// Regenerate EDC/ECC for all Mode 1 sectors in the data track.
    /// Returns the number of sectors regenerated.
    pub fn regenerate_edc_ecc(&mut self, data_sector_count: usize) -> usize {
        let tables = crate::disc::edc_ecc::EdcEccTables::new();
        tables.regenerate_all_sectors(&mut self.data, data_sector_count)
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Decode a BCD (Binary Coded Decimal) byte to its decimal value.
///
/// For example `0x12` becomes `12`.
pub fn bcd_to_dec(bcd: u8) -> u8 {
    (bcd >> 4) * 10 + (bcd & 0x0F)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "sector_tests.rs"]
mod tests;
