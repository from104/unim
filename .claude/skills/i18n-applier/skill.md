---
name: i18n-applier
description: UNIM의 사용자 가시 문자열을 ko/en 양언어로 i18n. CLI(rust-i18n), GTK GUI(rust-i18n 확장), Qt GUI(qsTr+yaml), GNOME extension(gettext .po), 매뉴얼 문서까지 매크로/번역 함수로 치환하고 locale 파일 채우기. "i18n 적용", "다국어 적용", "ko/en 번역", "locale 추가", "한국어/영어 변환" 요청 시 반드시 트리거. 디버그/로그 메시지는 i18n 제외.
---

# i18n Applier — 다국어 적용 패턴

## 영역 매핑
| 영역 | 매크로/함수 | locale 파일 |
|------|-------------|-------------|
| Rust(CLI/GUI-GTK/GUI-QT bridge) | `t!("key")`, `t!("key", arg=val)` | `<crate>/locales/{ko,en}.yml` |
| QML | `qsTr("English original")` | Qt translation files (.ts/.qm) 또는 yaml |
| GNOME ext (JS) | `_("text")` (gettext) | `po/{ko,en}.po` → `.mo` |
| 문서 (md) | 한/영 짝 (`README.md` ↔ `README-ko.md`) | 별도 파일 |

## 키 명명 규칙
- 형식: `<영역>_<섹션>_<역할>` snake_case
- 영역 prefix: `cli_`, `settings_`, `tray_`, `popup_`, `error_`, `common_`
- 역할 suffix: `_label`, `_subtitle`, `_tooltip`, `_desc`, `_title`, `_button`, `_msg`
- 동일 의미는 단일 키로 (`common_ok`, `common_cancel`)

## 검출 휴리스틱

### 한글 하드코딩 검출
```bash
grep -rn '"[가-힣]' /home/from104/work/unim/unim-{cli,gui-gtk,gui-common,gui-qt,daemon,dbus} \
  --include='*.rs' | grep -v '/tests/' | grep -v '/locales/' | grep -v 'log::\|tracing::\|println!\|eprintln!\|debug!\|info!\|warn!\|error!\|trace!'
```
로깅 매크로 안의 한글은 i18n 제외(개발자 가시).

### 영어 자연어 검출
```bash
grep -rEn '"[A-Z][a-z]+ [a-z]+( [a-z]+){2,}' /home/from104/work/unim --include='*.rs'
```
3단어 이상의 영어 자연어는 사용자 가시 가능성 高.

## 적용 우선순위
1. CLI 잔여 하드코딩 (이미 80% 적용)
2. GTK 설정 다이얼로그 (위젯 라벨/툴팁) — 가장 가시성 높음
3. 트레이 메뉴 (unim-gui-common/tray.rs)
4. 팝업 헤더 텍스트 (한자/이모지/특수문자)
5. Qt GUI (QML qsTr 마크업)
6. GNOME extension 누락 키
7. 에러 메시지 (사용자에게 노출되는 것만)

## rust-i18n 도입 절차 (신규 크레이트)
```toml
# Cargo.toml [dependencies]
rust-i18n = "3"
```
```rust
// src/main.rs 또는 lib.rs 최상단
rust_i18n::i18n!("locales", fallback = "en");

fn init_locale() {
    let lang = std::env::var("LANG").unwrap_or_default();
    let locale = if lang.starts_with("ko") { "ko" } else { "en" };
    rust_i18n::set_locale(locale);
}
```
빌드 시점 검증: `cargo build`에서 누락 키 컴파일 에러 발생.

## locale 파일 양식 (yml)
```yaml
# en.yml
settings_general_korean_layout_label: "Korean Keyboard Layout"
settings_general_korean_layout_tooltip: "Choose between dubeolsik (standard) or sebeolsik (390/391) layout."
common_ok: "OK"
common_cancel: "Cancel"
```
```yaml
# ko.yml
settings_general_korean_layout_label: "한국어 자판"
settings_general_korean_layout_tooltip: "두벌식(표준) 또는 세벌식(390/391) 중 선택합니다."
common_ok: "확인"
common_cancel: "취소"
```

## GNOME .po 갱신
```bash
cd /home/from104/work/unim/unim-gnome-extension
xgettext --keyword=_ --output=po/messages.pot extension.js prefs.js *.js
msgmerge -U po/ko.po po/messages.pot
msgmerge -U po/en.po po/messages.pot
# 번역 추가
msgfmt po/ko.po -o locale/ko/LC_MESSAGES/unim-gnome@from104.github.io.mo
msgfmt po/en.po -o locale/en/LC_MESSAGES/unim-gnome@from104.github.io.mo
```

## 검증
1. 빌드: `cargo build --workspace` warning 0
2. 테스트: `cargo test --workspace` 통과
3. 양언어 키 동일성: ko.yml과 en.yml의 키 집합 일치
4. 시각 검증: `LANG=en_US.UTF-8 unim-gui-gtk` / `LANG=ko_KR.UTF-8 unim-gui-gtk`

## 출력 보고서
`_workspace/release/02_i18n_report.md`:
- 영역별 검출/처리/미처리 카운트
- 추가된 키 전체 목록 (ko/en 짝)
- i18n 제외 사유(디버그/로그/식별자)
- 빌드/테스트 검증 결과

## 주의
- `format!()` 안의 인자 문자열은 i18n에서 `t!("key", arg=value)`로 처리
- 다국어로 길이가 달라지는 텍스트는 GUI 레이아웃이 깨지지 않게 검증
- 약어(IME, GTK, DBus 등)는 번역 안 하거나 단순 음역
