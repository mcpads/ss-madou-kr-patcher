use serde::{Deserialize, Serialize};

use crate::text::glyph::GlyphTable;
use crate::text::patcher::SeqType;

/// 원본 FONT.CEL 기준 글리프 타일 상한 (844 glyphs × 4 tiles + 438 start).
const DEFAULT_GLYPH_END: u16 = 3814;

/// 알려진 제어코드의 16-bit word 파라미터 개수
pub fn control_code_param_count(code: u16) -> usize {
    match code {
        0xFF00 => 0, // 줄바꿈
        0xFF02 => 0, // 라인 표시 (대사 줄바꿈)
        0xFF03 => 1, // 씬 초기화
        0xFF04 => 1, // 텍스트 블록 종료 (특수) — 1 param (타일 코드)
        0xFF05 => 0, // 텍스트 블록 종료 (일반)
        0xFF06 => 2, // 플래그 설정
        0xFF09 => 0, // 점프/분기
        0xFF0B => 3, // 선택지 메뉴 (p1-p2=flags, p3=커서 타일코드 0x0A0A)
        0xFF0F => 1, // 씬 시작
        0xFF11 => 1, // 알 수 없음 (1 param 확인)
        0xFF12 => 1, // 레코드 종료 — 1 param (타일 코드 또는 타입 지시자)
        0xFF16 => 1, // 알 수 없음 (1 param, 97% 일치)
        0xFF18 => 1, // 레코드 마커 — 1 param (타일 코드)
        0xFF1A => 1, // 레코드 마커 — 1 param (타일 코드)
        0xFF1B => 1, // 알 수 없음 (1 param, 99.3% 일치)
        0xFF30 => 3, // 화자/초상화 설정 (p1=타입, p2=0, p3=화자 타일코드)
        0xFF33 => 3, // 알 수 없음 (3 params, 97.5% 일치)
        0xFF36 => 3, // 알 수 없음 (3 params, 100% 일치)
        0xFF37 => 1, // 알 수 없음 (1 param, 99.2% 일치)
        0xFF39 => 0, // 섹션 종료
        0xFF3D => 1, // 텍스트 블록 시작
        0xFFFF => 0, // 종료 마커
        _ => 0,      // 미확인: 파라미터 없다고 가정
    }
}

/// 엔트리 경계를 만드는 구조적 제어코드 여부
fn is_entry_boundary(code: u16) -> bool {
    matches!(code, 0xFFFF | 0xFF0F | 0xFF0B | 0xFF03 | 0xFF3D | 0xFF39)
}

/// 제어코드를 {ctrl:XXXX} 또는 {ctrl:XXXX:YYYY:ZZZZ} 형식으로 포맷
fn format_control(code: u16, params: &[u16]) -> String {
    if params.is_empty() {
        format!("{{ctrl:{:04X}}}", code)
    } else {
        let param_str: Vec<String> = params.iter().map(|p| format!("{:04X}", p)).collect();
        format!("{{ctrl:{:04X}:{}}}", code, param_str.join(":"))
    }
}

/// 바이트 슬라이스를 공백 구분 hex 문자열로 변환
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 하나의 SEQ 파일에서 추출한 전체 스크립트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptDump {
    pub source: String,
    pub source_md5: String,
    pub entries: Vec<ScriptEntry>,
    /// 노이즈로 판정된 엔트리 (제어코드도 와이드 문자도 없는 단독 타일 코드)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filtered: Vec<ScriptEntry>,
}

fn is_false(v: &bool) -> bool { !v }

/// 번역 항목의 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    #[default]
    Untranslated,
    NeedsReview,
    NeedsHumanReview,
    Done,
}

/// 하나의 번역 가능 텍스트 항목
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEntry {
    pub id: String,
    pub offset: String,
    pub raw_hex: String,
    pub text: String,
    pub ko: Option<String>,
    #[serde(default)]
    pub status: TranslationStatus,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pad_to_original: bool,
    pub notes: String,
}

// ---------------------------------------------------------------------------
// Text region detection
// ---------------------------------------------------------------------------

