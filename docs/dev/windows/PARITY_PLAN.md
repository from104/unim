# UNIM Windows 포팅 — Linux 기능 동등화 계획

> **구현 상태 (2026-05-28, feat/windows-msi-redesign, 미커밋)**: **TSF 완전 네이티브 아키텍처**로 전환 완료.
> `unim-windows` 크레이트 **완전 제거**(workspace/디렉토리/MSI/Makefile/CI). 모든 UI 가 `unim_tsf.dll` 내부:
> - config reload(OnSetFocus+mtime)·팝업(한자/특수/이모지, 9x9·북마크·페이징)·AutoTypeFix(자동/수동·blacklist·undo) — 유지.
> - **네이티브 Win32 설정 다이얼로그**(`settings_dialog.rs`, 슬라이더/체크박스/콤보).
> - **ITfFunctionProvider+ITfFnConfigure**(`fn_configure.rs`) — Windows 키보드 "옵션" 버튼 진입점.
> - **랭귀지바 버튼**(`lang_bar.rs` 등록) — 한/영 상태 동기 + 메뉴(한영 전환/기본 입력기/설정 열기), 엔진 양방향 토글.
>
> 공유 크레이트(코어/capi) 수정 0건. Linux 회귀 0, Windows gnu sanity PASS. **런타임은 Windows VM 검증 대기**
> (msvc CI 그린, 랭귀지바·옵션버튼·설정창·팝업·AutoTypeFix 실앱, MSI 설치).
>
> 이하 본문의 Phase 1~2(unim-windows 트레이/egui 설정)는 **폐기된 이전 설계**다 — 네이티브 전환으로 대체됨.

> 목표: Windows 포팅(`unim_tsf.dll` 시스템 입력기 + `unim-windows.exe` 상주 컨트롤패널)을
> Linux 버전과 기능 동등하게 만든다. 최종 검증은 VM에서 실제 타이핑으로 한다
> (Linux cross-compile 은 sanity 전용).

## 아키텍처 분담 (확정)

| 역할 | 산출물 | 컨텍스트 |
|---|---|---|
| 시스템 전역 입력 (모든 앱) | `unim_tsf.dll` | OS 로드, SYSTEM 설치 |
| 상주 트레이 + 설정 UI + 기본입력기 토글 | `unim-windows.exe` | 로그인 사용자 |
| 설정 저장소 (공유) | `%APPDATA%\unim\config.yaml` | `Config::load/save_to_default_path` |

핵심: 두 프로세스는 daemon/DBus 없이 **config.yaml 파일을 공유**한다. unim-windows 가 쓰고
TSF DLL 이 읽는다. Linux 의 DBus `ConfigChanged` 전파가 없으므로 **TSF 측 config reload** 가 필요.

## 기능 격차 매핑

| 기능 | Linux | unim-windows.exe | unim_tsf.dll |
|---|---|---|---|
| 한글 조합/커밋 | ✅ | ✅ | ✅ |
| 한/영 전환 | ✅ | ✅(엔진) | ✅ (preserved-key 버그 수정 완료) |
| 자판 선택(두벌/세벌390·391·순아래, QWERTY/Dvorak/Colemak/Workman) | ✅ | ✅(메뉴) | ✅(엔진+config) |
| 한자 변환 팝업 | ✅ | ✅(자체 창) | ✅ 구현됨(런타임 검증 대기) — `popup_ipc.rs` 1323줄 |
| 특수문자 팝업 | ✅ | ✅ | ✅ 구현됨(런타임 검증 대기) — `popup_ipc.rs` 1323줄 |
| 이모지 팝업 (Super+.) | ✅ | 부분 | ✅ 구현됨(런타임 검증 대기) — `popup_ipc.rs` 1323줄 |
| 한자 북마크(★) / 9x9 확장 격자 | ✅(일부 FE) | 부분 | ✅ 구현됨(런타임 검증 대기) — `popup_ipc.rs` 1323줄 |
| AutoTypeFix 자동 (순/역방향) | ✅ | ✅(자체 창) | ✅ 구현됨(런타임 검증 대기) — `auto_typefix.rs` 464줄 |
| AutoTypeFix 수동 (Ctrl+Shift+Space) | ✅ | ✅ | ✅ 구현됨(런타임 검증 대기) — `auto_typefix.rs` 464줄 |
| 트레이/인디케이터 메뉴 | ✅ | ❌ 스텁 | n/a |
| 설정 GUI (전체 config) | ✅(gtk/qt) | 부분(자판+기본입력기만) | n/a |
| 설정 변경 전파/reload | ✅(DBus) | 저장만 | ✅ 구현됨(런타임 검증 대기) — `maybe_reload_config` 존재 |
| **32-bit 앱 입력 (KakaoTalk 등)** | ✅ | n/a | ✅ **i686 `unim_tsf.dll` 32-bit TSF 등록으로 지원**(카톡 한글 입력 실증, SOLVED 2026-06-22) |

