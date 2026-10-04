//! Korean role titles and character names for the ending staff roll.
//!
//! `ED_STF.SPR` (CNX) is a stack of headerless 4bpp color-bank strips, one per
//! credit line: 41 cast strips of 192x16 followed by staff strips of 160x16.
//! Text is italic, anti-aliased, and outlined: index 1 is the one-pixel dark
//! outline, 2..=8 the gray ramp of ordinary lines, and 9..=15 the gold ramp of
//! the two section headings.
//!
//! The final copyright screen labels in `ED_CR.CEL` (「キャラクター著作」,
//! 「製作・著作」) use the same outlined gray ramp without italics.
//!
//! By project decision (2026-09-29) only section headings, role titles,
//! labels, and character names are translated. Staff and voice-actor names, pen names,
//! and studio names keep their original strips.

use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

pub const LINE_HEIGHT: usize = 16;
pub const CAST_LINES: usize = 41;
pub const CAST_WIDTH: usize = 192;
pub const STAFF_WIDTH: usize = 160;
/// The staff section holds 98 credit strips; data after them is untouched.
pub const STAFF_LINES: usize = 98;

const FONT_SIZE: f32 = 13.0;
const SUPERSAMPLE: usize = 4;
/// Horizontal shift per row, in pixels, measured from the bottom row.
const ITALIC_SHEAR: f32 = 0.22;
const LETTER_SPACING: f32 = 1.0;
const INK_THRESHOLD: f32 = 0.18;
/// Thin Hangul strokes cover fewer full pixels than the original kana; scale
/// coverage so the brightest ramp step appears about as often.
const COVERAGE_GAIN: f32 = 1.4;
const OUTLINE_INDEX: u8 = 1;
const NORMAL_RAMP: (u8, u8) = (2, 8);
const HEADING_RAMP: (u8, u8) = (9, 15);
/// Indentation of personal-name lines in the original layout.
const NAME_INDENT: usize = 24;

const ORIGINAL_SHA256: &str = "5a365df81a4547dc74a015abc21a01b5dd91716952332dec1cf253125b07a5bd";
const FONT_SHA256: &str = "389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60";

/// One replaced strip: global line number (cast lines first), text, indent,
/// and whether it is a section heading.
pub struct CreditLine {
    pub line: usize,
    pub source: &'static str,
    pub text: &'static str,
    pub indent: usize,
    pub heading: bool,
}

const fn cast(line: usize, source: &'static str, text: &'static str) -> CreditLine {
    CreditLine { line, source, text, indent: 0, heading: line == 0 }
}

const fn staff(index: usize, source: &'static str, text: &'static str) -> CreditLine {
    CreditLine { line: CAST_LINES + index, source, text, indent: 0, heading: index == 0 }
}

pub const TRANSLATED_LINES: &[CreditLine] = &[
    cast(0, "キャスト", "캐스트"),
    cast(1, "アルル・ナジャ", "아르르·나쟈"),
    cast(3, "ルルー", "루루"),
    cast(5, "シェゾ・ウィグィィ", "셰죠·위그이"),
    cast(7, "スケルトンT", "스켈톤T"),
    cast(9, "インキュバス", "인큐버스"),
    cast(11, "カーバンクル", "카방클"),
    cast(13, "ミノタウロス", "미노타우로스"),
    cast(15, "すけとうだら", "스케토우다라"),
    cast(17, "ハーピー", "하피"),
    cast(19, "ウィッチ", "위치"),
    cast(21, "ドラコケンタウロス", "드라코켄타우로스"),
    cast(23, "ももも", "모모모"),
    cast(25, "アウルベア/まもの", "아울베어/마물"),
    cast(27, "コドモドラゴン/スキヤポデス", "아기드래곤/스키야포데스"),
    cast(29, "サキュバス", "서큐버스"),
    cast(31, "ジャーン/スキュラ/キキーモラ", "자안/스킬라/키키모라"),
    cast(33, "ぞう大魔王", "코끼리 대마왕"),
    cast(35, "アーちゃん/オトヒメ", "아짱/오토히메"),
    cast(37, "サタン", "사탄"),
    cast(39, "ラグナス・ビシャシ", "라그나스·비샤시"),
    staff(0, "スタッフ", "스태프"),
    staff(1, "シナリオ", "시나리오"),
    staff(6, "システム", "시스템"),
    staff(9, "マップ", "맵"),
    staff(12, "キャラクターグラフィック", "캐릭터 그래픽"),
    staff(15, "戦闘グラフィック", "전투 그래픽"),
    staff(21, "背景グラフィック", "배경 그래픽"),
    staff(23, "マップグラフィック", "맵 그래픽"),
    staff(25, "グラフィックサポート", "그래픽 지원"),
    staff(34, "キャラクターイラスト", "캐릭터 일러스트"),
    staff(36, "プログラム", "프로그램"),
    staff(42, "プログラムサポート", "프로그램 지원"),
    staff(47, "音楽", "음악"),
    staff(53, "効果音", "효과음"),
    staff(57, "録音技術", "녹음 기술"),
    staff(59, "録音技術サポート", "녹음 기술 지원"),
    staff(62, "製作サポート", "제작 지원"),
    staff(65, "マニュアル", "매뉴얼"),
    staff(71, "営業・販促", "영업·판촉"),
    staff(76, "スペシャルサンクス", "스페셜 땡스"),
    CreditLine {
        line: CAST_LINES + 91,
        source: "コンパイル社員みんな",
        text: "컴파일 사원 여러분",
        indent: NAME_INDENT,
        heading: false,
    },
    staff(92, "ディレクター", "디렉터"),
    staff(94, "プロデューサー", "프로듀서"),
    staff(96, "EX プロデューサー", "EX 프로듀서"),
];