/// SEQ 파일에서 텍스트 데이터가 시작되는 오프셋을 결정한다.
///
/// SEQ 파일 전반부는 스크립트 코드(포인터, 구조체, 게임 로직)이고
/// 텍스트는 후반부에 위치한다. 코드 영역의 구조체 필드가 우연히
/// 유효 글리프 코드와 일치하면 거짓양성이 발생하므로, 텍스트 시작점
/// 이전은 스캔하지 않는다.
///
/// `max_tile` 파라미터로 확장된 FONT.CEL의 타일 범위를 전달할 수 있다.
/// `None`이면 원본 844 글리프 기준(3814)을 사용한다.
pub fn find_text_start(data: &[u8], source_name: &str, max_tile: Option<u16>) -> usize {
    let seq_type = SeqType::from_filename(source_name);

    match seq_type {
        SeqType::Mp => {
            let base = find_min_ptr_target(data, 0x24, &[0x00, 0x00, 0x00, 0x05]);
            if base > 0 {
                scan_backward_for_text_boundary(data, base)
            } else {
                find_text_start_by_density(data, max_tile)
            }
        }
        SeqType::Pt => {
            let base = find_min_ptr_target(data, 0x25, &[0x05, 0x00, 0x00, 0x00]);
            if base > 0 {
                scan_backward_for_text_boundary(data, base)
            } else {
                // 포인터 없는 PT: FF30 화자 코드를 앵커로 사용, 역방향 스캔
                match find_first_control(data, 0xFF30) {
                    Some(pos) => scan_backward_for_text_boundary(data, pos),
                    None => find_text_start_by_density(data, max_tile),
                }
            }
        }
        SeqType::Diary => {
            find_first_control(data, 0xFF3D).unwrap_or(0)
        }
        _ => find_text_start_by_density(data, max_tile),
    }
}

/// 8바이트 포인터 패턴(`00 [seg] XX XX [suffix 4B]`)에서 최소 타겟 오프셋을 찾는다.
fn find_min_ptr_target(data: &[u8], segment: u8, suffix: &[u8; 4]) -> usize {
    let mut min_target = usize::MAX;
    for i in 0..data.len().saturating_sub(7) {
        if data[i] != 0x00 || data[i + 1] != segment {
            continue;
        }
        if data[i + 4..i + 8] != *suffix {
            continue;
        }
        let tgt = ((data[i + 2] as usize) << 8) | (data[i + 3] as usize);
        if tgt > 0 && tgt < data.len() && tgt < min_target {
            min_target = tgt;
        }
    }
    if min_target == usize::MAX { 0 } else { min_target }
}

/// 앵커 지점에서 역방향으로 스캔하여 텍스트 영역의 실제 시작점을 찾는다.
///
/// SEQ 텍스트 영역은 min(포인터 타겟)보다 앞에서 시작할 수 있다.
/// 순차 실행되는 대사 블록이 첫 씬 점프 타겟 앞에 위치하기 때문이다.
/// 앵커에서 최대 4096바이트를 역방향 스캔하여 FF0F(씬 시작)나
/// FF30(화자 설정) 같은 텍스트 영역 마커를 찾는다.
fn scan_backward_for_text_boundary(data: &[u8], anchor: usize) -> usize {
    let search_start = anchor.saturating_sub(4096);

    // 2바이트 정렬 기준으로 앵커보다 앞에 있는 가장 빠른 FF0F/FF30을 찾는다
    // (FF0F는 씬 시작, FF30은 화자 설정 — 둘 다 텍스트 영역의 확실한 마커)
    let mut earliest = anchor;
    let aligned_start = search_start & !1; // 짝수 정렬
    for pos in (aligned_start..anchor).step_by(2) {
        if pos + 1 >= data.len() {
            continue;
        }
        if data[pos] == 0xFF && (data[pos + 1] == 0x0F || data[pos + 1] == 0x30) {
            earliest = pos;
            break;
        }
    }

    earliest
}

