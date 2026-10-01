# UNIM 상황 알림(토스트) 설계서 (NOTIFY_SPEC) — v2.1-draft

> 입력기 내부에서 일어난 "사용자가 알아야 할 상황"(자동교정 발동·억제·비밀번호 필드 진입 등)을
> **프런트엔드·플랫폼 독립적인 알림 이벤트**로 정의하고, 플랫폼별 표시 경로가
> 포커스를 뺏지 않는 자동 소멸 토스트로 보여 주는 기능의 규격이다.
> 한자/특수문자 팝업 규격 [`POPUP_SPEC.md`](POPUP_SPEC.md) 는 **한 글자도 바꾸지 않는다** —
> 토스트는 팝업과 별개 표면이며, Windows 에서만 팝업 렌더러 프로세스·IPC 를 **재사용**한다.
> 설정은 [`GEMINI.md` Settings Synchronization](../architecture/GEMINI.md) 의 5지점 규칙을 따른다.

- 상태: **v2.1-draft — 결정 D1~D6·Q1~Q10·E1~E5 확정 반영, 미결 없음**. 구현 전, develop `b94da2a` 기준 사실 확인.
- 관련 문서: `POPUP_SPEC.md`, `unim-dbus/SPEC.md` §5.2(시그널)·§5.4(키 표)·§5.5(RegisterFrontend)·§5.6(TriggerAction)·§13.3(목적 감지 불가 환경), `docs/dev/windows/popup-renderer-design.md`(unim-popup-win), 사용자 매뉴얼 §4.1 접근성.
- 작성: 2026-10-01 v0 planner(Fable) → 같은 날 v1 개정(Opus, 검증 지적 17건·사용자 결정 반영) → v2 보강(재검증 지적 10건·결정 E1~E5 반영, §8.2) → v2.1 손질(opus 재검증 PASS 의 LOW 7건, §8.3). 코드 변경 없음.

---

## 0. 결정 항목

### 0.1 확정 (2026-10-01 기현님)

| # | 항목 | 확정 내용 |
|---|------|-----------|
| D1 | 교정 전후 텍스트 노출 | **기본 가림**("자동 교정됨"만). `notify.show_text=true` 일 때만 `{before}→{after}`·단어 표시 — 목적 감지 못 한 비번칸(§2.3) 대책 |
| D2 | 코어 API 변경 범위 | **한/영 전환 차단 여부 + ATF 억제 사유 반환까지만**(§3.1). 차단 여부는 E4 에 따라 `InputResult` 가 아닌 엔진 getter. 그 외 코어 공개 API 무변경(신규 순수 모듈 `src/notify.rs` 는 E3) |
| D3 | 확장 없는 GNOME·트레이 없는 환경 | 데몬이 **직접** `org.freedesktop.Notifications` 호출. indicator·popup-service 등 **다른 프로세스에 의존하지 않음**(§3.3) |
| D4 | GNOME 알림 목록 잔류 | **남기지 않음** — 모든 경로 transient |
| D5 | 비밀번호 칸 진입 알림 빈도 | **같은 칸이면 1회만**(§2.2 규칙 4). "칸" 의 구현 단위·10분 재알림은 E5 |
| D6 | Windows 다중 모니터 위치 | **현재 포커스 창이 있는 모니터**의 작업영역 모서리(§3.4) |
| Q1 | 문구 생성 주체 | 데몬/TSF 가 코어 `render()` 로 ko·en 완성 문구를 만들고 **`kind` 를 동봉**. 언어 출처는 §3.1 `Lang` 결정 규칙 |
| Q2 | GNOME 확장 표시 | P2 = 확장 전용 `MessageTray.Source` 1개 재사용 + **transient** 알림, P3 = 패널 인디케이터 아래 St 토스트 |
| Q3 | X11·KDE·기타·XIM | 데몬 직접 fdo(D3). 트레이 프로세스와 분리. popup-service 자체 토스트는 v1 제외 |
| Q4 | Windows 백엔드 | `unim-popup-win.exe` 토스트 창(기존 파이프 재사용) |
| Q5 | 이벤트별 기본값 | §2.1 "기본" 열. 비번 사유 `atf_suppressed` 는 **삭제**(`password_enter` 와 중복) |
| Q6 | 중복 억제 | §2.2. `password_enter` 는 칸당 1회(D5) |
| Q7 | 설정 위치 | 독립 섹션 `engine.notify`(선례 `engine.auto_typefix`). 이벤트 on/off 는 **쉼표 리스트 키 하나**(`word-mode-apps` 선례). 근거 정정은 §9 |
| Q8 | 접근성 | GNOME St 토스트: `accessible_role = Atk.Role.NOTIFICATION` + `accessible_name`. fdo 경로는 알림 서버의 스크린리더 통보에 위임. Windows: `UiaRaiseNotificationEvent` |
| Q9 | 비프와의 관계 | 별도 유지(설정도 분리) |
| Q10 | 확장 로컬 알림(`extension.js:731~732, 785~791, 796~800`) | **확장 로컬 발행 유지**, `notify.*` 설정으로 게이트(데몬 보고 메서드 신설 안 함). gschema `show-notification` 처리는 E1 |
| E1 | 확장 gschema `show-notification`(구 Q11) | **당분간 유지**, `show-notification && notify.enabled && events∋feature_result` **AND 게이트**. 대상은 이 키가 지금 게이트하는 확장 로컬 3곳(`extension.js:731, 785, 796`)뿐 — 데몬발 `Notify` 표시는 `notify.*` 만 따른다. 키 제거·`prefs.js:91` 스위치 삭제는 **이번 범위 아님** |
| E2 | `unim-dbus/SPEC.md` 갱신(구 Q12) | **승인**: §5.2 `Notify` 시그널, §5.4 키 표 6행, §5.5 `gnome-shell-notify` 이름 + **등록자 추적**(unique name 기록·`NameOwnerChanged` 정리·재연결 시 재등록, §3.2), §5.6 `notify_test` 액션. P0 에서 갱신 |
| E3 | 신규 순수 모듈 `src/notify.rs` | 플랫폼 의존 0 이면 코어 변경 허용 범위(D2) 안으로 **인정** |
| E4 | C ABI | `InputResult` 구조체에 **필드를 추가하지 않는다**. 차단 여부는 엔진 getter `InputEngine::last_toggle_blocked()` + C 함수 `unim_engine_last_toggle_blocked()` 로 노출 → `UnimInputResult` 레이아웃·soname(`libunim_capi.so.0`)·패키징 무변경. drift guard 에 새 함수 반영, 레이아웃 고정 단언(§3.1a) |
| E5 | 비번 진입 알림 "칸" 단위(재검증 지적 2) | **같은 `context_id` 에서 1회, 단 마지막 알림 후 10분이 지나면 다시 알림**(브라우저 한 창 여러 사이트 대책). 한계는 §2.2 규칙 4 |

### 0.2 미결

없음. 구현 중 실기로 확정할 사항(GNOME `urgency` 배너 동작, `desktop-entry`·아이콘 이름, `Atk.Role.NOTIFICATION` 동작 버전)은 §6 각 Phase 검증 열에서 다룬다.

---

## 1. 목표 / 비목표

### 1.1 목표
- 사용자가 **눈치채지 못하고 지나가는 입력기 상황**을 1~2초짜리 토스트로 알린다.
- 포커스 불변·자동 소멸·조작 불필요. 키 응답 경로 지연 0(§7, 시그널·fdo 호출은 알림 전담 태스크 1개가 채널로 받아 처리, §3.2).
- 코어는 이벤트 타입·게이트·문구를 정의하고, 이벤트 **생성**은 데몬/TSF 가 코어의 두 판정 결과(§3.1 D2)를 받아 한다. 표시 경로 부재 시 **조용히 로그(kind 만)** 로 떨어진다.
- 입력 텍스트는 **기본적으로 화면에 드러내지 않는다**(D1, §2.3).

### 1.2 비목표
- 알림 센터/히스토리, 클릭 액션(되돌리기 버튼 등) — v1 이후. GNOME 목록 잔류 없음(D4).
- 한자/특수문자/이모지 팝업의 모양·위치 변경(POPUP_SPEC 영역).
- 비프(`unim-dbus/src/beep.rs`) 대체.
- unim-indicator·unim-popup-service 의 토스트 표시(D3 — 데몬 직접 fdo 로 대체). XIM 환경도 데몬 fdo 로 커버된다.
- 확장의 기타 `Main.notify`(`indicator.js:332` 데몬 미실행, `:458`·`:492` 도움말) — 범위 밖, 현행 유지.

---

## 2. 이벤트 카탈로그

### 2.1 이벤트 표

`kind` 는 DBus 문자열 상수(snake_case), `key` 는 중복 억제용 식별자, "설정명" 은 `notify.events` 리스트 원소(§5).
문구는 **가림(기본)** / `show_text=true` 두 벌. 줄번호는 develop `b94da2a` 기준.

