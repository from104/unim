# UNIM DBus Popup 책임 이관 계획

## 메타
- 목적: daemon(`unim-dbus`)에 자리 잡은 popup 관련 DBus 책임을 `unim-popup-service`로 이관하여 "한 책임, 한 프로세스" 원칙 완성.
- 범위: 한자 popup / 특수문자 popup / 이모지 popup 의 DBus interface(signal·method) + view-model 변환 + popup-owner routing.
- 비범위: 한글 조합 engine 본연(input_engine), candidate 생성 알고리즘, 사용자 사전 persistence — daemon에 그대로 둠.
- 대원칙: **모든 phase는 cargo build zero warning + cargo test all PASS 통과 후 commit.** 깨지면 즉시 중단·진단·롤백.

## 현 상태 진단 (popup 책임 분포 매트릭스)

### daemon(`unim-dbus/src/service.rs`) 의 popup 책임

| 위치 | 종류 | 이름 | 비고 |
|---|---|---|---|
| L346 | interface | `org.atit.unim.InputMethod` | path `/org/atit/unim/InputMethod` |
| L679 | method (IM-level) | `get_emoji_recent` | emoji recent MRU 조회 |
| L1203 | method (IM-level) | `set_emoji_category` | 이모지 카테고리 전환 |
| L1309 | method (IM-level) | `commit_emoji` | 마지막 active context 로 redirect |
| L1484 | helper | `build_emoji_show_payload` | `EmojiShowPayload` 빌더 |
| L1553 | helper | `redirect_commit_and_hide` | 글로벌 commit redirect |
| L1614 | helper | `emit_popup_render` | `PopupRender` signal emit |
| L1653 | interface | `org.atit.unim.InputContext` | path `/org/atit/unim/contexts/{id}` |
| L2161 | signal | `show_hanja_popup` | daemon → frontend |
| L2174 | signal | `show_special_popup` | daemon → frontend |
| L2199 | signal | `show_emoji_popup_v2` | daemon → frontend (실제 emit 은 IM-level helper 가 InputContext path 로 redirect) |
| L2215 | signal | `hide_popup` | daemon → frontend |
| L2219 | signal | `popup_navigate` | daemon → frontend (페이지·선택 변경) |
| L2246 | signal | `popup_render` | daemon → frontend (통합 view-model) |
| L2261 | signal | `hanja_bookmark_changed` | daemon → frontend |
| L2276 | signal | `hanja_candidates_reordered` | daemon → frontend |
| L2508 | method | `get_hanja_candidates` | popup 트리거 + 초기 view-model |
| L2564 | method | `select_hanja` | 한자 commit |
| L2604 | method | `get_hanja_bookmark_states` | 즐겨찾기 상태 일괄 조회 |
| L2635 | method | `toggle_hanja_bookmark` | 즐겨찾기 토글 |
| L2716 | method | `popup_change_page` | 페이지 이동 (◀/▶) |
| L2777 | method | `toggle_popup_expand` | 한자 compact↔expanded |
| L2825 | method | `cancel_hanja` | 한자 모드 취소 |
| L2866 | method | `get_special_char_candidates` | 특수문자 popup 트리거 |
| L2922 | method | `select_special_char` | 특수문자 commit |
| L2957 | method | `cancel_special_char` | 특수문자 모드 취소 |
| L2993 | method (IC-level) | `commit_emoji` | InputContext 경유 emoji commit |

### `unim-dbus/src/engine_worker.rs` 의 popup 헬퍼

| 위치 | 이름 | 책임 |
|---|---|---|
| L67 | `build_render_state` | engine.PopupViewModel → PopupRenderPayload 변환 |
| L149 | `resolve_popup_owner` | popup-owner context routing (호출 ctx≠popup ctx 보정) |

### `unim-gui-common/src/popup_dbus.rs` 의 client helper