/// Byte offset and width of a credit strip in decompressed `ED_STF.SPR`.
pub fn strip(line: usize) -> (usize, usize) {
    if line < CAST_LINES {
        (line * CAST_WIDTH * LINE_HEIGHT / 2, CAST_WIDTH)
    } else {
        let staff = line - CAST_LINES;
        (
            CAST_LINES * CAST_WIDTH * LINE_HEIGHT / 2 + staff * STAFF_WIDTH * LINE_HEIGHT / 2,
            STAFF_WIDTH,
        )
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify_hash(bytes: &[u8], expected: &str, label: &str) -> Result<(), String> {
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(format!("{label} SHA-256 mismatch: expected {expected}, got {actual}"));
    }
    Ok(())
}

/// Render one strip as 4bpp indices (row-major, `width` x 16).
fn render_strip(font: &Font, entry: &CreditLine, width: usize) -> Result<Vec<u8>, String> {
    render_band(font, entry.text, entry.indent, width, entry.heading, ITALIC_SHEAR)
}

/// Render `text` into a `width` x 16 band with ink on rows 1..=14.
fn render_band(
    font: &Font,
    text: &str,
    indent: usize,
    width: usize,
    heading: bool,
    shear: f32,
) -> Result<Vec<u8>, String> {
    let scale = SUPERSAMPLE as f32;
    let size = FONT_SIZE * scale;
    let metrics = font
        .horizontal_line_metrics(size)
        .ok_or("font has no horizontal line metrics")?;
    let hi_width = (width + LINE_HEIGHT) * SUPERSAMPLE;
    let hi_height = LINE_HEIGHT * SUPERSAMPLE;
    let mut canvas = vec![0f32; hi_width * hi_height];
    let text_height = metrics.ascent - metrics.descent;
    // Original glyphs occupy rows 1..=14.
    let baseline = scale + (14.0 * scale - text_height) / 2.0 + metrics.ascent;
    let mut pen = indent as f32 * scale;
    for ch in text.chars() {
        let (glyph, raster) = font.rasterize(ch, size);
        let left = (pen + glyph.xmin as f32).round() as isize;
        let top = (baseline - (glyph.ymin as f32 + glyph.height as f32)).round() as isize;
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                let cx = left + x as isize;
                let cy = top + y as isize;
                if cx < 0 || cy < 0 || cx as usize >= hi_width || cy as usize >= hi_height {
                    continue;
                }
                let cell = &mut canvas[cy as usize * hi_width + cx as usize];
                *cell = cell.max(raster[y * glyph.width + x] as f32 / 255.0);
            }
        }
        pen += glyph.advance_width + LETTER_SPACING * scale;
    }

    // Italic shear with linear interpolation, then box-filter downsample.
    let mut sheared = vec![0f32; hi_width * hi_height];
    for y in 0..hi_height {
        let shift = (hi_height - 1 - y) as f32 * shear;
        let whole = shift.floor() as usize;
        let frac = shift - whole as f32;
        for x in 0..hi_width {
            let value = canvas[y * hi_width + x];
            if value == 0.0 {
                continue;
            }
            if x + whole < hi_width {
                sheared[y * hi_width + x + whole] += value * (1.0 - frac);
            }
            if x + whole + 1 < hi_width {
                sheared[y * hi_width + x + whole + 1] += value * frac;
            }
        }
    }
    let mut coverage = vec![0f32; width * LINE_HEIGHT];
    for y in 0..LINE_HEIGHT {
        for x in 0..width + LINE_HEIGHT {
            let mut sum = 0.0;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    sum += sheared[(y * SUPERSAMPLE + sy) * hi_width + x * SUPERSAMPLE + sx];
                }
            }
            let value = sum / (SUPERSAMPLE * SUPERSAMPLE) as f32;
            if x >= width {
                if value >= INK_THRESHOLD {
                    return Err(format!("「{text}」 is wider than its {width}px strip"));
                }
                continue;
            }
            coverage[y * width + x] = value;
        }
    }

    let (low, high) = if heading { HEADING_RAMP } else { NORMAL_RAMP };
    let steps = (high - low + 1) as f32;
    let ink = |x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < width
            && (y as usize) < LINE_HEIGHT
            && coverage[y as usize * width + x as usize] >= INK_THRESHOLD
    };
    let mut indices = vec![0u8; width * LINE_HEIGHT];
    for y in 0..LINE_HEIGHT as isize {
        for x in 0..width as isize {
            let index = y as usize * width + x as usize;
            indices[index] = if ink(x, y) {
                let level = ((coverage[index] * COVERAGE_GAIN).min(1.0) * steps).round() as i32
                    + low as i32
                    - 1;
                level.clamp(low as i32, high as i32) as u8
            } else if (-1..=1).any(|dy| (-1..=1).any(|dx| ink(x + dx, y + dy))) {
                OUTLINE_INDEX
            } else {
                0
            };
        }
    }
    let edge_ink = (0..width).any(|x| indices[x] > OUTLINE_INDEX)
        || (0..width).any(|x| indices[(LINE_HEIGHT - 1) * width + x] > OUTLINE_INDEX);
    if edge_ink {
        return Err(format!("「{text}」 touches the band's top or bottom row"));
    }
    Ok(indices)
}

