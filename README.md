# ss-madou-kr-patcher

세가 새턴 **마도물어** (Madou Monogatari, 1998)의 한글 패치 코드베이스.

JP ROM (BIN/CUE)에서 텍스트를 추출하고, 한글 폰트/텍스트를 삽입해 패치된 ROM 및 BPS 패치를 생성하는 Rust CLI 도구입니다.

## 빌드

```bash
cargo build
cargo test
```

## 한글 패치 ROM 빌드

### 필요 파일

| 파일 | 경로 (기본값) | 설명 |
|------|---------------|------|
| JP ROM | `roms/Madou_Monogatari_JAP.bin` | BIN/CUE 원본 (T-6607G V1.003, ~140MB). `SS_MADOU_ROM` 환경변수로 경로 변경 가능 |
| 글리프 매핑 | `assets/glyph_mapping.csv` | 타일 인덱스 → 문자 매핑 테이블 (동봉) |
| 번역 JSON | `assets/translations/scripts/` | SEQ별 번역 JSON. `needs_review/`, `complete/` 등 서브디렉터리 자동 탐색 |
| 대화 폰트 | `assets/fonts/Galmuri11.ttf` | 16x16 한글 폰트 TTF (`-f` 플래그로 변경) |
| 프롤로그 폰트 | `assets/fonts/MaplestoryBold.ttf` | 프롤로그 스프라이트용 (`--prologue-font`) |
| 전투 UI 폰트 | `assets/fonts/dalmoori.ttf` | 8px 전투 UI용 (`--battle-ui-font`) |
| 메뉴 탭 폰트 | `assets/fonts/Galmuri9.ttf` | 배틀 메뉴 탭용 (`--menu-tab-font`) |
| 레벨업 폰트 | `assets/fonts/MaplestoryBold.ttf` | 레벨업 스프라이트용 (`--levelup-font`) |

### 실행

```bash
# 기본 빌드
cargo run -- build-rom

# 옵션
cargo run -- build-rom -O out/debug/                    # 출력 디렉터리
cargo run -- build-rom -f path/to/font.ttf              # 대화 폰트 변경
cargo run -- build-rom --only-seq MP0101                 # 특정 SEQ만
cargo run -- build-rom --skip-seq                        # 폰트만 패치
cargo run -- build-rom --no-prologue --no-battle-ui --no-menu-tabs --no-levelup  # 스프라이트 패치 스킵
```

### 출력

- `out/Madou_Monogatari_KO.bin` — 패치된 ROM
- `out/Madou_Monogatari_KO.cue` — CUE 시트
- `out/Madou_Monogatari_KO.bps` — BPS 패치

## 번역 JSON 형식

```json
{
  "source": "MP0101.SEQ",
  "entries": [
    {
      "id": "MP0101_0001",
      "offset": "0x00100",
      "raw_hex": "01 B6 02 16 ...",
      "text": "こんにちは{ctrl:FF02}アルルだよ!{ctrl:FF05}",
      "ko": "안녕하세요{ctrl:FF02}아르르예요!{ctrl:FF05}",
      "status": "done"
    }
  ]
}
```

- `raw_hex`: 원본 바이너리 (변경 금지)
- `ko`: 한글 번역 (빈 문자열이면 스킵)
- 제어코드: `{ctrl:FF02}` 줄바꿈, `{ctrl:FF05}` 종료, `{ctrl:FF00}` 서브아이템 구분

## 기타 커맨드

```bash
cargo run -- info                           # 디스크 헤더 출력
cargo run -- decompress-all                 # 전체 CNX 압축 해제
cargo run -- dump-script --all              # 전체 SEQ 텍스트 덤프
cargo run -- check-glyphs                   # 글리프 슬롯 할당 현황
cargo run -- check-overflow                 # 텍스트 오버플로우 검사
cargo run -- disasm                         # SH-2 디스어셈블리
```

## 패치 배포

완성된 BPS 패치는 [madou-monogatari-kr-patch](https://github.com/mcpads/madou-monogatari-kr-patch)에서 배포됩니다.

## 라이선스

MIT License. [LICENSE](LICENSE) 참조.

게임 ROM 파일은 이 프로젝트에 포함되지 않으며, 사용자가 별도로 준비해야 합니다.
