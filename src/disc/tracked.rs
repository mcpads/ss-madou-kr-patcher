//! [`TrackedDisc`]: a write-tracking wrapper around [`DiscImage`].
//!
//! Every write operation requires a human-readable label that records *who*
//! wrote *which* sectors. After all writes are complete, call [`check()`] to
//! detect overlapping regions before saving.
//!
//! **Design**: No `Deref<Target=DiscImage>` — callers must use explicit read
//! delegation methods. This prevents accidental access to the label-free
//! `DiscImage::write_*` methods through auto-deref.

use std::path::Path;

use super::sector::{DiscError, DiscImage, Sector, USER_DATA_SIZE};
use super::tracked_regions::SectorRegionTracker;

pub struct TrackedDisc {
    disc: DiscImage,
    tracker: SectorRegionTracker,
}

impl TrackedDisc {
    pub fn new(disc: DiscImage) -> Self {
        Self {
            disc,
            tracker: SectorRegionTracker::new(),
        }
    }

    // --- Read delegation (no tracking needed) ---

    /// Borrow the underlying `DiscImage` for read-only operations.
    pub fn disc(&self) -> &DiscImage {
        &self.disc
    }

    pub fn sector_count(&self) -> usize {
        self.disc.sector_count()
    }

    pub fn read_sector(&self, index: usize) -> Result<Sector<'_>, DiscError> {
        self.disc.read_sector(index)
    }

    pub fn read_user_data(&self, lba: usize) -> Result<&[u8], DiscError> {
        self.disc.read_user_data(lba)
    }

    pub fn extract_file(&self, lba: u32, size: u32) -> Result<Vec<u8>, DiscError> {
        self.disc.extract_file(lba, size)
    }

    // --- Tracked writes (label required) ---

    /// Write data to a single sector's user-data region.
    /// Registers the write in the tracker with the given `label`.
    pub fn write_user_data_at(
        &mut self,
        lba: usize,
        data: &[u8],
        label: &str,
    ) -> Result<(), DiscError> {
        self.tracker.register(lba, 1, label);
        #[allow(deprecated)]
        self.disc.write_user_data_at(lba, data)
    }

    /// Write a file spanning multiple consecutive sectors.
    /// Returns the number of sectors written.
    pub fn write_file_at(
        &mut self,
        lba: u32,
        data: &[u8],
        label: &str,
    ) -> Result<usize, DiscError> {
        let sectors_needed = (data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
        self.tracker
            .register(lba as usize, sectors_needed, label);
        #[allow(deprecated)]
        self.disc.write_file_at(lba, data)
    }

    // --- Collision check ---

    /// Check all registered write regions for overlaps.
    pub fn check(&self) -> Result<(), String> {
        self.tracker.check()
    }

    /// Print the sector region map to stderr.
    pub fn dump_regions(&self) {
        self.tracker.dump();
    }

    /// Return the number of tracked write regions.
    pub fn region_count(&self) -> usize {
        self.tracker.len()
    }

    // --- Save / convert ---

    /// Save the disc image to a file.
    pub fn save(&self, path: &Path) -> Result<(), DiscError> {
        self.disc.save(path)
    }

    /// Regenerate EDC/ECC for all Mode 1 sectors in the data track.
    pub fn regenerate_edc_ecc(&mut self, data_sector_count: usize) -> usize {
        self.disc.regenerate_edc_ecc(data_sector_count)
    }

    /// Consume the wrapper and return the inner `DiscImage`.
    pub fn into_inner(self) -> DiscImage {
        self.disc
    }
}

#[cfg(test)]
#[path = "tracked_tests.rs"]
mod tests;
