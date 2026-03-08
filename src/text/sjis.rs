/// Check if a byte is a Shift-JIS lead byte (first byte of a 2-byte character).
pub fn is_sjis_lead_byte(b: u8) -> bool {
    (0x81..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b)
}

/// Check if a byte is a Shift-JIS trail byte (second byte of a 2-byte character).
pub fn is_sjis_trail_byte(b: u8) -> bool {
    (0x40..=0x7E).contains(&b) || (0x80..=0xFC).contains(&b)
}

/// Check if a byte is a printable ASCII character (0x20-0x7E).
pub fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

/// Check if a byte is a half-width katakana (0xA1-0xDF).
pub fn is_halfwidth_katakana(b: u8) -> bool {
    (0xA1..=0xDF).contains(&b)
}

/// Check if a byte is a valid single-byte Shift-JIS character
/// (printable ASCII or half-width katakana).
pub fn is_sjis_single(b: u8) -> bool {
    is_printable_ascii(b) || is_halfwidth_katakana(b)
}

#[cfg(test)]
#[path = "sjis_tests.rs"]
mod tests;
