/// FONT.CEL 타일 인덱스 ↔ 유니코드 문자 매핑 테이블.
///
/// SEQ 파일의 텍스트는 SJIS가 아닌 타일 인덱스로 인코딩된다.
/// - 타일 0-95: 8x8 ASCII
/// - 타일 96-127: 8x8 UI 그래픽
/// - 타일 128-437: 16x8 와이드 문자 (155쌍)
/// - 타일 438-3813: 16x16 JP 글리프 (844개, 2x2 타일 그룹)

use std::collections::HashMap;

/// 타일 범위 상수
const WIDE_START: u16 = 128;
const WIDE_END: u16 = 438; // exclusive
const GLYPH_START: u16 = 438;
const GLYPH_TILES_PER: u16 = 4; // 2x2 = 4 tiles per 16x16 glyph
const WIDE_TILES_PER: u16 = 2; // 2 tiles per 16x8 wide char

/// 타일 인덱스 → 유니코드 문자 룩업 테이블
#[derive(Debug, Clone)]
pub struct GlyphTable {
    /// 16x16 글리프 매핑 (인덱스 0-843 → 문자)
    glyphs: Vec<String>,
    /// 최대 타일 코드 (글리프 영역 끝)
    max_tile: u16,
    /// 문자 → 타일 코드 역방향 조회 (O(1) encode)
    reverse: HashMap<String, u16>,
}

impl GlyphTable {
    /// glyph_mapping.csv로부터 테이블 생성
    ///
    /// CSV 형식: index,char,confidence,source
    pub fn from_csv(csv_data: &str) -> Result<Self, String> {
        let mut glyphs: Vec<(usize, String)> = Vec::new();

        for (line_num, line) in csv_data.lines().enumerate() {
            if line_num == 0 {
                // 헤더 스킵
                continue;
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.splitn(4, ',').collect();
            if fields.len() < 2 {
                return Err(format!("line {}: 필드 부족", line_num + 1));
            }
            let index: usize = fields[0]
                .parse()
                .map_err(|e| format!("line {}: 인덱스 파싱 실패: {}", line_num + 1, e))?;
            let ch = fields[1].to_string();
            glyphs.push((index, ch));
        }

        // 인덱스 순서로 정렬 후 벡터 구축
        glyphs.sort_by_key(|(idx, _)| *idx);
        let max_idx = glyphs.last().map(|(idx, _)| *idx).unwrap_or(0);
        let mut table = vec![String::new(); max_idx + 1];
        for (idx, ch) in glyphs {
            if idx < table.len() {
                table[idx] = ch;
            }
        }

        let max_tile = GLYPH_START + (table.len() as u16) * GLYPH_TILES_PER;

        let mut reverse = HashMap::with_capacity(table.len());
        for (idx, ch) in table.iter().enumerate() {
            if !ch.is_empty() {
                let tile_code = GLYPH_START + (idx as u16) * GLYPH_TILES_PER;
                reverse.insert(ch.clone(), tile_code);
            }
        }

        Ok(GlyphTable {
            glyphs: table,
            max_tile,
            reverse,
        })
    }

    /// 빈 테이블 (테스트용)
    pub fn empty() -> Self {
        GlyphTable {
            glyphs: Vec::new(),
            max_tile: GLYPH_START,
            reverse: HashMap::new(),
        }
    }

    /// 2바이트 타일 코드를 문자열로 디코딩
    pub fn decode(&self, tile_code: u16) -> String {
        // 16x16 JP 글리프 (타일 438+)
        if tile_code >= GLYPH_START && tile_code < self.max_tile {
            let offset = tile_code - GLYPH_START;
            if offset % GLYPH_TILES_PER == 0 {
                let idx = (offset / GLYPH_TILES_PER) as usize;
                if idx < self.glyphs.len() && !self.glyphs[idx].is_empty() {
                    return self.glyphs[idx].clone();
                }
            }
        }

        // 와이드 문자 (타일 128-437)
        if tile_code >= WIDE_START && tile_code < WIDE_END {
            let offset = tile_code - WIDE_START;
            if offset % WIDE_TILES_PER == 0 {
                let wide_idx = offset / WIDE_TILES_PER;
                return decode_wide(wide_idx);
            }
        }

        // 미인식 타일 코드
        format!("{{tile:{:04X}}}", tile_code)
    }

    /// 해당 타일 코드가 실제 텍스트(16x16 JP 글리프)인지 여부
    pub fn is_text_glyph(&self, tile_code: u16) -> bool {
        if tile_code >= GLYPH_START && tile_code < self.max_tile {
            let offset = tile_code - GLYPH_START;
            if offset % GLYPH_TILES_PER == 0 {
                let idx = (offset / GLYPH_TILES_PER) as usize;
                return idx < self.glyphs.len() && !self.glyphs[idx].is_empty();
            }
        }
        false
    }

    /// 문자 → 타일 코드 역방향 룩업 (16x16 글리프 영역, O(1) HashMap)
    pub fn encode(&self, ch: char) -> Option<u16> {
        let mut buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut buf);
        self.reverse.get(ch_str).copied()
    }

    /// 글리프 개수
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// 최대 타일 코드 (글리프 영역 끝, exclusive).
    ///
    /// 확장된 FONT.CEL에서 텍스트 감지 범위를 동적으로 조절하기 위해 사용.
    pub fn max_tile(&self) -> u16 {
        self.max_tile
    }

