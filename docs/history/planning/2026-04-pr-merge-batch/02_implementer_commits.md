# Hanja Bookmark UI — 프런트엔드 이식 결과

브랜치: `claude/hanja-bookmark-frontends` (develop에서 분기)
기반 HEAD: `423191f` Merge PR #3
기준선: `unim-gnome-extension/hanja_popup.js`

## 결과 요약

| 프런트엔드 | 커밋 | 변경 규모 | 빌드 결과 | 상태 |
|---|---|---|---|---|
| GTK Standalone (`unim-gui-gtk`) | `573f12d` | 6파일 / +145/-1 | `cargo check -p unim-gui-gtk` clean | DONE |
| gtk-common (GTK3/GTK4 IM module) | `45d7535` | 7파일 / +400/-11 | `cmake --build` clean (gtk3+gtk4) | DONE |
| qt-common (Qt5/Qt6 IM module) | `b15d7e1` | 6파일 / +244/-4 | `cmake --build` clean (qt5+qt6, moc 재생성) | DONE |
| XIM (`unim-xim`) | `6a32cd9` | 3파일 / +149 | `cargo check` clean, tests 통과 | DONE |
| Wayland | — | — | — | DEFERRED (파악 에이전트 결정) |

총합: **4 프런트엔드 / 4 커밋 / +938 lines**. Rust workspace 전체 lib tests 통과 (engine 377, dbus 11, 기타 0).

## 커밋별 상세

### 1. `573f12d` feat(gui-gtk): hanja bookmark UI (☆/★ · signal-driven refresh)

변경 파일 (6):
- `unim-dbus/src/client.rs` (+10) — `InputContextProxy`에 `GetHanjaBookmarkStates` / `ToggleHanjaBookmark` 메서드 + `HanjaBookmarkChanged` signal 추가
- `unim-gui-common/src/types.rs` (+5) — `GuiAction::HanjaBookmarkChanged { index, bookmarked }` variant
- `unim-gui-common/src/dbus_client.rs` (+14) — `handle_popup_signal`에 `HanjaBookmarkChanged` 분기
- `unim-gui-gtk/src/hanja_popup.rs` (+107) — `bookmarks: Vec<bool>` + ☆/★ GtkLabel, `set_bookmark()` API, show 시 비동기 `GetHanjaBookmarkStates` fetch → `HanjaBookmarkChanged` GuiAction 경유 UI 갱신, CSS(`.hanja-bookmark` / `.bookmarked`)
- `unim-gui-gtk/src/gtk_ui.rs` (+7) — `GuiAction::HanjaBookmarkChanged` → `HanjaPopup::set_bookmark` 호출
- `unim-gui-qt/src/bridge.rs` (+3) — 새 variant를 match에서 no-op으로 처리 (Qt GUI는 팝업을 표시하지 않음)

수동 검증 필요:
- `unim-gui-gtk` 실행 후 한자 팝업에서 Space 키 → 선택 행의 ☆ 이 ★ 로 바뀌는지
- 다른 프런트엔드(GNOME extension 등)에서 토글해도 이 팝업이 실시간 반영하는지

### 2. `45d7535` feat(gtk-common): hanja bookmark UI (☆/★ · Space toggle · live signal)

변경 파일 (7):
- `unim-capi/include/unim.h` (+1/-5 포맷) — `UNIM_POPUP_RESULT_TOGGLE_BOOKMARK 5` 상수 노출 (cbindgen 재생성은 다른 드리프트 유발 위험으로 수동 추가)
- `unim-frontends/gtk-common/include/unim_dbus_client.h` (+41) — `unim_dbus_get_hanja_bookmark_states` / `unim_dbus_toggle_hanja_bookmark` 프로토타입 + `UnimHanjaBookmarkChangedCallback` typedef + `unim_dbus_set_hanja_bookmark_callback` 등록 API
- `unim-frontends/gtk-common/include/unim_hanja_popup.h` (+41) — `UnimHanjaToggleBookmarkCallback` typedef + `set_toggle_bookmark_callback` / `set_bookmark_states` / `set_bookmark` API
- `unim-frontends/gtk-common/src/unim_dbus_client.c` (+144) — 위 프로토타입 구현 (`g_dbus_connection_call_sync`, `g_dbus_connection_signal_subscribe`). AutoTypeFix / CommitText 구독 패턴을 그대로 답습. `free()`에서 시그널 구독 해제.
- `unim-frontends/gtk-common/src/unim_hanja_popup.c` (+100) — `bookmarks` 배열 + 행에 ☆/★ GtkLabel 추가, `UNIM_POPUP_RESULT_TOGGLE_BOOKMARK` 분기 처리, `set_bookmark[_states]` / `set_toggle_bookmark_callback` 구현, bookmark 관련 CSS 추가, `show()` 시 배열 재할당 / `free()` 시 해제
- `unim-frontends/gtk3/src/immodule.c` (+38) — `on_hanja_toggle_bookmark` / `on_hanja_bookmark_changed` 콜백 정의, 초기화 시 wire-up, show 직후 `GetHanjaBookmarkStates` fetch → `set_bookmark_states`
- `unim-frontends/gtk4/src/immodule.c` (+36) — 동일 패턴 (GTK4)

수동 검증 필요:
- `gtk3/build`·`gtk4/build`의 `im-unim.so`·`libim-unim.so` 재설치 후 지원 앱(예: gedit, gnome-terminal)에서 한자 변환 → Space 토글 → 별 바뀌는지
- 다른 프런트엔드에서 토글 시 실시간 반영

### 3. `b15d7e1` feat(qt-common): hanja bookmark UI (☆/★ · Space toggle · live signal)

