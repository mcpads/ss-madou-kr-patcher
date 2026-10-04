//! Development integration for explicitly audited PT skill-name references.
//! Runtime reachability is established separately; never infer new catalog sites.
use super::{
    appendix::append_referenced_block,
    patcher::{TextToken, TranslationEntry, encode_entry},
};
use anyhow::{Context, Result, ensure};
use std::{collections::HashMap, ops::Range};

// Test fixtures load at the usual PT address; real files use their header.
#[cfg(test)]
const LOAD: u32 = 0x0025_0000;
const RAM_END: usize = 0x0030_0000;
const NAME: [u8; 10] = [0x04, 0xee, 0x02, 0x16, 0x08, 0xa6, 0x01, 0xba, 0xff, 0x00];
const RECORD_TAIL: [u8; 8] = [0xff, 0x00, 0x01, 0x50, 0x64, 0xc8, 0, 0];
// Original JP offsets only. Recorded KO offsets from the diagnostic recipe are
// deliberately not part of this catalog: every build computes its own shifts.
const SITES: &[(&str, usize, usize)] = &[
    ("PARTY_06_0006", 0xb0c8, 0x5e80),
    ("PARTY_06_0007", 0xb0ec, 0x6d10),
    ("PT0305_0011", 0x8e10, 0x5b18),
    ("PT0305A_0006", 0x9400, 0x25dc),
    ("PT0703A_S0017", 0x6cac, 0x1c08),
    ("PT0802_0004", 0x9dcc, 0x3eb4),
    ("PT0805_0004", 0x4a4c, 0xb8c),
];

// Audited original JP ダイナマイトアロー records. Each name has exactly
// one aligned direct reference and no references into its interior.
const ARROW_NAME: [u8; 20] = [
    0x03, 0xda, 0x02, 0xfe, 0x03, 0x4a, 0x03, 0x72, 0x02, 0xfe, 0x03, 0x46, 0x02, 0xfa, 0x03, 0xa2,
    0x04, 0x82, 0xff, 0x00,
];
const ARROW_TAIL: [u8; 8] = [0xff, 0x00, 0x01, 0x64, 0x64, 0x64, 0, 0];
const ARROW_SITES: &[(&str, usize, usize)] = &[
    ("PARTY_02_0053", 0xe85c, 0x9fac),
    ("PT0303A_0005", 0x4634, 0x2204),
    ("PT0305_0007", 0x8cbc, 0x0330),
];

// Audited original JP 闇の剣よ 切り裂け records (Schezo as an enemy). Each name
// has exactly one aligned direct reference whose record tail matches the
// arrow records. PT0103/PT0104 load at 0x0025B000, the others at 0x00250000.
const SWORD_NAME: [u8; 20] = [
    0x04, 0x9a, 0x02, 0x16, 0x06, 0xee, 0x02, 0x4a, 0x00, 0xb2, 0x0a, 0x0a, 0x02, 0x52, 0x0e, 0x86,
    0x01, 0xd6, 0xff, 0x00,
];
const SWORD_SITES: &[(&str, usize, usize)] = &[
    ("PT0103_S0005", 0x3a20, 0x0088),
    ("PT0104_S0005", 0x3a20, 0x0088),
    ("PT0602_0004", 0xdc1c, 0x7f3c),
    ("PT0801_0005", 0xbdbc, 0x6140),
    ("PARTY_10_0028", 0xc584, 0x3c60),
];

pub(super) struct PendingName {
    id: String,
    slot: usize,
    reference: usize,
    replacement: Vec<u8>,
    original: Vec<u8>,
    load: u32,
}

fn slots(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut start = 0;
    let mut result = Vec::new();
    for i in (0..bytes.len().saturating_sub(1)).step_by(2) {
        if bytes[i..i + 2] == [0xff, 0] {
            result.push(start..i + 2);
            start = i + 2;
        }
    }
    if start < bytes.len() {
        result.push(start..bytes.len());
    }
    result
}

