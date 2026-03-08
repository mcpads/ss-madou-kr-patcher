//! Disc image parsing for Sega Saturn BIN/CUE images.
//!
//! This module provides:
//!
//! - [`sector::DiscImage`] -- raw 2352-byte sector access over a BIN file.
//! - [`ip_bin::SaturnHeader`] -- Saturn IP.BIN header parser (sector 0).
//! - [`iso9660::Iso9660`] -- ISO 9660 filesystem parser (PVD at sector 16,
//!   directory listing, file lookup/extraction).

pub mod bps;
pub mod edc_ecc;
pub mod ip_bin;
pub mod iso9660;
pub mod sector;
pub mod tracked;
pub mod tracked_regions;

/// Test helpers for building synthetic disc images.
/// Public (not `#[cfg(test)]`) because integration tests in `tests/` need access.
pub mod test_helpers;

// Re-export the most commonly used types at the module level.
pub use ip_bin::SaturnHeader;
pub use iso9660::{DirectoryEntry, Iso9660, PrimaryVolumeDescriptor};
pub use sector::{DiscError, DiscImage};
pub use tracked::TrackedDisc;