| kind | 설정명 | 발생 지점 (근거) | 문구 ko (가림 / 표시) | 문구 en (가림 / 표시) | 기본 | key |
|------|--------|------------------|----------------------|----------------------|------|-----|
| `atf_corrected_forward` | `atf_corrected` | `unim-dbus/src/engine_worker.rs:1557` `if let Some(ref fix) = fix` 블록 안, `Direction::Forward`. before=`fix.original`(ASCII), after=`fix.corrected`. Windows `unim-tsf/src/auto_typefix.rs:302` 대응 블록 | `자동 교정됨 (영→한)` / `자동 교정: {before} → {after}` | `Auto-fixed (EN→KO)` / `Auto-fixed: {before} → {after}` | ON | 가림: `""`, 표시: `before` |
| `atf_corrected_reverse` | `atf_corrected` | 같은 블록, `Direction::Reverse`. **`fix.original` 은 빈 문자열**(`src/auto_typefix/reverse.rs:86`, 사유 `engine_worker.rs:1577`) → before=`unim::typefix::eng_to_kor(&buf_ascii, kl, el)`(`buf_ascii` 는 `:1626` 에서 `buf.clear()` 전에 이미 캡처됨, 화면 글자의 근사치), after=`fix.corrected`. Windows `auto_typefix.rs:312` 대응 | `자동 교정됨 (한→영)` / `자동 교정: {before} → {after}` | `Auto-fixed (KO→EN)` / 동형 | ON | 위와 같음 |
| `atf_suppressed` | `atf_suppressed` | 코어 `check_*_outcome` 이 `AtfOutcome::Suppressed(AtfSuppressReason::Blacklist)` 를 돌려준 키(§3.1). 판정 원천 `src/auto_typefix/forward.rs:34`·`reverse.rs:37` `blacklist.is_suppressed`. 데몬 호출부 `engine_worker.rs:1481~1503` | `자동 교정 건너뜀: 제외 단어` / `자동 교정 건너뜀: '{word}'` | `Auto-fix skipped: excluded word` / `Auto-fix skipped: '{word}'` | ON | `context_id` |
| `blacklist_learned` | `blacklist_learned` | `engine_worker.rs:1523` `blacklist.add_or_hit_tentative` 직후(재트리거 감지 분기). `observe_rollback_event`(`:908`)는 플래그만 세우므로 발생 지점이 아님. 등록 상태는 **Tentative** — `tentative_expiry_hours`(기본 4, `src/config.rs:406`) 뒤 `expire_tentatives`(`src/typefix_blacklist.rs:318`)로 Inactive. 재트리거 분기는 `fix` 를 `None` 으로 바꾸므로(`:1508~1555`, 재트리거 시 `:1549` `None`) 같은 키에서 `atf_corrected_*` 와 **상호 배타** | `자동 교정 임시 제외 ({h}시간)` / `'{word}' 자동 교정 임시 제외 ({h}시간)` | `Auto-fix paused for this word ({h}h)` / `'{word}' auto-fix paused ({h}h)` | ON | `word` |
| `password_enter` | `password_enter` | **포커스된 컨텍스트 기준 전이**일 때만(v2.2, P2 실측 반영): ① 포커스된 컨텍스트의 `SetContentType` 이 비차단→차단(`should_block_hangul`)으로 바꿀 때, ② FocusIn 시 직전 포커스 컨텍스트와 차단 상태를 비교해 새로 들어간 쪽이 차단이면 enter·떠난 쪽이 차단이면 leave. **포커스 없는 컨텍스트의 `SetContentType` 은 알림을 만들지 않는다** — GTK 는 포커스 전인 비밀번호 위젯의 목적을 미리 보내므로, 이를 전이로 보면 일반 칸에서 오발한다(규칙 2 리셋만 수행). 같은 컨텍스트 재포커스는 생략, `DestroyContext` 는 직전 포커스 기록을 비운다. 엔진 재생성 경로(`:834` 직접 `set_content_purpose`)는 발화 안 함. Windows `unim-tsf/src/input_scope.rs:25` 판정 전이 | ATF 켜짐: `비밀번호 칸: 영문 고정, 자동 교정 꺼짐` / ATF 꺼짐: `비밀번호 칸: 영문 고정` (고정 문자열) | `Password field: English locked, auto-fix off` / `Password field: English locked` | ON | `context_id` |
| `password_leave` | `password_leave` | 같은 함수, 차단→비차단 전이 | `비밀번호 칸 벗어남` | `Left password field` | OFF | `context_id` |
| `mode_toggle_suppressed` | `mode_toggle_suppressed` | 코어 `src/input_engine/press_key.rs:140~149`(차단 분기, `:143` 로그 후 `:148` 에서 `consumed()` 반환)이 엔진 상태 `last_toggle_blocked=true` 를 남김(E4, §3.1a) → 데몬 `engine_worker.rs:1347` `engine.press_key` 직후 `engine.last_toggle_blocked()` 로 소비. `:1847` 은 모드 **비프** 지점이라 무관 | `비밀번호 칸: 한/영 전환 막힘` (고정) | `Password field: language toggle blocked` | ON | `context_id` |
| `mode_changed` | `mode_changed` | `engine_worker.rs:1847`(ProcessKey 최종 `mode_changed` 소비, `announce_mode` 와 동일). ATF 가 바꾼 모드(`:1615` `mode_changed = Some(..)`)는 §2.2 규칙 6 으로 생략될 수 있다. 및 `:2086`(SetGlobalMode Global 분기 비프 지점 — 요청 `SetGlobalMode { is_korean }`(`service.rs:74`)은 응답 채널·`context_id` 가 없다 → `context_id=0`(전역 표지; 실제 id 는 `service.rs:408` 에서 1부터) 으로 `NotifyEvent` 를 만들되, 이 지점(`:2063` 처리 블록)도 엔진 워커 스레드 안이므로 **채널로 곧장 넣지 않고 워커 소유 `NotifyGate::offer` 를 거친 뒤** 통과분만 알림 채널(§3.2)로 보낸다 — 게이트(규칙 1·5)를 우회하지 않는다. PerApp 분기는 비프와 같이 무발화) | `한글` / `영문` | `Korean` / `English` | OFF | `is_korean` |
| `feature_toggled` | `feature_toggled` | `engine_worker.rs:1938` `resp.atf_toggled = Some((kind, new_value))`(ATF 토글 단축키; 비프 `:1946`). Windows 대응 지점 P4 확인 | `자동 교정 {켜짐/꺼짐}` (순/역방향 토글은 `자동 교정(영→한) …`) | `Auto-fix {on/off}` | ON | `feature+value` |
| `feature_result` | `feature_result` | **확장 로컬 발행**(Q10): `extension.js:731~732`(TypeFIX 수동 변환), `:785~791`(사전 등록 실패), `:796~800`(사전 등록 성공). 데몬 경유 안 함 | 가림: `변환 완료` / `사전에 등록됨`; 표시: 기존 문구(`%s` 포함) | 동일 규칙 | ON | `text` |

문구 규칙: 한 줄, 40자(ko)/60자(en) 이내. 제목 "UNIM" 은 표시 경로가 붙인다. `{before}`/`{after}`/`{word}` 는 `show_text=true` 일 때만, 최대 16자(초과분 `…`).
`atf_corrected` 판정 블록 밖의 한자 단어 교체(`engine_worker.rs:1803~1811`, 같은 `auto_typefix_result` 채널 사용)는 **알림 대상이 아니다** — `if let Some(ref fix)` 블록에서만 생성하므로 섞이지 않는다.
v0 의 `atf_suppressed` reason "앱 제외" 는 **존재하지 않는 기능**이라 삭제(`AppRule`, `src/config.rs:1002` 은 `app_pattern`·`default_category` 뿐). reason "비밀번호 필드" 도 삭제(Q5, `password_enter` 와 중복).

### 2.2 중복 억제(rate limit)
1. 동일 `(kind, key)` 가 **3,000 ms** 내 재발하면 폐기 — Linux 는 데몬의 코어 `NotifyGate`, Windows 는 **렌더러(popup-win)** 측(§3.4, TSF DLL 은 호스트 프로세스별이라 앱 간 중복을 못 본다).
2. `atf_suppressed`·`mode_toggle_suppressed` 는 `context_id` 당 **1회**, FocusOut·목적 변경 시 리셋.
3. 표시 경로는 화면에 **최신 1장만** 유지(새 토스트가 오면 교체). 기본 표시 2,000 ms(`notify.duration_ms`).
4. `password_enter` 는 **같은 칸이면 1회만, 단 10분 지나면 재알림**(D5·E5): `NotifyGate` 가 `password_announced: HashMap<u32, Instant>`(context_id → 마지막 알림 시각)를 두고, 비차단→차단 전이 때 항목이 없거나 `now - t ≥ PASSWORD_REANNOUNCE`(600 s, 코어 상수 — 설정 키 아님) 이면 발화·시각 갱신, 아니면 폐기. 항목 제거는 `DestroyContext` 에서만(FocusOut·재포커스·엔진 재생성으로는 리셋 안 함). `password_leave` 는 맵에 들어 있는 칸에서만 발화. **전이 기록(맵 갱신)은 규칙 5(`events` 필터)와 무관하게 수행**한다 — `password_enter` 를 `events` 에서 뺀 사용자가 `password_leave` 만 켰을 때도 진입 시각이 맵에 남아야 leave 가 나간다(필터는 *발화* 여부만 막고 *기록* 은 막지 않는다. 단 `notify.enabled=false` 면 게이트 전체가 꺼져 기록도 하지 않는다). Windows 는 문서(ITfDocumentMgr) 단위 동등 규칙(P4).
   - **한계(문서화)**: UNIM 이 보는 "칸" 은 IM 컨텍스트다. 브라우저처럼 창 하나 = 컨텍스트 하나인 앱에서는 첫 비번칸 알림 뒤 **10분 안에** 같은 창의 다른 사이트 비번칸으로 가도 알리지 않는다. FocusOut(`engine_worker.rs:2006`)은 버퍼만 비우고 purpose 를 초기화하지 않으므로 같은 칸 재포커스는 전이를 만들지 않으며(같은 칸 1회 규칙과 부합), 전이는 앱이 `SetContentType` 을 다시 보낼 때만 생긴다. 목적을 전달하지 않는 환경(XIM 등, §2.3)에서는 아예 발화하지 않는다.