/// Keep the old JP slot available to existing patch/pointer logic, and retain
/// the full translated name for a later append. Other slots use existing rules.
pub(super) fn prepare(
    source: &[u8],
    entries: &[TranslationEntry],
    table: &HashMap<char, u16>,
) -> Result<(Vec<TranslationEntry>, Vec<PendingName>)> {
    let mut prepared = entries.to_vec();
    let mut pending = Vec::new();
    let load = super::patcher::pt_load_address(source);
    for entry in &mut prepared {
        let site = SITES
            .iter()
            .map(|&(id, slot, reference)| {
                (id, slot, reference, NAME.as_slice(), RECORD_TAIL.as_slice())
            })
            .chain(ARROW_SITES.iter().map(|&(id, slot, reference)| {
                (
                    id,
                    slot,
                    reference,
                    ARROW_NAME.as_slice(),
                    ARROW_TAIL.as_slice(),
                )
            }))
            .chain(SWORD_SITES.iter().map(|&(id, slot, reference)| {
                (
                    id,
                    slot,
                    reference,
                    SWORD_NAME.as_slice(),
                    ARROW_TAIL.as_slice(),
                )
            }));
        let Some((_, slot, reference, original_name, record_tail)) =
            site.into_iter().find(|s| s.0 == entry.entry_id)
        else {
            continue;
        };
        let end = entry
            .offset
            .checked_add(entry.orig_len)
            .context("entry extent overflow")?;
        let original = source
            .get(entry.offset..end)
            .context("catalog entry outside SEQ")?;
        ensure!(
            source.get(slot..slot + original_name.len()) == Some(original_name),
            "{}: catalog JP slot differs",
            entry.entry_id
        );
        let original_slots = slots(original);
        let index = original_slots
            .iter()
            .position(|r| entry.offset + r.start == slot && r.len() == original_name.len())
            .with_context(|| {
                format!(
                    "{}: catalog name is not a complete FF00 slot",
                    entry.entry_id
                )
            })?;
        let encoded = encode_entry(entry, table);
        let translated_slots = slots(&encoded);
        ensure!(
            original_slots.len() == translated_slots.len(),
            "{}: catalog slot count differs",
            entry.entry_id
        );
        let translated = translated_slots[index].clone();
        if translated.len() <= original_name.len() {
            continue;
        }
        ensure!(
            encoded[translated.end - 2..translated.end] == [0xff, 0],
            "{}: name lacks FF00",
            entry.entry_id
        );
        ensure!(
            source.get(reference + 4..reference + 12) == Some(record_tail),
            "{}: catalog reference record differs",
            entry.entry_id
        );
        let target = (load + slot as u32).to_be_bytes();
        let readers: Vec<_> = (0..source.len().saturating_sub(3))
            .step_by(2)
            .filter(|&i| source[i..i + 4] == target)
            .collect();
        ensure!(
            readers == [reference],
            "{}: catalog reference inventory differs",
            entry.entry_id
        );
        for interior in 1..original_name.len() {
            let target = (load + (slot + interior) as u32).to_be_bytes();
            ensure!(
                !(0..source.len().saturating_sub(3))
                    .step_by(2)
                    .any(|i| source[i..i + 4] == target),
                "{}: undeclared interior reference",
                entry.entry_id
            );
        }
        pending.push(PendingName {
            id: entry.entry_id.clone(),
            slot,
            reference,
            replacement: encoded[translated.clone()].to_vec(),
            original: original_name.to_vec(),
            load,
        });
        let mut inline = encoded;
        inline.splice(translated, original_name.iter().copied());
        ensure!(inline.len() % 2 == 0, "incomplete encoded word");
        entry.tokens = inline
            .chunks_exact(2)
            .map(|w| TextToken::Tile(u16::from_be_bytes([w[0], w[1]])))
            .collect();
    }
    Ok((prepared, pending))
}

