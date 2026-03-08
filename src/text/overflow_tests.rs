use super::*;
use crate::text::patcher::SeqType;

#[test]
fn measure_lines_plain_text() {
    let text = "가나다라마바사{ctrl:FF02}아자차카타{ctrl:FF02}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![
            LineInfo { char_count: 7, text: "가나다라마바사".into() },
            LineInfo { char_count: 5, text: "아자차카타".into() },
        ]
    );
}

#[test]
fn measure_lines_with_ctrl_codes() {
    let text = "{ctrl:FF30:0001:0000:03A6}안녕하세요{ctrl:FF02}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![LineInfo { char_count: 5, text: "안녕하세요".into() }]
    );
}

#[test]
fn measure_lines_with_wide_tiles() {
    let text = "{wide:063}대봉인탑{wide:065}{ctrl:FF02}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![LineInfo { char_count: 6, text: "「대봉인탑」".into() }]
    );
}

#[test]
fn measure_lines_empty() {
    let text = "{ctrl:FF02}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![LineInfo { char_count: 0, text: "".into() }]
    );
}

#[test]
fn measure_lines_no_ff02_terminator() {
    let text = "파이어볼{ctrl:FF00}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![LineInfo { char_count: 4, text: "파이어볼".into() }]
    );
}

#[test]
fn measure_lines_ff00_separator() {
    let text = "파이어볼{ctrl:FF00}아이스{ctrl:FF00}썬더{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(
        lines,
        vec![
            LineInfo { char_count: 4, text: "파이어볼".into() },
            LineInfo { char_count: 3, text: "아이스".into() },
            LineInfo { char_count: 2, text: "썬더".into() },
        ]
    );
}

// --- 경계값 및 위반 검사 테스트 ---

#[test]
fn measure_lines_exactly_19_chars() {
    let text = "가나다라마바사아자차카타파하일이삼사오{ctrl:FF02}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(lines[0].char_count, 19);
}

#[test]
fn check_entry_no_violation() {
    let limit = TextLimit {
        max_chars_per_line: 19,
        max_lines: Some(3),
        label: "test",
    };
    let text = "가나다라마{ctrl:FF02}바사아자차{ctrl:FF02}카타파하일{ctrl:FF02}{ctrl:FF05}";
    let violations = check_entry("TEST_0001", "TEST.SEQ", text, &limit, None, None, false);
    assert!(violations.is_empty());
}

#[test]
fn check_entry_line_overflow() {
    let limit = TextLimit {
        max_chars_per_line: 5,
        max_lines: None,
        label: "test",
    };
    let text = "가나다라마바{ctrl:FF02}{ctrl:FF05}";
    let violations = check_entry("TEST_0001", "TEST.SEQ", text, &limit, None, None, false);
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        &violations[0].kind,
        ViolationKind::LineOverflow { char_count: 6, .. }
    ));
}

#[test]
fn check_entry_too_many_lines() {
    let limit = TextLimit {
        max_chars_per_line: 19,
        max_lines: Some(3),
        label: "test",
    };
    let text = "가{ctrl:FF02}나{ctrl:FF02}다{ctrl:FF02}라{ctrl:FF02}{ctrl:FF05}";
    let violations = check_entry("TEST_0001", "TEST.SEQ", text, &limit, None, None, false);
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        &violations[0].kind,
        ViolationKind::TooManyLines {
            line_count: 4,
            limit: 3
        }
    ));
}

#[test]
fn limit_for_mp() {
    let limit = limit_for_seq_type(SeqType::Mp);
    assert_eq!(limit.max_chars_per_line, 19);
    assert_eq!(limit.max_lines, Some(3));
}

#[test]
fn limit_for_diary() {
    let limit = limit_for_seq_type(SeqType::Diary);
    assert_eq!(limit.max_chars_per_line, 19);
    assert_eq!(limit.max_lines, None);
}

// --- COMMON FF09 분리 테스트 ---

#[test]
fn common_ff09_splits_segments() {
    // COMMON_0031 스타일: 캐릭터 이름이 FF09로 구분
    let text = "아르르{ctrl:FF09}루루{ctrl:FF09}셰죠{ctrl:FF09}미노타우로스";
    // 일반 모드: FF09는 분리 안 됨 → 전체가 하나의 세그먼트 (제어코드 제외)
    let lines = measure_lines(text);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].char_count, 13); // 아르르루루셰죠미노타우로스

    // Common 모드: FF09도 분리
    let lines = measure_lines_for(text, Some(SeqType::Common));
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], LineInfo { char_count: 3, text: "아르르".into() });
    assert_eq!(lines[1], LineInfo { char_count: 2, text: "루루".into() });
    assert_eq!(lines[2], LineInfo { char_count: 2, text: "셰죠".into() });
    assert_eq!(lines[3], LineInfo { char_count: 6, text: "미노타우로스".into() });
}