pub fn compile(original: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_hash(original, ORIGINAL_SHA256, "original ED_STF.SPR")?;
    verify_hash(font_bytes, FONT_SHA256, "NEXONLv2Gothic font")?;
    let font = Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| format!("failed to load NEXONLv2Gothic: {error}"))?;
    let mut compiled = original.to_vec();
    for entry in TRANSLATED_LINES {
        let (offset, width) = strip(entry.line);
        let bytes = width * LINE_HEIGHT / 2;
        if original[offset..offset + bytes].iter().all(|&byte| byte == 0) {
            return Err(format!("credit line {} ({}) is empty in the source", entry.line, entry.source));
        }
        let indices = render_strip(&font, entry, width)?;
        for (index, pair) in indices.chunks(2).enumerate() {
            compiled[offset + index] = (pair[0] << 4) | pair[1];
        }
    }
    Ok(compiled)
}

// ---------------------------------------------------------------------------
// Final copyright screen (ED_CR.CEL + ED_CR.MAP)
// ---------------------------------------------------------------------------

/// `ED_CR.CEL` holds 8bpp 8x8 VDP2 cells; `ED_CR.MAP` is a 64x64 one-word
/// pattern-name table whose character numbers count 32-byte units, so an 8bpp
/// cell index is the map value divided by two. The two labels occupy cells
/// used nowhere else; company names stay untouched.
pub const COPYRIGHT_LABELS: &[CopyrightLabel] = &[
    CopyrightLabel { source: "キャラクター著作", text: "캐릭터 저작", band_top: 72 },
    CopyrightLabel { source: "製作・著作", text: "제작·저작", band_top: 145 },
];

pub struct CopyrightLabel {
    pub source: &'static str,
    pub text: &'static str,
    /// Top row of the 16-pixel band. The first label matches the original ink
    /// rows; the second sits 3px higher because the map has no cell under the
    /// original middle dot in its bottom cell row (row 20, column 9).
    pub band_top: usize,
}

const MAP_SIZE: usize = 64;
const CELL: usize = 8;
const CELL_BYTES: usize = CELL * CELL;
/// Label ink starts at x=45 in both labels; the outline column is x=44.
const LABEL_LEFT: usize = 44;
const LABEL_WIDTH: usize = 104;
const ORIGINAL_CR_CEL_SHA256: &str =
    "59f8085c83366d1c0a07ebc1f0a7a3181eadaae054d5258e963477311d389f13";
const ORIGINAL_CR_MAP_SHA256: &str =
    "3d98181c0f2301ac876da975ee2ad8cfec8d3d0513f76e6f019639eb8b5d33a9";

fn map_cell(map: &[u8], column: usize, row: usize) -> Option<usize> {
    let offset = (row * MAP_SIZE + column) * 2;
    let value = u16::from_be_bytes([map[offset], map[offset + 1]]) as usize;
    (value != 0).then_some(value / 2)
}

