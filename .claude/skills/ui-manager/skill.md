---
name: ui-manager
description: UNIM CLI/GTK/Qt/GNOME UI·UX 작업 패턴. 위젯 추가/배치, 라이브 도움말(title/subtitle/tooltip), i18n 키 등록, 슬라이더 우선·다크 자동·즉시 저장 정책. "위젯 추가", "GUI", "툴팁", "라이브 도움말", "i18n 키", "트레이 메뉴", "CLI 출력" 트리거.
---

# UI Manager Operating Pattern

## 위젯 추가 절차
1. 엔진 매니저가 src/config.rs 필드 추가 후
2. GTK + Qt + GNOME prefs 3곳 위젯 신설
3. i18n 키 3종 등록 (title / subtitle / tooltip)
4. ko.yml + en.yml 텍스트 작성 (한국어 우선)
5. `make sandbox-gtk4`로 시각 확인
6. settings-sync-check 에이전트 호출 (PM 협업)

## i18n 키 명명
`<영역>_<섹션>_<역할>` snake_case
- 영역: `settings_` `tray_` `popup_` `error_` `common_` `cli_`
- 역할: `_label` `_subtitle` `_tooltip` `_desc` `_title` `_button` `_msg`

## 시각 정책 (Zero Tolerance)
- **수치 입력은 슬라이더** (메모리: feedback_slider_for_numeric)
  - SpinRow 금지, gtk::Scale + tick 마크
- **다크/라이트 자동**: ColorScheme::Default
- **변경 즉시 저장**: Apply 버튼 없음
- **약어 풀이**: IME/IM/DBus/XIM/TSF 첫 등장 풀이

## i18n 누락 검출
```bash
# 한글 하드코딩 (디버그 매크로 제외)
grep -rn '"[가-힣]' unim-{cli,gui-gtk,gui-common,gui-qt} --include='*.rs' \
  | grep -v 'unim_log\|tracing\|log::\|println\|eprintln\|debug\|info\|warn\|error\|trace'

# t!() 사용 키 vs locales 정의 키
grep -roh 't!("[^"]*"' unim-* --include='*.rs' | sort -u
```

## 라이브 도움말 적용 패턴 (GTK)
```rust
let row = adw::SwitchRow::builder()
    .title(t!("settings_X_Y_label"))
    .subtitle(t!("settings_X_Y_subtitle"))
    .build();
row.set_tooltip_text(Some(&t!("settings_X_Y_tooltip")));
```

## ko/en 키 동일성 검증
```bash
yq -r 'keys[]' unim-gui-gtk/locales/ko.yml | sort > /tmp/ko_keys
yq -r 'keys[]' unim-gui-gtk/locales/en.yml | sort > /tmp/en_keys
diff /tmp/ko_keys /tmp/en_keys
```
차이 있으면 양쪽 동기화.

## 검증
- 빌드: `PATH=$HOME/.cargo/bin:$PATH cargo build --workspace --release` warning 0
- 시각: `LANG=en_US.UTF-8`/`ko_KR.UTF-8` 양쪽
- 키 페어 동일성 검사 통과

## 출력 양식
```markdown
## UI Manager Report — {ID}

### 위젯
| 파일:line | 위젯 | i18n 키 |

### i18n
- 추가 키: N
- ko/en 동일성: PASS/FAIL

### 시각
- LANG=en: ...
- LANG=ko: ...
```