| 함수 | RPC target (현) |
|---|---|
| `popup_change_page_via_dbus` | `org.atit.unim.InputContext` |
| `toggle_popup_expand_via_dbus` | `org.atit.unim.InputContext` |
| `toggle_hanja_bookmark_via_dbus` | `org.atit.unim.InputContext` |
| `select_hanja_via_dbus` | `org.atit.unim.InputContext` |
| `cancel_hanja_via_dbus` | `org.atit.unim.InputContext` |
| `select_special_via_dbus` | `org.atit.unim.InputContext` |
| `cancel_special_via_dbus` | `org.atit.unim.InputContext` |
| `commit_emoji_via_dbus` | `org.atit.unim.InputMethod` |
| `set_emoji_category_via_dbus` | `org.atit.unim.InputMethod` |
| `fetch_bookmark_states_async` | `org.atit.unim.InputContext` (hanja.rs:11 import) |

### 호출자
- popup-service `popup/{hanja,special,emoji}.rs` — 위 헬퍼들의 유일한 호출자 (GNOME ext popup JS 는 Phase 3 에서 제거 완료).
- popup-service `main.rs` 의 DBus watcher — daemon `InputContext` signal 구독.
- GNOME extension `dbus_ime.js` — `ShowEmojiPopupV2`·`HidePopup` 구독 (ZWSP preedit trick 용).

## 목표 아키텍처

### 새 DBus interface
- **Name**: `org.atit.unim.Popup`
- **Path**: `/org/atit/unim/popup` (singleton)
- **Owner**: `unim-popup-service` 프로세스
- **Bus name**: `org.atit.unim.PopupService` (well-known)

### Interface 구성
모든 popup signal·method 는 본 interface 로 이관. daemon InputContext / InputMethod 측 popup 표면은 점진적으로 deprecate 후 internal-only 로 격리.

```
[Signals — popup-service → frontend(=GNOME ext, popup-service GUI)]
  show_hanja_popup(target:s, candidates:a(ss), top_row:s, x:i, y:i, w:i, h:i)
  show_special_popup(target:s, characters:as, top_row:s, x:i, y:i, w:i, h:i)
  show_emoji_popup_v2(target_cat_id:s, items:as, top_row:s, recent:as,
                      categories:a(sssu), x:i, y:i, w:i, h:i, home_row:s)
  hide_popup()
  popup_navigate(page:u, total_pages:u, selected:u, rows:u, cols:u, sel_row:u, sel_col:u)
  popup_render(...PopupRenderPayload 평면 표현...)
  hanja_bookmark_changed(index:u, bookmarked:b)
  hanja_candidates_reordered(target:s, hanjas:as, meanings:as, bookmarks:ab,
                             new_cursor:u, page:u, sel_row:u, sel_col:u,
                             bookmarked:b, was_bookmarked:b)

[Methods — frontend → popup-service]
  GetHanjaCandidates() → (target:s, candidates:a(ss), top_row:s)
  SelectHanja(index:u) → (hanja:s)
  GetHanjaBookmarkStates() → (flags:ab)
  ToggleHanjaBookmark(index:u) → (idx:u, bookmarked:b)
  PopupChangePage(direction:i)
  TogglePopupExpand()
  CancelHanja() → (text:s)
  GetSpecialCharCandidates() → (target:s, characters:as, top_row:s)
  SelectSpecialChar(idx:u) → (ch:s)
  CancelSpecialChar() → (text:s)
  CommitEmoji(emoji:s)
  SetEmojiCategory(idx:u)
  GetEmojiRecent() → (recent:as)
```

### 데이터 흐름
```
사용자 키 입력
   ↓
daemon InputContext.process_key()
   ↓ (engine 내부 popup_state 변경)
daemon engine_worker → daemon internal "popup_event" 채널 emit
   ↓
popup-service 의 daemon InputContext signal 구독 (current 그대로 유지)
   ↓ popup-service 내부 변환 (build_render_state 호출)
popup-service Popup interface signal 발행 (new — Phase 2)
   ↓
frontend(popup-service GUI, GNOME ext) subscribe → render
   ↓ 사용자 popup 클릭
popup-service Popup interface method 호출 (new — Phase 3)
   ↓
popup-service 내부에서 daemon InputContext popup method 위임 호출 (internal — Phase 3)
   ↓
daemon engine state 변경 → popup_event → 위와 같이 popup-service signal 발행
```

