//! Resident, lossless 8×8 tile sharing for the FONT.CEL text path.
//!
//! Virtual glyphs still occupy four consecutive *codes*. The PND writer maps
//! each code to an independently stored tile, so no displayed tile is evicted.
use anyhow::{Result, ensure};
use sh2_asm::{Assembler, Instruction as I, Register as R};
use std::collections::{BTreeMap, HashMap};

pub const VIRTUAL_START: u16 = 0x4000;
pub const CODE_OFFSET: usize = 0x18000;
const DIRECT_CODE_OFFSET: usize = 0x18400;
pub const MAP_OFFSET: usize = 0x19000;
const FONT_ADDRESS: u32 = 0x25e60000;
const FONT_SIZE: usize = 122_048;
const MAP_END: usize = 438 * 32 + 832 * 128;

/// Runtime particle markers, authored as `{josa:을}` etc. Each marker is a
/// pseudo-glyph after the real glyphs. The mapper replaces it with the
/// final-consonant form or the no-final form, chosen from the last real
/// glyph it mapped. The pairs are (marker, after a final consonant, otherwise).
pub const JOSA_MARKERS: [(char, char, char); 4] = [
    ('\u{E000}', '을', '를'),
    ('\u{E001}', '이', '가'),
    ('\u{E002}', '은', '는'),
    ('\u{E003}', '과', '와'),
];

/// Authoring form → marker character.
pub fn josa_marker(form: &str) -> Option<char> {
    JOSA_MARKERS
        .iter()
        .find(|(_, final_form, _)| form.chars().eq([*final_form]))
        .map(|(marker, ..)| *marker)
}

pub fn is_josa_marker(ch: char) -> bool {
    JOSA_MARKERS.iter().any(|(marker, ..)| *marker == ch)
}

/// 0 = no final consonant, 1 = final ㄹ, 2 = another final consonant.
/// Characters outside the Hangul syllable block count as no final consonant.
fn final_class(ch: char) -> u16 {
    match ch as u32 {
        code @ 0xac00..=0xd7a3 => match (code - 0xac00) % 28 {
            0 => 0,
            8 => 1,
            _ => 2,
        },
        _ => 0,
    }
}

/// FONT offsets of the particle tables that follow the quadrant map.
struct JosaLayout {
    normal_end: u32,
    marker_end: u32,
    class: usize,
    pairs: usize,
    scratch: usize,
}

impl JosaLayout {
    fn new(glyphs: usize) -> Self {
        let class = (MAP_OFFSET + glyphs * 8 + 3) & !3;
        // One extra zero class word: the scratch starts at `normal_end`, so a
        // marker before any real glyph selects the no-final form.
        let pairs = (class + (glyphs + 1) * 2 + 3) & !3;
        let scratch = pairs + JOSA_MARKERS.len() * 4;
        let normal_end = VIRTUAL_START as u32 + glyphs as u32 * 4;
        Self {
            normal_end,
            marker_end: normal_end + JOSA_MARKERS.len() as u32 * 4,
            class,
            pairs,
            scratch,
        }
    }

    fn end(&self) -> usize {
        self.scratch + 2
    }
}

pub struct SharedFont {
    pub bytes: Vec<u8>,
    pub char_table: HashMap<char, u16>,
    pub unique_tiles: usize,
}