/// 텍스트 밀도 기반 폴백 (COMMON, Other 등 포인터 패턴 없는 파일용).
///
/// FF30은 사용하지 않음 — 데이터 구조에서 우연히 FF 30 바이트가
/// 출현할 수 있어 거짓양성 위험이 높다.
fn find_text_start_by_density(data: &[u8], max_tile: Option<u16>) -> usize {
    // 실제 SEQ 파일은 최소 수천 바이트 — 소형 데이터는 테스트용
    if data.len() < 256 {
        return 0;
    }

    let glyph_end = max_tile.unwrap_or(DEFAULT_GLYPH_END);

    // 근접 텍스트가 있는 첫 FF02 (대사 줄바꿈)
    // 전후 20바이트에 글리프 2개 이상이면 실제 텍스트 영역
    if let Some(pos) = find_first_text_break(data, glyph_end) {
        return pos.saturating_sub(32);
    }

    // 텍스트 패턴 없음 — 데이터 전용 파일이므로 스킵
    data.len()
}

/// 특정 제어코드의 첫 번째 출현 위치를 찾는다 (2바이트 정렬).
fn find_first_control(data: &[u8], code: u16) -> Option<usize> {
    let hi = (code >> 8) as u8;
    let lo = (code & 0xFF) as u8;
    for i in (0..data.len().saturating_sub(1)).step_by(2) {
        if data[i] == hi && data[i + 1] == lo {
            return Some(i);
        }
    }
    None
}

