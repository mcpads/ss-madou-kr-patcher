use regex::Regex;
use std::sync::LazyLock;

use crate::text::patcher::SeqType;
use crate::text::script::ScriptDump;

// ---------------------------------------------------------------------------
// Line measurement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineInfo {
    pub char_count: usize,
    pub text: String,
}

static RE_CTRL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{ctrl:[0-9A-Fa-f]{4}(?::[0-9A-Fa-f]{4})*\}").unwrap()
});

static RE_JOSA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{josa:(.)\}").unwrap());

static RE_WIDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{wide:(\d{3})\}").unwrap()
});

fn wide_to_display(code: u16) -> &'static str {
    match code {
        63 => "「",
        65 => "」",
        _ => "□",
    }
}

/// 텍스트 문자열을 세그먼트 단위로 분리하고 각 세그먼트의 표시 글자 수를 측정한다.
///
/// - `{ctrl:FF02}`, `{ctrl:FF00}`을 세그먼트 구분자로 처리
/// - `seq_type`이 `Common`이면 `{ctrl:FF09}`도 세그먼트 구분자로 처리
/// - `{ctrl:FF05}`, `{ctrl:FF39}`, `{ctrl:FFFF}` 이후는 무시
/// - `{ctrl:...}` 제어코드는 글자 수에서 제외
/// - `{wide:XXX}`는 표시 문자 1개로 카운트
pub fn measure_lines(text: &str) -> Vec<LineInfo> {
    measure_lines_for(text, None)
}

pub fn measure_lines_for(text: &str, seq_type: Option<SeqType>) -> Vec<LineInfo> {
    let text = if let Some(pos) = find_terminator(text) {
        &text[..pos]
    } else {
        text
    };

    let raw_lines = split_on_separators(text, seq_type);

    raw_lines
        .into_iter()
        .map(|line| {
            let display = resolve_display_text(&line);
            let char_count = display.chars().count();
            LineInfo {
                char_count,
                text: display,
            }
        })
        .collect()
}

fn find_terminator(text: &str) -> Option<usize> {
    ["{ctrl:FF05}", "{ctrl:FF39}", "{ctrl:FFFF}"]
        .iter()
        .filter_map(|t| text.find(t))
        .min()
}

/// 기본 구분자: FF02, FF00. COMMON이면 FF09도 추가.
fn split_on_separators(text: &str, seq_type: Option<SeqType>) -> Vec<String> {
    let separators: &[&str] = if seq_type == Some(SeqType::Common) {
        &["{ctrl:FF02}", "{ctrl:FF00}", "{ctrl:FF09}"]
    } else {
        &["{ctrl:FF02}", "{ctrl:FF00}"]
    };

    let mut segments = vec![text.to_string()];
    for sep in separators {
        let mut next = Vec::new();
        for s in &segments {
            for part in s.split(sep) {
                next.push(part.to_string());
            }
        }
        segments = next;
    }

    while segments.last().is_some_and(|l| l.is_empty()) {
        segments.pop();
    }
    if segments.is_empty() {
        segments.push(String::new());
    }
    segments
}

fn resolve_display_text(line: &str) -> String {
    let with_wide = RE_WIDE.replace_all(line, |caps: &regex::Captures| {
        let code = caps[1].parse::<u16>().unwrap_or(0);
        wide_to_display(code).to_string()
    });
    // A particle marker draws one glyph.
    let with_josa = RE_JOSA.replace_all(&with_wide, "$1");
    RE_CTRL.replace_all(&with_josa, "").to_string()
}

// ---------------------------------------------------------------------------
// Text limits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct TextLimit {
    pub max_chars_per_line: usize,
    pub max_lines: Option<usize>,
    pub label: &'static str,
}

pub fn limit_for_seq_type(seq_type: SeqType) -> TextLimit {
    match seq_type {
        SeqType::Mp | SeqType::Pt => TextLimit {
            max_chars_per_line: 19,
            max_lines: Some(3),
            label: "dialogue",
        },
        SeqType::Diary => TextLimit {
            max_chars_per_line: 19,
            max_lines: None,
            label: "diary",
        },
        SeqType::Common => TextLimit {
            max_chars_per_line: 19,
            max_lines: None,
            label: "common",
        },
        SeqType::Other => TextLimit {
            max_chars_per_line: 19,
            max_lines: None,
            label: "default",
        },
    }
}