/// Preserve original UI/symbol tiles and reconstruct every rendered glyph
/// exactly, including its outline and quadrant order.
pub fn pack(source: &[u8], glyphs: &[(char, [u8; 128])]) -> Result<SharedFont> {
    ensure!(
        source.len() >= FONT_SIZE,
        "FONT.CEL is shorter than the original layout"
    );
    ensure!(!glyphs.is_empty(), "shared font requires glyphs");
    let layout = JosaLayout::new(glyphs.len());
    ensure!(
        layout.marker_end <= 0xfeff,
        "virtual font codes overlap script controls"
    );
    ensure!(
        layout.end() <= MAP_END,
        "shared font map overlaps preserved icons"
    );
    let mut bytes = source[..FONT_SIZE].to_vec();
    let preserve = crate::text::patcher::preserved_glyph_slots();
    let mut available =
        (438..CODE_OFFSET / 32).filter(|tile| !preserve.contains(&((tile - 438) / 4)));
    let mut tiles = BTreeMap::<[u8; 32], u16>::new();
    let mut char_table = HashMap::new();
    for (index, (ch, bitmap)) in glyphs.iter().enumerate() {
        ensure!(
            char_table
                .insert(*ch, VIRTUAL_START + index as u16 * 4)
                .is_none(),
            "duplicate glyph {ch}"
        );
        for (quadrant, raw) in bitmap.chunks_exact(32).enumerate() {
            let tile: [u8; 32] = raw.try_into()?;
            let number = if let Some(&number) = tiles.get(&tile) {
                number
            } else {
                let number = available
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("shared tiles exceed their FONT.CEL region"))?;
                bytes[number * 32..number * 32 + 32].copy_from_slice(raw);
                tiles.insert(tile, number as u16);
                number as u16
            };
            let offset = MAP_OFFSET + index * 8 + quadrant * 2;
            bytes[offset..offset + 2].copy_from_slice(&number.to_be_bytes());
        }
    }
    for (index, (ch, _)) in glyphs.iter().enumerate() {
        let offset = layout.class + index * 2;
        bytes[offset..offset + 2].copy_from_slice(&final_class(*ch).to_be_bytes());
    }
    bytes[layout.class + glyphs.len() * 2..layout.class + glyphs.len() * 2 + 2].fill(0);
    for (index, (marker, final_form, plain_form)) in JOSA_MARKERS.iter().enumerate() {
        char_table.insert(*marker, (layout.normal_end + index as u32 * 4) as u16);
        for (slot, form) in [final_form, plain_form].into_iter().enumerate() {
            // A marker needs both forms; a font without them cannot draw particles.
            let code = char_table.get(form).copied().unwrap_or(0);
            let offset = layout.pairs + index * 4 + slot * 2;
            bytes[offset..offset + 2].copy_from_slice(&code.to_be_bytes());
        }
    }
    bytes[layout.scratch..layout.scratch + 2].copy_from_slice(&(layout.normal_end as u16).to_be_bytes());
    let code = pnd_mapper(&layout)?;
    ensure!(
        CODE_OFFSET + code.len() <= DIRECT_CODE_OFFSET,
        "font mapper overlaps direct writer"
    );
    bytes[CODE_OFFSET..CODE_OFFSET + code.len()].copy_from_slice(&code);
    let direct = direct_quad_mapper(&layout)?;
    ensure!(
        DIRECT_CODE_OFFSET + direct.len() <= MAP_OFFSET,
        "direct writer overlaps its table"
    );
    bytes[DIRECT_CODE_OFFSET..DIRECT_CODE_OFFSET + direct.len()].copy_from_slice(&direct);
    // Validate the delivered representation, not merely the deduplication map.
    for (index, (_, bitmap)) in glyphs.iter().enumerate() {
        for q in 0..4 {
            let offset = MAP_OFFSET + index * 8 + q * 2;
            let tile = u16::from_be_bytes(bytes[offset..offset + 2].try_into()?) as usize;
            ensure!(
                bytes[tile * 32..tile * 32 + 32] == bitmap[q * 32..q * 32 + 32],
                "shared glyph reconstruction differs"
            );
        }
    }
    Ok(SharedFont {
        bytes,
        char_table,
        unique_tiles: tiles.len(),
    })
}

/// Literal pool shared by both mappers.
const LIT_VIRTUAL_START: u8 = 0;
const LIT_MARKER_END: u8 = 1;
const LIT_MAP: u8 = 2;
const LIT_MASK: u8 = 3;
const LIT_NORMAL_END: u8 = 4;
const LIT_SCRATCH: u8 = 5;
const LIT_CLASS: u8 = 6;
const LIT_PAIRS: u8 = 7;
const LIT_DIRECT_RETURN: u8 = 8;

fn literals(layout: &JosaLayout) -> Vec<u32> {
    vec![
        VIRTUAL_START as u32,
        layout.marker_end,
        FONT_ADDRESS + MAP_OFFSET as u32,
        0xfff,
        layout.normal_end,
        FONT_ADDRESS + layout.scratch as u32,
        FONT_ADDRESS + layout.class as u32,
        FONT_ADDRESS + layout.pairs as u32,
        0x060348d4,
    ]
}

