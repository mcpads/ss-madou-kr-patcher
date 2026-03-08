//! Sector-level write region tracking with overlap detection.

use std::fmt;

/// A named region of sectors on disc.
#[derive(Debug, Clone)]
pub struct SectorRegion {
    pub start_lba: usize,
    pub sector_count: usize,
    pub label: String,
}

impl fmt::Display for SectorRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let end = self.start_lba + self.sector_count;
        write!(
            f,
            "LBA {:#06X}..{:#06X} ({} sectors) [{}]",
            self.start_lba, end, self.sector_count, self.label
        )
    }
}

/// Tracks disc sector regions written by different pipeline stages.
///
/// After all writes are registered, call [`check()`](SectorRegionTracker::check)
/// to detect overlapping regions.
#[derive(Default)]
pub struct SectorRegionTracker {
    regions: Vec<SectorRegion>,
}

impl SectorRegionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a write region. Zero-length regions are silently ignored.
    pub fn register(&mut self, start_lba: usize, sector_count: usize, label: &str) {
        if sector_count == 0 {
            return;
        }
        self.regions.push(SectorRegion {
            start_lba,
            sector_count,
            label: label.to_string(),
        });
    }

    /// Return the number of registered regions.
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Whether the tracker has no registered regions.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Check for overlapping regions. Returns `Ok(())` if no overlaps,
    /// or `Err` with a human-readable report of all collisions.
    pub fn check(&self) -> Result<(), String> {
        if self.regions.len() < 2 {
            return Ok(());
        }

        let mut sorted: Vec<_> = self.regions.iter().collect();
        sorted.sort_by_key(|r| (r.start_lba, r.start_lba + r.sector_count));

        let mut collisions = Vec::new();
        // Track the furthest-reaching region to detect containment overlaps.
        // Without this, A(100..200), B(110..115), C(150..160) would miss A↔C.
        let mut max_end = sorted[0].start_lba + sorted[0].sector_count;
        let mut max_region = sorted[0];
        for i in 1..sorted.len() {
            let curr = sorted[i];
            if max_end > curr.start_lba && max_region.label != curr.label {
                collisions.push(format!(
                    "  OVERLAP: [{}] LBA {:#06X}..{:#06X} vs [{}] LBA {:#06X}..{:#06X}",
                    max_region.label,
                    max_region.start_lba,
                    max_end,
                    curr.label,
                    curr.start_lba,
                    curr.start_lba + curr.sector_count,
                ));
            }
            let curr_end = curr.start_lba + curr.sector_count;
            if curr_end > max_end {
                max_end = curr_end;
                max_region = curr;
            }
        }

        if collisions.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Sector region collisions detected ({}):\n{}",
                collisions.len(),
                collisions.join("\n")
            ))
        }
    }

    /// Print all registered regions to stderr (debug aid).
    pub fn dump(&self) {
        eprintln!("=== Sector Region Map ({} regions) ===", self.regions.len());
        let mut sorted: Vec<_> = self.regions.iter().collect();
        sorted.sort_by_key(|r| r.start_lba);
        for r in &sorted {
            eprintln!("  {}", r);
        }
        eprintln!("===");
    }

    /// Return a reference to the registered regions.
    pub fn regions(&self) -> &[SectorRegion] {
        &self.regions
    }
}

#[cfg(test)]
#[path = "tracked_regions_tests.rs"]
mod tests;
