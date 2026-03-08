//! Saturn IP.BIN header parser.
//!
//! The first 256 bytes of sector 0's user data contain the Saturn disc header.
//! This module parses that header into a [`SaturnHeader`] struct.
//!
//! ## Header layout (offsets relative to sector 0 user data)
//!
//! | Offset | Size | Field              |
//! |--------|------|--------------------|
//! | 0x000  | 16   | Hardware ID        |
//! | 0x010  | 16   | Maker ID           |
//! | 0x020  | 10   | Product Number     |
//! | 0x02A  | 6    | Version            |
//! | 0x030  | 8    | Release Date       |
//! | 0x038  | 8    | Device Information |
//! | 0x040  | 8    | Area Code          |
//! | 0x060  | 128  | Game Title         |
//! | 0x0E0  | 4    | IP Size            |
//! | 0x0E4  | 12   | Reserved (0)       |
//! | 0x0F0  | 4    | 1st Read Address   |
//! | 0x0F4  | 4    | 1st Read Size      |
//! | 0x0F8  | 8    | Reserved (0)       |

use super::sector::DiscError;

/// Expected content of the Hardware Identifier field for a valid Saturn disc.
pub const SATURN_MAGIC: &str = "SEGA SEGASATURN ";

/// Minimum size of the header data we need (first 256 bytes of sector 0 user data).
pub const SATURN_HEADER_SIZE: usize = 256;

/// Parsed Sega Saturn disc header from IP.BIN (sector 0, user data bytes 0x00..0xFF).
#[derive(Debug, Clone)]
pub struct SaturnHeader {
    /// Hardware identifier. Must be "SEGA SEGASATURN " (with trailing space) for a
    /// valid Saturn disc; stored here with trailing spaces trimmed.
    pub hardware_id: String,
    /// Maker/publisher identifier (e.g. "SEGA ENTERPRISES").
    pub maker_id: String,
    /// Product number (e.g. "T-6607G").
    pub product_number: String,
    /// Version string (e.g. "V1.003").
    pub version: String,
    /// Release date in YYYYMMDD format.
    pub release_date: String,
    /// Device compatibility information (e.g. "CD-1/1").
    pub device_info: String,
    /// Region/area code (e.g. "JT" for Japan+Taiwan).
    pub area_code: String,
    /// Game title (ASCII, up to 128 characters).
    pub game_title: String,
    /// Size of the Initial Program boot code.
    pub ip_size: u32,
    /// Memory address where the 1st Read program is loaded (big-endian in header).
    pub first_read_addr: u32,
    /// Size in bytes of the 1st Read program (big-endian in header).
    pub first_read_size: u32,
}

impl SaturnHeader {
    /// Parse a Saturn header from the first 256 bytes of sector 0 user data.
    ///
    /// `sector0_user_data` must be at least [`SATURN_HEADER_SIZE`] bytes.
    pub fn parse(sector0_user_data: &[u8]) -> Result<Self, DiscError> {
        if sector0_user_data.len() < SATURN_HEADER_SIZE {
            return Err(DiscError::HeaderParse(format!(
                "user data too short: {} bytes (need at least {SATURN_HEADER_SIZE})",
                sector0_user_data.len()
            )));
        }

        let read_str = |offset: usize, len: usize| -> String {
            String::from_utf8_lossy(&sector0_user_data[offset..offset + len])
                .trim()
                .to_string()
        };

        let read_be_u32 = |offset: usize| -> u32 {
            u32::from_be_bytes([
                sector0_user_data[offset],
                sector0_user_data[offset + 1],
                sector0_user_data[offset + 2],
                sector0_user_data[offset + 3],
            ])
        };

        Ok(SaturnHeader {
            hardware_id: read_str(0x00, 16),
            maker_id: read_str(0x10, 16),
            product_number: read_str(0x20, 10),
            version: read_str(0x2A, 6),
            release_date: read_str(0x30, 8),
            device_info: read_str(0x38, 8),
            area_code: read_str(0x40, 8),
            game_title: read_str(0x60, 128),
            ip_size: read_be_u32(0xE0),
            first_read_addr: read_be_u32(0xF0),
            first_read_size: read_be_u32(0xF4),
        })
    }

    /// Returns `true` if the hardware identifier matches the Saturn magic string.
    ///
    /// The raw field is "SEGA SEGASATURN " (16 chars, trailing space), but we
    /// store the trimmed version, so we compare against the trimmed form.
    pub fn is_valid(&self) -> bool {
        self.hardware_id == SATURN_MAGIC.trim()
    }
}

impl std::fmt::Display for SaturnHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Hardware ID:    {}", self.hardware_id)?;
        writeln!(f, "Maker ID:       {}", self.maker_id)?;
        writeln!(f, "Product Number: {}", self.product_number)?;
        writeln!(f, "Version:        {}", self.version)?;
        writeln!(f, "Release Date:   {}", self.release_date)?;
        writeln!(f, "Device Info:    {}", self.device_info)?;
        writeln!(f, "Area Code:      {}", self.area_code)?;
        writeln!(f, "Game Title:     {}", self.game_title)?;
        writeln!(f, "IP Size:        0x{:08X}", self.ip_size)?;
        writeln!(f, "1st Read Addr:  0x{:08X}", self.first_read_addr)?;
        write!(f, "1st Read Size:  0x{:08X}", self.first_read_size)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "ip_bin_tests.rs"]
mod tests;
