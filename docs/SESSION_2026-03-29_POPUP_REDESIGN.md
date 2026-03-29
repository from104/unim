# 세션 기록: GNOME 팝업 재설계 (2026-03-29)

## 개요

GNOME extension의 한자/특수문자 팝업을 제로베이스에서 재설계하고, 발견된 버그들을 수정한 세션.

## 커밋 목록

| 커밋 | 설명 |
|------|------|
| `3c0ab06` | GNOME extension 팝업 통합 및 데몬 개선 (기존 작업 커밋) |
| `e157c4b` | 한자 팝업을 순수 UI 컴포넌트로 재설계 (엔진 위임 모드 전용) |
| `12e74cd` | 마우스 클릭 커밋/닫힘 수정 + 특수문자 팝업 재설계 |
| `8e97aa2` | Makefile dev-daemon의 pkill -f → -x 수정 |
| `71aa12b` | focus-out/reset 이벤트에서 팝업 닫힘 + preedit 커밋 |
| `209cbb1` | 팝업 열릴 때 preedit 유지 (preedit_changed 플래그 수정) |

## 주요 변경 내용

### 1. 팝업 아키텍처 정리 — 스탠드얼론 vs 엔진 위임

**문제**: 이전 팝업 통합 작업에서 두 가지 모드의 코드가 혼재
- 스탠드얼론 모드: 팝업이 직접 키 처리 (`handleKey()`, `_prevPage()`, `_nextPage()`)
- 엔진 위임 모드: 모든 키가 ProcessKeyEvent → 엔진 → DBus 시그널로 UI 갱신

**해결**: 엔진 위임 모드 전용으로 재설계, 스탠드얼론 코드 전량 제거

**영향 파일**: `hanja_popup.js`, `special_popup.js`, `key_handler.js`

### 2. POPUP_SPEC.md 제정

팝업 동작의 절대적 명세를 `unim-gnome-extension/POPUP_SPEC.md`에 기록:
- 팝업은 글자 위에 표시 (글자 가리지 않게)
- 팝업 중 preedit 유지
- 취소 조건: ESC, 포커스 아웃, 창 전환, 필드 변경, 리셋, 네비키 외 입력
- 팝업은 순수 UI — 키 처리는 엔진이 담당

### 3. 마우스 클릭 선택이 커밋 안 되는 버그

**근본 원인 2개**:
1. `service.rs`의 `SelectHanja`/`SelectSpecialChar`가 **HidePopup 시그널을 발행하지 않음** (마우스 클릭은 ProcessKeyEvent를 거치지 않으므로)
2. Chrome 위젯 클릭 시 포커스 경쟁 조건 — `_cleanupPopups()`가 `_onSelect` 콜백보다 먼저 실행

**수정**:
- `service.rs`: SelectHanja/SelectSpecialChar에서 HidePopup 시그널 명시적 발행
- `extension.js`: 팝업 visible + focusWindow null일 때 early return

### 4. 마우스 호버 vs 선택 분리

**문제**: 마우스 호버가 `.selected` 클래스를 변경해서 엔진 선택 위치를 덮어씀

**수정**:
- `.selected` = 엔진이 관리하는 선택 위치 (항상 표시)
- `.hovered` = 마우스 커서 위치 (표시만, 선택과 무관)
- `.selected.hovered` = 합성 스타일 (더 진한 색)

### 5. focus-out/reset 시 팝업 안 닫히는 버그

**문제**:
- `vfunc_focus_out`: 엔진이 팝업 활성 시 FocusOut을 무시 → 팝업 안 닫힘
- `vfunc_reset`: 팝업 UI 닫기 로직 없음

**수정**:
- `extension.js`: focusOutHandler에서 `_cleanupPopups()`를 `focusOut()` 전에 호출
- `unim_input_method.js`: `_resetHandler` + `setResetHandler()` 추가, `vfunc_reset`에서 호출
- `extension.js`: resetHandler 등록하여 팝업 정리

### 6. 팝업 열릴 때 preedit 깜빡임

**문제**: 한자키 누르면 ProcessKeyEvent가 `preedit=''`을 반환 → 입력 필드에서 글자 사라짐

**원인 추적**:
- `InputResult::hanja_candidates()`가 `preedit_changed: false` 반환
- engine_worker가 `None` 반환 → service.rs가 `unwrap_or_default()`로 `''` 변환
- key_handler.js가 `updatePreedit('')` 호출

**수정**: `preedit_changed: true`로 변경하여 실제 preedit 값을 반환

### 7. 기타

- `dbus_ime.js`: `cancelHanja`/`cancelSpecialChar`의 `call_sync` 비표준 인자 제거 (6개→5개)
- `key_handler.js`: 레거시 `_popupActive`, `_popupKeyHandler`, `setPopupKeyHandler()` 제거
- `Makefile`: `pkill -f` → `pkill -x` (make 프로세스 자기 자신을 죽이는 문제)
- `stylesheet.css`: `:hover` 의사 클래스를 `.hovered` 클래스 기반으로 전환, 특수문자 행 번호 `.active` 강조 추가

## 아직 남은 것

- 특수문자 팝업의 Nav 버튼(◀▶)/스크롤 → 엔진 동기화 DBus 메서드가 없어서 제거됨. 향후 `PopupNavigateRequest` 메서드 추가 시 복구 가능
- `config.rs`의 `PopupMode::Standalone`/`Embedded` 설정 — GNOME extension과는 무관한 별개 경로