### 분리 원칙
- daemon: engine state 마스터 (process_key 처리, popup_state 갱신, candidate 생성, bookmark 사전). **외부 facing popup interface 없음.**
- popup-service: popup interface 마스터 (signal 발행, method 처리). daemon engine 에 대한 internal RPC client.
- frontend(GNOME ext / popup-service GUI 자체): popup-service Popup interface 만 사용.

### 단계 분리의 의의
- daemon 의 InputContext interface 가 한글 조합 본연(process_key·focus·preedit·commit)만 노출.
- popup 책임이 popup-service 한 곳으로 집중 → 한 책임, 한 프로세스 원칙 완성.
- view-model 변환 로직(`build_render_state`)을 popup-service 또는 `unim-popup-types` 로 이동 가능 (Phase 5에서 결정).

## Phase 분해

각 phase 의 종료 조건:
- (B) cargo build --workspace zero warning
- (T) cargo test --workspace all PASS / FAILED 0
- (L) GNOME ext 정적 검사 syntax OK (해당 phase 가 ext 건드릴 때만)
- (V) phase 별 추가 검증

---

### Phase 0 — popup UI 미표시 원인 진단 (선행)
**목적**: 책임 이관과 무관한 환경 이슈를 먼저 차단. 사용자 환경 GNOME+X11.

**점검 항목**:
1. `pgrep -lf unim-popup-service` — 프로세스 실행 여부
2. `ls -la /etc/xdg/autostart/unim-popup-service.desktop /usr/local/share/applications/ /usr/share/xdg/autostart/` — 설치된 desktop 파일 + `NotShowIn=GNOME` 잔존 여부
3. `which unim-popup-service` — 바이너리 경로
4. `unim-popup-service 2>&1 | head -30` — 직접 실행 시 daemon 구독 + GTK4 init 결과
5. `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod | grep -E "(Show|Popup)" | head -10` — daemon signal 표면 확인
6. `~/.unim-errors.log` — popup-service 에러 로그

**의심 가설**:
- (가) `sudo make install` 이전 빌드 + 옛 desktop 파일 (`NotShowIn=GNOME`) 잔존 → autostart 차단
- (나) 새 빌드는 했지만 `make install` 안 함 → 옛 popup-service 또는 미설치
- (다) X11 backend 가 GNOME Xorg 에서 override-redirect 실패 / GTK4 init 실패
- (라) daemon signal 발행은 되나 popup-service watcher 미구독 (single_instance flock 충돌 등)

**조치**:
- (가)/(나) → `sudo make uninstall && make build && sudo make install PREFIX=/usr` 재설치. 로그아웃·로그인.
- (다)/(라) → 본 plan 의 Phase 1 부터 이관 진행하면서 popup-service 내부 로그 정리. backend 분기는 본 plan 의 부수 효과로 정비.

**Phase 0 결과를 받아 본 plan 의 Phase 1 진입.**

---

### Phase 1 — popup-service 신규 DBus interface 정의 + skeleton
**파일**:
- `unim-popup-service/src/dbus_server.rs` (신규)
- `unim-popup-service/src/main.rs` (server 부착)
- `unim-popup-service/Cargo.toml` (zbus 노출 export 옵션 활성)

**작업**:
1. `dbus_server.rs` 에 struct `PopupServer` + `#[interface(name = "org.atit.unim.Popup")]` impl 작성.
2. signals 8개·methods 13개 모두 stub 으로 등록 (panic!/log only). 시그너처는 위 목표 아키텍처 그대로.
3. `main.rs` 에 `ConnectionBuilder::session().name("org.atit.unim.PopupService").serve_at("/org/atit/unim/popup", PopupServer::new(...))` 추가.
4. `PopupServer::new()` 가 daemon `InputMethodProxy` 핸들을 받아 internal forwarding 시 활용.

**검증**: (B)(T). 추가로 `busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup` 출력에 8 signal + 13 method 노출 확인.

**롤백**: 본 phase 의 신규 코드는 daemon · GNOME ext 에 영향 없음. 단독 revert 가능.

---