> **앱 아키텍처 커버리지 (2026-06-22 SOLVED):** 64-bit 앱(Edge/Chrome/메모장/wezterm)은 x64
> `unim_tsf.dll`로, **32-bit 앱(KakaoTalk·한컴 등)은 i686 `unim_tsf.dll`을 빌드해 32-bit COM/TSF로
> 양면 등록**해 커버한다. IMM32 `.ime` 갈래는 헛다리로 폐기. 근거·구현: **[imm32-win11-SOLUTION.md](imm32-win11-SOLUTION.md)**.
> (현재 32-bit 등록은 수동 regsvr32로 실증된 상태 — MSI 영구 배선은 잔여 작업.)

엔진 API 는 공유다 (`press_key`→`InputResult`, `take_popup_action`→`PopupAction`,
`popup_change_page`, `typefix_convert`, `commit_str`/`preedit_str`). unim-windows 가 이미
이 API 로 팝업·autotypefix 를 자체 창에 구현했으므로 **그 코드가 TSF 포팅의 참조 템플릿**이다.

## Phase 구성

### Phase 1 — unim-windows 트레이 상주앱화  *(독립, 가장 빠른 가치)*
- `tray.rs` 스텁 → `tray-icon` 0.19 실제 구현: 한/영 상태 아이콘 + 컨텍스트 메뉴.
- 메뉴: `설정 열기` / `한/영 전환` / `기본 입력기로 설정`(토글) / `종료`.
- eframe(winit) 이벤트 루프 + tray-icon 메뉴 이벤트 통합, 창 표시/숨김(close=tray로 최소화).
- 아이콘 리소스(data/icons) 재사용.

### Phase 2 — 전체 설정 UI + config reload  *(Phase 1 위)*
- unim-windows 설정 화면: AutoTypeFix(순/역 토글, 임계값 슬라이더 — 슬라이더 정책 준수),
  toggle_keys / hanja_keys, auto_english, 자판을 전부 노출. `save_to_default_path` 연동.
- TSF reload: `OnSetFocus`/주기적 mtime 체크로 config.yaml 변경 시 엔진 재생성.

### Phase 3 — TSF 팝업 (한자 / 특수문자 / 이모지)
- `key_handler` 에서 `engine.take_popup_action()` 처리 (현재 무시).
- 렌더: (A) `ITfCandidateListUIElement`(candidate_ui.rs 연결) 또는
  (B) 자체 layered popup window. Linux 팝업 UX(페이징·★북마크·9x9 격자) 매핑.
- `popup_change_page`, 키 매핑(Space/Period 등) 연결.

### Phase 4 — TSF AutoTypeFix  *(가장 난도 높음)*
- `key_handler` 에서 자동(press_key 내부 결과) + 수동(Ctrl+Shift+Space) 트리거.
- surrounding text 삭제(N자 backward) + 교체 삽입을 TSF `ITfRange`/composition 으로 구현.
- 참조: unim-windows `check_auto_typefix`/`handle_manual_typefix`, 엔진 `typefix_convert`.

### Phase 5 — 통합 검증 / 문서
- phase 별 `cargo check`(gnu) + `RUSTFLAGS=-Dwarnings`, CI(msvc) 통과.
- VM 스모크 테스트 체크리스트(SMOKE_TEST.md 확장): 메모장/브라우저 한글·한자·오타교정.

## 의존 관계 / 권장 순서
1. **Phase 1** (트레이) — 독립, 사용자가 가장 먼저 요청, 즉시 체감.
2. **Phase 2** (설정+reload) — Phase 3·4 효과를 VM 에서 켜고 끄며 확인하려면 필요.
3. **Phase 3** (팝업) → **Phase 4** (autotypefix) — TSF 문서 조작 난도 순.
4. **Phase 5** — 각 phase 종료 시 부분 수행 + 마지막 종합.

## 제약
- 런타임 동작(타이핑·팝업·교정)은 Linux 에서 검증 불가 → 각 phase 후 VM/CI 확인 필수.
- 배포 MSI 는 GitHub Actions(MSVC+WiX)에서만. gnu 빌드는 컴파일 sanity 전용.