pub(super) fn finish(
    mut result: Vec<u8>,
    pending: &[PendingName],
    map: impl Fn(usize) -> Result<usize>,
    overlaps: impl Fn(Range<usize>) -> bool,
) -> Result<Vec<u8>> {
    for name in pending {
        ensure!(
            !overlaps(name.reference..name.reference + 4),
            "{}: reference overlaps text patch",
            name.id
        );
        let slot = map(name.slot)?;
        let reference = map(name.reference)?;
        let end = result
            .len()
            .checked_add(name.replacement.len())
            .and_then(|n| n.checked_add(3))
            .context("appendix length overflow")?
            & !3;
        ensure!(
            end <= RAM_END - name.load as usize,
            "{}: appended SEQ exceeds Work RAM Low",
            name.id
        );
        result = append_referenced_block(
            &result,
            name.load,
            slot..slot + name.original.len(),
            &name.original,
            &[reference],
            &name.replacement,
        )
        .with_context(|| format!("{}: relocated name reference validation", name.id))?;
        // Padding follows FF00, outside the displayed name.
        result.resize((result.len() + 3) & !3, 0);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::patcher::{PatchOptions, SeqType, apply_patches};
    use super::*;

    fn entry(
        source: &[u8],
        id: &str,
        offset: usize,
        len: usize,
        bytes: &[u8],
        fixed: bool,
    ) -> TranslationEntry {
        TranslationEntry {
            offset,
            orig_len: len,
            entry_id: id.into(),
            tokens: bytes
                .chunks_exact(2)
                .map(|w| TextToken::Tile(u16::from_be_bytes([w[0], w[1]])))
                .collect(),
            expected_bytes: Some(source[offset..offset + len].to_vec()),
            pad_to_original: fixed,
        }
    }
    fn fixture() -> (Vec<u8>, TranslationEntry, Vec<u8>) {
        let (id, slot, reference) = SITES[0];
        let mut source = vec![0; slot + 32];
        source[reference..reference + 4].copy_from_slice(&(LOAD + slot as u32).to_be_bytes());
        source[reference + 4..reference + 12].copy_from_slice(&RECORD_TAIL);
        source[slot - 4..slot].copy_from_slice(&[0x01, 0x12, 0xff, 0]);
        source[slot..slot + 10].copy_from_slice(&NAME);
        source[slot + 10..slot + 14].copy_from_slice(&[0x01, 0x14, 0xff, 0]);
        let name = vec![
            1, 0x22, 1, 0x24, 1, 0x26, 1, 0x28, 1, 0x2a, 1, 0x2c, 1, 0x2e, 0xff, 0,
        ];
        let mut translated = source[slot - 4..slot].to_vec();
        translated.extend(&name);
        translated.extend(&source[slot + 10..slot + 14]);
        let item = entry(&source, id, slot - 4, 18, &translated, true);
        (source, item, name)
    }

    // PT0103/PT0104 load at 0x0025B000: event pointers into the text region
    // and the appended skill name must use that address, not 0x00250000.
    #[test]
    fn files_loaded_above_the_bank_start_relocate_with_their_own_address() {
        const HIGH: u32 = 0x0025_B000;
        let (_, slot, reference) = SWORD_SITES[0];
        let mut source = vec![0; slot + 0x80];
        source[4..8].copy_from_slice(&(HIGH + 0xc).to_be_bytes());
        source[slot..slot + SWORD_NAME.len()].copy_from_slice(&SWORD_NAME);
        source[reference..reference + 4].copy_from_slice(&(HIGH + slot as u32).to_be_bytes());
        source[reference + 4..reference + 12].copy_from_slice(&ARROW_TAIL);
        // Dialogue after the name grows by 4 bytes; an event record jumps past it.
        let dialogue = slot + SWORD_NAME.len();
        source[dialogue..dialogue + 4].copy_from_slice(&[1, 0x30, 0xff, 5]);
        let block = dialogue + 0x20;
        let event = 0x200;
        source[event..event + 4].copy_from_slice(&[0x41, 0x03, 0, 0]);
        source[event + 4..event + 8].copy_from_slice(&(HIGH + block as u32).to_be_bytes());
        source[event + 8..event + 12].copy_from_slice(&[0, 0, 0, 0x0b]);
        let mut name = vec![0x40, 0x01].repeat(10);
        name.extend([0xff, 0]);
        let entries = [
            entry(&source, SWORD_SITES[0].0, slot, SWORD_NAME.len(), &name, true),
            entry(&source, "PT0103_0000", dialogue, 4, &[1, 0x30, 1, 0x32, 1, 0x34, 0xff, 5], false),
        ];
        let (out, _) = apply_patches(
            &source,
            &entries,
            &HashMap::new(),
            SeqType::Pt,
            &PatchOptions::default(),
        )
        .unwrap();
        let read = |at: usize| u32::from_be_bytes(out[at..at + 4].try_into().unwrap());
        assert_eq!(read(event + 4), HIGH + block as u32 + 4);
        let appended = (read(reference) - HIGH) as usize;
        assert_eq!(appended, source.len() + 4);
        assert_eq!(&out[appended..appended + name.len()], &name);
        assert_eq!(&out[slot..slot + SWORD_NAME.len()], &SWORD_NAME);
    }

    #[test]
    fn arrow_names_append_without_changing_other_record_fields() {
        for &(id, slot, reference) in ARROW_SITES {
            for fixed in [false, true] {
                let mut source = vec![0; slot + 24];
                source[slot..slot + ARROW_NAME.len()].copy_from_slice(&ARROW_NAME);
                source[reference..reference + 4]
                    .copy_from_slice(&(LOAD + slot as u32).to_be_bytes());
                source[reference + 4..reference + 12].copy_from_slice(&ARROW_TAIL);
                let mut replacement = vec![0x40, 0x01].repeat(10);
                replacement.extend([0xff, 0]);
                let item = entry(&source, id, slot, ARROW_NAME.len(), &replacement, fixed);
                let (out, _) = apply_patches(
                    &source,
                    &[item],
                    &HashMap::new(),
                    SeqType::Pt,
                    &PatchOptions::default(),
                )
                .unwrap();
                let target = u32::from_be_bytes(out[reference..reference + 4].try_into().unwrap())
                    as usize
                    - LOAD as usize;
                assert_eq!(target, source.len());
                assert_eq!(&out[target..target + replacement.len()], &replacement);
                assert_eq!(&out[slot..slot + ARROW_NAME.len()], &ARROW_NAME);
                assert_eq!(&out[reference + 4..reference + 12], &ARROW_TAIL);
                assert_eq!(out.len() % 4, 0);
            }
        }
    }

    // A build with edits on both sides of a reference must relocate the field
    // and target independently while retaining every neighboring slot byte.
    #[test]
    fn earlier_translation_growth_moves_field_and_target_independently() {
        for earlier_len in [8, 12] {
            let (mut source, name_entry, replacement) = fixture();
            let (_, slot, reference) = SITES[0];
            let before = 0x100;
            let between = reference + 32;
            for at in [before, between] {
                source[at..at + 4].copy_from_slice(&[1, 0x30, 0xff, 5]);
            }
            let mut earlier = vec![1; earlier_len];
            earlier[earlier_len - 2..].copy_from_slice(&[0xff, 5]);
            let mut middle = vec![1; 12];
            middle[10..].copy_from_slice(&[0xff, 5]);
            let entries = vec![
                entry(&source, "PT_TEST_0001", before, 4, &earlier, false),
                entry(&source, "PT_TEST_0002", between, 4, &middle, false),
                name_entry,
            ];
            let (out, _) = apply_patches(
                &source,
                &entries,
                &HashMap::new(),
                SeqType::Pt,
                &PatchOptions::default(),
            )
            .unwrap();
            let field_shift = earlier_len - 4;
            let target_shift = field_shift + 8;
            let field = reference + field_shift;
            let appended = u32::from_be_bytes(out[field..field + 4].try_into().unwrap()) as usize
                - LOAD as usize;
            assert_eq!(appended, source.len() + target_shift);
            assert_eq!(&out[appended..appended + replacement.len()], &replacement);
            assert_eq!(
                &out[slot - 4 + target_shift..slot + 14 + target_shift],
                &source[slot - 4..slot + 14]
            );
            assert_eq!(&out[field + 4..field + 12], &RECORD_TAIL);
            assert_eq!(out.len() % 4, 0);
        }
    }

    #[test]
    fn invalid_source_declarations_and_disabled_relocation_are_rejected() {
        let (source, name, _) = fixture();
        let (_, slot, reference) = SITES[0];
        for changed in [slot, reference + 3, reference + 6] {
            let mut bad = source.clone();
            bad[changed] ^= 2;
            assert!(
                apply_patches(
                    &bad,
                    &[name.clone()],
                    &HashMap::new(),
                    SeqType::Pt,
                    &PatchOptions::default()
                )
                .is_err()
            );
        }
        let opts = PatchOptions {
            skip_script_ptrs: true,
            ..Default::default()
        };
        assert!(
            apply_patches(
                &source,
                &[name.clone()],
                &HashMap::new(),
                SeqType::Pt,
                &opts
            )
            .is_err()
        );
        let overlap = entry(
            &source,
            "PT_OVERLAP",
            reference,
            4,
            &source[reference..reference + 4],
            false,
        );
        assert!(
            apply_patches(
                &source,
                &[name, overlap],
                &HashMap::new(),
                SeqType::Pt,
                &PatchOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn uncatalogued_fixed_overflow_still_fails_and_short_names_stay_inline() {
        let (source, mut name, _) = fixture();
        name.entry_id = "PT_UNKNOWN_0001".into();
        assert!(
            apply_patches(
                &source,
                &[name],
                &HashMap::new(),
                SeqType::Pt,
                &PatchOptions::default()
            )
            .is_err()
        );
        let (id, slot, _) = SITES[0];
        let unchanged = entry(
            &source,
            id,
            slot - 4,
            18,
            &source[slot - 4..slot + 14],
            true,
        );
        let opts = PatchOptions {
            skip_script_ptrs: true,
            ..Default::default()
        };
        let (out, count) =
            apply_patches(&source, &[unchanged], &HashMap::new(), SeqType::Pt, &opts).unwrap();
        assert_eq!(out, source);
        assert_eq!(count, 0);
    }
}
