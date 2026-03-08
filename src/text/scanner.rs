use super::sjis::{is_sjis_lead_byte, is_sjis_single, is_sjis_trail_byte};
use encoding_rs::SHIFT_JIS;

/// A text string found during scanning.
#[derive(Debug, Clone)]
pub struct StringMatch {
    /// Byte offset within the input data.
    pub offset: usize,
    /// Raw bytes of the matched string.
    pub raw: Vec<u8>,
    /// Decoded UTF-8 text.
    pub text: String,
    /// Number of logical characters.
    pub char_count: usize,
}

/// Scan binary data for Shift-JIS text strings.
///
/// Returns all runs of valid Shift-JIS characters (double-byte or single-byte)
/// that contain at least `min_chars` logical characters. Strings are terminated
/// by a null byte (0x00) or any byte that is not valid Shift-JIS.
pub fn scan_strings(data: &[u8], min_chars: usize) -> Vec<StringMatch> {
    let mut results = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let b = data[i];

        // Try to start a string here
        if is_sjis_lead_byte(b) || is_sjis_single(b) {
            let start = i;
            let mut char_count = 0;
            let mut j = i;

            while j < data.len() {
                let c = data[j];

                if c == 0x00 {
                    break; // null terminator
                }

                if is_sjis_lead_byte(c) {
                    if j + 1 < data.len() && is_sjis_trail_byte(data[j + 1]) {
                        char_count += 1;
                        j += 2;
                        continue;
                    }
                    break; // invalid trail byte
                } else if is_sjis_single(c) {
                    char_count += 1;
                    j += 1;
                    continue;
                }

                break; // non-SJIS byte
            }

            if char_count >= min_chars {
                let raw = data[start..j].to_vec();
                let (decoded, _, _) = SHIFT_JIS.decode(&raw);
                let text = decoded.into_owned();
                results.push(StringMatch {
                    offset: start,
                    raw,
                    text,
                    char_count,
                });
            }

            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }

    results
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
