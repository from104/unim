# Linux 프런트엔드 한/영 토글키 config 미준수 (추적·미해결)

## 원칙
한/영 토글키 집합은 **전부 `config.engine.toggle_keys`**(기본 `["Korean","RightAlt"]`)에서만 와야 한다.
엔진이 이를 `InputEngine::toggle_keys: Vec<KeyCode>`로 파싱하고 `InputEngine::is_toggle_key()` /
`press_key`에서 사용한다. **프런트엔드는 자체 키 정책을 두면 안 되고**, 키를 엔진(데몬)에 넘긴 뒤
`consumed` 플래그만 따라야 한다.

## 상태
- **Windows (TSF·IMM32·엔진): 완료·검증 (2026-06-17).** `is_toggle_key` 기반, 하드코딩 토글 리터럴 없음.
  엔진 `press_key`가 토글 분기를 `is_modifier`/단축키 가드보다 앞에서 처리하도록 수정됨.
- **Linux 프런트엔드: 미해결 — 본 문서가 추적 항목.** Windows 머신에서 GTK/Qt/GNOME 빌드 검증이
  불가하여 보류(사용자 결정: "Windows만, Linux는 추적").

## 위반 위치 (감사 결과)
각 IM 모듈이 **수정자 키심 하드코딩 스킵 목록에 `Alt_R`/`Key_Alt`를 포함** → RightAlt가 엔진에
도달하기 전에 `return FALSE`로 버려짐 → config의 RightAlt 토글이 Linux에서 절대 작동 안 함.

| 프런트엔드 | 파일:라인 | 스니펫 |
|---|---|---|
| GTK3 | `unim-frontends/gtk3/src/immodule.c:598` (블록 596–606) | `event->keyval == GDK_KEY_Alt_R ... return FALSE;` |
| GTK4 | `unim-frontends/gtk4/src/immodule.c:678` (블록 676–686) | `GDK_KEY_Alt_R ... return FALSE;` |
| Qt5  | `unim-frontends/qt5/src/input_context.cpp:306` (블록 305–312) | `key == Qt::Key_Alt ... return false;` |
| Qt6  | `unim-frontends/qt6/src/input_context.cpp:290` (블록 289–296) | 동일 |
| GNOME | `unim-gnome-extension/key_handler.js:19` (`MODIFIER_KEYSYMS`에 `Clutter.KEY_Alt_R`) | 소비 경로 3곳: `_handleVfuncKey`(177–180), `_drainKeyQueue`(~282), `_handleKeyPress`(421–423) |

참고: GTK/Qt는 Ctrl/Alt/Super **조합** 스킵(`BYPASS_MODIFIER_MASK` / `modifiers.alt`)도 있으나,
RightAlt-down 이벤트 시점엔 보통 Alt 마스크가 아직 안 켜지므로 2차 사안. **1차 = 위 bare `Alt_R`
키심 스킵.**

## 클린(수정 불필요, 참고)
XIM(`unim-frontends/xim/`), Wayland(`unim-frontends/wayland/`), 데몬(`unim-daemon/`·
`unim-dbus/src/service.rs::process_key_event`)은 키를 그대로 `engine.press_key`로 전달 → 엔진이
config로 토글 판정. 위반 없음.

## 권장 수정 (엔진 = 유일 권위)
각 프런트엔드에서 수정자 키심 early-return을 제거(또는 완화)하여 **수정자 키도 엔진/데몬으로 전달**하고
`consumed` 결과만 따른다.
- 비토글 수정자(Shift/Ctrl/LeftAlt/Super/Caps)는 엔진이 `is_modifier() && !is_toggle_key →
  not_consumed`로 처리 → 프런트엔드는 passthrough(기존 동작과 동일).
- RightAlt 등 config 토글키는 엔진이 토글 후 `consumed=true` → 프런트엔드가 소비(스왈로우).
- 대안(성능 유지형): 데몬에서 `toggle_keys`를 받아 키심으로 매핑해, **토글 집합에 든 수정자만 스킵에서
  제외**. 프런트엔드별 키심 매핑(문자열 "RightAlt" ↔ `GDK_KEY_Alt_R`/`Qt::Key_Alt`/`Clutter.KEY_Alt_R`)이
  필요해 더 복잡. 권장은 위의 엔진-권위 방식.

## 검증 (Linux 환경에서 수행)
1. `make build`로 GTK3/4·Qt5/6 모듈 빌드 + GNOME 확장 로드.
2. toggle_keys 기본값에서 GTK 앱(gedit 등)·Qt 앱·GNOME 텍스트필드에서 **RightAlt로 한/영 전환** 확인.
3. config에서 RightAlt 제거 시 RightAlt가 일반 Alt로 통과(스왈로우 안 됨)되는지 — config 준수 회귀 확인.
4. Shift/Ctrl 단독·조합이 정상 passthrough 되는지 회귀 확인.

---
*출처: 토글키 config 준수 감사 (2026-06-17). 관련 엔진 수정: RightAlt 토글 데드코드 해결
(`src/input_engine/press_key.rs`, `engine.rs::is_toggle_key`, `tests_toggle.rs`).*
