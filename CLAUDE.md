# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sega Saturn 버전 마도물어(Madou Monogatari)의 한국어 번역 ROM 핵 프로젝트.

- **원본 ROM**: `roms/Madou_Monogatari_JAP.bin` (BIN/CUE, 2352-byte sectors, ~140MB)
- **게임 ID**: T-6607G, V1.003, 빌드일자 1998-06-04
- **번역 방향**: 일본어 → 한국어
- **주 언어**: Rust (edition 2024)

## Build & Run

```bash
cargo build            # 빌드
cargo test             # 전체 테스트
cargo test <test_name> # 단일 테스트
cargo run              # 실행
```

## Directory Structure

- `roms/` — 원본 일본어 ROM (gitignored, 로컬에만 존재)
- `out/` — 생성된 패치/수정 ROM 출력 (gitignored)
  - `out/dec/` — CNX 압축 해제된 파일 (FONT.CEL, *.SEQ 등)
- `assets/` — 프로젝트 에셋
  - `assets/glyph_mapping.csv` — 글리프 인덱스 → 문자 매핑 테이블 (844 엔트리, git tracked)
  - `assets/fonts/` — 한글 TTF 폰트 (사용자 준비, gitignored)
  - `assets/translations/` — 번역 JSON (사용자 준비, gitignored)
- `src/` — Rust 소스 코드
  - `src/commands/` — CLI 명령어 구현 (disc, font, text, build, decode, disasm, diff)
  - `src/compression/` — CNX v2 컴프레서/디컴프레서
  - `src/disc/` — ISO 9660, TrackedDisc, BPS, EDC/ECC, 섹터 핸들링
  - `src/disasm/` — SH-2 재귀/리니어 디스어셈블러, xref, 콜그래프
  - `src/font/` — TTF → 4bpp 글리프 생성, 프롤로그/배틀 UI/메뉴 탭/레벨업 스프라이트 렌더러
  - `src/text/` — SEQ 파서, 텍스트 패칭, 포인터 수정, 글리프 할당, 오버플로우 검사
  - `src/sh2/` — SH-2 명령어 디코딩/표시
  - `src/output/` — 디스어셈블리 출력 포맷
- `tests/` — 통합 테스트 (build pipeline, CNX round-trip)

## ROM Header (실측값)

```
Hardware ID:    "SEGA SEGASATURN "
Maker ID:       "SEGA TP T-66    "
Product Number: "T-6607G   "
Version:        "V1.003"
Release Date:   "19980604"
Media:          "CD-1/1  "
Area:           "J" (Japan only)
Game Title:     "MADOUMONOGATARI "
IP Size:        0x1800 (6144 bytes)
1st Read Addr:  0x06004000
```

디스크 구조: Track 01 = MODE1/2352 데이터 (57,318 sectors), Track 02 = CD-DA 오디오 (sector 57,318~)

## Technical Context

- 2352바이트 섹터: sync(12) + header(4) + **user data(2048)** + EDC(4) + reserved(8) + ECC(276)
- BIN 오프셋 → user data: `sector_number * 2352 + 16`
- Saturn SH-2 CPU: big-endian, 16-bit 고정 길이 명령어
- Work RAM High: `0x06000000` (1MB), 게임 코드 로드 주소
- 텍스트 인코딩: **FONT.CEL 타일 인덱스를 2-byte BE로 직접 사용** (SJIS가 아님)
- 모든 게임 파일이 CNX 커스텀 압축 사용 (자체 CNX v2 디컴프레서 구현 완료)
- 텍스트 스크립트: `.SEQ` 파일 (112개)
- 텍스트 렌더링: **VDP2 NBG3** (PND 12비트 하드웨어 제한: 최대 914 16x16 글리프)
- 폰트: `FONT.CEL` (CNX 압축, 4bpp 8x8 타일, 32 bytes/tile)
  - 타일 0-437: ASCII, UI, 와이드 문자
  - 타일 438+: 16x16 글리프 (2x2 타일 조합, TL-TR-BL-BR)

## Conventions

- CLI: `clap` derive 매크로
- 에러 처리: `anyhow` + `thiserror`
- 인코딩: `encoding_rs` (Shift-JIS)
- 이미지: `png` 크레이트 (폰트 그리드/글리프 렌더링)
- 폰트 래스터라이즈: `fontdue` (한글 TTF → 4bpp 타일 변환)
- 정규식: `regex` (텍스트 오버플로우 검사 — 제어코드/wide 타일 파싱)

