//! Append a complete text block without shifting existing SEQ structures.
//!
//! Callers must establish the complete block (including control arguments) and
//! its real reference fields. This module validates declared bytes and addresses;
//! it does not infer pointers or prove runtime reachability.

use anyhow::{Result, ensure};
use std::ops::Range;

/// Preserve the source block and redirect only explicitly declared 32-bit BE
/// references to a replacement at EOF. All offsets are relative to this SEQ.
pub fn append_referenced_block(
    source: &[u8],
    load_address: u32,
    block: Range<usize>,
    expected_block: &[u8],
    references: &[usize],
    replacement: &[u8],
) -> Result<Vec<u8>> {
    ensure!(
        block.start < block.end && block.end <= source.len(),
        "invalid source block"
    );
    ensure!(
        source.get(block.clone()) == Some(expected_block),
        "source block bytes differ"
    );
    ensure!(
        source.len() % 2 == 0 && block.start % 2 == 0 && block.end % 2 == 0,
        "unaligned source block or EOF"
    );
    ensure!(
        !replacement.is_empty() && replacement.len() % 2 == 0,
        "replacement must contain complete words"
    );
    ensure!(!references.is_empty(), "no declared references");
    let old_target = load_address
        .checked_add(u32::try_from(block.start)?)
        .ok_or_else(|| anyhow::anyhow!("source address overflow"))?;
    let new_target = load_address
        .checked_add(u32::try_from(source.len())?)
        .ok_or_else(|| anyhow::anyhow!("appendix address overflow"))?;
    let new_len = source
        .len()
        .checked_add(replacement.len())
        .ok_or_else(|| anyhow::anyhow!("appendix size overflow"))?;
    load_address
        .checked_add(u32::try_from(new_len - 1)?)
        .ok_or_else(|| anyhow::anyhow!("appendix end address overflow"))?;
    let mut fields = references.to_vec();
    fields.sort_unstable();
    for (i, &offset) in fields.iter().enumerate() {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| anyhow::anyhow!("reference offset overflow"))?;
        ensure!(
            offset % 2 == 0 && end <= source.len(),
            "invalid reference field at {offset:#x}"
        );
        ensure!(
            end <= block.start || offset >= block.end,
            "reference overlaps source block"
        );
        ensure!(
            i == 0 || fields[i - 1] + 4 <= offset,
            "overlapping reference fields"
        );
        ensure!(
            source[offset..end] == old_target.to_be_bytes(),
            "reference at {offset:#x} does not point to source block"
        );
    }
    let mut result = source.to_vec();
    result.extend_from_slice(replacement);
    for offset in fields {
        result[offset..offset + 4].copy_from_slice(&new_target.to_be_bytes());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_block_preserves_old_layout_and_redirects_all_declared_readers() {
        let source = [
            0x00, 0x25, 0x00, 0x0c, 0xab, 0xcd, 0x00, 0x25, 0x00, 0x0c, 0x12, 0x34, 0x01, 0xba,
            0xff, 0x00,
        ];
        let body = [0x01, 0xba, 0x00, 0xb2, 0x01, 0xbe, 0xff, 0x00];
        let out = append_referenced_block(&source, 0x250000, 12..16, &source[12..], &[0, 6], &body)
            .unwrap();
        assert_eq!(&out[16..], &body);
        assert_eq!(&out[0..4], &0x250010u32.to_be_bytes());
        assert_eq!(&out[6..10], &0x250010u32.to_be_bytes());
        assert_eq!(&out[4..6], &source[4..6]);
        assert_eq!(&out[10..16], &source[10..]);
    }

    #[test]
    fn stale_or_overlapping_declarations_are_rejected() {
        let source = [0x00, 0x25, 0x00, 0x04, 0xff, 0x00];
        assert!(
            append_referenced_block(&source, 0x250000, 4..6, &[0xff, 0x01], &[0], &[0xff, 0])
                .is_err()
        );
        assert!(
            append_referenced_block(&source, 0x240000, 4..6, &[0xff, 0], &[0], &[0xff, 0]).is_err()
        );
        assert!(
            append_referenced_block(&source, 0x250000, 4..6, &[0xff, 0], &[0, 0], &[0xff, 0])
                .is_err()
        );
        assert!(
            append_referenced_block(&source, 0x250000, 4..6, &[0xff, 0], &[0], &[0xff]).is_err()
        );
    }

    #[test]
    fn appendix_target_carries_across_the_low_word_boundary() {
        let mut source = vec![0; 0x10002];
        source[..4].copy_from_slice(&0x250008u32.to_be_bytes());
        source[8..10].copy_from_slice(&[0xff, 0]);
        let out = append_referenced_block(&source, 0x250000, 8..10, &[0xff, 0], &[0], &[0xff, 0])
            .unwrap();
        assert_eq!(&out[..4], &0x260002u32.to_be_bytes());
    }
}