    /// 모든 (char, tile_code) 매핑을 반환 (sec7 글리프 영역).
    pub fn all_mappings(&self) -> Vec<(char, u16)> {
        self.glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.is_empty() && g.chars().count() == 1)
            .map(|(idx, g)| {
                let ch = g.chars().next().unwrap();
                let tc = GLYPH_START + (idx as u16) * GLYPH_TILES_PER;
                (ch, tc)
            })
            .collect()
    }
}

/// 알려진 와이드 문자 디코딩
/// wide_mapping.csv 기반, bottom-half 제어코드는 {wide:NNN} 유지
fn decode_wide(wide_idx: u16) -> String {
    match wide_idx {
        25 => " ".to_string(),          // 스페이스 (tile 178)
        27 => "0".to_string(),          // Digit 0 (tile 182)
        29 => "1".to_string(),          // Digit 1 (tile 186)
        31 => "2".to_string(),          // Digit 2 (tile 190)
        33 => "3".to_string(),          // Digit 3 (tile 194)
        35 => "4".to_string(),          // Digit 4 (tile 198)
        37 => "5".to_string(),          // Digit 5 (tile 202)
        39 => "6".to_string(),          // Digit 6 (tile 206)
        41 => "7".to_string(),          // Digit 7 (tile 210)
        43 => "8".to_string(),          // Digit 8 (tile 214)
        45 => "9".to_string(),          // Digit 9 (tile 218)
        47 => "-".to_string(),          // Dash (tile 222)
        49 => "\u{00B7}".to_string(),   // · Middle Dot (tile 226)
        51 => "!".to_string(),          // Exclamation (tile 230)
        53 => "?".to_string(),          // Question (tile 234)
        55 => "\u{3001}".to_string(),   // 、Ideographic Comma (tile 238)
        57 => "\u{3002}".to_string(),   // 。Ideographic Period (tile 242)
        59 => "\"".to_string(),         // Double Quote (tile 246)
        61 => "\u{2190}".to_string(),   // ← Arrow Left (tile 250)
        63 => "\u{300C}".to_string(),   // 「Corner Bracket L (tile 254)
        64 => "\u{3000}".to_string(),   // 전각 스페이스 (tile 256)
        65 => "\u{300D}".to_string(),   // 」Corner Bracket R (tile 258)
        67 => "A".to_string(),          // (tile 262)
        69 => "B".to_string(),          // (tile 266)
        71 => "C".to_string(),          // (tile 270)
        73 => "D".to_string(),          // (tile 274)
        75 => "E".to_string(),          // (tile 278)
        77 => "F".to_string(),          // (tile 282)
        79 => "G".to_string(),          // (tile 286)
        81 => "H".to_string(),          // (tile 290)
        83 => "I".to_string(),          // (tile 294)
        85 => "J".to_string(),          // (tile 298)
        87 => "K".to_string(),          // (tile 302)
        89 => "L".to_string(),          // (tile 306)
        91 => "M".to_string(),          // (tile 310)
        93 => "N".to_string(),          // (tile 314)
        95 => "O".to_string(),          // (tile 318)
        97 => "P".to_string(),          // (tile 322)
        99 => "Q".to_string(),          // (tile 326)
        101 => "R".to_string(),         // (tile 330)
        103 => "S".to_string(),         // (tile 334)
        105 => "T".to_string(),         // (tile 338)
        107 => "U".to_string(),         // (tile 342)
        109 => "V".to_string(),         // (tile 346)
        111 => "W".to_string(),         // (tile 350)
        113 => "X".to_string(),         // (tile 354)
        115 => "Y".to_string(),         // (tile 358)
        117 => "Z".to_string(),         // (tile 362)
        119 => "h".to_string(),         // small h (tile 366)
        121 => "~".to_string(),         // Tilde (tile 370)
        123 => "\u{300E}".to_string(),  // 『White Corner Bracket L (tile 374)
        125 => "\u{300F}".to_string(),  // 』White Corner Bracket R (tile 378)
        127 => "\u{2605}".to_string(),  // ★ Star (tile 382)
        129 => "\u{2026}".to_string(),  // … Ellipsis (tile 386)
        131 => "(".to_string(),         // (tile 390)
        133 => ")".to_string(),         // (tile 394)
        135 => "&".to_string(),         // (tile 398)
        137 => "/".to_string(),         // (tile 402)
        139 => "%".to_string(),         // (tile 406)
        141 => "\u{2192}".to_string(),  // → Arrow Right (tile 410)
        143 => "\u{2191}".to_string(),  // ↑ Arrow Up (tile 414)
        145 => "\u{266A}".to_string(),  // ♪ Music Note (tile 418)
        147 => "\u{30F4}".to_string(),  // ヴ Katakana Vu (tile 422)
        149 => "\u{201C}".to_string(),  // " Left DQ (tile 426)
        151 => "\u{201D}".to_string(),  // " Right DQ (tile 430)
        153 => "+".to_string(),         // Plus (tile 434)
        // bottom-half 제어코드 (0, 26, 36, 84, 86, 114, 128, 136, 144)는
        // {wide:NNN}으로 유지
        _ => format!("{{wide:{:03}}}", wide_idx),
    }
}

#[cfg(test)]
#[path = "glyph_tests.rs"]
mod tests;