5. `notify.enabled=false` 또는 해당 설정명이 `notify.events` 에 없으면 이벤트를 **만들지 않는다**(시그널·fdo 호출 0). 단 `password_*` 전이 기록(규칙 4)은 `events` 필터와 무관하게 수행한다(`enabled=false` 면 기록도 안 함).
6. **한 요청당 최대 1건 우선순위**: 같은 `ProcessKey` 처리에서 `atf_corrected_*` 가 게이트를 통과했으면 그 키의 `mode_changed`(ATF 유발 모드 전환, `:1615`)는 **생략**한다 — 교정 문구의 방향 표기(영→한/한→영)가 모드 변화를 이미 알린다. `atf_corrected` 가 꺼져 있으면 `mode_changed` 가 나간다. 그 밖의 조합은 발생 구조상 겹치지 않는다(`blacklist_learned` 는 교정과 배타, `mode_toggle_suppressed`·`feature_toggled` 는 조기 반환 경로).

### 2.3 민감정보 규칙(절대)
- **위협**: 목적 감지 불가 환경(XIM 레거시·Chrome Wayland 플래그 미활성, `unim-dbus/SPEC.md` §13.3)에서는 비번칸도 `purpose=Normal` 로 보이므로 Password 게이트가 무력하다. 이때 ATF 가 비밀번호 일부를 교정하면 그 텍스트가 토스트·알림 서버·잠금 화면으로 샐 수 있다.
- **대책 1 (D1)**: `notify.show_text` 기본 `false` — `atf_corrected_*`·`atf_suppressed`·`blacklist_learned`·`feature_result` 문구에 입력 텍스트를 넣지 않는다. 설정 UI 에 "감지 못 한 비밀번호 칸에서 입력 일부가 보일 수 있음" 경고를 붙인다.
- **대책 2 (D4)**: 모든 경로 transient(fdo `transient` hint, 확장 transient 알림, St 토스트는 원래 목록 없음).
- **대책 3**: `ContentPurpose::Password|Pin` 상태에서는 `show_text` 와 무관하게 위 4종을 **코어 `NotifyGate::offer` 가 폐기**(`atf_active_for_field` 원천 봉쇄 원칙, `engine_worker.rs:560~567`; 원래 ATF 자체도 발화하지 않으므로 이중 방어).
- `password_*`·`mode_toggle_suppressed` 문구는 **고정 문자열만**(파라미터 금지).
- fdo body 는 `show_text=true` 일 때 `<`·`>`·`&` 를 이스케이프(body-markup 서버 대비).
- **로그**: `Notify` 의 title/body 는 **로그에 남기지 않는다** — `kind` 와 표시 경로(signal/fdo/skip)만. 선례는 ATF·한자 교체 채널의 "글자 수만" 로그(`unim-dbus/src/service.rs:2240~2253`).
- **별건(본 기능 범위 밖, 기존 누출 경로)**: `engine_worker.rs:1595~1600` 이 `corrected='{}'`(`:1597` 형식, `:1599` `fix.corrected`)를, `:1541~1546` 가 재트리거 키 `'{}'` 를 평문 로그로 남긴다 — 감지 못 한 비번칸에서 이미 새는 경로다. `ProcessKeyEvent` preedit/commit 평문 로그(`service.rs:2350~2357`)와 함께 별도 과제로 다룬다(알림 구현이 이를 늘리지 않는다).

---

## 3. 아키텍처

```
┌──────────────────── 코어 src/ (플랫폼 무관) ─────────────────────┐
│ InputEngine::last_toggle_blocked() (getter)   ← D2·E4           │
│ auto_typefix::check_{forward,reverse}_outcome → AtfOutcome ← D2 │
│ notify::{NotifyKind, NotifyEvent, NotifyGate, render(ev, Lang)}  │
└───────────────┬──────────────────────────────────┬───────────────┘
         Linux  │ unim-dbus (데몬, 세션 버스)        │ Windows unim-tsf (DLL, UI 금지)
                ▼                                  ▼
  'gnome-shell-notify' 등록됨?           popup_ipc 파이프 cmd="toast"
     ├─ 예 → InputMethod 시그널 `Notify`   (\\.\pipe\unim-popup-win.<sid>)
     │        └ GNOME 확장: P2 transient       │
     │          Source / P3 St 토스트           ▼
     └─ 아니오 → 데몬이 직접             unim-popup-win.exe 토스트 창
        org.freedesktop.Notifications       (포커스 창 모니터 모서리, 3초 dedupe,
        .Notify (transient)                  최신 1장, UIA notification)
        (KDE·X11·wlroots·XIM·확장 꺼진 GNOME)
```

### 3.1 코어 (D2 범위 엄수)

**(a) 한/영 전환 차단 여부 — 엔진 getter (E4, `InputResult` 무변경)** — `src/input_engine/{engine,press_key}.rs`

`InputResult`(`src/input_engine/types.rs:261~262`, `#[repr(C)]`)에는 **필드를 추가하지 않는다**. 이 구조체는 `unim-capi`(`src/lib.rs:185, 366` 값 반환, `include/unim.h:70~76` `UnimInputResult`)로 노출되고, 그 라이브러리·헤더는 패키지 `unim-common` 이 `libunim_capi.so.0`·`/usr/include/unim.h` 로 배포한다(`debian/unim-common.install:6~8, 11`, soname 은 `unim-capi/build.rs:13`). 선례도 같다 — `pending_atf_toggle`·`popup_pending_action` 은 "InputResult 에 필드를 추가하지 않는 out-of-band 채널"로 명문화돼 있다(`types.rs:55~61, 223~224`).

```rust
// src/input_engine/engine.rs — InputEngine (선례 pending_atf_toggle :185, 초기화 :339)
pub(super) last_toggle_blocked: bool,

impl InputEngine {
    /// 직전 `press_key` 가 ContentPurpose(Password/Pin) 때문에 한/영 전환을 거부했는가.
    /// `press_key` 진입 시 false 로 리셋되므로 드레인 없이 "마지막 호출" 의미를 갖는다.
    pub fn last_toggle_blocked(&self) -> bool { self.last_toggle_blocked }
}
```
- `press_key` **맨 앞**에서 `self.last_toggle_blocked = false`, 차단 분기(`press_key.rs:140~149`)의 `:148` `return InputResult::consumed();` 직전에 `true`. 로그(`:143`)·반환값은 그대로 → 기존 소비자 무회귀.
- C 함수 1개 추가(선례 `unim_engine_is_composing(engine: &InputEngine) -> bool`):
  ```c
  /* 직전 unim_engine_press_key 가 비밀번호 칸 때문에 한/영 전환을 거부했으면 true. */
  bool unim_engine_last_toggle_blocked(const UnimEngine *engine);
  ```
  `unim-capi/src/lib.rs` + 커밋된 `include/unim.h` 동시 추가. drift guard(`unim-capi/build.rs:77 exported_fns`)는 **함수 목록만** 비교하므로 새 함수 누락은 잡지만 구조체 변경은 못 잡는다 → 아래 레이아웃 단언을 함께 둔다. 심볼 추가는 하위 호환이라 soname·패키지 파일 목록 무변경. 트리 안에서 `unim_engine_press_key` 를 부르는 C 소비자는 `examples/capi-c/minimal_session.c` 뿐(GTK/Qt 모듈은 데몬 DBus 경유) — 예제 컴파일만 확인.
- **레이아웃 고정 단언**(Rust 쪽 변경이 C 구조체로 새지 않게): `unim-capi/src/lib.rs` 에 `const _: () = assert!(core::mem::size_of::<InputResult>() == 5 && core::mem::align_of::<InputResult>() == 1);` — 현재 `bool` 5개. 누가 `InputResult` 에 필드를 더하면 capi 빌드가 깨진다. 헤더 쪽은 `examples/capi-c` 에 `_Static_assert(sizeof(UnimInputResult) == 5, ...)`.
- Windows TSF 는 코어를 직접 링크하므로 `engine.last_toggle_blocked()` 를 그대로 쓴다(§3.4).

**(b) ATF 억제 사유 반환** — `src/auto_typefix/{mod,forward,reverse}.rs`