변경 파일 (6):
- `unim-frontends/qt-common/include/unim_dbus_client.hpp` (+37) — `getHanjaBookmarkStates` / `toggleHanjaBookmark` 메서드, `HanjaBookmarkChangedCallback` typedef + `setHanjaBookmarkChangedCallback`, `UnimHanjaBookmarkReceiver` QObject 서브클래스
- `unim-frontends/qt-common/include/unim_hanja_popup.hpp` (+18) — `setBookmarkStates` / `setBookmark` / `setToggleBookmarkCallback` API, `m_bookmarks` 필드
- `unim-frontends/qt-common/src/unim_dbus_client.cpp` (+92) — 위 메서드 구현 (`QDBusMessage` 동기 호출, `QDBusArgument::beginArray` + `QVariantList` 이중 파싱), `m_bus.connect()` 로 `HanjaBookmarkChanged` 구독
- `unim-frontends/qt-common/src/unim_hanja_popup.cpp` (+60/-2) — `UNIM_POPUP_RESULT_TOGGLE_BOOKMARK` 분기, 행 텍스트 suffix `"  ☆/★"` 인라인 렌더 (고정 `m_labels[9]` 슬롯 구조 유지), `bookmarked` QLabel property 설정
- `unim-frontends/qt5/src/input_context.cpp` (+20) — `ensurePopups`에서 toggle 콜백 wire-up, `setCommitTextCallback` 옆에 `setHanjaBookmarkChangedCallback` 추가, showPopup 직후 `getHanjaBookmarkStates` + `setBookmarkStates`
- `unim-frontends/qt6/src/input_context.cpp` (+21) — 동일 (Qt6)

수동 검증 필요:
- `qt5/build`·`qt6/build`의 `libunim.so` 재설치 후 Qt 앱(예: KeePassXC, Kate)에서 한자 변환 → Space 토글 → `1. 漢  한자  ★` 형식으로 별이 갱신되는지
- CSS `[bookmarked="true"]` 속성이 세팅되지만 현재 스타일시트가 이를 참조하지 않음 — 향후 행 배경 강조에 활용 가능 (지금은 텍스트 suffix 만으로 충분)

### 4. `6a32cd9` feat(xim): hanja bookmark UI (☆/★ · Xft fallback · live signal)

변경 파일 (3):
- `unim-frontends/xim/src/dbus_client.rs` (+89) — `PopupEvent::HanjaBookmarkChanged` variant, `DbusRequest::GetHanjaBookmarkStates` + `DbusResponse::HanjaBookmarkStates`, `ToggleHanjaBookmark` (XIM은 Space를 엔진에 직접 포워드하므로 미사용, `#[allow(dead_code)]` + 주석), `subscribe_popup_signals`에 `receive_hanja_bookmark_changed` 스트림 + select arm 추가, `run_dbus_client` match에 `GetHanjaBookmarkStates` dispatch
- `unim-frontends/xim/src/hanja_window.rs` (+50) — `redraw()`에서 각 후보 행 오른쪽 끝에 ☆/★ Xft 렌더 (bookmarked 여부에 따라 `text_color`/`page_color` 전환, `draw_string_with_fallback` 로 CJK 폰트 fallback), `HanjaWindow::set_bookmark` / `set_bookmark_flags` 메서드 (`PopupState`에 위임)
- `unim-frontends/xim/src/handler.rs` (+11) — `PopupEvent::HanjaBookmarkChanged` → `HanjaWindow::set_bookmark`, `HanjaWindow` 생성 직후 `GetHanjaBookmarkStates` 동기 fetch → `set_bookmark_flags`

수동 검증 필요:
- XIM 환경(`GTK_IM_MODULE=xim`) 앱(예: xterm, emacs --no-client-mode)에서 한자 변환 → Space 토글 → 별 갱신 (Xft에서 ★ U+2605 / ☆ U+2606 CJK 폰트 fallback이 실제로 렌더되는지 확인 필요, D2Coding 등 코딩 폰트는 ★ 글리프 미포함일 수 있음)
- 별 렌더가 깨지면 `hanja_window.rs`에서 `"★"/"☆"` 를 `"[B]"/"[ ]"` 같은 ASCII 대체로 교체 (파악 매트릭스에서 caveat로 지적됨)

## Deferred

### Wayland
**상태**: DEFERRED (파악 에이전트가 "순수 Wayland 팝업 미해결"로 판정, 이 PR 범위 밖)

**차단 요인**:
- `project_popup_architecture.md` 메모리: "순수 Wayland 미해결"
- `text-input-v3` popup surface 포커스·포지셔닝 이슈 상존
- 현재 `popup_renderer.rs`는 전체 재렌더만 지원 — 기능은 가능하지만 검증 비용이 크고 별도 팝업 아키텍처 이슈와 얽힘

**후속 권장**: 별도 이슈 + Wayland 팝업 아키텍처 리팩터링과 함께 재개. 엔진/DBus 서피스는 이미 준비되어 있어 기반은 무변.

## 빌드 환경 메모

- Rust: `export PATH="$HOME/.cargo/bin:$PATH"` (rustup toolchain, Cargo.lock v4 필요)
- C/C++: `cmake --build unim-frontends/<gtk3|gtk4|qt5|qt6>/build` 로 검증 (CMakeCache 보존)
- cbindgen: v0.26 설치했으나 실제 재생성은 수행하지 않음 (기존 `unim.h`가 hand-edited 혼재 상태라 diff drift 우려). 향후 cbindgen 표준화는 별도 작업 권장.

## 3지점 싱크 영향

`feedback_config_3way_sync.md` 확인: **영향 없음**. 이번 PR은 설정 항목을 추가/삭제하지 않으며, 엔진(`src/config.rs`) / GUI(`unim-gui-gtk`) / CLI(`unim-cli`) 싱크는 손대지 않았다.