// ---------------------------------------------------------------------------
// Violation checking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    LineOverflow {
        line_index: usize,
        char_count: usize,
        limit: usize,
        line_text: String,
    },
    TooManyLines {
        line_count: usize,
        limit: usize,
    },
    /// 선택지 옵션이 JP 원문 기준 최대 폭을 초과
    ChoiceLineOverflow {
        option_index: usize,
        char_count: usize,
        jp_max_width: usize,
        line_text: String,
    },
    /// 선택지 옵션 수가 JP 원문과 불일치
    ChoiceOptionCountMismatch {
        ko_count: usize,
        jp_count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Violation {
    pub entry_id: String,
    pub source: String,
    pub kind: ViolationKind,
}

/// `jp_text`가 주어지면 줄별로 `max(limit, JP원문_줄_길이)`를 effective limit으로 사용.
/// 원문이 이미 제한을 넘는 경우 다른 렌더링 모드를 사용하는 것으로 간주.
///
/// `pad_to_original`이면 JP 원문 글자수가 절대 상한 (고정길이 패치).
pub fn check_entry(
    entry_id: &str,
    source: &str,
    ko_text: &str,
    limit: &TextLimit,
    seq_type: Option<SeqType>,
    jp_text: Option<&str>,
    pad_to_original: bool,
) -> Vec<Violation> {
    let ko_lines = measure_lines_for(ko_text, seq_type);
    let jp_lines = jp_text.map(|jp| measure_lines_for(jp, seq_type));
    let mut violations = Vec::new();

    for (i, line) in ko_lines.iter().enumerate() {
        let jp_chars = jp_lines
            .as_ref()
            .and_then(|jl| jl.get(i))
            .map(|l| l.char_count)
            .unwrap_or(0);
        let effective_limit = if pad_to_original {
            // 고정길이: JP 글자수가 한도 (JP가 없으면 기본 limit 사용)
            if jp_chars > 0 { jp_chars } else { limit.max_chars_per_line }
        } else {
            limit.max_chars_per_line.max(jp_chars)
        };

        if line.char_count > effective_limit {
            violations.push(Violation {
                entry_id: entry_id.to_string(),
                source: source.to_string(),
                kind: ViolationKind::LineOverflow {
                    line_index: i,
                    char_count: line.char_count,
                    limit: effective_limit,
                    line_text: line.text.clone(),
                },
            });
        }
    }

    // 줄 수 검사: pad_to_original이면 JP 줄 수와 정확히 일치해야 함
    let jp_line_count = jp_lines.as_ref().map(|jl| jl.len()).unwrap_or(0);
    if pad_to_original {
        if jp_line_count > 0 && ko_lines.len() != jp_line_count {
            violations.push(Violation {
                entry_id: entry_id.to_string(),
                source: source.to_string(),
                kind: ViolationKind::TooManyLines {
                    line_count: ko_lines.len(),
                    limit: jp_line_count,
                },
            });
        }
    } else if let Some(max) = limit.max_lines {
        let effective_max = max.max(jp_line_count);
        if ko_lines.len() > effective_max {
            violations.push(Violation {
                entry_id: entry_id.to_string(),
                source: source.to_string(),
                kind: ViolationKind::TooManyLines {
                    line_count: ko_lines.len(),
                    limit: effective_max,
                },
            });
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// Choice entry detection & checking
// ---------------------------------------------------------------------------

/// 선택지(choice menu) 엔트리인지 판별한다.
///
/// 두 가지 패턴을 감지:
/// - FF1B 종료형: 옵션들이 FF02로 분리되고 `{ctrl:FF1B:XXXX}`로 종료
/// - FF0B 포함형: `{ctrl:FF0B:...}`로 시작하는 공식 선택지 메뉴
pub fn is_choice_entry(text: &str) -> bool {
    text.contains("{ctrl:FF1B:") || text.contains("{ctrl:FF0B")
}

/// 선택지 텍스트에서 개별 옵션을 추출한다.
///
/// FF1B/FF0B/FF05 이전의 텍스트를 FF02로 분할하여 각 옵션의 표시 문자열을 반환.
/// 빈 옵션(제어코드만 포함)은 제외.
fn extract_choice_options(text: &str) -> Vec<LineInfo> {
    // FF1B 이전만 사용 (FF1B 이후는 제어코드)
    let effective = if let Some(pos) = text.find("{ctrl:FF1B:") {
        &text[..pos]
    } else if let Some(pos) = text.find("{ctrl:FF05}") {
        &text[..pos]
    } else {
        text
    };

    // FF0B 제어코드 이후부터 옵션 시작
    let options_text = if let Some(pos) = effective.find("{ctrl:FF0B") {
        // FF0B 제어코드 전체를 건너뛰기 ({ctrl:FF0B:...} 패턴)
        if let Some(end) = effective[pos..].find('}') {
            &effective[pos + end + 1..]
        } else {
            effective
        }
    } else {
        effective
    };

    // FF02로 분할
    let raw_options: Vec<&str> = options_text.split("{ctrl:FF02}").collect();

    raw_options
        .into_iter()
        .map(|opt| {
            // FF00 제거 (옵션 앞에 FF00이 올 수 있음)
            let cleaned = opt.replace("{ctrl:FF00}", "");
            let display = resolve_display_text(&cleaned);
            let char_count = display.chars().count();
            LineInfo {
                char_count,
                text: display,
            }
        })
        .filter(|info| info.char_count > 0)
        .collect()
}

/// 선택지 엔트리의 옵션별 오버플로우를 검사한다.
///
/// JP 원문의 가장 긴 옵션 길이(`jp_max_width`)를 기준으로,
/// 각 KO 옵션이 이를 초과하면 `ChoiceLineOverflow` 위반을 생성한다.
/// 옵션 수가 다르면 `ChoiceOptionCountMismatch` 위반도 생성한다.
pub fn check_choice_entry(
    entry_id: &str,
    source: &str,
    ko_text: &str,
    jp_text: Option<&str>,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    let ko_options = extract_choice_options(ko_text);
    let jp_options = jp_text.map(|jp| extract_choice_options(jp));

    let jp_max_width = jp_options
        .as_ref()
        .map(|opts| opts.iter().map(|o| o.char_count).max().unwrap_or(0))
        .unwrap_or(0);

    // JP 정보가 없으면 일반 limit 사용
    if jp_max_width == 0 {
        return violations;
    }

    // 옵션 수 불일치 검사
    if let Some(ref jp_opts) = jp_options {
        if ko_options.len() != jp_opts.len() {
            violations.push(Violation {
                entry_id: entry_id.to_string(),
                source: source.to_string(),
                kind: ViolationKind::ChoiceOptionCountMismatch {
                    ko_count: ko_options.len(),
                    jp_count: jp_opts.len(),
                },
            });
        }
    }

    // 각 KO 옵션이 JP 최대 폭을 초과하는지 검사
    for (i, opt) in ko_options.iter().enumerate() {
        if opt.char_count > jp_max_width {
            violations.push(Violation {
                entry_id: entry_id.to_string(),
                source: source.to_string(),
                kind: ViolationKind::ChoiceLineOverflow {
                    option_index: i,
                    char_count: opt.char_count,
                    jp_max_width,
                    line_text: opt.text.clone(),
                },
            });
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// Batch checking (ScriptDump level)
// ---------------------------------------------------------------------------

pub fn check_script(dump: &ScriptDump, limit: &TextLimit, seq_type: SeqType) -> Vec<Violation> {
    let mut violations = Vec::new();
    for entry in &dump.entries {
        if let Some(ko) = &entry.ko {
            if ko.is_empty() {
                continue;
            }
            let jp: Option<&str> = if entry.text.is_empty() { None } else { Some(&entry.text) };

            // 선택지 엔트리는 별도 검사 (JP 최대 옵션 폭 기준)
            if is_choice_entry(ko) || jp.is_some_and(|j| is_choice_entry(j)) {
                violations.extend(check_choice_entry(
                    &entry.id,
                    &dump.source,
                    ko,
                    jp,
                ));
            }

            // 기존 일반 검사도 병행 (줄 수, 라인 길이)
            violations.extend(check_entry(
                &entry.id,
                &dump.source,
                ko,
                limit,
                Some(seq_type),
                jp,
                entry.pad_to_original,
            ));
        }
    }
    violations
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct OverflowStats {
    pub total_entries: usize,
    pub total_lines: usize,
    pub overflow_lines: usize,
    pub overflow_entries: usize,
    pub max_char_count: usize,
    pub max_line_count: usize,
    pub char_distribution: Vec<(usize, usize)>,
}

pub fn compute_stats(dumps: &[&ScriptDump], limit: &TextLimit, seq_type: SeqType) -> OverflowStats {
    let mut stats = OverflowStats::default();
    let mut dist: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    for dump in dumps {
        for entry in &dump.entries {
            let ko = match &entry.ko {
                Some(k) if !k.is_empty() => k,
                _ => continue,
            };
            stats.total_entries += 1;
            let lines = measure_lines_for(ko, Some(seq_type));
            let line_count = lines.len();
            stats.total_lines += line_count;

            if line_count > stats.max_line_count {
                stats.max_line_count = line_count;
            }

            let mut entry_overflow = false;
            for line in &lines {
                *dist.entry(line.char_count).or_default() += 1;
                if line.char_count > stats.max_char_count {
                    stats.max_char_count = line.char_count;
                }
                if line.char_count > limit.max_chars_per_line {
                    stats.overflow_lines += 1;
                    entry_overflow = true;
                }
            }
            if entry_overflow || limit.max_lines.is_some_and(|m| line_count > m) {
                stats.overflow_entries += 1;
            }
        }
    }

    let mut sorted: Vec<_> = dist.into_iter().collect();
    sorted.sort_by_key(|&(k, _)| k);
    stats.char_distribution = sorted;
    stats
}

// ---------------------------------------------------------------------------
// Byte-level delta analysis
// ---------------------------------------------------------------------------

/// Per-entry byte delta between original (raw_hex) and Korean translation.
#[derive(Debug, Clone)]
pub struct ByteDeltaEntry {
    pub entry_id: String,
    pub source: String,
    pub raw_bytes: usize,
    pub ko_bytes: usize,
    pub delta: isize,
    pub pad_to_original: bool,
}

/// Per-file byte delta summary.
#[derive(Debug, Clone)]
pub struct ByteDeltaSummary {
    pub source: String,
    pub total_entries: usize,
    pub grow_entries: usize,
    pub shrink_entries: usize,
    pub total_delta: isize,
    pub max_grow: isize,
    pub max_shrink: isize,
    /// Entries with largest byte growth (top 5).
    pub top_growers: Vec<ByteDeltaEntry>,
}

/// Count raw_hex bytes (space-separated hex pairs).
fn count_raw_hex_bytes(raw_hex: &str) -> usize {
    if raw_hex.is_empty() {
        return 0;
    }
    raw_hex.split_whitespace().count()
}

/// Estimate Korean translation byte count from ko text.
///
/// Each token (character, control code, wide tag, tile tag) produces 2 bytes.
pub fn estimate_ko_bytes(ko_text: &str) -> usize {
    use crate::text::patcher::parse_ko_tokens;
    use crate::text::patcher::TextToken;

    let tokens = parse_ko_tokens(ko_text);
    let mut count = 0usize;
    for token in &tokens {
        match token {
            TextToken::Text(s) => count += s.chars().count(),
            TextToken::Tile(_) | TextToken::Ctrl(_) => count += 1,
        }
    }
    count * 2
}

/// Compute byte-level deltas for all entries in a script dump.
pub fn compute_byte_deltas(dump: &ScriptDump) -> ByteDeltaSummary {
    let mut entries = Vec::new();
    let mut total_delta: isize = 0;
    let mut grow_entries = 0usize;
    let mut shrink_entries = 0usize;
    let mut max_grow: isize = 0;
    let mut max_shrink: isize = 0;

    for entry in &dump.entries {
        let ko = match &entry.ko {
            Some(k) if !k.is_empty() => k,
            _ => continue,
        };

        let raw_bytes = count_raw_hex_bytes(&entry.raw_hex);
        let ko_bytes = estimate_ko_bytes(ko);
        let delta = ko_bytes as isize - raw_bytes as isize;

        if delta > 0 {
            grow_entries += 1;
            if delta > max_grow {
                max_grow = delta;
            }
        } else if delta < 0 {
            shrink_entries += 1;
            if delta < max_shrink {
                max_shrink = delta;
            }
        }
        total_delta += delta;

        entries.push(ByteDeltaEntry {
            entry_id: entry.id.clone(),
            source: dump.source.clone(),
            raw_bytes,
            ko_bytes,
            delta,
            pad_to_original: entry.pad_to_original,
        });
    }

    // Top 5 growers (largest positive delta)
    entries.sort_by(|a, b| b.delta.cmp(&a.delta));
    let top_growers: Vec<ByteDeltaEntry> = entries
        .iter()
        .filter(|e| e.delta > 0)
        .take(5)
        .cloned()
        .collect();

    ByteDeltaSummary {
        source: dump.source.clone(),
        total_entries: entries.len(),
        grow_entries,
        shrink_entries,
        total_delta,
        max_grow,
        max_shrink,
        top_growers,
    }
}

#[cfg(test)]
#[path = "overflow_tests.rs"]
mod tests;