/// R1 = particle marker code (R2 = VIRTUAL_START, R3 = normal end) →
/// R1 = particle glyph code of the same quadrant. Uses R0 and R3; the T bit
/// is clobbered. Ends by reloading R2 = VIRTUAL_START.
fn resolve_marker(a: &mut Assembler) {
    a.emit(I::MovReg { rm: R::R1, rn: R::R0 });
    a.emit(I::AndImm { imm: 3 }); // R0 = quadrant
    a.emit(I::Sub { rm: R::R3, rn: R::R1 });
    a.emit(I::Shlr2 { rn: R::R1 });
    a.emit(I::Shll2 { rn: R::R1 }); // R1 = marker index * 4 (pair table bytes)
    a.emit(I::MovLPcRel { disp: LIT_SCRATCH, rn: R::R3 });
    a.emit(I::MovWLoad { rm: R::R3, rn: R::R3 });
    a.emit(I::ExtuW { rm: R::R3, rn: R::R3 });
    a.emit(I::Sub { rm: R::R2, rn: R::R3 });
    a.emit(I::Shlr2 { rn: R::R3 });
    a.emit(I::Add { rm: R::R3, rn: R::R3 }); // previous glyph index * 2
    a.emit(I::MovLPcRel { disp: LIT_CLASS, rn: R::R2 });
    a.emit(I::Add { rm: R::R2, rn: R::R3 });
    a.emit(I::MovWLoad { rm: R::R3, rn: R::R3 });
    a.emit(I::Tst { rm: R::R3, rn: R::R3 }); // T = no final consonant
    a.emit(I::MovT { rn: R::R3 });
    a.emit(I::Add { rm: R::R3, rn: R::R3 });
    a.emit(I::Add { rm: R::R3, rn: R::R1 });
    a.emit(I::MovLPcRel { disp: LIT_PAIRS, rn: R::R2 });
    a.emit(I::Add { rm: R::R2, rn: R::R1 });
    a.emit(I::MovWLoad { rm: R::R1, rn: R::R1 });
    a.emit(I::ExtuW { rm: R::R1, rn: R::R1 });
    a.emit(I::Add { rm: R::R0, rn: R::R1 });
    a.emit(I::MovLPcRel { disp: LIT_VIRTUAL_START, rn: R::R2 });
}

/// Classify R1 (zero-extended code, R2 = VIRTUAL_START). A real glyph is
/// recorded as the particle context and continues at `glyph`; a marker is
/// resolved first; anything else continues at `native`.
fn classify(a: &mut Assembler, native: &str, glyph: &str, marker: &str) {
    a.emit(I::CmpHs { rm: R::R2, rn: R::R1 });
    a.bf(native);
    a.emit(I::MovLPcRel { disp: LIT_MARKER_END, rn: R::R3 });
    a.emit(I::CmpHs { rm: R::R3, rn: R::R1 });
    a.bt(native);
    a.emit(I::MovLPcRel { disp: LIT_NORMAL_END, rn: R::R3 });
    a.emit(I::CmpHs { rm: R::R3, rn: R::R1 });
    a.bt(marker);
    a.emit(I::MovLPcRel { disp: LIT_SCRATCH, rn: R::R3 });
    a.bra(glyph);
    a.emit(I::MovWStore { rm: R::R1, rn: R::R3 });
}