## CLI Commands

```bash
# 디스크 정보
cargo run -- info
cargo run -- files
cargo run -- extract -f <filename> -o <output>

# 압축 해제
cargo run -- decompress -f <filename> -o <output>
cargo run -- decompress-all                       # 모든 CNX 파일 → out/dec/

# 텍스트 분석
cargo run -- dump-script -i out/dec/MP0001.SEQ    # 단일 스크립트 덤프
cargo run -- dump-script --all                    # 전체 112 SEQ → out/scripts/

# 폰트
cargo run -- font-dump -i out/dec/FONT.CEL --skip 438 --combine-2x2 --cols 16 --scale 3

# 텍스트 디코딩
cargo run -- decode-text "tile hex"               # 타일코드 hex → 일본어/한국어 디코딩

# 전체 번역 ROM 빌드
cargo run -- build-rom                            # 전체 번역 JSON → 패치 ROM 생성
cargo run -- build-rom -O out/debug/              # 출력 디렉터리 지정
cargo run -- build-rom -f path/to/font.ttf        # 대화 폰트 지정 (기본: assets/fonts/Galmuri11.ttf)
cargo run -- build-rom --font-size 12.0           # 대화 폰트 크기 (기본 12.0)
cargo run -- build-rom --only-seq MP0101          # 특정 SEQ만 패치
cargo run -- build-rom --except-seq COMMON        # 특정 SEQ 제외
cargo run -- build-rom --skip-seq                 # SEQ 전체 스킵 (폰트만 패치)
cargo run -- build-rom --dump-seq --dump-ptrs     # 디버그 덤프 출력
cargo run -- build-rom --skip-common-ptrs         # COMMON.SEQ 포인터 수정 스킵
cargo run -- build-rom --skip-script-ptrs         # 스크립트 포인터 수정 스킵

# 프롤로그 스프라이트 (OP_SP02.SPR)
cargo run -- build-rom --no-prologue              # 프롤로그 패치 스킵
cargo run -- build-rom --prologue-font path/to/font.ttf
cargo run -- build-rom --prologue-font-size 14.0

# 배틀 UI 스프라이트 (SYSTEM.SPR)
cargo run -- build-rom --no-battle-ui             # 배틀 UI 패치 스킵
cargo run -- build-rom --battle-ui-font path/to/font.ttf
cargo run -- build-rom --battle-ui-font-size 8.0

# 배틀 메뉴 탭 스프라이트 (SYSTEM.SPR)
cargo run -- build-rom --no-menu-tabs             # 메뉴 탭 패치 스킵
cargo run -- build-rom --menu-tab-font path/to/font.ttf
cargo run -- build-rom --menu-tab-font-size 10.0

# 레벨업 스프라이트 (SYSTEM.SPR)
cargo run -- build-rom --no-levelup               # 레벨업 패치 스킵
cargo run -- build-rom --levelup-font path/to/font.ttf
cargo run -- build-rom --levelup-font-size 14.0

# 글리프 슬롯 검사 (dry-run, ROM 불필요)
cargo run -- check-glyphs
cargo run -- check-glyphs --verbose

# 텍스트 오버플로우 검사
cargo run -- check-overflow
cargo run -- check-overflow --verbose

# ROM 비교/디버깅
cargo run -- rom-diff -a rom_a.bin -b rom_b.bin
cargo run -- test-recompress --seq MP0101.SEQ

# 디스어셈블리
cargo run -- disasm                               # SH-2 재귀 디스어셈블리
cargo run -- disasm -m linear --start 0x06046780 --end 0x060468C0
cargo run -- disasm -m find-string -q "COMMON.SEQ"
cargo run -- disasm -m xrefs -q 0x06004040
cargo run -- disasm -m func -q 0x06017B70
cargo run -- disasm -m scan-funcs
cargo run -- disasm -m call-graph -q 0x06048A34
cargo run -- disasm -m callers -q 0x06048A00
cargo run -- disasm -m strings
cargo run -- disasm -m mem-refs -q 0x00200000-0x00300000
```
