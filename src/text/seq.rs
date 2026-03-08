use super::scanner::{StringMatch, scan_strings};
use std::fmt;

/// A potential pointer found in an offset table at the start of SEQ data.
#[derive(Debug, Clone)]
pub struct TextPointer {
    /// Byte offset of the pointer value itself within the SEQ data.
    pub table_offset: usize,
    /// The target offset the pointer points to within the SEQ data.
    pub target_offset: u32,
}

/// Analysis results for a single SEQ file.
#[derive(Debug)]
pub struct SeqAnalysis {
    /// Total size of the decompressed SEQ data.
    pub size: usize,
    /// Detected offset table entries (if any).
    pub offset_table: Vec<TextPointer>,
    /// Text strings found in the data.
    pub strings: Vec<StringMatch>,
}

impl fmt::Display for SeqAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SEQ Analysis ({} bytes):", self.size)?;
        writeln!(f, "  Offset table: {} entries", self.offset_table.len())?;
        writeln!(f, "  Text strings: {} found", self.strings.len())?;

        if !self.offset_table.is_empty() {
            writeln!(f, "\n  Offset table:")?;
            for ptr in &self.offset_table {
                writeln!(
                    f,
                    "    [0x{:04X}] -> 0x{:04X}",
                    ptr.table_offset, ptr.target_offset
                )?;
            }
        }

        if !self.strings.is_empty() {
            writeln!(f, "\n  Strings:")?;
            for s in &self.strings {
                writeln!(
                    f,
                    "    0x{:06X} ({:3} chars): {}",
                    s.offset, s.char_count, s.text
                )?;
            }
        }

        Ok(())
    }
}

/// Detect a potential offset table at the start of SEQ data.
///
/// Heuristic: read consecutive big-endian u16 values starting at offset 0.
/// Each value should be:
/// - Greater than or equal to the previous value (roughly ascending)
/// - Less than `data.len()` (points within the file)
/// - Non-zero
///
/// The table ends when a value violates these constraints or when we encounter
/// a value that points within the table area itself (self-referencing).
pub fn detect_offset_table(data: &[u8]) -> Vec<TextPointer> {
    if data.len() < 4 {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let mut offset = 0;
    let mut prev_target: u32 = 0;

    while offset + 1 < data.len() {
        let target = u16::from_be_bytes([data[offset], data[offset + 1]]) as u32;

        if target == 0 {
            break;
        }

        if target < (offset + 2) as u32 {
            // Points within the table area already read — table is done
            break;
        }

        if target >= data.len() as u32 {
            break;
        }

        if target < prev_target {
            break;
        }

        entries.push(TextPointer {
            table_offset: offset,
            target_offset: target,
        });

        prev_target = target;
        offset += 2;
    }

    // Need at least 2 entries to be considered a valid table
    if entries.len() < 2 {
        return Vec::new();
    }

    entries
}

/// Analyze a decompressed SEQ file.
pub fn analyze_seq(data: &[u8]) -> SeqAnalysis {
    let offset_table = detect_offset_table(data);
    let strings = scan_strings(data, 3);

    SeqAnalysis {
        size: data.len(),
        offset_table,
        strings,
    }
}

#[cfg(test)]
#[path = "seq_tests.rs"]
mod tests;