/// Resolve R1 and perform the displaced PND calculation/write. R0, SR, and
/// all non-output registers are preserved. Original output registers are R1–3
/// and MACL; the enclosing writer already saves the caller's PR.
fn pnd_mapper(layout: &JosaLayout) -> Result<Vec<u8>> {
    let mut a = Assembler::new();
    a.emit(I::MovLStorePreDec { rm: R::R0, rn: R::R15 });
    a.emit(I::StcLSr { rn: R::R15 });
    a.emit(I::ExtuW { rm: R::R1, rn: R::R1 });
    a.emit(I::MovLPcRel { disp: LIT_VIRTUAL_START, rn: R::R2 });
    classify(&mut a, "native", "glyph", "marker");
    a.label("marker");
    resolve_marker(&mut a);
    a.label("glyph");
    a.emit(I::Sub { rm: R::R2, rn: R::R1 });
    a.emit(I::Add { rm: R::R1, rn: R::R1 });
    a.emit(I::MovLPcRel { disp: LIT_MAP, rn: R::R2 });
    a.emit(I::Add { rm: R::R2, rn: R::R1 });
    a.emit(I::MovWLoad { rm: R::R1, rn: R::R1 });
    a.label("native");
    a.emit(I::MulU { rm: R::R12, rn: R::R1 });
    a.emit(I::MovLLoad { rm: R::R11, rn: R::R3 });
    a.emit(I::StsMacl { rn: R::R2 });
    a.emit(I::Add { rm: R::R13, rn: R::R2 });
    a.emit(I::MovLPcRel { disp: LIT_MASK, rn: R::R1 });
    a.emit(I::And { rm: R::R1, rn: R::R2 });
    a.emit(I::LdcLSr { rm: R::R15 });
    a.emit(I::MovLLoadPostInc { rm: R::R15, rn: R::R0 });
    a.emit(I::MovWR0StoreIndexed { rm: R::R2, rn: R::R3 });
    a.emit(I::Rts);
    a.emit(I::Nop);
    assemble_with_literals(a, FONT_ADDRESS + CODE_OFFSET as u32, &literals(layout))
}

// The original direct writer (06034780) computes the plane, palette, font
// depth and coordinates before dispatching to its 2×2 branch. Replace only
// that branch; its original single-cell and wide-character paths remain valid.
// R12 holds the top-left code. A particle marker is resolved once, before the
// four quadrants. R0 is saved around the resolver, which needs it.
fn direct_quad_mapper(layout: &JosaLayout) -> Result<Vec<u8>> {
    let mut a = Assembler::new();
    a.emit(I::StcLSr { rn: R::R15 });
    a.emit(I::MovReg { rm: R::R12, rn: R::R4 });
    a.emit(I::MovReg { rm: R::R5, rn: R::R6 });
    a.emit(I::ExtuW { rm: R::R4, rn: R::R1 });
    a.emit(I::MovLPcRel { disp: LIT_VIRTUAL_START, rn: R::R2 });
    classify(&mut a, "quads", "quads", "marker");
    a.label("marker");
    a.emit(I::MovLStorePreDec { rm: R::R0, rn: R::R15 });
    resolve_marker(&mut a);
    a.emit(I::MovLLoadPostInc { rm: R::R15, rn: R::R0 });
    a.emit(I::MovReg { rm: R::R1, rn: R::R4 });
    a.label("quads");
    for q in 0..4 {
        let native = format!("native_{q}");
        a.emit(I::ExtuW { rm: R::R4, rn: R::R1 });
        a.emit(I::MovLPcRel { disp: LIT_VIRTUAL_START, rn: R::R2 });
        a.emit(I::CmpHs { rm: R::R2, rn: R::R1 });
        a.bf(&native);
        a.emit(I::MovLPcRel { disp: LIT_MARKER_END, rn: R::R3 });
        a.emit(I::CmpHs { rm: R::R3, rn: R::R1 });
        a.bt(&native);
        a.emit(I::Sub { rm: R::R2, rn: R::R1 });
        a.emit(I::Add { rm: R::R1, rn: R::R1 });
        a.emit(I::MovLPcRel { disp: LIT_MAP, rn: R::R2 });
        a.emit(I::Add { rm: R::R2, rn: R::R1 });
        a.emit(I::MovWLoad { rm: R::R1, rn: R::R1 });
        a.label(native);
        a.emit(I::MulU { rm: R::R8, rn: R::R1 });
        a.emit(I::StsMacl { rn: R::R1 });
        a.emit(I::Add { rm: R::R9, rn: R::R1 });
        a.emit(I::MovLPcRel { disp: LIT_MASK, rn: R::R2 });
        a.emit(I::And { rm: R::R2, rn: R::R1 });
        a.emit(I::Or { rm: R::R10, rn: R::R1 });
        a.emit(I::MovWStore { rm: R::R1, rn: R::R6 });
        if q < 3 {
            a.emit(I::AddImm { imm: 1, rn: R::R4 });
            a.emit(I::AddImm {
                imm: if q == 1 { 126 } else { 2 },
                rn: R::R6,
            });
        }
    }
    a.emit(I::LdcLSr { rm: R::R15 });
    a.emit(I::MovLPcRel { disp: LIT_DIRECT_RETURN, rn: R::R1 });
    a.emit(I::Jmp { rm: R::R1 });
    a.emit(I::MovReg { rm: R::R5, rn: R::R0 });
    assemble_with_literals(a, FONT_ADDRESS + DIRECT_CODE_OFFSET as u32, &literals(layout))
}

