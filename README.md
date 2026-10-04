# 마도물어 (세가 새턴) 한글 패처

세가 새턴용 《마도물어》(1998) 디스크 이미지에 한글 패치를 적용하는 Rust 코드입니다. BIN/ISO 9660 읽기·쓰기, CNX 압축·해제, SEQ 스크립트 추출과 재삽입, 한글 폰트와 스프라이트 생성, SH-2 코드 패치와 BPS 생성을 제공합니다.

배포용 패치와 적용 방법은 [마도물어 시리즈 한글 번역 프로젝트](https://github.com/mcpads/madou-monogatari-kr-patch/tree/main/ss-madou)에서 제공합니다.

## 제공하지 않는 것

이 저장소에는 원본 디스크, 패치를 적용한 디스크, 번역 스크립트 JSON, 폰트 파일과 한글 타이틀 로고 원화가 없습니다. 따라서 이 저장소만으로는 배포 패치를 다시 만들 수 없습니다. 아래 입력을 직접 갖춘 경우에만 `build-rom`이 디스크를 생성합니다.

## 빌드와 테스트

```bash
cargo build --release
cargo test
```

기본 테스트는 합성 입력만 사용합니다. 원본 디스크, 폰트, 디스크에서 풀어낸 파일이 필요한 테스트는 `#[ignore = "requires ..."]`로 필요한 입력을 밝혀 두었습니다. 입력을 갖춘 뒤 `cargo test -- --ignored`로 실행하며, 입력이 없으면 성공으로 넘어가지 않고 실패합니다. 원본 디스크 경로는 `SS_MADOU_ROM` 환경 변수로 바꿀 수 있습니다.

## 지원 원본

| 원본 | 기본 경로 | 크기 | SHA-256 |
| --- | --- | --- | --- |
| JP (T-6607G) | `roms/Madou_Monogatari_JAP.bin` | 146,661,312바이트 | `c2f174280295ea3e992cfd8bfc04b474caf4ee017c3c4aa8896b85e3ff96e904` |
| US | `-r`로 지정 | 146,308,512바이트 | `64de3234eef76b18cc68faca68c2591e220c258342b99aabe21df55f760fa2cd` |

`build-rom`은 디스크 전체 해시를 검사하지 않습니다. 그래픽 단계는 바꾸는 원본 파일의 SHA-256을 확인하지만 모든 단계가 그런 것은 아닙니다. 위 표와 다른 디스크에서 만든 결과는 지원하지 않습니다.

## 빌드 입력

| 입력 | 기본 경로 | 비고 |
| --- | --- | --- |
| 번역 스크립트 | `assets/translations/scripts/needs_review/` | SEQ별 JSON (`-t`로 변경) |
| 글리프 매핑 | `assets/glyph_mapping.csv` | 원본 폰트 타일 번호와 문자의 대응표 (동봉) |
| 대화 폰트 | `assets/fonts/Galmuri11.ttf` | [Galmuri](https://github.com/quiple/galmuri) |
| 메뉴 탭·타이틀 저작권 폰트 | `assets/fonts/Galmuri9.ttf` | [Galmuri](https://github.com/quiple/galmuri) |
| 프롤로그·레벨업 폰트 | `assets/fonts/MaplestoryBold.ttf` | [메이플스토리 서체](https://maplestory.nexon.com/Media/Font) |
| 전투 UI 폰트 | `assets/fonts/dalmoori.ttf` | [Dalmoori](https://github.com/RanolP/dalmoori-font) |
| 벼룩 표식·카드 결과 폰트 | `assets/fonts/DNFBitBitv2.ttf` | 던파 비트비트체 v2 |
| 엔딩 스태프롤 폰트 | `assets/fonts/NEXONLv2Gothic.ttf` | 넥슨 Lv.2 고딕 |
| 타이틀 로고 원화 | `assets/title_logo/title_external_corrected.png` | 850×365 RGBA, SHA-256 `e7009dea86c80965440488d28fdff502934e5f16b92df82b60870283d8c3a958` |

배포 패치 v2.0.0은 다음 폰트 파일로 만들었습니다. 타이틀 로고, 타이틀 저작권, 벼룩 표식, 카드 결과, 엔딩 스태프롤 단계는 폰트나 원화의 SHA-256이 이 값과 다르면 진행하지 않습니다.

```text
2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f  Galmuri11.ttf
5cb68052ee0a15571747e91c20f145e24b51bb459c6cd58226fafee78d9c0b16  Galmuri9.ttf
d57eaff48a793ff872a0f33bba2943d058d07c81ed64c68054858a287b85811a  MaplestoryBold.ttf
ba40b8f9ada002005d1f5f3676a82b7bff07ddbd186a923073101099131999e6  dalmoori.ttf
eebf8c20fea14a927e74216f972d6484d8a2398efdea356f6d0c5adcae531743  DNFBitBitv2.ttf
389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60  NEXONLv2Gothic.ttf
```

스프라이트 단계에 필요한 입력이 없으면 `--no-prologue`, `--no-battle-ui`, `--no-menu-tabs`, `--no-levelup`, `--no-title-logo`, `--no-title-copyright`, `--no-flea-marker`, `--no-ending-credits`로 해당 단계를 끌 수 있습니다. 다만 카드 결과 그래픽 단계에는 끄는 옵션이 없어 `assets/fonts/DNFBitBitv2.ttf`가 항상 필요합니다.

## 디스크 생성

```bash
# JP 원본
cargo run --release -- build-rom --shared-font-tiles -O out/jp

# US 원본
cargo run --release -- build-rom --shared-font-tiles -r path/to/us.bin -O out/us
```

출력 디렉터리에 `Madou_Monogatari_KO.bin`, `.cue`, `.bps`와 글리프 배정 보고서 `Madou_Monogatari_KO.font.json`이 생깁니다. 오디오 트랙을 쓰려면 원본 CUE의 트랙 구성을 유지해야 합니다.

## 번역 스크립트 형식

`dump-script`가 SEQ 파일에서 추출하는 JSON과 같은 형식입니다. `ko`가 비어 있지 않고 `status`가 `untranslated`가 아닌 항목만 패치에 반영됩니다.

```json
{
  "source": "MP0101.SEQ",
  "source_md5": "…",
  "entries": [
    {
      "id": "MP0101_0001",
      "offset": "0x00100",
      "raw_hex": "01 B6 02 16 …",
      "text": "こんにちは{ctrl:FF02}",
      "ko": "안녕하세요{ctrl:FF02}",
      "status": "needs_human_review",
      "notes": ""
    }
  ]
}
```

`{ctrl:XXXX}` 제어 코드와 파라미터는 원문과 같은 순서로 유지해야 합니다. `check-overflow`와 `check-glyphs`로 줄 길이와 글리프 수를 원본 디스크 없이 미리 검사할 수 있습니다.

## 그 밖의 명령

`info`, `files`, `extract`, `decompress`, `decompress-all`, `font-dump`, `dump-script`, `test-recompress`, `rom-diff`, `decode-text`, `disasm`의 사용법은 `cargo run -- help <명령>`으로 확인할 수 있습니다.

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다.