```rust
/// 감지 판정 3분: 교정 / 억제(사유) / 미해당.
#[derive(Debug)]
pub enum AtfOutcome {
    Fix(AutoTypeFixResult),
    Suppressed(AtfSuppressReason),
    NoMatch,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtfSuppressReason {
    /// 학습형 블랙리스트(Tentative|Confirmed) 적중 — forward.rs:34 / reverse.rs:37.
    Blacklist,
}

impl AtfOutcome {
    pub fn into_fix(self) -> Option<AutoTypeFixResult>;
    pub fn suppressed(&self) -> Option<AtfSuppressReason>;
}

pub fn check_forward_outcome(
    buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig,
    korean_layout: &str, english_layout: &str,
    blacklist: &dyn BlacklistGate,
) -> AtfOutcome;

pub fn check_reverse_outcome(
    buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig,
    korean_layout: &str, english_layout: &str,
    blacklist: &dyn BlacklistGate, user_dict: &dyn UserDictGate,
) -> AtfOutcome;

// 기존 시그니처 유지(무회귀): 본문은 `check_*_outcome(..).into_fix()` 한 줄.
pub fn check_forward(...) -> Option<AutoTypeFixResult>;
pub fn check_reverse(...) -> Option<AutoTypeFixResult>;
```
- 블랙리스트 분기만 `Suppressed(Blacklist)`, 나머지 `return None`(방향 꺼짐·버퍼 짧음·영어 사전·완성 음절 등)은 전부 `NoMatch` — 이들은 "억제" 가 아니라 "해당 없음" 이라 알림 대상이 아니다.
- 데몬 `engine_worker.rs:1481~1503` 과 TSF `auto_typefix.rs:302·312` 만 `*_outcome` 으로 바꾸고, `tests.rs` 의 기존 `check_*` 테스트는 그대로 통과해야 한다. `suppressed_by_*_blacklist` 테스트(`tests.rs:741, 767, 843`)에 `Suppressed(Blacklist)` 단언을 추가.
- `typefix_blacklist::add_or_hit_tentative` 등 다른 코어 API 는 바꾸지 않는다(D2). 재트리거 분기는 해당 키가 활성 블랙리스트였다면 애초에 `Fix` 가 나오지 않으므로 `:1523` 도달 = 신규 또는 Inactive 부활 등록이다 → 반환값 없이 발화해도 정확.

**(c) `src/notify.rs` 신규(순수 모듈, UI·env 의존 0)**
- `pub enum NotifyKind { AtfCorrectedForward, AtfCorrectedReverse, AtfSuppressed, BlacklistLearned, PasswordEnter, PasswordLeave, ModeToggleSuppressed, ModeChanged, FeatureToggled }` + `as_str()`(DBus `kind`)·`setting_name()`(§5 리스트 원소, 두 `AtfCorrected*` → `"atf_corrected"`).
- `pub struct NotifyEvent { kind: NotifyKind, key: String, context_id: u32, params: NotifyParams }` — `params` 는 kind 별 타입드 값(`before/after/word/hours/is_korean/feature/value/atf_on`).
- `NotifyGate::offer(&mut self, ev, purpose: ContentPurpose, cfg: &NotifyConfig, now: Instant) -> Option<NotifyEvent>` — §2.2 규칙 1·2·4(10분 재알림 포함)·5, §2.3 대책 3. `on_focus_out(ctx)`·`on_purpose_change(ctx)`·`on_destroy(ctx)` 로 규칙 2·4 상태 관리. 규칙 6 은 `offer_batch(&mut self, evs: &[NotifyEvent], ..) -> Vec<NotifyEvent>`(한 요청분을 함께 받아 ATF 교정이 통과하면 같은 묶음의 `ModeChanged` 제거).
- `render(ev, lang: Lang, show_text: bool) -> Rendered { title, body }` — §2.1 문구 표. 길이 절단·이스케이프 포함.
- **`Lang` 결정 규칙(Q1 보강)**: `notify.language` 가 `ko|en` 이면 그대로. `auto` 면 **호출자**가 결정해 넘긴다(코어는 env 를 읽지 않는다) — Linux 데몬: POSIX 우선순위 **`LC_ALL` > `LC_MESSAGES` > `LANG`** 중 처음으로 비어 있지 않은 값이 `ko` 접두면 ko, 그 외·전부 비면 en. 기존 선례 `unim-settings-gtk/src/main.rs:16 detect_locale` 은 `LANG` → `LC_ALL` 순(POSIX 와 반대)이라 **따르지 않는다**(선례 정정은 별건). 데몬은 세션 기동 방식에 따라 env 가 비어 있을 수 있어 설정 UI 에서 명시 선택을 권장. Windows: P4 에서 `unim-settings/src/platform/windows.rs` 로케일 판정 재사용 검토.

### 3.2 데몬 (`unim-dbus`)
- **이벤트 생성**: §2.1 발생 지점에서 `NotifyEvent` → 워커 소유 `NotifyGate::offer` → 통과 시 `render` → `NotifyOut { kind, rendered, duration_ms, flags }` 를 **알림 채널**로 보낸다. 엔진 워커는 `std::thread`(`engine_worker.rs:972`)이므로 `tokio::sync::mpsc::Sender::try_send`(bounded 32, 비차단; 가득 차면 폐기·`kind` 만 로그)로 넣는다. `spawn_engine_worker(config)`(`:969`)에 `notify_tx` 인자를 추가한다. `EngineResponse`·`apply_content_type` 반환형은 **바꾸지 않는다** — 응답 채널이 없는 `SetGlobalMode`(`:2063`, `service.rs:491` 단방향 send)도 같은 채널로 해결되고, 키 1회에 이벤트가 여럿이어도 자리 다툼이 없다(우선순위는 §2.2 규칙 6 이 게이트 단계에서 정함).
  - **`ProcessKey` 처리 중 이벤트 수집(규칙 6 실현)**: `press_key` 직후·ATF 판정·`apply_content_type` 등에서 생기는 `NotifyEvent` 는 **즉시 `offer` 하지 않고** 요청 처리 지역 `Vec<NotifyEvent>` 에 모은다. 최종 `mode_changed` 소비 지점(`engine_worker.rs:1847` 부근, 비프 `announce_mode` 와 같은 자리)에서 그 `Vec` 을 `NotifyGate::offer_batch` 로 **정확히 1회** 넘겨 통과분만 `try_send` 한다(ATF 교정이 통과하면 같은 묶음의 ATF 유발 `ModeChanged` 제거). 조기 반환 경로(`mode_toggle_suppressed`·`feature_toggled`)는 반환 직전에 같은 방식으로 `offer_batch` 1회. 단건 이벤트(`SetGlobalMode` 등)는 `offer` 를 직접 쓴다.
