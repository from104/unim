# Hanja Bookmark UI — Reviewer Report

브랜치: `claude/hanja-bookmark-frontends` (develop `423191f` 기준, 4 커밋)
검증 일시: 2026-04-25
기준선: `unim-gnome-extension/hanja_popup.js`

## 1. 요약 표

| 프런트엔드 | 커밋 | 판정 | 사유 (한 줄) |
|---|---|---|---|
| GTK Standalone (`unim-gui-gtk`) | `573f12d` | **PASS** | cargo check/test clean, ☆/★ · `GetHanjaBookmarkStates` 초기 fetch · `HanjaBookmarkChanged` 분기 모두 확인 |
| gtk-common (GTK3/GTK4 IM) | `45d7535` | **PASS** | cmake gtk3+gtk4 clean, TOGGLE_BOOKMARK 분기 · signal subscribe/unsubscribe · ☆/★ 렌더 확인 |
| qt-common (Qt5/Qt6 IM) | `b15d7e1` | **PASS** | cmake qt5+qt6 clean, `UnimHanjaBookmarkReceiver` QObject · Space 콜백 · suffix 렌더 확인 |
| XIM (`unim-xim`) | `6a32cd9` | **PASS** | cargo clean, `PopupEvent::HanjaBookmarkChanged` · Xft ★/☆ 렌더 · select arm 확인 / 폰트 fallback은 PENDING |

**빌드/테스트 요약**
- `cargo check --workspace --lib --tests --bins`: **0 errors, 0 warnings**
- `cargo test --workspace --lib`: **392/392 통과** (engine 377 + capi 4 + dbus 11, 기타 0)
- `cmake --build` gtk3/gtk4/qt5/qt6: **all clean** (`[100%] Built target`)

## 2. 커밋별 체크리스트 결과

### 573f12d — GTK Standalone
- A. 빌드/테스트: OK (cargo clean, workspace tests pass)
- B. 기능 정합성:
  - ☆/★ 라벨: `unim-gui-gtk/src/hanja_popup.rs:187`
  - `ToggleHanjaBookmark` 호출: bridge를 통해 `HanjaBookmarkChanged` GuiAction 경유 (`dbus_client.rs:361`)
  - show 시 `GetHanjaBookmarkStates`: `hanja_popup.rs:135, 294`
  - signal 수신: `dbus_client.rs:361–369` → `gtk_ui.rs:108` 분기
- C. 규칙: 커밋 메시지 `feat(gui-gtk): ...` OK. Config 3지점 영향 없음.
- D. 경계: signal 수신 시 popup이 없을 수 있는데 `set_bookmark`가 길이 체크 후 resize — crash 없음.

### 45d7535 — gtk-common
- A. cmake gtk3/gtk4 모두 clean (`[100%] Built target im-unim`).
- B. 기능:
  - `UNIM_POPUP_RESULT_TOGGLE_BOOKMARK=5`: `unim-capi/include/unim.h:464`
  - TOGGLE 분기 → `unim_dbus_toggle_hanja_bookmark()`: `unim_hanja_popup.c:754`
  - ☆/★ GtkLabel: `unim_hanja_popup.c:174`
  - `GetHanjaBookmarkStates` 초기 fetch: gtk3/4 `immodule.c`에서 show 직후 호출
  - signal subscribe + unsubscribe on free: `unim_dbus_client.c:1164, 190–192`
- C. 커밋 메시지 `feat(gtk-common): ...` OK.
- D. `free()`에서 `g_dbus_connection_signal_unsubscribe` 호출 — 해제 시 crash 방지.