#[test]
fn common_ff09_battle_message_fragments() {
    // COMMON_0030 스타일: 전투 메시지 조각
    let text = "경험치{ctrl:FF09}는「{ctrl:FF09}」을 익혔다!{ctrl:FF09}아무것도 얻지 못했다";
    let lines = measure_lines_for(text, Some(SeqType::Common));
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].char_count, 3); // 경험치
    assert_eq!(lines[1].char_count, 2); // 는「
    assert_eq!(lines[2].char_count, 7); // 」을 익혔다!
    assert_eq!(lines[3].char_count, 11); // 아무것도 얻지 못했다
}

#[test]
fn common_check_entry_no_false_positive() {
    let limit = limit_for_seq_type(SeqType::Common);
    // FF09로 구분된 짧은 조각들 → 위반 없어야 함
    let text = "경험치{ctrl:FF09}는「{ctrl:FF09}」을 익혔다!{ctrl:FF09}아무것도 얻지 못했다";
    let violations = check_entry("COMMON_0030", "COMMON.SEQ", text, &limit, Some(SeqType::Common), None, false);
    assert!(violations.is_empty());
}

#[test]
fn jp_baseline_skips_overflow_when_original_exceeds() {
    let limit = TextLimit {
        max_chars_per_line: 19,
        max_lines: Some(3),
        label: "test",
    };
    // JP 원문이 이미 20자 → KO 20자는 위반 아님
    let jp = "12345678901234567890{ctrl:FF02}{ctrl:FF05}";
    let ko = "가나다라마바사아자차카타파하일이삼사오영{ctrl:FF02}{ctrl:FF05}"; // 20자
    let violations = check_entry("TEST", "TEST.SEQ", ko, &limit, None, Some(jp), false);
    assert!(violations.is_empty());
}

#[test]
fn jp_baseline_flags_when_ko_exceeds_original() {
    let limit = TextLimit {
        max_chars_per_line: 19,
        max_lines: Some(3),
        label: "test",
    };
    // JP 원문 20자, KO 21자 → 위반
    let jp = "12345678901234567890{ctrl:FF02}{ctrl:FF05}";
    let ko = "가나다라마바사아자차카타파하일이삼사오영일{ctrl:FF02}{ctrl:FF05}"; // 21자
    let violations = check_entry("TEST", "TEST.SEQ", ko, &limit, None, Some(jp), false);
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        &violations[0].kind,
        ViolationKind::LineOverflow { char_count: 21, limit: 20, .. }
    ));
}

#[test]
fn jp_baseline_line_count() {
    let limit = TextLimit {
        max_chars_per_line: 19,
        max_lines: Some(3),
        label: "test",
    };
    // JP 원문 4줄 → KO 4줄은 위반 아님, 5줄은 위반
    let jp = "가{ctrl:FF02}나{ctrl:FF02}다{ctrl:FF02}라{ctrl:FF02}{ctrl:FF05}";
    let ko4 = "A{ctrl:FF02}B{ctrl:FF02}C{ctrl:FF02}D{ctrl:FF02}{ctrl:FF05}";
    let ko5 = "A{ctrl:FF02}B{ctrl:FF02}C{ctrl:FF02}D{ctrl:FF02}E{ctrl:FF02}{ctrl:FF05}";
    assert!(check_entry("T", "T.SEQ", ko4, &limit, None, Some(jp), false).is_empty());
    assert_eq!(check_entry("T", "T.SEQ", ko5, &limit, None, Some(jp), false).len(), 1);
}

#[test]
fn wide_tag_decimal_parsing_brackets() {
    // glyph.rs encodes wide_idx as decimal: {wide:063} means idx=63
    // overflow.rs must parse as decimal too
    let text = "{wide:063}테스트{wide:065}{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(lines[0].text, "「테스트」");
    assert_eq!(lines[0].char_count, 5);
}

#[test]
fn wide_tag_high_decimal_value() {
    // {wide:128} is decimal 128 — should parse correctly as decimal
    let text = "테스트{wide:128}끝{ctrl:FF05}";
    let lines = measure_lines(text);
    assert_eq!(lines[0].char_count, 5); // 테스트 + □ + 끝
    assert_eq!(lines[0].text, "테스트□끝");
}