- **알림 전담 태스크(1개)**: 데몬 기동 시 `spawn_notify_task(conn, active_frontends, rx)` 를 한 번 띄운다. 이 태스크만 `Notify` 시그널 발행·fdo 호출·`last_id: u32`(replaces_id)·백오프 상태(`retry_after: Option<Instant>`)를 소유한다 → 알림마다 `tokio::spawn` 할 때 생기는 `replaces_id` 경쟁(뒤늦은 응답이 `last_id` 를 덮어 "최신 1장" 붕괴)이 구조적으로 없다. 수신 시 `try_recv` 로 대기열을 비워 **마지막 1건만** 표시(§2.2-3). 키 응답 경로(`service.rs:2360` 반환)는 채널 투입만 하고 await 하지 않는다.
- **라우팅(D3)**: 태스크가 발행 직전 등록 목록을 읽어 **`gnome-shell-notify`** 등록자가 1명 이상이면 `Notify` 시그널만, 없으면 fdo 직접 호출만. 기존 `gnome-shell` 이름(`extension.js:111`, `unim-gui-common/src/dbus_client.rs:252` 의 정확 일치 비교)은 건드리지 않고 **별도 이름을 추가 등록**한다 — 구버전 확장(`gnome-shell` 만 등록, `Notify` 미구독)이면 fdo 로 떨어져 유실이 없다.
- **등록자 추적(E2, 재검증 지적 1)**: 현재 `active_frontends: Arc<RwLock<HashSet<String>>>`(`service.rs:348`)는 이름만 담고 `register_frontend`(`:506`)·`unregister_frontend`(`:531`)는 호출자를 기록하지 않으며, `NameOwnerChanged` 처리도 없다 → 확장이 해제 없이 죽으면 `gnome-shell-notify` 가 남아 fdo 폴백이 막히고 알림이 아무도 안 듣는 시그널로만 나가 사라진다. 설계:
  - 자료형을 `HashMap<String /*이름*/, HashSet<OwnedUniqueName> /*등록자*/>` 로 바꾸고, 두 메서드에 `#[zbus(header)] hdr: Header<'_>`(zbus 4, `unim-dbus/Cargo.toml:21`)를 받아 `hdr.sender()` 를 기록·삭제한다. `UnregisterFrontend` 는 **호출자 자신의** 등록만 지운다. 이름은 등록자 집합이 비면 목록에서 빠진다.
  - 데몬 기동 시 `org.freedesktop.DBus.NameOwnerChanged` 를 구독(`zbus::fdo::DBusProxy::receive_name_owner_changed`)해 **`name` 이 `:` 접두(unique name)이고 `new_owner` 가 비면**(연결 종료) 그 unique name 을 등록자로 가진 항목을 모든 이름에서 제거하고, 이름 집합이 바뀌었으면 기존 `ActiveFrontendsChanged`(`:565`)를 발행한다. 이 정리는 `gnome-shell` 등 기존 이름에도 똑같이 적용된다(동작 개선, `GetActiveFrontends` 반환형 `Vec<String>` 무변경).
  - **한계(명기)**: 확장은 gnome-shell 프로세스의 **버스 연결을 공유**하므로 등록자 unique name 은 셸 전체의 것이다. 셸이 죽거나 재시작하면 정리되지만, **셸은 살아 있고 확장만 고장(예외·비활성 실패)** 난 경우엔 unique name 이 유지돼 `gnome-shell-notify` 등록이 남고 정리되지 않는다 — 이때는 알림이 구독자 없는 시그널로 나가 유실될 수 있다(확장 정상 disable 은 `UnregisterFrontend` 로 해제되므로 해당 없음). 완화책은 v1 범위 밖.
  - 데몬 재시작 뒤엔 목록이 비므로 확장이 **재등록**한다: 기존 재연결 훅 `_onDaemonReady`(`extension.js:500`, `registerFrontend('gnome-shell')` `:503`)에서 브리지가 켜져 있으면 `gnome-shell-notify` 도 등록(**"브리지가 켜져 있다"** = 확장이 활성이고 `dbus_ime.js` 의 `Notify` 시그널 구독이 만들어진 상태 — 구독 실패·미지원 버전이면 등록하지 않아 fdo 폴백이 유지된다). enable(`:111`)·disable(`:142` 해제)과 같은 자리에서 짝을 맞춘다.
  - **테스트 용이성**: `header` 인자가 붙은 `#[zbus]` 메서드는 기존 테스트(`service.rs:3480~`)처럼 직접 호출하기 어렵다. 등록·해제·정리 로직은 `(name: &str, sender: &str)` 를 받는 **순수 헬퍼**(예: `register_frontend_inner`/`unregister_frontend_inner`/`drop_owner`, 대상은 맵 `&mut`)로 분리하고, zbus 메서드는 `hdr.sender()` 를 뽑아 헬퍼에 넘기기만 하는 얇은 래퍼로 둔다. 테스트는 헬퍼를 직접 부른다.
  - 단위 테스트: 기존 `active_frontends` 테스트(`service.rs:3479~`)를 등록자 맵 + 순수 헬퍼 호출로 갱신 + "등록자 소멸 시 이름 제거", "다른 등록자가 남으면 유지", "타인 등록 해제 불가".
- 시그널 초안(E2, `unim-dbus/SPEC.md` §5.2 추가):

  | 시그널 | 파라미터 | 비고 |
  |---|---|---|
  | `Notify` | `kind: s, title: s, body: s, duration_ms: u, flags: u` | InputMethod 인터페이스(`INPUT_METHOD_PATH`, `config_changed` 와 같은 SignalContext). flags 0x01=text_shown(`show_text` 로 입력 텍스트 포함 — 수신측 로그 금지) 0x02=replace |

  세션 버스 시그널은 같은 사용자 프로세스가 볼 수 있다 — 기존 `AutoTypefixApply` 와 같은 노출 수준이며, 기본(가림)에서는 입력 텍스트가 없다.
- fdo 직접 호출(D3): 데몬은 이미 세션 버스에 연결돼 있다(`unim-daemon/src/main.rs:387 Connection::session()`) → zbus 프록시로 `org.freedesktop.Notifications.Notify(app_name="UNIM", replaces_id=last_id, app_icon="unim-korean"(P2 확정: `unim-common` 이 항상 설치하는 hicolor 아이콘 — 트레이용 `…Indicator` 는 `unim-desktop` 에만 있어 제외), summary=title, body, actions=[], hints, expire_timeout=duration_ms)`.
  - hints: `transient: true`(D4), `urgency: byte 1(normal)` — GNOME Shell 은 low 긴급도를 배너로 띄우지 않는 것으로 알려져 있어 low 를 쓰지 않는다(P2 실기 확인), `desktop-entry` 는 `io.github.from104.unim.Settings`(P2 확정: `unim-desktop` 이 설치하는 desktop 파일 — 없으면 서버가 `app_name` 으로 폴백), `category` 는 `x-unim.input`(표준 목록에 입력기 상태가 없어 vendor 형식). 서버 오류 시 `last_id` 는 0 으로 초기화.
  - 반환 id 를 전담 태스크의 지역 상태 `last_id: u32` 에 보관해 다음 호출의 `replaces_id` 로 써 최신 1장 규칙(§2.2-3)을 서버에 위임(호출이 직렬이므로 잠금 불필요).
  - 서버 부재(`ServiceUnknown`)·거부 시: 로그 1회, 60초 백오프(전담 태스크의 `retry_after`; 그동안 도착한 알림은 폐기) 후 재시도. 재시도 없는 영구 차단은 하지 않는다(알림 데몬 늦은 기동 대비).
  - GNOME Shell 은 `expire_timeout` 을 무시(고정 배너 시간)한다 — `duration_ms` 는 KDE 등에서만 유효. GNOME 확장 P2(MessageTray transient)도 배너 시간을 제어하지 못하므로 무시, P3 St 토스트부터 적용 → 설정 UI 표기(§5).
- 테스트 액션: `TriggerAction("notify_test")` 를 §5.6 범위에 추가(E2) — L2·L3 검증용, `kind="test"`.
- 설정 핫 리로드(SPEC §7.3)로 `notify.*` 즉시 반영.

### 3.3 Linux 표시 경로
| 경로 | 프로세스 | 백엔드 | 비고 |
|---|---|---|---|
| L-fdo | **데몬 자신** | `org.freedesktop.Notifications` 직접(§3.2) | GNOME(확장 꺼짐·구버전)·KDE·기타 X11·wlroots·XIM 전용 환경 공통. 다른 UNIM 프로세스 불필요(D3). `unim-indicator` 는 autostart 에 `NotShowIn=GNOME`(`unim-indicator/data/io.github.from104.unim.Indicator.desktop:18`)이라 애초에 GNOME 폴백 주체가 될 수 없었다 |
| L-gnome | `unim-gnome-extension`(`dbus_ime.js` 구독, 활성 시 `RegisterFrontend('gnome-shell-notify')`, 비활성 시 해제, 데몬 재연결 `_onDaemonReady`(`extension.js:500`) 에서 재등록, 비정상 종료는 데몬의 `NameOwnerChanged` 정리(§3.2)) | P2: 확장 전용 `MessageTray.Source` 1개 재사용 + `Notification` **transient**(GNOME 46+ 생성자 `isTransient: true`, 이전 버전 `setTransient(true)`), 새 알림 전 직전 알림 `destroy()`; P3: 패널 인디케이터(`indicator.js` PanelMenu.Button) `get_transformed_position()` 아래 `St.BoxLayout` 토스트(`Main.layoutManager.addTopChrome`) | 접근성 Q8. 확장 로컬 `feature_result` 도 같은 Source·transient 로 통일(Q10, E1 AND 게이트) |

### 3.4 Windows 경로
- `unim-tsf`(DLL) 는 UI 를 만들지 않는다(`docs/dev/windows/popup-renderer-design.md`). 판정 지점(`auto_typefix.rs:302·312` outcome, `input_scope.rs:25` 전이, 한/영 토글 `press_key` 직후 `engine.last_toggle_blocked()`(E4) — `text_service.rs:1067` 이후 경로에서 소비 지점 P4 확인)에서 `NotifyEvent` → 문서 단위 규칙(§2.2-2·4)만 DLL 에서 적용 → `render` → `popup_ipc.rs` worker 로 전송.
- **와이어**: 기존 `WireMsg`(`unim-tsf/src/popup_ipc.rs`, 동결 사본 `unim-popup-win/src/protocol.rs:69`)는 `cmd: String` + 옵셔널 필드 구조다. 규칙(`popup_ipc.rs:29` "필드 추가는 반드시 `#[serde(default)]` + 설계서 갱신 + `WIRE_VERSION` 유지")대로 `cmd="toast"` + `toast: Option<ToastPayload>`(`#[serde(default, skip_serializing_if = "Option::is_none")]`)를 추가한다. `ToastPayload { kind: String, key_hash: u64, title: String, body: String, duration_ms: u32, corner: String, flags: u32 }` — key 원문은 보내지 않고 해시만. `owner_hwnd` 는 기존 필드 재사용(포커스 창).
  - **구버전 렌더러 호환 확인됨**: 미지 `cmd` 는 `unim-popup-win/src/main.rs:267~268` 에서 로그 후 무시, 새 옵셔널 필드는 serde 기본 동작으로 무시. **`WIRE_VERSION` 을 올리면 안 된다** — 불일치 메시지는 `pipe_server.rs:380~386` 에서 통째로 버려진다. 회귀 테스트: 골든 라인 테스트(`protocol.rs:234`)에 toast 라인 파싱·구 구조체로의 역파싱 추가.