### b15d7e1 — qt-common
- A. cmake qt5/qt6 모두 clean (moc 재생성 포함).
- B. 기능:
  - TOGGLE 분기: `unim_hanja_popup.cpp:234`
  - ☆/★ suffix 렌더 + `bookmarked` property: `unim_hanja_popup.cpp:298, 304`
  - signal: `UnimHanjaBookmarkReceiver` QObject + `m_bus.connect("HanjaBookmarkChanged", SLOT(...))`: `unim_dbus_client.cpp:624`
  - show 시 `getHanjaBookmarkStates`: `qt5/input_context.cpp:451`, `qt6/input_context.cpp:453`
  - wire-up: `ensurePopups`에서 toggle 콜백 + `setHanjaBookmarkChangedCallback` (qt5:122, qt6:123)
- C. 커밋 메시지 OK.
- D. `m_hanjaPopup` null 체크 후 setBookmark — crash 방지.

### 6a32cd9 — XIM
- A. cargo clean, lib tests 통과.
- B. 기능:
  - `PopupEvent::HanjaBookmarkChanged`: `dbus_client.rs:53`
  - `GetHanjaBookmarkStates` + `HanjaBookmarkStates` request/response: `dbus_client.rs:130, 183, 591–616`
  - signal stream select arm: `dbus_client.rs:715, 785–790`
  - ☆/★ Xft 렌더 + fallback 색상: `hanja_window.rs:710, 728`
  - 초기 fetch: `handler.rs:1242–1251` (show 직후 동기)
  - signal → set_bookmark: `handler.rs:634`
- C. 커밋 메시지 `feat(xim): ...` OK.
- D. `ToggleHanjaBookmark` 변형은 XIM이 Space를 엔진에 포워드하므로 `#[allow(dead_code)]` — 의도적.

## 3. FAIL 상세

**없음.** 모든 커밋 PASS.

## 4. PENDING — 수동 확인 체크리스트

자동 테스트로 검증 불가한 시각/실기 시나리오 (리더 또는 사용자가 실행):

1. **GTK Standalone 실기**
   - `unim-gui-gtk` 실행 → 한자 팝업 → Space 키 → 선택 행 ☆ → ★ 전환
   - GNOME extension에서 토글한 즐겨찾기가 GTK standalone 팝업에 즉시 반영

2. **gtk3/gtk4 IM module**
   - `gtk3/build/im-unim.so`, `gtk4/build/libim-unim.so` 재설치
   - gedit, gnome-terminal 등에서 한자 변환 → Space 토글 → 별 변경
   - 타 프런트엔드 토글 시 실시간 반영

3. **qt5/qt6 IM module**
   - `qt5/build/libunim.so`, `qt6/build/libunim.so` 재설치
   - KeePassXC, Kate에서 한자 변환 → Space 토글 → `1. 漢  한자  ★` suffix 갱신
   - `bookmarked` property는 QSS에서 미활용 (향후 행 배경 강조 여지 — 이번 PR 범위 밖)

4. **XIM ★/☆ 폰트 fallback (가장 위험)**
   - `GTK_IM_MODULE=xim` 환경 (xterm, emacs --no-client-mode) 에서 한자 변환
   - Xft의 `draw_string_with_fallback`이 U+2605/U+2606 글리프를 실제로 CJK 폰트에서 가져오는지 시각 확인
   - **깨질 시 대체안**: `hanja_window.rs` 내 `"★"/"☆"` → `"[B]"/"[ ]"` ASCII 교체 (구현자 caveat)

## 5. 엔진 측 이슈 / 기타

- **영향 없음.** 이번 PR은 프런트엔드만 수정, `unim-dbus` client.rs에 메서드/시그널 프록시만 추가 (엔진 로직 변경 없음).
- **Wayland DEFERRED**: `project_popup_architecture.md`와 부합, 별도 이슈로 정식 분리하는 것이 바람직.

## 6. 전체 판정

**전체 PASS (4/4 커밋)**. 자동화 가능한 검증(빌드·테스트·grep 기반 UI 시맨틱) 모두 통과.
PENDING 4개 항목은 실기 smoke test가 필요하나 FAIL 요인은 아니다.
리더 판단으로 실기 시나리오를 수행 후 push + PR open 권장.