/// 주변에 텍스트 글리프가 있는 첫 FF02를 찾는다 (2바이트 정렬 스캔).
///
/// `glyph_end`는 글리프 타일 범위 상한 (exclusive).
/// 확장된 FONT.CEL에서는 3814보다 클 수 있다.
fn find_first_text_break(data: &[u8], glyph_end: u16) -> Option<usize> {
    const GLYPH_START: u16 = 438;

    let is_glyph = |pos: usize| -> bool {
        if pos + 1 >= data.len() {
            return false;
        }
        let w = u16::from_be_bytes([data[pos], data[pos + 1]]);
        w >= GLYPH_START && w < glyph_end && (w - GLYPH_START) % 4 == 0
    };

    // 2바이트 정렬: 비정렬 FF02는 데이터 바이트가 우연히 일치한 것
    for i in (0..data.len().saturating_sub(1)).step_by(2) {
        if data[i] != 0xFF || data[i + 1] != 0x02 {
            continue;
        }
        // 전후 20바이트에서 글리프 수 카운트
        let mut count = 0;
        let start = i.saturating_sub(20);
        for j in (start..i).step_by(2) {
            if is_glyph(j) {
                count += 1;
            }
        }
        for j in ((i + 2)..((i + 22).min(data.len()))).step_by(2) {
            if is_glyph(j) {
                count += 1;
            }
        }
        if count >= 2 {
            return Some(i);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Script parsing
// ---------------------------------------------------------------------------

/// SEQ 파일 바이너리를 파싱하여 ScriptDump를 생성한다.
///
/// 타일 인덱스 기반 디코딩: SEQ 텍스트는 SJIS가 아닌 FONT.CEL 타일 인덱스.
/// 2바이트 big-endian 워드를 읽어 GlyphTable로 디코딩한다.
/// 0xFF__ 패턴은 제어코드, 나머지는 타일 인덱스로 처리.
/// 구조적 제어코드(FFFF, FF0F 등)에서 엔트리를 분할.
/// 16x16 JP 글리프가 하나도 없는 구간은 건너뛴다.
///
/// 포인터 테이블 기반으로 텍스트 시작점을 자동 감지하여
/// 코드 영역의 거짓양성을 제거한다.
pub fn parse_script(data: &[u8], source_name: &str, table: &GlyphTable) -> ScriptDump {
    let prefix = source_name
        .trim_end_matches(".SEQ")
        .trim_end_matches(".seq");

    let max_tile = table.max_tile();
    let text_start = find_text_start(data, source_name, Some(max_tile));

    let mut entries = Vec::new();
    let mut filtered = Vec::new();
    let mut pos = text_start;
    let mut entry_offset: Option<usize> = None;
    let mut text_buf = String::new();
    let mut raw_buf: Vec<u8> = Vec::new();
    let mut has_text = false;
    let mut has_control = false;
    let mut has_wide = false;
    let mut entry_count: usize = 0;
    let mut filtered_count: usize = 0;

    while pos + 1 < data.len() {
        let hi = data[pos];

        // 0xFF 제어코드
        if hi == 0xFF {
            let code = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let param_count = control_code_param_count(code);
            let total_bytes = 2 + param_count * 2;

            let mut params = Vec::new();
            for i in 0..param_count {
                let poff = pos + 2 + i * 2;
                if poff + 1 < data.len() {
                    params.push(u16::from_be_bytes([data[poff], data[poff + 1]]));
                }
            }

            // 구조적 경계에서 텍스트가 있으면 엔트리 플러시
            if is_entry_boundary(code) {
                if has_text {
                    if has_control || has_wide {
                        entries.push(make_entry(prefix, entry_count, entry_offset.unwrap_or(pos), &raw_buf, text_buf.clone()));
                        entry_count += 1;
                    } else {
                        let mut e = make_entry(prefix, filtered_count, entry_offset.unwrap_or(pos), &raw_buf, text_buf.clone());
                        e.id = format!("{}_F{:04}", prefix, filtered_count);
                        filtered.push(e);
                        filtered_count += 1;
                    }
                }
                // 리셋 후 경계 코드를 다음 엔트리의 시작으로 기록
                text_buf.clear();
                raw_buf.clear();
                has_text = false;
                has_control = false;
                has_wide = false;
                entry_offset = Some(pos);
                let end = (pos + total_bytes).min(data.len());
                raw_buf.extend_from_slice(&data[pos..end]);
                text_buf.push_str(&format_control(code, &params));
                // FFFF는 종료 마커일 뿐이므로 텍스트 맥락 표시로 불인정
                // FF0F, FF0B, FF3D 등 다른 경계 코드는 텍스트 구조적 마커
                if code != 0xFFFF {
                    has_control = true;
                }
                pos = end;
            } else {
                // 비경계 제어코드: 누적
                if entry_offset.is_none() {
                    entry_offset = Some(pos);
                }
                let end = (pos + total_bytes).min(data.len());
                raw_buf.extend_from_slice(&data[pos..end]);
                text_buf.push_str(&format_control(code, &params));
                has_control = true;
                pos = end;
            }
        }
        // 타일 인덱스 (non-0xFF 워드)
        else {
            let word = u16::from_be_bytes([data[pos], data[pos + 1]]);

            if table.is_text_glyph(word) || is_wide_char(word) {
                // 유효한 텍스트 타일
                if entry_offset.is_none() {
                    entry_offset = Some(pos);
                }
                raw_buf.push(data[pos]);
                raw_buf.push(data[pos + 1]);
                text_buf.push_str(&table.decode(word));
                if table.is_text_glyph(word) {
                    has_text = true;
                }
                if is_wide_char(word) {
                    has_wide = true;
                }
                pos += 2;
            } else {
                // 비텍스트 워드: 텍스트가 있으면 플러시
                if has_text {
                    if has_control || has_wide {
                        entries.push(make_entry(prefix, entry_count, entry_offset.unwrap_or(pos), &raw_buf, text_buf.clone()));
                        entry_count += 1;
                    } else {
                        let mut e = make_entry(prefix, filtered_count, entry_offset.unwrap_or(pos), &raw_buf, text_buf.clone());
                        e.id = format!("{}_F{:04}", prefix, filtered_count);
                        filtered.push(e);
                        filtered_count += 1;
                    }
                }
                text_buf.clear();
                raw_buf.clear();
                has_text = false;
                has_control = false;
                has_wide = false;
                entry_offset = None;
                pos += 2;
            }
        }
    }

    // 잔여 텍스트 플러시
    if has_text {
        if has_control || has_wide {
            entries.push(make_entry(prefix, entry_count, entry_offset.unwrap_or(data.len()), &raw_buf, text_buf));
        } else {
            let mut e = make_entry(prefix, filtered_count, entry_offset.unwrap_or(data.len()), &raw_buf, text_buf);
            e.id = format!("{}_F{:04}", prefix, filtered_count);
            filtered.push(e);
        }
    }

    let digest = md5::compute(data);

    // 항상 기술명 테이블 스캔 — 대사와 기술명이 공존하는 파일 지원
    let skill_entries = scan_skill_table(data, source_name, table);
    if !skill_entries.is_empty() {
        if entries.is_empty() {
            // 기술명만 있는 파일 (per-entry pad_to_original이 이미 true)
            return ScriptDump {
                source: source_name.to_string(),
                source_md5: format!("{:x}", digest),
                entries: skill_entries,
                filtered,
            };
        }
        // 대사 + 기술명 혼합 파일 → 대사 범위와 겹치지 않는 기술명만 병합
        // (per-entry pad_to_original은 빌드 시 _S ID로 판별)
        let existing_ranges: Vec<(usize, usize)> = entries
            .iter()
            .map(|e| {
                let start = usize::from_str_radix(
                    e.offset.trim_start_matches("0x"),
                    16,
                )
                .unwrap_or(0);
                let byte_count = e.raw_hex.split_whitespace().count();
                (start, start + byte_count)
            })
            .collect();
        for se in skill_entries {
            let s_start = usize::from_str_radix(
                se.offset.trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0);
            let s_byte_count = se.raw_hex.split_whitespace().count();
            let s_end = s_start + s_byte_count;
            let overlaps = existing_ranges
                .iter()
                .any(|&(d_start, d_end)| s_start < d_end && s_end > d_start);
            if !overlaps {
                entries.push(se);
            }
        }
    }

    ScriptDump {
        source: source_name.to_string(),
        source_md5: format!("{:x}", digest),
        entries,
        filtered,
    }
}

/// 와이드 문자 코드인지 확인 (타일 128-437, 정렬된 2의 배수)
fn is_wide_char(tile_code: u16) -> bool {
    tile_code >= 128 && tile_code < 438 && (tile_code - 128) % 2 == 0
}

// ---------------------------------------------------------------------------
// Factory functions
// ---------------------------------------------------------------------------

/// 대사 엔트리를 생성하는 팩토리 함수.
fn make_entry(prefix: &str, count: usize, offset: usize, raw: &[u8], text: String) -> ScriptEntry {
    ScriptEntry {
        id: format!("{}_{:04}", prefix, count),
        offset: format!("0x{:05X}", offset),
        raw_hex: bytes_to_hex(raw),
        text,
        ko: None,
        status: TranslationStatus::default(),
        pad_to_original: false,
        notes: String::new(),
    }
}

/// 기술명(스킬) 엔트리를 생성하는 팩토리 함수.
/// `pad_to_original: true`로 설정된다.
fn make_skill_entry(prefix: &str, count: usize, offset: usize, raw: &[u8], text: String) -> ScriptEntry {
    ScriptEntry {
        id: format!("{}_S{:04}", prefix, count),
        offset: format!("0x{:05X}", offset),
        raw_hex: bytes_to_hex(raw),
        text,
        ko: None,
        status: TranslationStatus::default(),
        pad_to_original: true,
        notes: String::new(),
    }
}

/// 하드코딩된 기술명 테이블: `(SEQ 파일명, &[(range_start, range_end)])`.
///
/// 각 범위는 해당 SEQ 파일 내에서 FF00 구분자로 나열된 기술명이 위치하는
/// 바이트 영역이다. 번역 JSON의 `_S` 엔트리 오프셋을 기준으로 산출.
///
/// 범위는 첫 기술명의 시작 오프셋부터 마지막 기술명+FF00 끝 오프셋까지.
const SKILL_TABLE_RANGES: &[(&str, &[(usize, usize)])] = &[
    ("PARTY_01.SEQ", &[(0x0E548, 0x0E864)]),
    ("PARTY_03.SEQ", &[(0x0B1A0, 0x0B3E4)]),
    ("PARTY_04.SEQ", &[(0x0E814, 0x0E9FE)]),
    ("PARTY_05.SEQ", &[(0x0C4B4, 0x0C6E2)]),
    ("PARTY_07.SEQ", &[(0x044A0, 0x04518)]),
    ("PARTY_09.SEQ", &[(0x0A7EC, 0x0A830)]),
    ("PT0001A.SEQ", &[(0x073B8, 0x073FC)]),
    ("PT0001B.SEQ", &[(0x077C8, 0x0780C)]),
    ("PT0103.SEQ",  &[(0x03A20, 0x03A7C)]),
    ("PT0104.SEQ",  &[(0x03A20, 0x03A7C)]),
    ("PT0401.SEQ",  &[(0x017E0, 0x0181C)]),
    ("PT0402.SEQ",  &[(0x06F80, 0x07044)]),
    ("PT0501.SEQ",  &[(0x04DB0, 0x04E8C)]),
    ("PT0503.SEQ",  &[(0x0403C, 0x040E2)]),
    ("PT0701.SEQ",  &[(0x0384C, 0x038FA)]),
    ("PT0702.SEQ",  &[(0x08540, 0x086E0)]),
    ("PT0703.SEQ",  &[(0x06AF8, 0x06C2A)]),
    ("PT0703A.SEQ", &[(0x06C7C, 0x06D80)]),
];

/// 하드코딩된 테이블을 사용하여 기술명을 추출한다.
///
/// 이전의 260줄 휴리스틱 대신, 알려진 SEQ 파일의 기술명 바이트 범위를
/// 직접 참조한다. FF00 구분자로 분리된 각 청크를 GlyphTable로 디코딩하여
/// ScriptEntry 목록을 반환한다.
fn scan_skill_table(data: &[u8], source_name: &str, table: &GlyphTable) -> Vec<ScriptEntry> {
    let upper = source_name.to_ascii_uppercase();
    let ranges = match SKILL_TABLE_RANGES.iter().find(|(name, _)| *name == upper) {
        Some((_, ranges)) => *ranges,
        None => return Vec::new(),
    };

    let prefix = source_name
        .trim_end_matches(".SEQ")
        .trim_end_matches(".seq");

    let mut entries = Vec::new();

    for &(range_start, range_end) in ranges {
        let end = range_end.min(data.len());
        if range_start >= end {
            continue;
        }
        extract_skill_entries_in_range(data, range_start, end, prefix, table, &mut entries);
    }

    entries
}

/// 지정된 바이트 범위 내에서 FF00 구분자로 분리된 기술명 엔트리를 추출한다.
///
/// 범위 내의 2바이트 정렬된 워드를 순차 스캔하여:
/// - FF00을 만나면 현재까지 누적된 타일코드를 하나의 기술명으로 플러시
/// - 0000은 서브테이블 구분자로 건너뜀
/// - 유효하지 않은 워드는 누적 버퍼를 리셋
fn extract_skill_entries_in_range(
    data: &[u8],
    start: usize,
    end: usize,
    prefix: &str,
    table: &GlyphTable,
    entries: &mut Vec<ScriptEntry>,
) {
    let mut pos = start;
    let mut entry_start: Option<usize> = None;
    let mut text_buf = String::new();
    let mut raw_buf: Vec<u8> = Vec::new();

    while pos + 1 < end {
        let word = u16::from_be_bytes([data[pos], data[pos + 1]]);

        if word == 0xFF00 {
            // FF00 구분자 — 누적된 텍스트가 있으면 기술명 엔트리로 플러시
            if let Some(e_start) = entry_start {
                // raw_hex에 FF00도 포함
                raw_buf.push(0xFF);
                raw_buf.push(0x00);
                text_buf.push_str("{ctrl:FF00}");

                let entry_idx = entries.len();
                entries.push(make_skill_entry(prefix, entry_idx, e_start, &raw_buf, text_buf.clone()));

                text_buf.clear();
                raw_buf.clear();
                entry_start = None;
            }
            pos += 2;
        } else if word == 0x0000 {
            // 서브테이블 구분자 — 무시하고 진행
            pos += 2;
        } else if table.is_text_glyph(word) || is_wide_char(word) {
            // 유효 타일코드
            if entry_start.is_none() {
                entry_start = Some(pos);
            }
            raw_buf.push(data[pos]);
            raw_buf.push(data[pos + 1]);
            text_buf.push_str(&table.decode(word));
            pos += 2;
        } else {
            // 비유효 워드 — 버퍼 리셋
            text_buf.clear();
            raw_buf.clear();
            entry_start = None;
            pos += 2;
        }
    }

    // 범위 끝에서 플러시하지 않음 — FF00 종료가 없으면 불완전한 엔트리
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