- **렌더러 측 억제**: 렌더러는 세션당 1프로세스이므로 3초 `(kind, key_hash)` dedupe·최신 1장 교체를 **렌더러가** 한다(호스트 앱 간 중복 제거).
- **첫 토스트 지연**: 파이프 연결은 lazy(`popup_ipc.rs:347`), 렌더러 기동은 5초 rate-limit(`:621`). worker 는 미연결 중 도착한 toast 를 **1건만 1,500 ms TTL** 로 보류하고 연결되면 전달, 초과 시 폐기.
- 위치(D6): `MonitorFromWindow(owner_hwnd, MONITOR_DEFAULTTONEAREST)` → `GetMonitorInfoW().rcWork`(작업표시줄 제외 작업영역)의 `notify.corner` 모서리(auto=우하단). owner 가 0 이면 `GetForegroundWindow()`.
- 창: 기존 `window.rs:98` 플래그(`WS_EX_NOACTIVATE|WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_LAYERED`) 그대로 두 번째 창. 한자 팝업과 겹치면 토스트가 숨는다.
- 접근성: `UiaRaiseNotificationEvent(NotificationKind_ActionCompleted, NotificationProcessing_MostRecent)`.
- 설정 반영: TSF 는 설정 파일을 스스로 다시 읽는다(`unim-tsf/src/text_service.rs:429 maybe_reload_config`, `unim-settings/src/platform/windows.rs:75`) → `notify.*` 도 이 경로로 반영. 렌더러는 설정을 읽지 않고 페이로드의 `duration_ms`·`corner` 를 따른다.
- IMM32 경로(`unim-imm32`): v1 범위 밖(P4 후속).

---

## 4. 플랫폼별 표시 매트릭스

| 환경 | 1순위 | 폴백 | 근거/한계 |
|---|---|---|---|
| GNOME Wayland·X11 + 신버전 확장 활성 | 확장 P2 transient 알림 → P3 St 토스트(인디케이터 좌표 있음) | — | 데몬은 GNOME Wayland 에 직접 그릴 수 없음(POPUP_SPEC §1.2, `popup_position.rs:21`) |
| GNOME, 확장 꺼짐·구버전 | 데몬 fdo → GNOME Shell 배너(상단 중앙, transient) | 로그 | indicator 는 GNOME 에서 미기동(`NotShowIn=GNOME`) — 의존하지 않음 |
| KDE Wayland·X11 | 데몬 fdo(Plasma 가 트레이 근처 표시) | 로그 | SNI 아이콘 좌표 API 없음(`tray.rs:199 activate(_x,_y)` 는 클릭 좌표뿐) |
| 기타 X11 / wlroots | 데몬 fdo(mako·dunst 등 있으면) | 로그(1회)+60초 백오프 | 알림 서버 부재 시 표시 불가 |
| XIM 전용 | 데몬 fdo | 로그 | XIM 프런트 구독 불필요 — 데몬이 직접 보냄 |
| Windows 10/11 | popup-win 토스트(포커스 창 모니터 모서리) | 로그 | TSF DLL UI 금지; WinRT 토스트는 AUMID 등록 필요해 제외(Q4) |

---

## 5. 설정 키 (5지점 동기화)

`engine.notify` 섹션(Q7, 선례 `engine.auto_typefix`; `Config`(`src/config.rs:1075`)는 현재 `engine` 하나만 갖는다). YAML:

```yaml
engine:
  notify:
    enabled: true
    duration_ms: 2000        # 500~5000, 슬라이더(gtk::Scale + tick). GNOME 배너는 무시
    language: auto           # auto | ko | en
    show_text: false         # D1 — 교정 전후 텍스트·단어 표시
    corner: auto             # auto | top_right | bottom_right | top_left | bottom_left (Windows 전용)
    events:                  # 켤 이벤트 목록. 기본 = ON 7종
      - atf_corrected
      - atf_suppressed
      - blacklist_learned
      - password_enter
      - mode_toggle_suppressed
      - feature_toggled
      - feature_result
```
- 키 6개(`enabled`, `duration_ms`, `language`, `show_text`, `corner`, `events`). `events` 원소 9종 = §2.1 "설정명" 열(`password_leave`, `mode_changed` 는 기본 제외). 미지 원소는 무시. 리스트를 사용자가 한 번 저장하면 이후 추가될 새 이벤트는 자동으로 켜지지 않는다(의도된 보수적 동작).
- `NotifyConfig::clamp_ranges()`(`duration_ms` 500~5000) 를 `AutoTypeFixConfig::clamp_ranges`(`src/config.rs:535`) 와 같은 지점(`unim-dbus/src/service.rs:986~1029` set 경로, YAML 로드 경로)에서 호출.

| 지점 | 파일 | 할 일 |
|---|---|---|
| 설정 코어 | `src/config.rs` (`EngineConfig`, `toggle_announce_beep` `:1040`·기본값 `:1066` 패턴) | `NotifyConfig` + `#[serde(default)]` + `default_*` + `clamp_ranges` + 라운드트립 테스트(선례 `:1905`) |
| CLI | `unim-cli/src/main.rs` `ConfigKey`(kebab-case, 선례 `:636 toggle-announce-beep`, 리스트 키 `:633 word-mode-apps`·처리 `:1768`) | `notify-enabled`, `notify-duration-ms`, `notify-language`, `notify-show-text`, `notify-corner`, `notify-events`(쉼표 리스트) |
| CLI 로케일 | `unim-cli/locales/{ko,en}.yml` | `help_ck_notify_*` |
| 데몬 레거시 키 | `unim-dbus/src/service.rs:811`(get)·`:1104`(set) 디스패치 | `notify_enabled` … `notify_events`(snake_case) |
| GTK 설정 | `unim-settings-gtk/src/settings_dialog.rs` + `unim-gui-common/src/settings_helpers.rs` + `unim-settings-gtk/locales/{ko,en}.yml` | "알림" 그룹: 전역 스위치, `gtk::Scale`(duration, tick), 언어 콤보, `show_text` 스위치(경고 문구), 이벤트별 스위치 9개(리스트로 직렬화), corner 는 "(Windows)" 표기 |
| Slint 설정(크로스 플랫폼) | `unim-settings/src/main.rs` + `unim-settings/ui/settings.slint` (Linux `platform/linux.rs` 도 존재 — Windows 전용 아님) | 동일 항목 |
| 잔존 TSF 탭 | `unim-tsf/src/settings_dialog.rs` | 동일 항목(해당 탭이 살아 있는 동안) |
| 도움말 | `help/unim-help-{ko,en}.html`, `help/windows/unim-help-{ko,en}.html`, `docs/user/user-guide/README{,-ko}.md` | 알림 절·설정 표 |
| Slint 번역 | `unim-settings/translations/en/LC_MESSAGES/unim-settings.po` | 알림 그룹 문자열 en 번역(Slint 원문은 한국어) |
| DBus 규격 | `unim-dbus/SPEC.md` §5.4 키 표 | 6행 추가(E2 승인됨) |

GNOME gschema 에는 추가하지 않는다(GEMINI.md). 확장은 `ConfigChangedJson`(전체 config) 구독으로 `engine.notify` 를 읽어 `feature_result` 게이트에 쓴다(Q10). `show-notification` 은 유지하고 AND 조건(E1).
- **표시 시간 표기**: `duration_ms` 슬라이더 설명에 "GNOME 알림 배너에는 적용되지 않음(GNOME 은 셸 고정 시간)" 을 붙인다 — 데몬 fdo 경로·확장 P2 모두 GNOME 에서 무시(§3.2). KDE·Windows·GNOME P3 토스트에만 적용.

---

## 6. 단계별 구현 계획