// Reuse the pinned typed assembler for encoding, branch labels, delay slots
// and byte/semantic round trips. Placeholder disp fields name literal entries.
fn assemble_with_literals(a: Assembler, origin: u32, constants: &[u32]) -> Result<Vec<u8>> {
    let program = a.assemble(origin)?;
    let mut instructions = program.instructions().to_vec();
    let pool_offset = (program.bytes().len() + 3) & !3;
    for (index, op) in instructions.iter_mut().enumerate() {
        if let I::MovLPcRel { disp, .. } = op {
            let target = origin + pool_offset as u32 + *disp as u32 * 4;
            *disp = ((target - ((origin + index as u32 * 2 + 4) & !3)) / 4).try_into()?;
        }
    }
    let mut checked = Assembler::new();
    checked.emit_all(instructions);
    let mut bytes = checked.assemble(origin)?.bytes().to_vec();
    bytes.resize(pool_offset, 0);
    for value in constants {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    Ok(bytes)
}

/// Guarded hooks for the two multi-cell branches of 060343C4.
/// Other renderers are deliberately not inferred to share this path.
pub fn patch_pnd_writer(first_read: &mut [u8]) -> Result<()> {
    for (address, source_reg, expected) in [
        (
            0x060344b8u32,
            R::R7,
            [
                0x6171, 0x21ce, 0x63b2, 0x021a, 0x32dc, 0x9160, 0x2219, 0x0325,
            ],
        ),
        (
            0x0603452eu32,
            R::R6,
            [
                0x6161, 0x21ce, 0x63b2, 0x021a, 0x32dc, 0x9125, 0x2219, 0x0325,
            ],
        ),
    ] {
        let offset = (address - 0x06004000) as usize;
        let original: Vec<_> = expected.into_iter().flat_map(u16::to_be_bytes).collect();
        ensure!(
            first_read.get(offset..offset + 16) == Some(original.as_slice()),
            "PND hook original differs at {address:08X}"
        );
        let literal = (address + 13) & !3;
        let mut a = Assembler::new();
        a.emit(I::MovLPcRel {
            disp: ((literal - ((address + 4) & !3)) / 4) as u8,
            rn: R::R1,
        });
        a.emit(I::Jsr { rm: R::R1 });
        a.emit(I::MovWLoad {
            rm: source_reg,
            rn: R::R1,
        });
        a.emit(I::Bra { disp: 3 }); // address+6+4+6 = address+16
        a.emit(I::Nop);
        let mut patch = a.assemble(address)?.bytes().to_vec();
        while patch.len() < (literal - address) as usize {
            patch.extend_from_slice(&sh2_asm::encode_word(&I::Nop)?.to_be_bytes());
        }
        patch.extend_from_slice(&(FONT_ADDRESS + CODE_OFFSET as u32).to_be_bytes());
        while patch.len() < 16 {
            patch.extend_from_slice(&sh2_asm::encode_word(&I::Nop)?.to_be_bytes());
        }
        ensure!(patch.len() == 16, "PND trampoline length differs");
        first_read[offset..offset + 16].copy_from_slice(&patch);
    }
    let offset = 0x06034886 - 0x06004000;
    let expected: Vec<_> = [0x7101u16, 0x641d, 0x6343, 0x922e, 0x6063, 0x6753, 0x2c8e]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    ensure!(
        first_read.get(offset..offset + 14) == Some(expected.as_slice()),
        "direct writer hook original differs"
    );
    let mut a = Assembler::new();
    a.emit(I::MovLPcRel { disp: 2, rn: R::R1 }); // literal at 06034890
    a.emit(I::Jmp { rm: R::R1 });
    a.emit(I::Nop);
    a.emit(I::Nop);
    a.emit(I::Nop);
    let mut patch = a.assemble(0x06034886)?.bytes().to_vec();
    patch.extend_from_slice(&(FONT_ADDRESS + DIRECT_CODE_OFFSET as u32).to_be_bytes());
    first_read[offset..offset + 14].copy_from_slice(&patch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sharing must preserve every pixel and all engine-owned symbol slots.
    #[test]
    fn shared_quadrants_reconstruct_without_overwriting_symbols() {
        let source = vec![0xa5; FONT_SIZE];
        let a = [0x16; 128];
        let mut b = a;
        b[32..64].fill(0x61);
        let font = pack(&source, &[('가', a), ('나', b)]).unwrap();
        assert_eq!(font.unique_tiles, 2);
        for slot in crate::text::patcher::preserved_glyph_slots()
            .into_iter()
            .filter(|&s| s < 844)
        {
            let offset = 438 * 32 + slot * 128;
            assert_eq!(
                &font.bytes[offset..offset + 128],
                &source[offset..offset + 128]
            );
        }
        assert_ne!(font.char_table[&'가'], font.char_table[&'나']);
    }

    // Markers follow the real glyphs; each pair names the two particle glyphs
    // and every glyph carries the final-consonant class the mapper reads.
    #[test]
    fn particle_markers_point_at_both_forms_and_classify_glyphs() {
        let source = vec![0; FONT_SIZE];
        let forms = ['을', '를', '이', '가', '은', '는', '과', '와', '달', '책'];
        let glyphs: Vec<_> = forms
            .iter()
            .enumerate()
            .map(|(i, &ch)| (ch, [i as u8 + 1; 128]))
            .collect();
        let font = pack(&source, &glyphs).unwrap();
        let layout = JosaLayout::new(glyphs.len());
        let word = |offset: usize| u16::from_be_bytes([font.bytes[offset], font.bytes[offset + 1]]);
        for (index, (marker, final_form, plain_form)) in JOSA_MARKERS.iter().enumerate() {
            assert_eq!(font.char_table[marker] as u32, layout.normal_end + index as u32 * 4);
            assert_eq!(word(layout.pairs + index * 4), font.char_table[final_form]);
            assert_eq!(word(layout.pairs + index * 4 + 2), font.char_table[plain_form]);
        }
        let class = |ch: char| {
            let glyph = (font.char_table[&ch] - VIRTUAL_START) as usize / 4;
            word(layout.class + glyph * 2)
        };
        assert_eq!((class('가'), class('달'), class('책')), (0, 1, 2));
        assert_eq!(word(layout.class + glyphs.len() * 2), 0);
        assert_eq!(word(layout.scratch) as u32, layout.normal_end);
    }

    #[test]
    fn particle_marker_authoring_forms() {
        assert_eq!(josa_marker("을"), Some('\u{E000}'));
        assert_eq!(josa_marker("과"), Some('\u{E003}'));
        assert_eq!(josa_marker("를"), None);
        let tokens = crate::text::patcher::parse_ko_tokens("」{josa:을} 익혔다");
        assert_eq!(
            format!("{tokens:?}"),
            r#"[Text("」"), Text("\u{e000}"), Text(" 익혔다")]"#
        );
        let lines = crate::text::overflow::measure_lines("「{josa:을}」");
        assert_eq!(lines[0].char_count, 3);
    }

    #[test]
    fn incompatible_font_and_script_ranges_fail_closed() {
        assert!(pack(&[0; 128], &[('가', [0; 128])]).is_err());
        assert!(pack(&vec![0; FONT_SIZE], &[('가', [0; 128]), ('가', [1; 128])]).is_err());
        assert!(patch_pnd_writer(&mut vec![0; 493216]).is_err());
        let too_many: Vec<_> = (0..=(MAP_END - MAP_OFFSET) / 8)
            .map(|i| (char::from_u32(0xac00 + i as u32).unwrap(), [0; 128]))
            .collect();
        assert!(pack(&vec![0; FONT_SIZE], &too_many).is_err());
    }
}