### Phase 2 — daemon InputContext signal → popup-service signal re-emit
**파일**:
- `unim-popup-service/src/dbus_server.rs` (signal forward 로직)
- `unim-popup-service/src/main.rs` (watcher 에서 PopupServer 핸들 전달)
- `unim-popup-service/src/popup/{hanja,special,emoji}.rs` (기존 daemon signal 수신 코드는 그대로 — Phase 4 에서 전환)

**작업**:
1. popup-service 의 daemon InputContext signal 구독 watcher 가 신호를 받으면, **(a)** 자기 popup window 처리 + **(b)** PopupServer 통해 자기 Popup interface 의 signal 재발행.
2. emoji 의 `ShowEmojiPopupV2` 는 InputMethod-level path 라 별도 구독 + 동일 패턴 re-emit.
3. re-emit signal payload 는 daemon 측과 1:1 동일 (`PopupRenderPayload` 직렬화 형태 보존).

**검증**: (B)(T). `dbus-monitor --session 'sender=org.atit.unim.PopupService'` 로 popup-service 가 자기 signal 발행하는지 관측.

**롤백**: signal re-emit 추가만이므로 frontend 영향 없음. revert 안전.

---

### Phase 3 — popup-service Popup interface method handler 구현
**파일**:
- `unim-popup-service/src/dbus_server.rs`
- `unim-popup-service/src/main.rs` (daemon InputContextProxy / InputMethodProxy 핸들 전달)

**작업**:
1. PopupServer 의 13 method 각각이 daemon InputContext / InputMethod 의 동일 method 를 forward 호출하도록 구현.
2. forward 시 popup-owner routing 은 daemon 측 `resolve_popup_owner` 가 처리하므로 popup-service 는 단순 forwarding.
3. method response 도 그대로 반환. timeout / error 도 그대로 propagate.

**검증**: (B)(T). `busctl --user call org.atit.unim.PopupService /org/atit/unim/popup org.atit.unim.Popup CancelHanja` 같은 직접 호출로 daemon 까지 도달하는지 확인.

**롤백**: method forward 자체는 frontend 영향 없음. revert 안전.

---

### Phase 4 — popup-service 내부 RPC client 헬퍼 target 전환
**파일**:
- `unim-gui-common/src/popup_dbus.rs` — target service name 을 `org.atit.unim.PopupService`, path 를 `/org/atit/unim/popup`, interface 를 `org.atit.unim.Popup` 으로 변경.
- popup-service 의 `popup/{hanja,special,emoji}.rs` — 헬퍼 호출 시그너처가 동일하므로 사용처 코드 무변경.

**중요 결정**:
- popup-service 가 자기 자신의 DBus 를 호출 (self-call). zbus 가 이를 정상 처리. internal function call 로 단축하지 않음 (interface 일관성 + 향후 다른 frontend 추가 용이).

**검증**: (B)(T). popup-service 자기 popup 클릭 → 자기 method → daemon forward 흐름 정상.

**롤백**: target name 만 변경. 한 줄 단위 git diff 로 즉시 revert.

---

### Phase 5 — GNOME extension signal subscription target 전환
**파일**:
- `unim-gnome-extension/dbus_ime.js` — `ShowEmojiPopupV2`·`HidePopup` signal subscribe 대상을 daemon InputContext 경로 → popup-service `/org/atit/unim/popup` 경로 + service name `org.atit.unim.PopupService` 로 변경.

**작업**:
1. `dbus_ime.js` 내 popup-service Proxy 추가 (`org.atit.unim.PopupService` + `/org/atit/unim/popup`).
2. `ShowEmojiPopupV2` 시그널 구독을 popup-service 로 이동.
3. `HidePopup` 시그널 구독도 동일.
4. 다른 popup signal(ShowHanjaPopup·ShowSpecialPopup 등) 은 GNOME ext 가 이미 구독 안 함 (Phase 3 에서 제거). 영향 없음.
5. `CommitText` signal 은 daemon InputContext 유지 — IM commit 본연이라 popup 책임 아님.

**검증**: (B)(T)(L). GNOME Shell 재시작 후 emoji popup 트리거 시 ZWSP preedit 가 정상 설정되는지 (log check).