| Phase | 내용 | 검증 |
|---|---|---|
| P0 | 본 문서 v2 확정(E1~E5 승인 완료), `unim-dbus/SPEC.md` §5.2·§5.4·§5.5(등록자 추적 포함)·§5.6 갱신, POPUP_SPEC 무변경 확인 | plan-reviewer PASS |
| P1 코어+데몬 | `InputEngine::last_toggle_blocked()` + `unim_engine_last_toggle_blocked`(+`unim.h`, 레이아웃 단언), `AtfOutcome`/`check_*_outcome`, `src/notify.rs`(E3), `engine.notify` 5지점, `engine_worker.rs` 발생 지점(§2.1)·`notify_tx` 채널, `service.rs` 알림 전담 태스크(signal/fdo 라우팅)·등록자 추적(`NameOwnerChanged`), `TriggerAction("notify_test")` | `cargo test`: gate 규칙 1·2·4(10분 재알림 경계값 599/600 s)·5·6(ATF 유발 `mode_changed` 생략), Password 상태 폐기, `show_text=false` 문구에 before/after 미포함, 문구 길이, `Suppressed(Blacklist)` 단언, `last_toggle_blocked` 단언(차단 키 뒤 true, 다음 키에서 false), `size_of::<InputResult>()==5` 컴파일 단언, 등록자 소멸 시 이름 제거; capi 예제 컴파일(새 함수 포함 — 전용 `make` 타깃은 없으므로 `examples/capi-c/README.md:25` 의 `gcc` 명령(`-I../../unim-capi/include -L../../target/release … -lunim`)을 그대로 실행하거나 CI 단계로 추가); L2: `gdbus monitor` 로 `Notify` 수신(`gnome-shell-notify` 등록 시), 등록 프로세스 강제 종료 후 `GetActiveFrontends` 에서 이름 사라짐·fdo 로 전환, `unim-cli config set notify-events atf_suppressed` 후 교정 미발행 |
| P2 Linux | 데몬 fdo 직접 경로 마감(hints·replaces_id·백오프), GNOME 확장 transient Source 브리지 + `gnome-shell-notify` 등록·재연결 재등록, `feature_result` AND 게이트(E1) | **L3**(`tests/harness/harness.py`): L3 에는 indicator 가 없고 D3 로 필요도 없다 → harness 가 세션 버스에 **mock `org.freedesktop.Notifications`**(python-dbus 20줄)를 띄워 호출 기록. 단언: ATF 교정 시나리오(`atf-forward-plain`) `Notify` 1회·중복 0·body 가 가림 문구와 정확히 일치; **비번 시나리오(`password-no-atf`, `tests/harness/scenarios/2bulstd.json:54`, b94da2a)** — GTK3/GTK4/Qt 는 corrected `Notify` 0건·`password_enter` 1건(같은 칸 재포커스 후에도 1건). **XIM 은 같은 시나리오가 `known_fail`(목적 전달 불가)이고, 이 시나리오는 한글 모드라 ATF 가 아니라 한글 조합이 일어난다 → XIM 은 알림 total 0건(`password_enter` 포함)을 단언하고, 가림 문구 정확 일치(입력 텍스트 미포함)는 XIM 의 `atf-forward-plain` 이 단언한다(v2.2, P2 L3 실측)**. 실기 gofu(GNOME Wayland): 확장 on → 확장 알림 1회·fdo 0회, 확장 off → GNOME 배너 1회·목록 잔류 0, 확장 프로세스 비정상 종료(셸 재시작) 후에도 fdo 로 표시 |
| P3 트레이 근처 토스트 | GNOME 확장 St 토스트 + `Atk.Role.NOTIFICATION` | 실기: 포커스 불변, 2초 자동 소멸, Orca 가 문구 읽음(대상 GNOME 버전에서 role 동작 확인) |
| P4 Windows | `cmd="toast"` 와이어 + 렌더러 토스트 창·dedupe·보류 TTL + D6 위치 + UIA, TSF outcome·`last_toggle_blocked()` 소비 | `make check-windows` 0경고; 와이어 골든/역호환 테스트; VM: 메모장 교정 토스트, 듀얼 모니터에서 포커스 창 쪽 표시, Edge 로그인 비번칸 입력값 미노출, Narrator 읽기 |
| P5 문서 | 사용자 매뉴얼 §4.1·설정 표·`show_text` 경고·`duration_ms` GNOME 미적용 표기·비번 알림 10분 한계(§2.2-4), CHANGELOG: ① **사용자 가시 변화** — 확장 TypeFIX 수동 변환 알림 `변환 완료: %s`(`extension.js:731~732`)·사전 등록 알림이 기본 가림으로 **입력 텍스트를 숨김**(`notify.show_text=true` 로 복원), ② C-API 함수 `unim_engine_last_toggle_blocked` 추가(하위 호환, soname 불변) | reviewer 문서 링크 검사 |

규모: 코어 3파일 수정(engine·auto_typefix·press_key)+1 신규(`notify.rs`), capi `lib.rs`·헤더 각 1(함수 1개), 데몬 2파일(+등록자 추적), 설정 5지점(+Slint·도움말), GNOME 확장 2파일, Windows 2컴포넌트+protocol 사본. 신규 프로세스 0, 다른 UNIM 프로세스 의존 0(Linux).

---

## 7. 위험

| 위험 | 대응 |
|---|---|
| 토스트가 키 응답을 지연 | 워커는 `try_send` 만, 발행·fdo 는 알림 전담 태스크(§3.2) — 현재 시그널들은 응답(`service.rs:2360`) 전 await 되므로 같은 자리에 넣으면 안 된다. 비프의 동기 블로킹 교훈(`unim-tsf/src/lang_bar.rs:525`) |
| GNOME 이중 표시(확장 + fdo) | 데몬 단일 라우팅(§3.2): `gnome-shell-notify` 등록 시 fdo 미호출. L3·실기에서 중복 0 |
| 확장 비정상 종료로 `gnome-shell-notify` 등록이 남아 알림 유실 | **설계로 확정**(E2): 등록자 unique name 기록 + `NameOwnerChanged` 정리 + 재연결 재등록(§3.2). 현재 코드(`service.rs:348, 506`)엔 셋 다 없음 |
| fdo 알림 서버 부재/거부 | 로그 1회 + 60초 백오프 |
| 같은 창 다른 사이트 비번칸 미알림 | E5 10분 재알림으로 완화, 한계 문서화(§2.2-4·P5) |
| 키 1회 이벤트 다중 | §2.2 규칙 6 우선순위 + 채널(자리 경쟁 없음) + 최신 1장 |
| 감지 못 한 비번칸에서 입력 일부 노출 | D1 기본 가림 + transient(D4) + 로그 미기록(§2.3). `show_text` 켤 때 경고 |
| 알림 피로 | 보수적 기본값, 전역 스위치, 3초 dedupe, 최신 1장, `password_enter` 칸당 1회(D5), 비번 사유 억제 알림 삭제 |
| C ABI 변경 | `InputResult`·`UnimInputResult` 무변경(E4), 함수 1개 추가만(하위 호환). `unim-common` 이 `libunim_capi.so.0`·`/usr/include/unim.h` 를 배포하므로 구조체 변경은 금지 — `size_of` 컴파일 단언으로 차단(§3.1a). drift guard 는 함수 목록만 비교 |
| 구버전 렌더러·확장 혼재 | 렌더러: 미지 cmd 무시 확인(§3.4). 확장: 구버전은 `gnome-shell-notify` 미등록 → fdo 폴백 |
| popup-win 토스트가 한자 팝업과 겹침 | 토스트는 모서리, 팝업은 커서 근처 — 겹치면 토스트가 숨음 |
| 5지점 누락 | §5 표를 plan-reviewer 체크리스트로 사용 + `unim-cli config get notify-*` L2 |

---

## 8. 검토 반영 기록

### 8.1 v1 (2026-10-01, opus 검증 REVISE 17건)

| # | 지적 | 판정 | 반영 |
|---|---|---|---|
| 1 | 한/영 차단은 코어 `press_key.rs:140` 로그뿐, `:1847` 은 비프 지점 | 수용 | `InputResult.toggle_blocked`(§3.1a; **v2 에서 E4 엔진 getter 로 대체**), §2.1 발생 지점 정정, "코어는 타입·게이트·문구만" 문구 삭제 |
| 2 | 블랙리스트는 코어 `None` 하나, "앱 제외" 없는 기능 | 수용 | `AtfOutcome`/`AtfSuppressReason`(§3.1b), 앱 제외 삭제 |
| 3 | `:908` 은 플래그만, 등록은 `:1523` Tentative | 수용 | 발생 지점 이동, 문구 "임시 제외 ({h}시간)" — 4시간은 설정값 `tentative_expiry_hours` 기본(`config.rs:406`) |
| 4 | 역방향 `original` 빈 값, 한자 교체도 같은 채널 | 수용(줄 정정) | 빈 값 근거는 `reverse.rs:86`·주석 `engine_worker.rs:1577`(지적의 `:1569~1572` 는 `UndoState` 생성부). `:1557` 블록에서만 생성, 역방향 before = `eng_to_kor(buf_ascii)` |
| 5 | 감지 불가 환경에서 비번 노출 | 수용 | D1 `show_text`(기본 false), transient, 경고(§2.3) |
| 6 | 시그널이 응답 전 await | 수용 | `tokio::spawn` 분리(§3.2, §7; **v2 에서 알림 전담 태스크+채널로 대체**) |
| 7 | indicator `NotShowIn=GNOME`, 등록 이름 `gnome-shell` | 수용 | D3 데몬 직접 fdo 로 해소, 별도 이름 `gnome-shell-notify` |
| 8 | XIM 모순 | 수용 | 비목표에서 indicator 언급 삭제, XIM 도 데몬 fdo |
| 9 | `password_enter` 소음 | 수용 | `apply_content_type` set 전 비교, 칸당 1회 집합(D5), ATF 꺼짐 문구 분기, 비번 사유 `atf_suppressed` 삭제 |
| 10 | CLI kebab-case·Q7 근거·키 수·매핑·clamp | 수용 | §5 전면 정정, `notify-events` 리스트 키, kind↔설정명 열, `clamp_ranges` |
| 11 | 동기화 누락 | 수용 | §5 표에 GTK 로케일·Slint(크로스 플랫폼)·도움말·SPEC §5.4·Windows 리로드 추가 |
| 12 | Q10 vs 메서드 없음 모순 | 수용 | Q10 확장 로컬 발행 유지(보고 메서드 없음), `%s` 는 `show_text` 로 가림 |
| 13 | Windows 와이어·구 렌더러·lazy·호스트별 게이트 | 부분 반박 | 구 렌더러 처리는 **확인 결과 안전**: 미지 cmd 는 `unim-popup-win/src/main.rs:267~268` 에서 무시, 단 `WIRE_VERSION` 상향 시 `pipe_server.rs:380~386` 에서 전부 폐기되므로 유지 필수. 나머지(규칙 준수·보류 TTL·렌더러 측 dedupe) 수용 |
| 14 | GJS announce 미확인, GTK4 4.14+ | 수용 | St `Atk.Role.NOTIFICATION`; GTK4 토스트(popup-service)는 v1 제외라 해당 없음 |
| 15 | `:2086` 은 SetGlobalMode 비프 | 수용 | `feature_toggled` 근거를 `:1938` 으로, `:2086` 은 `mode_changed` 로 이동 |
| 16 | 로그 평문 미기록 | 부분 반박(결론 수용) | "글자 수만" 은 ATF·한자 교체 채널 한정 방침(`service.rs:2240~2253`)이고 `ProcessKeyEvent` 로그(`:2350~2357`)는 preedit/commit 평문을 남긴다(본 기능 범위 밖 별건). Notify title/body 는 미기록 |
| 17 | L3 indicator 미기동 | 수용 | D3 로 indicator 불필요, harness mock fdo + 비번 시나리오 0건 단언(§6 P2) |