/// Replace the two label bands of the ending copyright screen.
pub fn compile_copyright(cel: &[u8], map: &[u8], font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    verify_hash(cel, ORIGINAL_CR_CEL_SHA256, "original ED_CR.CEL")?;
    verify_hash(map, ORIGINAL_CR_MAP_SHA256, "original ED_CR.MAP")?;
    verify_hash(font_bytes, FONT_SHA256, "NEXONLv2Gothic font")?;
    let font = Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| format!("failed to load NEXONLv2Gothic: {error}"))?;

    let mut uses = std::collections::HashMap::new();
    for row in 0..MAP_SIZE {
        for column in 0..MAP_SIZE {
            if let Some(cell) = map_cell(map, column, row) {
                *uses.entry(cell).or_insert(0usize) += 1;
            }
        }
    }

    let mut compiled = cel.to_vec();
    for label in COPYRIGHT_LABELS {
        let band = render_band(&font, label.text, 1, LABEL_WIDTH, false, 0.0)?;
        let rows = label.band_top / CELL..=(label.band_top + LINE_HEIGHT - 1) / CELL;
        let columns = LABEL_LEFT / CELL..=(LABEL_LEFT + LABEL_WIDTH - 1) / CELL;
        // Clear the original label in every cell it touches.
        for row in rows.clone() {
            for column in columns.clone() {
                if let Some(cell) = map_cell(map, column, row) {
                    if uses[&cell] != 1 {
                        return Err(format!("label cell {cell} at ({column},{row}) is shared"));
                    }
                    let start = cell * CELL_BYTES;
                    compiled[start..start + CELL_BYTES].fill(0);
                }
            }
        }
        for y in 0..LINE_HEIGHT {
            for x in 0..LABEL_WIDTH {
                let value = band[y * LABEL_WIDTH + x];
                if value == 0 {
                    continue;
                }
                let (px, py) = (LABEL_LEFT + x, label.band_top + y);
                let cell = map_cell(map, px / CELL, py / CELL).ok_or_else(|| {
                    format!("「{}」 pixel ({px},{py}) falls outside the label cells", label.text)
                })?;
                compiled[cell * CELL_BYTES + (py % CELL) * CELL + px % CELL] = value;
            }
        }
    }
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        let bytes = crate::test_input::read("assets/fonts/NEXONLv2Gothic.ttf");
        assert_eq!(sha256_hex(&bytes), FONT_SHA256);
        Font::from_bytes(bytes.as_slice(), FontSettings::default()).unwrap()
    }

    #[test]
    fn strips_cover_the_catalogued_layout() {
        assert_eq!(strip(40), (40 * 1536, 192));
        assert_eq!(strip(41), (41 * 1536, 160));
        assert_eq!(strip(41 + 97).0 + 1280, 41 * 1536 + 98 * 1280);
    }

    #[test]
    #[ignore = "requires assets/fonts/NEXONLv2Gothic.ttf"]
    fn every_translated_line_fits_and_uses_its_ramp() {
        let font = font();
        for entry in TRANSLATED_LINES {
            assert!(entry.line < CAST_LINES + STAFF_LINES);
            let (_, width) = strip(entry.line);
            let indices = render_strip(&font, entry, width).unwrap();
            let (low, high) = if entry.heading { HEADING_RAMP } else { NORMAL_RAMP };
            let body = indices.iter().filter(|&&index| index >= low && index <= high).count();
            let outline = indices.iter().filter(|&&index| index == OUTLINE_INDEX).count();
            let foreign = indices
                .iter()
                .filter(|&&index| index > OUTLINE_INDEX && (index < low || index > high))
                .count();
            assert!(body > 20, "{} renders ink", entry.text);
            assert!(outline > body / 2, "{} is outlined", entry.text);
            assert_eq!(foreign, 0, "{} stays in its ramp", entry.text);
        }
    }

    #[test]
    #[ignore = "requires assets/fonts/NEXONLv2Gothic.ttf"]
    fn copyright_labels_fit_their_bands() {
        let font = font();
        for label in COPYRIGHT_LABELS {
            let band = render_band(&font, label.text, 1, LABEL_WIDTH, false, 0.0).unwrap();
            assert!(band.iter().filter(|&&index| index >= 2).count() > 30, "{}", label.text);
        }
    }

    #[test]
    fn line_numbers_are_unique_and_names_are_not_listed() {
        let mut lines: Vec<usize> = TRANSLATED_LINES.iter().map(|entry| entry.line).collect();
        lines.sort();
        lines.dedup();
        assert_eq!(lines.len(), TRANSLATED_LINES.len());
        // Voice-actor strips alternate with character strips in the cast.
        assert!(TRANSLATED_LINES.iter().all(|entry| entry.line >= CAST_LINES || entry.line % 2 == 1 || entry.line == 0));
    }
}