**롤백**: dbus_ime.js 한 영역만 변경. revert 즉시 가능.

---

### Phase 6 — daemon InputContext popup signal·method 격리 (deprecate)
**파일**:
- `unim-dbus/src/service.rs`

**작업**:
1. popup-related signal trait 메서드들 (show_hanja_popup, show_special_popup, show_emoji_popup_v2, hide_popup, popup_navigate, popup_render, hanja_bookmark_changed, hanja_candidates_reordered) 은 popup-service 가 내부 forwarding 에 사용하므로 **시그너처 유지**. 다만 doc-comment 에 "popup-service internal only — 외부 frontend 는 `org.atit.unim.Popup` interface 사용" 명시.
2. popup-related method (get_hanja_candidates 등 13개) 도 동일. popup-service 가 forward 시 호출.
3. **외부 frontend(GNOME ext, popup-service 자기 client)의 호출은 모두 Phase 4·5 에서 popup-service interface 로 전환됨.** daemon 측 popup 표면은 internal-only 가 됨.

**대안**: 향후 cleanup 시점에 daemon InputContext 의 popup method 를 별도 internal interface (예: `org.atit.unim.PopupInternal`) 로 옮겨 외부 노출에서 완전 격리. 본 phase 에선 doc-only 격리.

**검증**: (B)(T). daemon DBus introspection 에서 popup method 가 여전히 보이지만, 호출자는 popup-service 만이라는 사실을 grep 으로 확인.

**롤백**: 본 phase 는 doc 변경만. 영향 없음.

---

### Phase 7 — 통합 검증 + 실기 테스트
**작업**:
1. `cargo build --workspace --release` zero warning.
2. `cargo test --workspace` all PASS.
3. `make build && sudo make install PREFIX=/usr` 재설치.
4. **GNOME Shell 로그아웃·로그인 (autostart 재로드)**.
5. 실기 시나리오:
   - 한글 입력 후 한자 키 → 한자 popup 표시 (popup-service GTK4 window).
   - 한자 popup 9x9 expand 토글 (Period 키 / ⊞⊟ 클릭).
   - 한자 즐겨찾기 토글 (우클릭 / Space).
   - 한자 commit (좌클릭 / Enter).
   - 특수문자 popup 동일 시나리오.
   - 이모지 popup (Super+. 트리거) — 카테고리 탭 클릭, 페이지 이동, commit.
6. `busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup` 로 신규 interface 노출 확인.
7. `journalctl --user -u unim-daemon -f` + `~/.unim-errors.log` 로 에러 없음 확인.

**롤백 절차** (각 phase 별):
- Phase 1: `git revert <hash>` — 신규 파일 단독 revert.
- Phase 2: signal re-emit 코드만 revert.
- Phase 3: method handler 코드만 revert.
- Phase 4: `popup_dbus.rs` 의 target 4개 상수만 원복.
- Phase 5: `dbus_ime.js` 의 Proxy/구독 코드만 원복.
- Phase 6: doc-comment 만 원복.

## 영향 매트릭스

| 파일 | Phase | 변경 종류 | 영향 |
|---|---|---|---|
| `unim-popup-service/src/dbus_server.rs` | 1·2·3 | 신규 | popup interface server impl |
| `unim-popup-service/src/main.rs` | 1·2 | 추가 | server 부착 + watcher 연계 |
| `unim-popup-service/Cargo.toml` | 1 | 추가 | zbus interface 매크로 옵션 (이미 있으면 무변경) |
| `unim-gui-common/src/popup_dbus.rs` | 4 | 변경 | RPC target 상수 4종 변경 |
| `unim-gnome-extension/dbus_ime.js` | 5 | 변경 | popup-service Proxy 추가 + signal target 전환 |
| `unim-dbus/src/service.rs` | 6 | doc-only | popup method/signal 에 internal-only 주석 |
| `docs/architecture/dbus-popup-migration-plan.md` | — | 신규 | 본 문서 |

## 위험 + 완화책