### 8.2 v2 (2026-10-01, opus 재검증 REVISE 경미 10건 + 결정 E1~E5)

| # | 지적 | 판정 | 반영 |
|---|---|---|---|
| 1 | `active_frontends` 이름만(`service.rs:348`), 등록자·`NameOwnerChanged` 없음(`:506`) | 수용(코드 확인) | 이름→등록자 맵, `#[zbus(header)]` sender 기록, `NameOwnerChanged` 정리, `_onDaemonReady` 재등록(§3.2·§3.3·§7, E2). 줄 정정: 재연결 등록 호출은 `extension.js:503`(지적의 `:505` 는 실패 경고 줄), 훅 시작 `:500` |
| 2 | 칸=context_id 라 브라우저 한 창 여러 사이트 미알림, FocusOut(`engine_worker.rs:2006`) purpose 미초기화 | 수용 | E5: 같은 context_id 1회 + 10분 재알림, `HashMap<u32, Instant>`, 한계 명시(§2.2-4). FocusOut 이 전이를 안 만드는 것은 "같은 칸 재포커스 1회" 에는 맞는 동작이라 유지 |
| 3 | `EngineResponse.notify: Option` 자리 1개, ATF 모드 전환(`:1615`) 동시 발생, SetGlobalMode(`:2086`) 경로 미정 | 수용(설계 변경) | 응답 필드 대신 워커→전담 태스크 **채널**(`try_send`)로 전환 — 자리 다툼·`apply_content_type` 반환형 변경이 모두 사라짐. 우선순위 §2.2-6(ATF 유발 `mode_changed` 생략). SetGlobalMode 는 응답 채널이 없으므로(`:2063`, `service.rs:491`) 같은 채널로, `context_id=0`(실 id 는 `service.rs:408` 에서 1부터) |
| 4 | L3 비번 시나리오 XIM 은 known_fail(b94da2a) | 수용(코드 확인 `2bulstd.json:59~60`) | XIM 은 `password-no-atf` 알림 0건, 가림 문구 일치는 `atf-forward-plain` 에서 단언(§6 P2, v2.2 정정) |
| 5 | 알림마다 `tokio::spawn` → `replaces_id`/`last_id` 경쟁 | 수용 | 전담 태스크 1개가 `last_id`·백오프 소유, 대기열 drain 후 최신 1건(§3.2) |
| 6 | `unim-common` 이 `.so.0`·`unim.h` 배포, drift guard 는 함수 목록만 | 수용(코드 확인 `debian/unim-common.install:6~8, 11`, `build.rs:77`) | E4: 구조체 무변경 + getter 함수 + `size_of` 컴파일 단언(§3.1a). v1 의 "트리 밖 C 소비자 없음" 은 패키징 관점에서 부정확해 삭제 |
| 7 | `:1589~1595` corrected 평문 로그 | 수용(줄 정정) | 실제 로그는 `engine_worker.rs:1595~1600`(형식 `:1597`, 값 `:1599`; 지적 범위는 `recent_corrections` 기록부). 재트리거 키 평문 로그 `:1541~1546` 도 함께 별건 명기(§2.3) |
| 8 | Slint 번역 누락, GNOME P2 `duration_ms` 무시, Lang 순서 | 수용 | §5 표에 `unim-settings/translations/en/LC_MESSAGES/unim-settings.po`, 슬라이더 GNOME 미적용 표기, `LC_ALL > LC_MESSAGES > LANG`(선례 `detect_locale` 의 역순은 따르지 않음) |
| 9 | 줄번호 `press_key.rs:148`, `service.rs:2360` | 수용(확인) | 차단 분기 `:140~149`·로그 `:143`·반환 `:148`, 응답 반환 `:2360` 로 정정 |
| 10 | 기본 가림으로 확장 `변환 완료: %s` 가 텍스트 숨김 | 수용 | P5 CHANGELOG "사용자 가시 변화" 항목 |

반박: 없음(지적 1·7 은 결론 수용, 인용 줄만 정정).

### 8.3 v2.1 (2026-10-01, opus 재검증 PASS · LOW 7건)

| # | 지적 | 판정 | 반영 |
|---|---|---|---|
| 1 | §2.1 `mode_changed`: SetGlobalMode 를 알림 채널에 "직접" 넣으면 게이트(규칙 1·5) 우회. `:74` 는 `service.rs:74` | 수용 | 워커 안(`:2063`)이므로 `NotifyGate::offer` 를 거친 뒤 채널로(§2.1), 줄 출처 `service.rs:74` 명시 |
| 2 | §2.2 규칙 6 이 5 보다 앞 | 수용 | 5 → 6 순서 교정 |
| 3 | §3.2 vs §3.1c: `offer_batch` 호출 시점 불명 | 수용 | ProcessKey 중 `Vec` 수집 → `:1847` 부근 `offer_batch` 1회, 조기 반환 경로·단건 이벤트 규칙 명시(§3.2) |
| 4 | `password_enter` 를 `events` 에서 빼면 맵 미갱신 → `password_leave` 영영 불발 | 수용 | 전이 기록은 규칙 5 와 무관(§2.2-4) |
| 5 | §6 P1 "make 로 capi 예제 컴파일": 해당 타깃 없음 | 수용(확인) | `examples/capi-c/README.md:25` 의 `gcc` 명령 또는 CI 단계(§6 P1) |
| 6 | NameOwnerChanged 문구 부정확, 셸 생존·확장 고장 시 미정리 | 수용 | `name` `:` 접두 + `new_owner` 빔, 버스 연결 공유 한계, "브리지가 켜져 있다" 정의(§3.2) |
| 7 | header 인자 때문에 기존 테스트 직접 호출 곤란 | 수용 | `(name, sender)` 순수 헬퍼 분리 + 얇은 zbus 래퍼(§3.2) |

---

## 9. 이력
| 날짜 | 버전 | 내용 |
|---|---|---|
| 2026-10-01 | v0-draft | 초안. 결정 대기 Q1~Q10 |
| 2026-10-01 | v1-draft | opus 검증 17건 반영, 사용자 결정 D1~D6·Q1~Q10 확정. 코어 API(`InputResult.toggle_blocked`, `AtfOutcome`) 시그니처 설계, 데몬 직접 fdo 경로(D3), 설정 `engine.notify` 6키·리스트 키. 미결 Q11(gschema)·Q12(SPEC.md) |
| 2026-10-01 | v2-draft | opus 재검증 10건(REVISE 경미)·결정 E1~E5 반영: 등록자 추적+`NameOwnerChanged`, 비번 알림 10분 재알림, 워커→알림 전담 태스크 채널(우선순위 규칙 6), C ABI 무변경(getter `unim_engine_last_toggle_blocked`), L3 XIM 단언, Lang POSIX 순서, CHANGELOG 가시 변화. 미결 없음 |
| 2026-10-01 | v2.1-draft | opus 재검증(PASS) LOW 7건 반영: SetGlobalMode 게이트 경유, 규칙 5·6 순서, ProcessKey `offer_batch` 1회, 전이 기록과 `events` 필터 분리, capi 예제 컴파일 수단 정정, NameOwnerChanged 문구·한계, 등록 헬퍼 분리. P0 SPEC.md §5.2·§5.4·§5.5·§5.6 동시 갱신 |
| 2026-10-02 | v2.2-draft | P2 구현 실측 반영: `password_enter`/`leave` 발화 조건을 포커스된 컨텍스트 기준 전이로 정정(GTK 의 포커스 전 목적 선전송 오발 방지), L3 XIM `password-no-atf` 단언을 알림 0건으로 정정 |
| 2026-10-02 | v2.3-draft | P4 Windows 구현 반영: 와이어 `ToastPayload`(§3.4)는 설계서 `popup-renderer-design.md` §12 에 동결, TSF 순수 로직 `unim-tsf/src/toast_bridge.rs`, 렌더러 `toast.rs`·`toast_logic.rs`. VM 검증 항목은 §6 P4 행 그대로 |