| 위험 | 영향 | 완화 |
|---|---|---|
| popup-service self-call 시 zbus deadlock | popup 작동 불가 | self-call 은 zbus 가 정상 처리. 단 popup-service 가 자기 method 안에서 daemon proxy 호출 시 await 점 명시. 테스트로 검증 (Phase 4). |
| daemon signal re-emit 중복 비용 | 약간의 IPC 오버헤드 | 한 signal 당 < 1KB · < 1ms 라 무시 가능. |
| GNOME ext 의 `CommitText` 구독은 그대로 daemon InputContext 사용 — 일관성 깨짐 | 미감 (실제 영향 없음) | popup 외 메서드는 본 plan 범위 밖. 후속 별도 plan. |
| popup-service 가 daemon 보다 늦게 뜨거나 죽으면 popup 미작동 | 큰 영향 | systemd-style restart on failure 검토 (별도 작업). 본 plan 범위 밖. |
| Phase 4 후 popup-service 의 popup 클릭이 self-call 로 변환되면서 ordering issue | 잠재적 | Phase 4 종료 후 통합 테스트로 확인. |
| daemon InputContext popup method 가 internal-only 가 되어도 introspection 에서 노출됨 → 외부 클라이언트 혼란 | 작음 | doc-comment 로 명시. 향후 `internal_` prefix 또는 별도 interface 로 격리. |

## 검증 체크리스트 (실기)

- [ ] GNOME+X11 환경에서 `unim-popup-service` 프로세스가 autostart 됨 (`pgrep -lf unim-popup-service`).
- [ ] `busctl --user list | grep -i popup` 에 `org.atit.unim.PopupService` 노출.
- [ ] `busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup` 에 13 method + 8 signal.
- [ ] 한자 popup compact 모드 표시·dismiss·commit·bookmark 토글 모두 작동.
- [ ] 한자 popup 9x9 expanded 모드 토글 (Period 키 + ⊞⊟ 클릭) 작동.
- [ ] 특수문자 popup compact·9x9 표시·dismiss·commit 작동.
- [ ] 이모지 popup 카테고리 탭 클릭·페이지 이동·commit·MRU 작동.
- [ ] `~/.unim-errors.log` 에 본 plan 적용 이후 신규 에러 없음.
- [ ] `cargo test --workspace` 모든 suite PASS.
- [ ] `make build` zero warning.

## 부록 — UI 미표시 진단 명령 모음 (Phase 0)

```fish
# 1. popup-service 프로세스
pgrep -lf unim-popup-service
ps -ef | grep -E "unim-(daemon|popup-service|indicator)"

# 2. desktop 파일 + NotShowIn 잔존 확인
ls -la /etc/xdg/autostart/unim-popup-service.desktop \
       /usr/local/etc/xdg/autostart/unim-popup-service.desktop \
       /usr/share/applications/unim-popup-service.desktop 2>&1
grep -H NotShowIn /etc/xdg/autostart/unim-popup-service.desktop 2>&1
grep -H NotShowIn /usr/local/etc/xdg/autostart/unim-popup-service.desktop 2>&1

# 3. 바이너리 경로 + 빌드 신구 일치
which unim-popup-service unim-indicator unim-settings unim-daemon
stat -c '%n: %y' (which unim-popup-service) target/release/unim-popup-service 2>&1

# 4. DBus 표면 (daemon 이 popup signal 발행 가능?)
busctl --user list | grep -i unim
busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod 2>&1 | head -40

# 5. popup-service 직접 실행 (autostart 우회)
RUST_LOG=debug unim-popup-service 2>&1 | head -30

# 6. 에러 로그
tail -100 ~/.unim-errors.log 2>&1
journalctl --user -u unim-daemon -n 50 --no-pager 2>&1
```

## 작업 메모

- 본 plan 은 한 번에 끝나지 않는다. Phase 별 commit 분리.
- 각 phase 종료 시 todo 리스트 갱신 + 사용자에게 보고.
- 깨지면 즉시 중단·진단·복구. 임의 우회 (예: 임시 stub 또는 silent ignore) 금지.
- "지독한 완벽주의자" 모드 — 빠짐없이, 임의 단축 없이, 사용자가 명시한 의도 그대로.
