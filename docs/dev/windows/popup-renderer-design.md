# UNIM Windows — 팝업 렌더러 프로세스 분리 설계서 (unim-popup-win)

> 작성 2026-06-12, branch `feat/windows-msi-redesign`. 코드 변경 0건 — 설계 문서.
> 선행 조사: `docs/dev/windows/_archive/popup-tsf-fix-plan.md` (가설 17건 중 16 confirmed).
> 기준 스펙: `docs/dev/specs/POPUP_SPEC.md` (Linux 공통 규격, v3.3).
> 본 문서는 **렌더러 구현자(A)** 와 **TSF 클라이언트 구현자(B)** 가 서로의 파일을
> 건드리지 않고 동시 작업할 수 있도록 인터페이스를 **동결(frozen)** 한다.

---

## 0. 요약 — 무엇을, 왜

in-proc HWND 팝업(`unim-tsf/src/popup_window.rs`, 649줄)을 폐기하고
**별도 렌더러 프로세스 `unim-popup-win.exe`** 로 분리한다 (Mozc/Weasel 노선,
Linux `unim-popup-service` 와 아키텍처 대칭).

사용자 확정 정책:
1. **팝업 위치 = 무조건 화면 중앙** — 포그라운드 창이 있는 모니터의 작업영역(rcWork) 중앙.
   캐럿 추적(GetTextExt/LayoutSink) 전면 불필요. Linux v3.3 "popup 위치 화면 정중앙 고정
   (모든 frontend 공통)" 정책과 일치.
2. **디버그 로그 최대화** — DLL 측 `register::dbg_log`(이미 `UNIM_DEBUG_LOG=true`),
   렌더러 측 `%TEMP%\unim-popup-win.log`.
3. **코어(src/)·Linux 크레이트 변경 금지** — 변경 범위는 `unim-tsf/*`, 신규
   `unim-popup-win/*`, 루트 `Cargo.toml` members 1줄, `installer/wix/unim.wxs` 뿐.

### 0.1 이 설계로 소멸하는 가설 (fix-plan 대응표)

| 가설 | 소멸 방식 |
| --- | --- |
| H1 캐럿 좌표 실패 | 캐럿 추적 자체를 폐기 — 화면 중앙 고정. GetSelection/GetTextExt 호출 0건 |
| H3 격자 전치 | 와이어 페이로드가 엔진 SoT `PopupViewModel`(= column-major 확정 셀 배열)이라 렌더러는 전치 자체가 불가능 (§2) |
| H4 하이라이트 이중 오류 | `selected`(=sel_row) 필드 폐기. 렌더러는 `(row,col) == (sel_row,sel_col)` 좌표 비교만 사용 (§5.4) |
| H5 첫 표시 3×3 | 첫 `render` 메시지부터 rows/cols/total_pages 가 엔진 `update_page_layout()` 결과 그대로 포함 — Default 격자 개념 자체가 없음 |
| H8 이모지 탭 소실·점프 | view_model 의 tab_labels/active_tab_index 그대로 전송 + 위치는 항상 중앙(점프 불가) |
| H9 뜻풀이/헤더/footer 미렌더 | header_text/footer_text/meaning 이 daemon-SoT 포맷으로 페이로드에 포함, 렌더러 의무 렌더 |
| H10 PostQuitMessage | unim-tsf 에서 창 코드 전부 제거(popup_window.rs 삭제). 렌더러는 자체 프로세스라 호스트 오염 불가 (§8 금지 규칙) |
| H11 클램프/플립 | 중앙 배치는 정의상 화면 안 — 클램프/플립 불필요 (초대형 팝업만 rcWork 에 클램프) |
| H13 DPI | 렌더러 프로세스가 자체적으로 Per-Monitor V2 aware — 호스트 awareness 혼재 문제 원천 차단 (§5.5) |
| H14 초기 북마크 ★ | 엔진이 트리거 시점에 `set_bookmark_flags()` 를 이미 호출(candidates.rs:63) → view_model 의 `is_bookmarked` 가 첫 렌더부터 정확. Linux 의 비동기 fetch 보다 우월 |
| H2 (UWP 표시 차단) | 부분 해소 — 렌더러가 medium-IL 데스크톱 프로세스라 AppContainer/UIPI 제약 없이 표시 가능. (UILess 강제 호스트의 ITfCandidateListUIElement 데이터 경로는 별도 과제 — §10.3) |
| H15 마우스 / H17 ThreadFocusSink / H6 reset / H7 키 통과 | 본 분리의 범위 밖 — §10.3 잔여 과제로 명시 |

---

## 1. 아키텍처 개요

```
호스트 앱 프로세스 (메모장/Chrome/UWP/게임…)
┌──────────────────────────────────────────────┐
│ unim_tsf.dll (TSF TIP, STA)                  │
│  OnKeyDown → engine.press_key()              │
│   → drain_popup_actions()                    │      JSON line / named pipe
│   → engine.popup_state().view_model(...)     │  \\.\pipe\unim-popup-win.<sid>
│   → PopupClient (popup_ipc.rs)  ─────────────┼──────────────┐
│      └ worker thread (비차단 큐, 재연결,      │              │
│        on-demand spawn, 로그)                 │              ▼
└──────────────────────────────────────────────┘   ┌──────────────────────────┐
                                                   │ unim-popup-win.exe        │
   Linux 대칭:                                     │ (싱글턴, HKLM Run 자동시작)│
   engine→daemon(PopupRender DBus)→popup-service   │  pipe server thread(들)   │
                                                   │  UI thread: GDI 팝업 창   │
                                                   │  화면 중앙 배치, PMv2 DPI │
                                                   │  %TEMP%\unim-popup-win.log│
                                                   └──────────────────────────┘
```

핵심 결정: **TSF 가 raw `PopupAction` 을 재조립하지 않는다.** Linux daemon 이
`PopupRender` 시그널을 만들 때 쓰는 것과 동일한 엔진 공개 API —
`engine.popup_state()` (`src/input_engine/popup_dispatch.rs:244`) +
`PopupState::view_model(home_row)` (`src/popup/view_model.rs:82`) — 를 호출해
**완성된 view model 을 그대로 직렬화**한다. 헤더/푸터/셀/탭/북마크/격자 차원이
전부 엔진 SoT 산출이므로 Linux 와 표시 일관성이 구조적으로 보장된다.
코어 변경 0 — 기존 공개 API 소비만 한다.

---

## 2. 데이터 계약 — unim-popup-types 재사용 검토 결과

| 선택지 | 판정 | 근거 |
| --- | --- | --- |
| `unim-popup-types::PopupRenderPayload` 를 와이어 타입으로 직접 사용 | **기각** | serde derive 없음(추가 = Linux 크레이트 Cargo.toml/코드 변경 → 금지 위반). zbus 지향 튜플 표현. |
| unim-popup-types 에 cfg 없이 serde 타입 추가 | **기각** | 동일 — Linux 크레이트 변경 금지. serde 의존이 Linux 전 소비자 빌드 그래프에 전파됨. |
| **와이어 타입을 본 설계서에 동결하고 양 크레이트에 동일 사본 정의** | **채택** | A/B 완전 병렬(컴파일 의존 0, 파일 충돌 0). JSON 이 계약이고 타입은 구현 디테일. `PopupRenderPayload` 와 필드 1:1 대응(의미 동일)이라 추후 Linux 와 통합 여지 보존. |

> 후속(통합 단계, 선택): unim-popup-win 에 `[lib]` 타깃을 추가해 wire 모듈을 export
> 하고 unim-tsf 가 의존하도록 단일화할 수 있다. V1 에서는 하지 않는다.

cell flags 는 `unim_popup_types::popup_render_flags` 와 **비트 동일**:
`HAS_DATA=0x01, SELECTED=0x02, COL_HIGHLIGHT=0x04, ROW_HIGHLIGHT=0x08, BOOKMARKED=0x10`.

---

## 3. IPC 프로토콜 (동결 — 양측 독립 구현의 단일 근거)

### 3.1 전송 계층

- **명명 파이프**: `\\.\pipe\unim-popup-win.<session_id>`
  - `<session_id>` = `ProcessIdToSessionId(GetCurrentProcessId())` 10진수.
    양측이 각자 자기 세션 id 로 계산 → 같은 세션이면 같은 이름. 다중 세션/빠른 사용자
    전환에서 충돌 없음.
- **서버** = 렌더러. `CreateNamedPipeW(PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, …)`.
  여러 호스트 프로세스(각자 DLL 인스턴스)가 **동시에** 연결하므로 인스턴스 무제한 +
  연결당 reader 스레드.
- **보안 기술자 (필수)**: `SECURITY_ATTRIBUTES.lpSecurityDescriptor` 에
  `ConvertStringSecurityDescriptorToSecurityDescriptorW` 로 만든 SDDL:

  ```text
  D:(A;;GRGW;;;WD)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)
  ```

  - `(A;;GRGW;;;WD)` Everyone 읽기/쓰기
  - `(A;;GRGW;;;AC)` **ALL APPLICATION PACKAGES (S-1-15-2-1)** 읽기/쓰기 — UWP/AppContainer
    안의 DLL 이 연결 가능 (요구사항).
  - `S:(ML;;NW;;;LW)` 무결성 레이블 Low — 브라우저 렌더러 샌드박스 등 Low-IL 클라이언트의
    쓰기 허용 (Mozc 동일 정책).
  - 렌더러는 기동 시 적용된 SDDL 문자열을 로그에 남긴다.
- **인코딩**: UTF-8 JSON, **한 줄 = 한 메시지**, `\n`(0x0A) 종결. 메시지 내 개행 없음
  (serde_json 기본 직렬화가 보장). 최대 라인 1 MiB — 초과 라인은 렌더러가 버리고 로그.
- 클라이언트 쓰기는 fire-and-forget. V1 에서 클라이언트는 파이프를 **읽지 않는다**
  (응답 대기로 인한 블로킹 가능성 원천 제거). `pong`/`evt` 프레임은 진단 CLI 용 예약.

### 3.2 메시지 스키마 (클라이언트 → 렌더러)

공통 envelope 필드: `v`(=1), `cmd`, `pid`(클라이언트 프로세스 id), `seq`(클라이언트별 단조 증가 u64).

| cmd | 추가 필드 | 의미 |
| --- | --- | --- |
| `"render"` | `first:bool`, `flash:bool`, `owner_hwnd:u64`, `render:RenderState` | 팝업 표시/갱신. `first=true` 면 Show*(새 팝업), false 면 갱신. 렌더러 동작은 동일(전체 상태 교체 + 중앙 배치 + 표시) — first 는 로그/소유권 판단용. `flash=true` 면 선택 셀에 140ms `#f9e2af` flash (북마크 해제 신호). `owner_hwnd` = 호스트 포그라운드 창(모니터 선정용, 0 허용). |
| `"hide"` | — | 팝업 숨김. **현재 owner pid 가 보낸 경우에만** 숨김 (§3.4). |
| `"ping"` | — | 생존 확인 (진단용). 렌더러는 같은 연결로 `{"v":1,"evt":"pong","seq":N}\n` 회신. |
| `"shutdown"` | — | 렌더러 정상 종료 (업그레이드/제거/진단용). 모든 클라이언트가 보낼 수 있음. |

`RenderState` — `PopupViewModel`/`PopupRenderPayload` 의 평면 표현 (필드명 동결):

```json
{
  "kind": 0,                         // 0=Hanja, 1=SpecialChar, 2=Emoji
  "target": "한",
  "header_text": "「한」 → 한자",     // 엔진 SoT 포맷 그대로
  "footer_text": "1/3",
  "show_footer": true,               // 한자=항상 true(엔진이 보장), 특수/이모지=total_pages>1
  "rows": 9, "cols": 1,
  "sel_row": 0, "sel_col": 0,
  "current_page": 0, "total_pages": 3,
  "cells": [ {"t":"韓","m":"한나라 한","f":31}, ... ],
  "col_headers": [["Q",false],["W",false], ...],   // (text, is_active); 한자 compact = []
  "row_headers": [["1.",true],["2.",false], ...],
  "expand_visible": true, "expand_text": "⊞",
  "tab_labels": ["최근 (a)","스마일 (s)", ...],     // 이모지만 9개, 그 외 []
  "active_tab_index": 0
}
```

- **`cells` 는 column-major 평면 배열, 길이 = rows*cols**:
  `cells[col * rows + row]` 가 (row, col) 셀. `f` 비트는 §2 의 flags.
  `HAS_DATA=0` 이면 빈 셀(`t`/`m` 무시, 빈 문자열로 직렬화).
- `m`(meaning) 은 한자 전용, 그 외 빈 문자열.
- 좌표/치수는 일절 전송하지 않는다 — 위치는 렌더러가 중앙 배치로 전담.

### 3.3 Rust 와이어 타입 (양 크레이트에 **동일 사본** — 복붙 원본)

```rust
//! popup IPC wire 타입 — docs/dev/windows/popup-renderer-design.md §3 동결.
//! unim-tsf/src/popup_ipc.rs 와 unim-popup-win/src/protocol.rs 양쪽에 동일 정의.
//! 필드 추가는 반드시 #[serde(default)] (하위호환) + 설계서 갱신 + v 유지.
use serde::{Deserialize, Serialize};

pub const WIRE_VERSION: u32 = 1;
pub const PIPE_BASE_NAME: &str = r"\\.\pipe\unim-popup-win"; // + "." + session_id
pub const PIPE_SDDL: &str = "D:(A;;GRGW;;;WD)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)";
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const FLASH_MS: u32 = 140;

pub mod cell_flags {
    pub const HAS_DATA: u32 = 0x01;
    pub const SELECTED: u32 = 0x02;
    pub const COL_HIGHLIGHT: u32 = 0x04;
    pub const ROW_HIGHLIGHT: u32 = 0x08;
    pub const BOOKMARKED: u32 = 0x10;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCell {
    pub t: String, // text
    pub m: String, // meaning (한자 외 "")
    pub f: u32,    // cell_flags 비트합
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderState {
    pub kind: u32,
    pub target: String,
    pub header_text: String,
    pub footer_text: String,
    pub show_footer: bool,
    pub rows: u32,
    pub cols: u32,
    pub sel_row: u32,
    pub sel_col: u32,
    pub current_page: u32,
    pub total_pages: u32,
    /// column-major: cells[col * rows + row], len == rows*cols
    pub cells: Vec<WireCell>,
    pub col_headers: Vec<(String, bool)>,
    pub row_headers: Vec<(String, bool)>,
    pub expand_visible: bool,
    pub expand_text: String,
    pub tab_labels: Vec<String>,
    pub active_tab_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMsg {
    pub v: u32,
    /// "render" | "hide" | "ping" | "shutdown"
    pub cmd: String,
    pub pid: u32,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_hwnd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderState>,
}
```

렌더러 파싱 규약: 알 수 없는 `cmd`/필드 → 무시 + 로그(에러 아님). `v != 1` → 무시 + 로그.
파싱 실패 라인 → 버리고 로그 (연결 유지).

### 3.4 다중 클라이언트 소유권 규칙 (포커스 전환 race 방지)

여러 호스트 프로세스가 동시에 연결된다. 렌더러는 `current_owner: Option<u32>` (pid) 를 유지:

1. `render` 수신 → `current_owner = Some(pid)`, 상태 전체 교체, 표시.
2. `hide` 수신 → `pid == current_owner` 일 때만 숨기고 `current_owner = None`.
   다른 pid 의 hide 는 무시 + 로그 (`stale hide from pid=…`).
   → "새 앱이 Show 한 직후 옛 앱의 OnSetFocus Hide 가 늦게 도착해 팝업이 사라지는" race 차단.
3. owner 연결 단절(broken pipe) → 즉시 숨김 + `current_owner=None` + 로그
   (호스트 크래시/종료 시 팝업 잔존 방지 — H17 계열 보강).

### 3.5 프로토콜 변경 절차

필드 추가는 `#[serde(default)]` 옵셔널로만 (v 유지). 의미 변경/제거는 `v` 증가 +
본 설계서 §3 갱신 + 양측 사본 동시 수정. **설계서 갱신 없는 와이어 변경 금지.**

---

## 4. PopupViewModel → RenderState 변환 (TSF 클라이언트 측, 동결)

Linux `unim-dbus` 의 `build_render_state` 와 동일 평탄화 규칙. `popup_ipc.rs` 에 구현:

```rust
fn to_render_state(vm: &unim::popup::PopupViewModel) -> RenderState {
    use cell_flags::*;
    let rows = vm.cells.len();                       // = vm 의 행 수
    let cols = vm.cells.first().map_or(0, |r| r.len()); // = vm 의 열 수
    let mut cells = Vec::with_capacity(rows * cols);
    for c in 0..cols {
        for r in 0..rows {
            match vm.cells[r][c].as_ref() {
                Some(cd) => {
                    let mut f = HAS_DATA;
                    if cd.is_selected { f |= SELECTED; }
                    if cd.is_col_highlight { f |= COL_HIGHLIGHT; }
                    if cd.is_row_highlight { f |= ROW_HIGHLIGHT; }
                    if cd.is_bookmarked { f |= BOOKMARKED; }
                    cells.push(WireCell {
                        t: cd.text.clone(),
                        m: cd.meaning.clone().unwrap_or_default(),
                        f,
                    });
                }
                None => cells.push(WireCell { t: String::new(), m: String::new(), f: 0 }),
            }
        }
    }
    RenderState {
        kind: vm.kind as u32, // PopupKind: Hanja=0, SpecialChar=1, Emoji=2 (POPUP_SPEC §2.5)
        target: vm.target.clone(),
        header_text: vm.header_text.clone(),
        footer_text: vm.footer_text.clone(),
        show_footer: vm.show_footer,
        rows: rows as u32,
        cols: cols as u32,
        sel_row: vm.sel_row as u32,
        sel_col: vm.sel_col as u32,
        current_page: vm.current_page as u32,
        total_pages: vm.total_pages as u32,
        cells,
        col_headers: vm.col_headers.iter().cloned()
            .zip(vm.col_header_active.iter().copied()).collect(),
        row_headers: vm.row_headers.iter().cloned()
            .zip(vm.row_header_active.iter().copied()).collect(),
        expand_visible: vm.expand_visible,
        expand_text: vm.expand_text.clone(),
        tab_labels: vm.tab_labels.clone(),
        active_tab_index: vm.active_tab_index as u32,
    }
}
```

주의 — vm.cells 차원과 rows/cols:
- 한자 compact: `vm.cells.len() = page_item_count (≤9)`, 각 행 1열, `col_headers = []`.
- special/emoji/한자 expanded: `vm.cells.len() = 9` (rows 고정 정책), 열 수 =
  `state.cols()` (1..9 가변). `col_headers` 는 항상 9개 (popup 폭 고정 정책) —
  **렌더러의 격자 폭은 col_headers 9개 기준, 셀 데이터는 cols 열만 존재**할 수 있음.
  cols < 9 인 열 범위는 빈 셀로 렌더 (Linux gtk_ui 동일).
- `PopupKind` 의 `as u32` 캐스팅 값은 0/1/2 (POPUP_SPEC §2.5 kind 정의와 일치).
  enum repr 미지정이므로 구현 시 match 로 명시 변환 권장:
  `Hanja→0, SpecialChar→1, Emoji→2`.

---

## 5. unim-popup-win 렌더러 설계 (구현자 A 소유)

### 5.1 크레이트

```
unim-popup-win/
├── Cargo.toml
└── src/
    ├── main.rs        # 엔트리: 싱글턴, DPI, 로그, 파이프 서버 spawn, 메시지 루프
    ├── protocol.rs    # §3.3 와이어 타입 사본 (동결)
    ├── pipe_server.rs # CreateNamedPipe 루프, 연결당 reader 스레드, owner 규칙
    ├── window.rs      # 팝업 HWND 생성/배치/표시, WM_* 처리
    ├── render.rs      # GDI 렌더 (레이아웃·색·폰트·flash)
    └── logging.rs     # %TEMP%\unim-popup-win.log
```

`Cargo.toml` (bin 전용, 콘솔창 없음):

```toml
[package]
name = "unim-popup-win"
version.workspace = true
edition = "2021"
description = "UNIM Korean IME - out-of-process popup renderer (Windows)"
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[target.'cfg(windows)'.dependencies]
windows-core = "0.62"
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_HiDpi",
    "Win32_System_Pipes",
    "Win32_System_Threading",
    "Win32_System_SystemInformation",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_Storage_FileSystem",
    "Win32_System_LibraryLoader",
    "Win32_System_RemoteDesktop",
] }
```

- `main.rs` 상단 `#![windows_subsystem = "windows"]` — 콘솔 플래시 방지.
- 루트 `Cargo.toml` workspace members 에 `"unim-popup-win"` 1줄 추가 (A 소유 변경).
- 빌드: `cargo build -p unim-popup-win --target x86_64-pc-windows-msvc` zero-warning.

### 5.2 프로세스 수명주기

| 항목 | 결정 |
| --- | --- |
| 기동 | (a) 로그인 자동시작: MSI 가 **HKLM Run** 키 등록 (§7) (b) 폴백: DLL 클라이언트의 on-demand spawn (§6.4) |
| 싱글턴 | 명명 뮤텍스 `Local\unim-popup-win-singleton` (세션별 네임스페이스 → 세션당 1개). `CreateMutexW` 후 `ERROR_ALREADY_EXISTS` 면 "already running" 로그 후 즉시 exit 0 |
| 유휴 종료 | **없음 — 상주** (Mozc renderer 동일). 메모리 풋프린트 수 MB, 첫 팝업 표시 지연 제거. 종료 경로는 `shutdown` cmd, `WM_ENDSESSION`, 세션 로그오프 뿐 |
| DPI | 기동 직후 `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` (실패 시 로그 후 계속) |
| 스레드 모델 | UI 스레드(메시지 루프 + HWND 소유) / 파이프 accept 스레드 / 연결당 reader 스레드. reader → `Arc<Mutex<VecDeque<WireMsg>>>` push 후 `PostMessageW(msg_hwnd, WM_APP_CMD, 0, 0)` 로 UI 스레드 wake. **reader 스레드에서 HWND 직접 조작 금지** |
| 인자 | `--ping`(파이프로 ping 보내고 pong 확인 후 exit code 출력, 진단), `--shutdown`(기존 인스턴스 종료), 무인자=서버 기동 |

### 5.3 창 속성

- 클래스 `UNIM_PopupRendererWnd`, 스타일 `WS_POPUP | WS_BORDER`,
  확장 `WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED`.
- `SetLayeredWindowAttributes(alpha=255)` (기존과 동일, 추후 반투명 여지).
- `WM_MOUSEACTIVATE → MA_NOACTIVATE` 반환 — 클릭이 와도 포커스 절대 안 가져감.
- `ShowWindow(SW_SHOWNOACTIVATE)` / `SetWindowPos(... SWP_NOACTIVATE)` 만 사용.
- **V1 마우스 상호작용 없음** — 클릭은 로그만 남기고 무시 (§10.3 Phase 2).
- 창은 프로세스 기동 시 1회 생성(숨김), show/hide 만 반복 — 매번 Create/Destroy 안 함.

### 5.4 렌더링 규칙 (parity 의무 — POPUP_SPEC §3~§5 준수)

레이아웃 모드 판정 (동결): `render.col_headers.is_empty()` → **한자 compact 리스트**,
그 외 → **격자 모드** (특수/이모지/한자 expanded).

공통:
- **헤더 바**: `header_text` 그대로 (한자 expanded 는 엔진이 선택 한자+뜻을 헤더에 넣어줌 —
  렌더러 추가 가공 금지).
- **푸터 바**: `show_footer==true` 일 때 `footer_text` 중앙 표기. `expand_visible==true`
  면 우측 끝에 `expand_text`(⊞/⊟) 표기 (V1 표시만, 클릭 없음).
- **하이라이트 (H3/H4 재발 금지 핵심)**:
  - 셀 인덱싱은 **반드시** `cells[col * rows + row]` (column-major) 로만 접근.
    `i / cols`, `i % cols` 류 row-major 환산 **금지**.
  - 선택 셀 = `(row, col) == (sel_row, sel_col)` **좌표 비교가 1차 기준**.
    `f & SELECTED` 와 불일치하면 `parity-mismatch` 경고 로그 (디버그 신호, 렌더는 좌표 기준).
  - 행/열 레이블 active = `row_headers[r].1` / `col_headers[c].1` 그대로.
  - 행/열 보조 강조 = `f & ROW_HIGHLIGHT` / `f & COL_HIGHLIGHT` (연한 배경).
- **북마크 ★**: `f & BOOKMARKED` → 셀 텍스트 우측 ★ (compact 리스트도 동일).
- **flash**: envelope `flash==true` → 선택 셀 배경 `#f9e2af`, `SetTimer(FLASH_MS=140)`
  후 만료 시 재도색. (기존 popup_window.rs 의 GetTickCount64 + WM_TIMER 패턴 포팅.)
- **빈 셀** (`f & HAS_DATA == 0`): 연회색, 텍스트 없음.

한자 compact 리스트 (rows = N≤9, cols=1):
- 행 포맷: `[row_header] [한자(+★)] [meaning]` — `m` 을 보조색으로 같은 행에 렌더 (H9 해소).
- 행 폭 = max(GetTextExtentPoint32 측정치) 기반, 논리 280px 최소 / 560px 최대 클램프.

격자 모드:
- 좌측 행 레이블 열(1~9) + 상단 열 레이블 행(col_headers **9개 전부** — popup 폭 고정 정책).
- 이모지(`tab_labels` 비어있지 않음): 좌측에 9개 세로 탭 컬럼 추가, `active_tab_index`
  강조 (Linux GridLayout col 0 attach 와 동일 배치 철학).
- 한자 expanded: 셀에 한자(+★)만, 뜻은 헤더가 담당 (view_model 정책 그대로).

색상 (Catppuccin Mocha, POPUP_SPEC §5.1 — COLORREF 는 0x00BBGGRR 주의):

| 용도 | HEX(RGB) | COLORREF |
| --- | --- | --- |
| 배경 Base | #1e1e2e | 0x002e1e1e |
| 헤더 배경 Surface0 | #313244 | 0x00443231 |
| 본문 Text | #cdd6f4 | 0x00f4d6cd |
| 뜻풀이 Subtext0 | #a6adc8 | 0x00c8ada6 |
| 행/열 레이블 Overlay1 | #7f849c | 0x009c847f |
| 한자 선택 배경 | #89b4fa 계열 | 0x00fab489 |
| 특수/이모지 선택 배경 | #a6e3a1 계열 | 0x00a1e3a6 |
| flash Yellow | #f9e2af | 0x00afe2f9 |
| 활성 열 헤더 Green | #a6e3a1 | 0x00a1e3a6 |
| 비활성 열 헤더 Yellow | #f9e2af | 0x00afe2f9 |

폰트: "맑은 고딕", 논리 px — 헤더 13 bold / 셀 16 / 한자 18 bold / 뜻 12 / 레이블 11 bold
(DPI 스케일 적용, §5.5).

### 5.5 위치·DPI (동결 알고리즘)

매 `render` 처리 시:

```text
1. hwnd_owner = owner_hwnd != 0 && IsWindow(owner_hwnd) ? owner_hwnd : GetForegroundWindow()
2. hmon = MonitorFromWindow(hwnd_owner, MONITOR_DEFAULTTONEAREST)
   (hwnd_owner 무효 시 MonitorFromPoint((0,0), MONITOR_DEFAULTTOPRIMARY))
3. GetMonitorInfoW(hmon).rcWork = 작업영역 (작업표시줄 제외)
4. dpi = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI) (실패 시 96); scale = dpi / 96.0
5. (w, h) = 콘텐츠 측정치 × scale; w = min(w, rcWork.width), h = min(h, rcWork.height)
6. x = rcWork.left + (rcWork.width − w) / 2
   y = rcWork.top + (rcWork.height − h) / 2
7. SetWindowPos(HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW | SWP_NOACTIVATE)
8. 로그: owner_hwnd, hmon rcWork, dpi, (w,h), (x,y)
```

- 모든 레이아웃 상수(CELL_W/H, LABEL, MARGIN, 폰트 높이)는 논리값 × scale 정수 반올림.
- `WM_DPICHANGED` 수신 시 마지막 RenderState 로 재측정·재배치 (창이 보이는 동안 모니터
  DPI 변경 대응).
- 캐럿/마우스 좌표는 어떤 경로로도 사용하지 않는다 (`GetCursorPos` 호출 금지 — H1 폴백 잔재 금지).

### 5.6 렌더러 로그 (`%TEMP%\unim-popup-win.log`)

형식: `[unim-popup-win pid=N tid=N 2026-06-12T12:34:56.789] message` (ms 정밀도, append).
필수 기록 (전부):

- 기동/종료: 인자, 버전, 싱글턴 결과, DPI awareness 결과, 종료 사유(shutdown cmd/ENDSESSION).
- 파이프: 생성(이름·SDDL), 클라이언트 연결/해제(원격 pid — `GetNamedPipeClientProcessId`),
  broken pipe, 인스턴스 수.
- 수신 커맨드 요약(라인 원문 말고 요약 — 1줄): `cmd, pid, seq, first, flash, kind,
  rows×cols, page cur/total, sel(row,col), cells=N, owner_hwnd`.
- 파싱 실패/버전 불일치/스키마 경고/`parity-mismatch`(§5.4)/stale hide(§3.4) 전부.
- 창: show/hide/recenter(§5.5 8번 항목)/WM_PAINT(상태 해시 또는 seq)/WM_DPICHANGED/flash 시작·종료.
- 모든 Win32 실패는 `GetLastError()` 포함.

로그 회전: 기동 시 5 MiB 초과면 `.old` 로 rename 후 새로 시작.

---

## 6. unim-tsf 클라이언트 설계 (구현자 B 소유)

### 6.1 모듈 교체

- `unim-tsf/src/popup_window.rs` **삭제** (§8.1).
- 신규 `unim-tsf/src/popup_ipc.rs` — §3.3 와이어 타입 사본 + `PopupClient`.
- `lib.rs`: `mod popup_window;` → `mod popup_ipc;`.
- `unim-tsf/Cargo.toml`: `serde = { version = "1.0", features = ["derive"] }`,
  `serde_json = "1.0"` 추가. windows features 에 `"Win32_System_Pipes"`,
  `"Win32_Storage_FileSystem"`, `"Win32_System_Threading"`,
  `"Win32_System_RemoteDesktop"`(세션 id 불필요 — `ProcessIdToSessionId` 는
  Win32_System_Threading 에 있음, 확인 후 최소셋만) 추가.

### 6.2 PopupClient API (동결 — text_service/key_handler 가 보는 표면)

```rust
pub struct PopupClient { /* SyncSender<WireMsg>, active: bool, seq: u64, ... */ }

impl PopupClient {
    /// worker 스레드 기동. 파이프 연결은 lazy (첫 send 시).
    pub fn new() -> Self;
    /// 팝업 표시 중 여부 (로컬 미러 — test_key_down 의 popup_active 게이트용).
    pub fn is_active(&self) -> bool;
    /// Show*/갱신 — view model 전송. first=Show* 액션 직후 여부, flash=★해제 신호.
    pub fn send_render(&mut self, rs: RenderState, first: bool, flash: bool);
    /// HidePopup/포커스 전환/리로드 — Hide 전송 + active=false. 이미 비활성이면 no-op.
    pub fn hide(&mut self);
}
```

- **비차단 보장**: `std::sync::mpsc::sync_channel(8)` + `try_send`. 큐 가득 차면
  메시지 버리고 `dbg_log("popup_ipc: queue full, dropped cmd=…")`. IME 스레드에서
  파이프/프로세스 API 호출 0건 — 전부 worker 스레드.
- `owner_hwnd`: `send_render` 내부에서 `GetForegroundWindow()` (호스트 프로세스 기준,
  비차단·즉시). 실패 시 0.
- worker 는 마지막 render WireMsg 를 캐시 — 재연결 성공 시 `active` 상태면 재전송
  (렌더러 재시작/크래시 복원).

### 6.3 worker 스레드 — 연결·전송 상태기계

```text
recv cmd from channel →
  ensure_pipe():
    if handle.is_none():
      CreateFileW("\\.\pipe\unim-popup-win.<sid>", GENERIC_WRITE, ...)
      ERROR_FILE_NOT_FOUND / PIPE_BUSY →
        maybe_spawn_renderer()        # §6.4, 5초 rate-limit
        WaitNamedPipeW(300ms) → CreateFileW 재시도 1회
      실패 → cmd drop + dbg_log(원인·GetLastError) → return
  WriteFile(json + "\n"):
    ERROR_BROKEN_PIPE/ERROR_NO_DATA → close, ensure_pipe() 1회 재시도 → 재실패 시 drop+log
모든 단계 dbg_log: connect 성공/실패, spawn 시도/결과, write byte 수, drop 사유
```

- 어떤 실패도 panic 금지(`panic=abort` + COM 경계) — 전부 로그 후 무시.
  **팝업이 안 떠도 타이핑·조합은 절대 영향 없음** (최우선 불변식).

### 6.4 on-demand spawn 폴백

HKLM Run 자동시작이 없거나(개발 환경) 렌더러가 죽은 경우:

1. **AppContainer 면 spawn 생략**: `GetTokenInformation(TokenIsAppContainer)` true →
   spawn 불가 환경. 로그만 남기고 연결 재시도에 의존 (Run 키로 떠 있는 인스턴스 기대).
2. exe 탐색 순서: ① DLL 모듈 경로 기준 — `GetModuleFileNameW(dll_instance)` 의 디렉터리
   + `unim-popup-win.exe` (MSI 가 동일 INSTALLDIR 에 설치, `launch_settings_app` 과
   동일 패턴) ② `HKLM\SOFTWARE\atit.org\UNIM\InstallDir` 값 (§7) ③ 둘 다 실패 → 로그.
3. `CreateProcessW(exe, ..., CREATE_NO_WINDOW | DETACHED_PROCESS)` — 상속 핸들 없음.
   렌더러 싱글턴 뮤텍스가 중복 기동을 자체 차단하므로 spawn race 무해.
4. rate-limit: 마지막 시도 후 5초 이내 재시도 금지 (스폰 폭주 방지).

### 6.5 drain_popup_actions 재설계 (key_handler.rs)

```rust
fn drain_popup_actions(engine: &mut InputEngine, popup: &mut PopupClient) {
    let mut first = false;   // Show* 수신
    let mut hide = false;    // HidePopup 수신
    let mut flash = false;   // ★ 해제 신호
    let mut any = false;
    while let Some(action) = engine.take_popup_action() {
        any = true;
        match &action {
            PopupAction::ShowHanja{..} | PopupAction::ShowSpecial{..}
            | PopupAction::ShowEmoji{..} => first = true,
            PopupAction::HidePopup => hide = true,
            PopupAction::HanjaCandidatesReordered { bookmarked, was_bookmarked, .. } =>
                flash = *was_bookmarked && !*bookmarked,
            _ => {} // Navigate/PageJump/BookmarkChanged — view_model 이 전부 반영
        }
        crate::register::dbg_log(&format!("popup_ipc: action={:?}-variant", ...));
    }
    if hide { popup.hide(); return; }
    if !any { return; }
    // 엔진 SoT 에서 완성된 view model 추출 (H3/H4/H5/H9/H14 일괄 해소 지점)
    let home_row = engine.home_row_labels().to_string();
    if let Some(state) = engine.popup_state() {
        let rs = to_render_state(&state.view_model(&home_row)); // §4
        popup.send_render(rs, first, flash);
    } else if popup.is_active() {
        popup.hide(); // 액션은 있었는데 상태가 없음 — 방어적 Hide + 로그
    }
}
```

- **호출 지점 유지**: `handle_key_down` 의 기존 drain 위치 (press_key 직후, commit 처리 전).
- `get_composition_screen_pos()` 함수와 그 호출(팝업 경로) 제거 — **단, preedit
  오버레이(key_handler.rs:367)는 동일 함수를 계속 사용하므로 함수 자체는 존치**
  (preedit_window.rs 불가침 원칙).
- `PopupAction` 의 모든 variant 는 와일드카드 없이 명시 match (신규 variant 추가 시
  컴파일 에러로 감지) 하되 동작은 위 4분기.

### 6.6 text_service.rs 치환 지점

| 위치 | 현재 | 변경 |
| --- | --- | --- |
| 필드 (text_service.rs:46) | `popup_window: Mutex<Option<PopupWindow>>` | `popup_ipc: Mutex<PopupClient>` (생성자에서 `PopupClient::new()`) |
| OnTestKeyDown (:341) | `popup_window…is_active()` | `self.popup_ipc.lock().unwrap().is_active()` |
| OnKeyDown (:421, :445) | `&mut popup_win: &mut Option<PopupWindow>` 전달 | `&mut PopupClient` 전달 (handle_key_down 시그니처 변경) |
| handle_key_down ATF 게이트 (key_handler.rs:430) | `popup_win…is_active()` | `popup.is_active()` |
| OnSetFocus (:580) | `win.hide()` | `popup.hide()` — **Hide 송신** (요구사항) |
| maybe_reload_config (:184) | `win.hide()` | `popup.hide()` |
| Deactivate/종료 경로 | (Drop 이 DestroyWindow) | `popup.hide()` 송신만. 렌더러 프로세스는 건드리지 않음 |

락 순서 규약 유지: engine → config → composition → **popup_ipc** → atf
(기존 popup 슬롯과 동일 순번).

### 6.7 DLL 측 로그 (dbg_log — 이미 `UNIM_DEBUG_LOG=true`)

필수 기록: PopupClient 생성, 모든 send(cmd·seq·kind·rows×cols·page·sel·first·flash·
owner_hwnd), hide 송신, is_active 전이, 큐 drop, worker 의 connect/재연결/spawn
시도·결과·GetLastError, 캐시 재전송. 접두사 `popup_ipc:` 통일 (로그 grep 용이).

---

## 7. 설치·수명주기 통합 (installer/wix/unim.wxs)

`InstallScope="perMachine"` 이므로 자동시작은 **HKLM Run** 을 사용한다
(HKCU Run 은 per-machine MSI 에서 "설치 실행한 사용자"에게만 적용되는 함정 —
모든 사용자 로그인 시 기동이 요구 취지이므로 HKLM 이 정답. 동작은 동일: 로그인 시 자동시작).

추가 사항 (모두 신규 — 기존 Component 불변):

```xml
<!-- INSTALLDIR 하위에 추가 -->
<Component Id="UnimPopupWinExe" Guid="(신규 GUID 채번)" Win64="yes">
  <File Id="unim_popup_win_exe" Name="unim-popup-win.exe"
        Source="$(var.WIN_OUT_DIR)\unim-popup-win.exe" KeyPath="yes" />
  <!-- 로그인 자동시작 (모든 사용자) -->
  <RegistryValue Root="HKLM" Key="SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
                 Name="UnimPopupRenderer"
                 Type="string" Value="&quot;[INSTALLDIR]unim-popup-win.exe&quot;" />
  <!-- spawn 폴백용 설치 경로 (§6.4 ②) -->
  <RegistryValue Root="HKLM" Key="SOFTWARE\atit.org\UNIM"
                 Name="InstallDir" Type="string" Value="[INSTALLDIR]" />
</Component>
```

- `<Feature>` 에 `<ComponentRef Id="UnimPopupWinExe" />` 추가.
- **업그레이드 시 실행 중 exe 처리**: WixUtilExtension 의
  `<util:CloseApplication Id="CloseUnimPopupWin" Target="unim-popup-win.exe"
  CloseMessage="no" TerminateProcess="0" RebootPrompt="no" />` — light.exe 에
  `-ext WixUtilExtension` 추가 (build-msi.bat 수정 동반). 렌더러는 창을 안 띄우는
  프로세스라 CloseMessage 가 안 먹으므로 TerminateProcess 강제 종료가 정답
  (상태 없음 — 다음 render 가 전체 복원, §6.2 캐시 재전송).
- **설치 직후 즉시 기동** (재로그인 불필요):
  `<CustomAction Id="LaunchPopupRenderer" FileKey="unim_popup_win_exe"
  ExeCommand="" Return="asyncNoWait" />` + `<InstallExecuteSequence>` 에서
  `After="InstallFinalize"`, 조건 `NOT Installed OR REINSTALL` (impersonated 기본 —
  사용자 컨텍스트로 실행).
- `installer/wix/generated/guids.wxi` 는 건드리지 않음 (CLSID 전용). 신규 Component
  GUID 는 wxs 에 리터럴 채번 (기존 RegisterScripts 등과 동일 방식).
- 버전 범프: DLL 교체 규칙(메모리: 버전범프/직접복사)에 따라 workspace 버전 1패치 상향 권장.

---

## 8. 금지 규칙·잔재 처리

### 8.1 popup_window.rs — **삭제** (deprecated 잔존 아님)

- zero-warning 정책상 미사용 모듈은 dead_code 경고를 유발하고, "완성돼 보이는 부실
  코드"가 재참조될 위험이 있다. **B 가 popup_ipc.rs 가 동작하는 같은 PR 에서
  `git rm unim-tsf/src/popup_window.rs` + `lib.rs` 의 `mod popup_window;` 제거.**
  GDI 그리기 참고가 필요하면 git 히스토리로 충분 (렌더러 A 는 본 설계서 §5.4 가 SoT —
  구 코드의 row-major 루프를 포팅하지 말 것).
- `preedit_window.rs` 는 **불가침** — 한 줄도 수정 금지. `get_composition_screen_pos`
  는 preedit 가 쓰므로 존치 (§6.5).

### 8.2 PostQuitMessage 류 금지 (재발 방지 규칙 — 리뷰 게이트)

- **unim-tsf 내부에서 `PostQuitMessage` 호출 전면 금지** (grep 0건이 머지 조건).
  in-proc 창(preedit_window)의 WM_DESTROY 는 `LRESULT(0)` 반환만.
- unim-popup-win 에서도 `PostQuitMessage` 는 **shutdown 처리 핸들러 단 한 곳**에서만
  (cmd `shutdown` / `WM_ENDSESSION`). 팝업 창 WM_DESTROY 에서 호출 금지 — 창
  재생성 경로가 생겨도 프로세스가 죽지 않도록.
- unim-tsf 에서 `GetCursorPos` 기반 팝업 위치 폴백 금지 (H1 잔재). 렌더러에서도 금지 (§5.5).
- unim-tsf 에서 `CreateWindow*`(팝업 용도) 신규 작성 금지 — 팝업 표면은 렌더러 단독 소유.
- IME 스레드(OnKeyDown/OnSetFocus 스택)에서 `CreateFileW`(파이프)/`WaitNamedPipeW`/
  `CreateProcessW` 직접 호출 금지 — worker 스레드 경유만 (§6.3).

---

## 9. 작업 분해 — 파일 소유권 (동시 작업 무충돌)

| 구현자 | 소유 파일 (이 외 수정 금지) | 산출물 |
| --- | --- | --- |
| **A (렌더러)** | `unim-popup-win/**` (신규), 루트 `Cargo.toml` **members 배열에 `"unim-popup-win"` 1줄 추가만** | §5 전체. `cargo build -p unim-popup-win` zero-warning. 수동 테스트: `--ping`, 가짜 클라이언트 스크립트(파이프에 §3.2 JSON 직접 write)로 3종 팝업 렌더 확인 |
| **B (TSF 클라이언트)** | `unim-tsf/src/popup_ipc.rs` (신규), `unim-tsf/src/key_handler.rs`, `unim-tsf/src/text_service.rs`, `unim-tsf/src/lib.rs`, `unim-tsf/Cargo.toml`, `unim-tsf/src/popup_window.rs` (삭제) | §6 전체. `cargo build -p unim-tsf --target x86_64-pc-windows-msvc` zero-warning. 렌더러 없이도 빌드·타이핑 무영향 확인 |
| **통합 담당** | `installer/wix/unim.wxs`, `installer/wix/build-msi.bat`(또는 동급 빌드 스크립트), 버전 범프, 통합 검증 | §7 + §10 |

- 와이어 타입(§3.3)은 양측이 각자 사본으로 작성 — 컴파일 의존 없음. **JSON 키 이름·
  의미를 임의 변경 금지** (§3.5).
- 순서 의존 없음: A/B 는 본 설계서만으로 독립 완주 가능. 통합 담당은 A·B 머지 후 착수.
- B 의 `mod popup_ipc;` 추가로 lib.rs 1파일이 겹칠 수 있는 유일 지점이나 lib.rs 는 B
  단독 소유로 못박는다 (A 는 unim-tsf 를 일절 만지지 않음).

---

## 10. 검증 계획

### 10.1 자동 (cargo test)

- **A**: `protocol.rs` 직렬화 스냅숏 테스트 — §3.2 예시 JSON 과 byte-equal.
  column-major 접근 헬퍼(`cell_at(rs,row,col)`) 단위 테스트: 20개 아이템 rows=9/cols=3
  케이스에서 `cell_at(0,1).t == items[9]` (popup_layout.rs 의 special_global_index 와
  교차 일치 — fix-plan §6.3 자동화 항목 이행).
- **B**: `to_render_state` 단위 테스트 — `PopupState::new_special("ㄱ", 20개, "QWERTYUIO")`
  → rows=9, cols=3, cells.len()=27, `cells[9].t=="S9"`(col1 row0), `(0,0)` SELECTED,
  빈 셀 f==0. 한자 compact 2후보 → col_headers 빈 배열, meaning 전달.
  이모지 → tab_labels 9개. (view_model 테스트와 동일 픽스처 재사용.)
- 직렬화 골든 라인 1개를 **양 크레이트 테스트에 동일 문자열로 박아** 사본 드리프트를
  컴파일 타임은 아니어도 테스트 타임에 검출.

### 10.2 수동 (fix-plan §6 매트릭스 준용, 위치 항목 대체)

각 앱(메모장/WordPad/Chrome/wezterm/Telegram/Windows 검색창/Store 앱/풀스크린 게임 1종) ×
한자 compact / 한자 9×9 / 특수문자 / 이모지:

1. **위치**: 항상 포그라운드 창 모니터의 **정중앙** (마우스를 다른 모니터에 두고 — H1 회귀 감지).
   멀티모니터 100%+150% 혼합에서 크기 비율 동일.
2. **첫 표시**: 트리거 직후 격자·페이지가 즉시 올바름 (3×3 회귀 감지, H5).
3. **선택 일치**: 화살표 이동 → 셀 하이라이트=레이블 하이라이트 일치, Enter/숫자/Q~O 커밋
   문자 = 화면 문자. **2페이지 이상에서 반복** (H4 소실 회귀). `parity-mismatch` 로그 0건.
4. **내용**: 한자 뜻풀이(compact 행/expanded 헤더)·헤더·푸터(한자 1/1 상시), 이모지 9탭+활성
   강조, **첫 렌더에 기존 즐겨찾기 ★** (H14).
5. **북마크**: Space ON 승격·커서 추종 / OFF 강등·140ms #f9e2af flash / 페이지 점프.
6. **수명주기**: 팝업 연 채 문서 전환/타 앱 클릭 → Hide 도달(렌더러 로그 owner 규칙 동작),
   호스트 강제 종료 → 렌더러가 broken-pipe 감지로 자동 숨김. IME 전환(한영/Win+Space) →
   **호스트 앱 생존** (H10). 렌더러 프로세스 kill → 다음 팝업 트리거에서 spawn 폴백으로 복구
   (로그로 spawn 확인). 로그인 직후 Run 키 자동 기동 확인.
7. **UWP/AppContainer**: Store 앱·검색창에서 팝업 **표시됨** (파이프 AC ACE 동작 — 핵심
   회귀 항목). AppContainer 내 spawn 시도 0건(로그).
8. **비차단**: 렌더러 미기동 + spawn 실패 환경에서 타이핑 지연 체감 0 / unim-tsf.log 에
   drop 로그만.
9. **로그**: unim-tsf.log 의 `popup_ipc:` 송신열과 unim-popup-win.log 수신열의 seq 연속 대조.

### 10.3 범위 밖 — 후속 과제 (본 분리로 해결되지 않음을 명시)

- **H6** engine.reset() 팝업 미정리 / **H7** 팝업 중 비팝업 키 desync — 엔진·key_handler
  과제 (fix-plan P0-5/P0-6). 분리와 직교.
- **H2 완전 해소** — UILess 강제 호스트(TF_TMAE_UIELEMENTENABLEDONLY)용
  ITfCandidateListUIElement 데이터 경로 (fix-plan P1-1). 렌더러 분리로 표시 자체는 대개
  되지만 규약상 BeginUIElement 3-phase 는 별도 구현 필요.
- **마우스 상호작용** (H15) — 프로토콜에 렌더러→클라이언트 `evt` 프레임이 예약돼 있음.
  Phase 2 에서 `{"evt":"select","row":r,"col":c}` 등 정의 + DLL 측 수신 마샬링 설계.
- ThreadFocusSink (H17) — 렌더러 owner-단절 숨김이 부분 보완하나 同문서 内 포커스 이동은 추후.

---

## 11. 마우스 입력 + 컬러 이모지 + 폭 (Phase 2 — 동결)

> 작성 2026-06-15. V1(§5.3 "마우스 상호작용 없음", §10.3 "마우스 H15 잔여")을 대체한다.
> 코어 `src/` 무수정 — 엔진은 이미 마우스 API 보유(`handle_click`/`popup_change_page`/
> `refresh_emoji_category_items`/`cancel_emoji_popup`/`toggle_hanja_expanded` 경유
> `popup_state_mut`). 본 절은 **렌더러(A)** 와 **TSF 클라이언트(B)** 가 추가하는
> 인터페이스를 동결한다. 기존 §3 정방향(파이프는 **이미 `PIPE_ACCESS_DUPLEX`**,
> pipe_server.rs:19/109) 위에 역방향(렌더러→TSF)을 같은 연결로 얹는다.

사용자 4 요구 → 본 절 매핑: (1)→§11.A 컬러 이모지, (2)→§11.B 폭, (3)→§11.C/D/F
클릭 동작, (4)→§11.E 외부 클릭 취소+패스쓰루.

### 11.A 컬러 이모지 렌더링 (구현자 A — render.rs)

**문제**: 현 `draw_text`(render.rs:88) 가 GDI `DrawTextW` 로 그려 Segoe UI Emoji 가
흑백 글리프(COLR/CPAL 무시)로 나온다.

**결정 — Direct2D + DirectWrite 부분 도입 (이모지 셀만)**. 기존 GDI 텍스트(헤더·
한자·라벨·뜻·푸터)는 **그대로 유지**하고, **이모지 격자 셀 텍스트만** D2D 로 그린다.
근거:
- 전면 D2D 재작성은 GDI `FillRect`/색·DPI 측정 로직(검증됨)을 버리게 되어 회귀 위험·
  공수 과다. 컬러 글리프가 필요한 곳은 이모지 셀 1종뿐(한자/특수문자/라벨은 흑백 OK).
- `ID2D1DCRenderTarget` 은 **임의 HDC 에 BindDC** 가능 → §window.rs:241 의 더블버퍼
  메모리 HDC(`mem`)에 그대로 바인딩. GDI 가 그린 위에 D2D 가 덧그리고, 같은 HDC 라
  BitBlt 한 번으로 합성된다(별도 레이어/합성 불필요).
- `IDWriteTextLayout` + `ID2D1DeviceContext4::DrawTextLayout` (또는
  `ID2D1RenderTarget::DrawText`) 는 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT` 로
  COLR/CPAL/CBDT 컬러 글리프를 자동 래스터(DWRITE_GLYPH_IMAGE_FORMATS 자동 선택).

**구현 동결**:
1. `Cargo.toml` windows features 추가: `"Win32_Graphics_Direct2D"`,
   `"Win32_Graphics_Direct2D_Common"`, `"Win32_Graphics_DirectWrite"`,
   `"Win32_Graphics_Dxgi_Common"`(D2D1_PIXEL_FORMAT/DXGI_FORMAT 용). Direct3D11 **불필요**
   (DCRenderTarget 은 D3D 디바이스 없이 GDI HDC 위에서 동작).
2. 프로세스 1회 생성 후 재사용(thread_local, UI 스레드 전용):
   `ID2D1Factory`(`D2D1CreateFactory(SINGLE_THREADED)`),
   `IDWriteFactory`(`DWriteCreateFactory(SHARED)`),
   `ID2D1DCRenderTarget`(`CreateDCRenderTarget`, pixelFormat =
   `{DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE}`, dpiX/Y = 96 — 우리는 이미
   scale 을 곱한 픽셀 rect 를 넘기므로 RT DPI 는 96 고정이 단순),
   `IDWriteTextFormat`(맑은 고딕 + 이모지 폴백; 폰트 패밀리는 "Segoe UI Emoji" 를
   1순위로 두되 폴백 체인이 자동 처리). 생성 실패(어느 하나라도 Err) 시 `None` 캐시 →
   **GDI `draw_text` 로 폴백**(흑백이라도 표시는 됨, 패닉/공백 금지).
3. 렌더 시퀀스(paint_grid 의 이모지 셀): GDI 단계에서 배경 `FillRect`(선택/빈셀/하이라이트)
   까지는 그대로. 셀 **텍스트만** D2D 로:
   `dc_rt.BindDC(hdc, &full_rect)` (paint 진입 시 1회) → `BeginDraw` →
   각 이모지 셀에 `DrawText`(셀 rect, ENABLE_COLOR_FONT, 중앙정렬) → `EndDraw`.
   `EndDraw` 가 `D2DERR_RECREATE_TARGET` 반환 시 RT 폐기·재생성 1회.
   주의: BindDC~EndDraw 구간에는 **같은 HDC 에 GDI 그리기 금지**(드라이버 정의되지 않음) →
   GDI(배경·라벨·한자) 를 **먼저 전부** 그린 뒤 마지막에 D2D 이모지 텍스트를 덧그린다.
   `is_compact`(한자)·특수문자 셀은 D2D 경로 안 탐(GDI 유지) — `kind==2`(Emoji) 격자
   셀에만 D2D 적용.
4. **셀 측정 일관성**: 폭은 §11.B 의 고정 격자(`CELL_W`)라 텍스트 측정에 의존하지 않는다
   (이모지 1글자가 셀보다 작음). 따라서 이모지 측정용 DirectWrite 경로는 **불필요** —
   `measure_grid` 는 상수 기반 그대로. compact(한자)만 GDI `text_width` 측정 유지.
   (가변 측정이 필요해지면 그때 `IDWriteTextLayout::GetMetrics` 도입, V2 범위 밖.)
5. DPI: DCRenderTarget DPI=96 고정 + 우리가 scale 곱한 px rect 전달 → 기존 GDI 와 동일
   스케일. `WM_DPICHANGED` 재측정 경로(§5.5) 영향 없음(RT 는 BindDC 시 rect 만 바뀜).

### 11.B 팝업 폭 축소 (구현자 A — render.rs measure_grid)

**원인 특정** (render.rs:150-163, 비-compact 경로):
`grid_w = ROW_LABEL_W(22) + GRID_COLS_FIXED(9) * CELL_W(54) = 22 + 486 = 508` 논리 px.
`w = PAD*2(16) + tab_w + grid_w` → 특수/한자확장 = **524px**, 이모지 = `+TAB_W(88)` =
**612px**. "약간 넓다"의 주범은 **`CELL_W=54`** (단일 CJK/이모지 글리프 폭 ~18-26px 대비
과대) 와 이모지 **`TAB_W=88`**. compact(한자 기본)는 `measure_compact` 의 텍스트 측정
기반이라 **무관 — 건드리지 않는다**(요구 (2) 명시).

**결정 (상수만 축소, 공식 불변)**:
- `CELL_W: 54 → 44` (render.rs:36). 9열 폭 = 396, grid_w = 418 → 비-compact 폭 **434px**
  (524→434, 약 17% 축소). 셀 텍스트는 중앙정렬이라 좁혀도 잘림 없음(글리프 < 44px).
- `TAB_W: 88 → 76` (render.rs:40). 이모지 탭 라벨("최근 (a)" 등 한글 ~6자)이 76px 에
  들어가는지 확인 후 확정 — 들어가지 않으면 80 으로. 이모지 폭 612→510px.
- `CELL_H(34)`·`ROW_LABEL_W(22)`·`COL_LABEL_H(22)` 는 유지(높이/세로축은 요구 아님).
- **히트테스트(§11.D)는 이 새 상수를 자동 반영**(같은 `s(CELL_W,scale)` 사용) — 별도
  수정 불필요. 측정·그리기·히트테스트가 동일 상수를 참조하므로 드리프트 없음.
- 검증: `measure_grid` 가 셀 데이터 글리프 폭과 무관하게 상수만 쓰므로, 변경 후
  9×9/3열/이모지 모든 케이스에서 비율 동일. 폭 축소로 중앙배치 x 좌표만 바뀜.

### 11.C 역방향 IPC 프로토콜 (동결 — 렌더러→TSF)

전송: **같은 DUPLEX 파이프 연결**. 렌더러 reader_loop 이 있는 연결에서 렌더러가
`WriteFile`(서버→클라 방향), TSF worker 가 `ReadFile`. envelope 는 정방향과 구분되는
`cmd:"evt"` + `evt` 서브타입. **`owner_hwnd`+`seq` 로 어느 팝업 인스턴스 대상인지 식별**
(stale 이벤트 차단: TSF 가 마지막 send_render 의 owner_hwnd 와 불일치하면 무시).

역방향 메시지(렌더러 → TSF), JSON line, `\n` 종결, UTF-8:

```jsonc
// 공통 envelope: v=1, cmd="evt", evt=<서브타입>, owner_hwnd=<대상 호스트 HWND u64>,
//                seq=<렌더러가 마지막 수신한 render 의 seq, echo>
{"v":1,"cmd":"evt","evt":"cell_click","owner_hwnd":123456,"seq":7,"row":2,"col":3}
{"v":1,"cmd":"evt","evt":"page_click","owner_hwnd":123456,"seq":7,"dir":1}   // dir: 0=Prev,1=Next
{"v":1,"cmd":"evt","evt":"tab_click", "owner_hwnd":123456,"seq":7,"index":4} // 이모지 카테고리
{"v":1,"cmd":"evt","evt":"expand_toggle","owner_hwnd":123456,"seq":7}        // 한자 ⊞/⊟
{"v":1,"cmd":"evt","evt":"outside_cancel","owner_hwnd":123456,"seq":7}       // 외부클릭 취소
```

- `row`/`col` 은 0-based 격자 좌표(한자 compact 는 `col` 항상 0). 엔진 `handle_click(row,col)`
  에 그대로 전달.
- `page_click.dir`: `PageDirection::Prev=0 / Next=1` (types.rs:34 순서와 와이어 상수 일치).
  V1 은 dir 만; 직접 page 점프는 미사용(footer 에 ◀/▶ 영역만, 절대 페이지 라벨 클릭 없음).
- `tab_click.index`: 이모지 카테고리 0..8 (`refresh_emoji_category_items` 인자).
- `expand_toggle`: 한자 팝업 확장/축소(footer 우측 ⊞/⊟ 영역). `expand_visible==true` 일
  때만 렌더러가 송신.
- `outside_cancel`: §11.E 저수준 훅이 팝업 밖 클릭 감지 시. row/col 없음.
- 미래 확장 필드는 `#[serde(default)]` 옵셔널만(§3.5 절차 동일).

### 11.D 역방향 Rust 와이어 타입 (양 크레이트 **동일 사본** — 복붙 원본)

§3.3 `WireMsg` 에 역방향 필드를 추가한다(**기존 정방향 무영향** — 전부 옵셔널). 정방향
직렬화 골든 라인(protocol.rs:93 / popup_ipc.rs:883)은 `skip_serializing_if=Option::is_none`
덕에 **바이트 불변**(추가 필드가 None 이라 직렬화 생략).

```rust
// §3.3 WireMsg 에 추가 (정방향 필드 뒤). cmd 에 "evt" 서브커맨드 추가.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub evt: Option<String>,      // "cell_click"|"page_click"|"tab_click"|"expand_toggle"|"outside_cancel"
#[serde(default, skip_serializing_if = "Option::is_none")]
pub row: Option<u32>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub col: Option<u32>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub dir: Option<u32>,         // 0=Prev, 1=Next
#[serde(default, skip_serializing_if = "Option::is_none")]
pub index: Option<u32>,       // tab index

pub mod evt_kind {            // 양 크레이트 동일
    pub const CELL_CLICK: &str = "cell_click";
    pub const PAGE_CLICK: &str = "page_click";
    pub const TAB_CLICK: &str = "tab_click";
    pub const EXPAND_TOGGLE: &str = "expand_toggle";
    pub const OUTSIDE_CANCEL: &str = "outside_cancel";
    pub const PAGE_DIR_PREV: u32 = 0;
    pub const PAGE_DIR_NEXT: u32 = 1;
}
```

**역방향 골든 라인 5종**(byte-equal 교차 테스트 — 양 크레이트 테스트에 동일 문자열 박기):

```text
{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"cell_click","row":2,"col":3}
{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"page_click","dir":1}
{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"tab_click","index":4}
{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"expand_toggle"}
{"v":1,"cmd":"evt","pid":0,"seq":7,"owner_hwnd":123456,"evt":"outside_cancel"}
```

> 직렬화 키 순서 = 구조체 필드 선언 순서(serde_json). 위 라인은 `WireMsg` 필드 순서가
> `v,cmd,pid,seq,first,flash,owner_hwnd,render,evt,row,col,dir,index` 일 때의 출력.
> 렌더러는 `pid` 를 자기 pid 가 아니라 **0** 으로 채워 보낸다(역방향은 pid 의미 없음 —
> 식별은 owner_hwnd+seq). 양측 구현자는 위 5줄을 `serde_json::to_string` 결과와
> `assert_eq!` 하는 테스트를 동일 문자열로 추가한다.

### 11.E 마우스 히트테스트 (구현자 A — window.rs)

`WM_MOUSEACTIVATE → MA_NOACTIVATE` **유지**(§5.3, 포커스 강탈 금지). 클릭 처리는
`WM_LBUTTONDOWN`(현재 V1 no-op, window.rs:286 교체):

1. `GET_X_LPARAM/GET_Y_LPARAM(lparam)` = 클라이언트 좌표(px). UI_STATE 의 `last`(RenderState)
   `scale` 참조.
2. **paint 와 동일 공식으로 rect 계산**(render.rs 의 paint_grid/paint_compact/paint footer
   를 그대로 복제한 `hit_test(rs, scale, x, y) -> Option<RevEvent>` 를 render.rs 에 추가,
   paint 와 같은 모듈·같은 상수 사용해 드리프트 차단):
   - 공통: `pad=s(PAD)`, `body_top=s(HEADER_H)+pad`.
   - **격자 셀**(비-compact): `emoji=!tab_labels.is_empty()`, `tab_w=emoji?s(TAB_W):0`,
     `grid_left=pad+tab_w`, `cells_left=grid_left+s(ROW_LABEL_W)`,
     `cells_top=body_top+s(COL_LABEL_H)`.
     `col=(x-cells_left)/s(CELL_W)`, `row=(y-cells_top)/s(CELL_H)`; 범위 `0..9 / 0..rs.rows`
     이고 `rs.cell_at(row,col)` 의 `HAS_DATA` 셀이면 → `cell_click{row,col}`.
     **반드시 `cell_at`(=`cells[col*rows+row]`, column-major) 로 존재 확인** — paint 와 동일
     인덱싱(전치 금지, §5.4 H3 재발 방지).
   - **이모지 탭**: `emoji` 면 `tab_rect[i] = (pad, body_top+i*s(TAB_H))` (paint 의 탭 rect와
     동일). 적중 i<9 면 `tab_click{index:i}`.
   - **compact 행**(한자): `top0=s(HEADER_H)+pad`, `row=(y-top0)/s(COMPACT_ROW_H)`;
     `0..rs.rows` & HAS_DATA → `cell_click{row, col:0}`.
   - **footer ⊞/⊟**: `rs.expand_visible` & 클릭이 footer rect 우측(draw 의 DT_RIGHT 영역,
     폭 ~`s(28)`) → `expand_toggle`. footer 중앙 영역 클릭은 무시(페이지 라벨은 정보표시).
   - **페이지 ◀/▶**: footer 좌/우 끝(expand 와 겹치지 않게: 좌측 `s(24)`=Prev, 우측은
     expand 없을 때만 Next)에 가상 버튼 영역. paint footer 도 동일 좌표에 ◀/▶ 글리프를
     **추가로 그린다**(현재 footer_text 중앙 + expand 우측만 그림 → ◀/▶ 글리프 신설).
     `total_pages>1` 일 때만 적중 → `page_click{dir}`.
3. 적중 결과를 §11.C 역IPC 로 송신: pipe_server 의 reader 연결 핸들을 통해 **현재 owner
   연결로 WriteFile**. 송신 주체 = UI 스레드지만 WriteFile 자체는 비교적 짧음 — 그래도
   UI 블로킹 최소화 위해 송신은 **owner conn 핸들을 main 의 OwnerState 에 보관**하고
   (현재 conn_id 만 있음 → reader 가 핸들을 OwnerState 에 공유하도록 확장),
   `send_reverse(owner_conn_handle, &WireMsg)` 헬퍼로 fire-and-forget. 실패 시 로그만.
4. 적중 없으면(헤더/여백) no-op + 로그. **외부 클릭은 여기 안 옴**(창 밖이라 WM 안 들어옴) —
   §11.E 훅이 담당.
5. WM_MOUSEMOVE 호버 하이라이트는 V2 범위 밖(선택은 키보드 SoT 와 충돌 방지 위해 클릭만).

### 11.F 외부 클릭 취소 + 패스쓰루 (구현자 A — window.rs/main.rs)

**`WH_MOUSE_LL` 저수준 마우스 훅**을 렌더러 **UI 스레드**(메시지 루프 보유, main.rs:152)에
설치. LL 훅은 메시지 펌프 있는 스레드 필수 — UI 스레드가 정확히 그 스레드.

- **설치 시점/수명**: 팝업 **표시 중에만** 설치(`window::show_render` 성공 시
  `SetWindowsHookExW(WH_MOUSE_LL, ...)`, `window::hide` 시 `UnhookWindowsHookEx`).
  상주(항상 설치)는 전 시스템 마우스 이벤트를 가로채 성능·신뢰성 부담 → 표시 중 한정.
  훅 핸들은 thread_local(UI_STATE 동反 위치).
- **콜백**(`extern "system" LowLevelMouseProc`): `nCode==HC_ACTION` &
  `wParam==WM_LBUTTONDOWN`(또는 RBUTTONDOWN) 일 때 `MSLLHOOKSTRUCT.pt`(스크린 좌표) 를
  팝업 창 rect(`GetWindowRect`)와 비교:
  - 좌표가 **팝업 rect 밖** & 팝업 `visible` → 역IPC `outside_cancel` 송신
    (현재 owner 연결로) + **`CallNextHookEx`(비소비) 로 패스쓰루** — 클릭이 앱에 그대로
    전달(소비 금지, 요구 (4)). 즉시 `window::hide()` 는 **하지 않고** TSF 의
    OutsideCancel 처리(엔진 팝업 취소)가 정방향 hide 를 되돌려보내게 한다(SoT 일원화).
    단, 렌더러 자체도 즉시 `hide()` 하여 시각 지연 0 — 이중 hide 무해(idempotent).
  - 좌표가 **팝업 rect 안** → **훅에서 무시**(no 송신) + `CallNextHookEx`. 내부 클릭은
    `WM_LBUTTONDOWN`(§11.E wndproc)이 처리 → **중복 방지**(훅·wndproc 동시 발동 금지).
- **재진입/성능**: 콜백은 §최소 작업만(rect 비교 + 비차단 WriteFile). 무거운 작업·재진입
  유발 호출(MessageBox 등) 금지. WriteFile 도 owner conn 핸들 캐시로 즉시 — 5.5ms LL훅
  타임아웃(LowLevelHooksTimeout) 내 처리. 송신 실패는 무시.
- **WM_MOUSEACTIVATE=MA_NOACTIVATE 와 무관** — 외부 클릭은 우리 창에 안 오므로
  MOUSEACTIVATE 자체가 발생 안 함; 훅이 유일 경로.

### 11.G TSF 역채널 적용 (구현자 B — popup_ipc.rs / text_service.rs / key_handler.rs)

**핵심 제약**: 역명령 적용은 **반드시 TSF 스레드**(엔진 락 + edit session 컨텍스트 필요).
worker 스레드는 ReadFile 만, 적용은 마샬링 후 TSF 스레드에서.

1. **worker ReadFile**: `PipeConn` 의 핸들을 `GENERIC_WRITE` → **`GENERIC_READ|GENERIC_WRITE`**
   로 연다(popup_ipc.rs:408 `try_connect` 의 `CreateFileW` access 변경 — 파이프는 이미
   DUPLEX 라 OK). worker 루프를 송신 전용 → **송신(채널) + 수신(ReadFile) 멀티플렉싱**으로
   확장: 별도 **reader 서브스레드**가 같은 핸들에서 `ReadFile` 블로킹 라인 파싱(렌더러
   reader_loop 과 동형). 핸들 공유는 `Arc<HandleWrap>`; 핸들 close 시 reader 도 종료.
   (mpsc 채널은 송신용 유지; 수신은 reader 서브스레드가 직접 마샬링.)
2. **worker → TSF 스레드 마샬링**: `ActivateEx`(text_service.rs:198) 시 **message-only HWND**
   (`HWND_MESSAGE` 부모) 를 TSF 스레드에 생성하고 그 wndproc 을 TSF 스레드가 소유(TSF STA
   메시지 펌프가 처리). reader 서브스레드는 파싱한 `RevEvent` 를
   `Arc<Mutex<VecDeque<RevEvent>>>` push 후 `PostMessageW(msg_hwnd, WM_UNIM_REV, 0, 0)`.
   wndproc 이 큐 drain → 엔진 적용. **HWND 를 ActivateEx 에서 만들어 Deactivate 에서 파괴**;
   생성 실패해도 IME 무영향(역채널만 비활성, 로그).
   - HWND/큐/owner 식별용 마지막 owner_hwnd 는 `TextService` 필드(Mutex)로 보관.
3. **엔진 적용**(wndproc, TSF 스레드 — 엔진 락 잡고):
   - `cell_click{row,col}` → `engine.popup_state_mut().handle_click(row,col)`
     (popup_keys.rs:141). 반환이 `Select(idx)` 면 → **`engine.popup_select(idx)`**
     (popup_dispatch.rs:190; 키보드 Enter 와 동일 경로 — commit_buffer 채우고
     `HidePopup` pending). `Updated/Consumed` 면 재렌더만.
   - `page_click{dir}` → `engine.popup_change_page(PageDirection::{Prev|Next})`
     (popup_dispatch.rs:263; PageJump pending).
   - `tab_click{index}` → `engine.refresh_emoji_category_items(index)`
     (popup_dispatch.rs:389) + cat 변경 시 ShowEmoji 재발행 위해
     `process_popup_key` 의 카테고리 전환 로직과 동치 처리 필요 →
     **간단 경로**: `popup_state_mut().handle_click` 대신 cat 점프는 엔진의
     `refresh_emoji_category_items(index)` 호출 후 `popup_state_mut()` 에서
     `current_page/sel_row/sel_col=0` 리셋(popup_keys.rs:607 CatLetter 분기와 동일 효과).
     이후 view_model 재전송.
   - `expand_toggle` → `engine.popup_state_mut().toggle_hanja_expanded()`
     (popup_keys.rs:311 Period 분기가 호출하는 메서드 — `popup_state_mut` 로 직접 호출).
   - `outside_cancel` → `engine.popup_cancel()`(popup_dispatch.rs:225 — 원본 한글/초성
     commit + HidePopup pending) **또는** 단순 숨김만 원하면 cancel. 요구 (4)는 "팝업 취소"
     이므로 `popup_cancel()`(취소=원본 유지 커밋, 엔진 SoT 정의) 채택.
4. **적용 후 재렌더/확정 송신**: 위 적용 직후 `drain_popup_actions(engine, popup)`
   (key_handler.rs:476) **재호출** — pending PopupAction 을 소비해 view_model 재전송 또는
   hide. CellClick 이 `Select`→commit_buffer 채운 경우, **확정 텍스트를 문서에 삽입**해야
   한다:
   - **마지막 활성 `ITfContext` 보관**: `OnKeyDown`(text_service.rs:392) 진입 시
     `pic` 를 `TextService.last_context: Mutex<Option<ITfContext>>` 에 clone 저장
     (OnSetFocus 의 ITfDocumentMgr→GetTop 도 가능하나, 최신 키 컨텍스트가 가장 안전).
   - wndproc 적용 경로에서 `commit_buffer` 비어있지 않으면: 보관 컨텍스트 + tid 로
     `composition::insert_text(ctx, tid, &commit)` (composition.rs:395, edit session
     내부 — TSF 스레드라 RequestEditSession 정상) 후 `engine.clear_commit()`.
     조합 중이 아니므로 `replace_surrounding(ctx,tid,0,commit,"",sink)` 도 동치(둘 다 OK;
     `insert_text` 가 더 단순). **마우스 확정은 비조합 삽입**(키보드 Enter 와 달리 컴포지션
     컨텍스트 없음) → `insert_text` 권장.
   - 컨텍스트 부재(보관 None) 시 commit 보류 + 로그(드물게 포커스 없는 순간).
5. **전부 비차단·로그**: 엔진 락은 짧게, edit session 거부(TF_E_NOLOCK)는 조용히 skip
   (composition.rs 기존 패턴). 접두사 `popup_rev:` 로 통일. owner_hwnd 불일치 이벤트는
   적용 전 무시 + `popup_rev: stale evt owner=.. cur=..` 로그.

### 11.H 파일 소유권 (Phase 2 — §9 갱신)

| 구현자 | 추가 소유 | 산출물 |
| --- | --- | --- |
| **A (렌더러)** | `unim-popup-win/src/{render.rs,window.rs,protocol.rs,pipe_server.rs,main.rs}`, `Cargo.toml`(D2D features) | §11.A/B/D/E/F. zero-warning. 가짜 클라(파이프 read) 로 역IPC 5종 골든 검증 |
| **B (TSF)** | `unim-tsf/src/{popup_ipc.rs,key_handler.rs,text_service.rs}` | §11.D(사본)/G. zero-warning. 역IPC 미수신(렌더러 없음)에도 타이핑 무영향 |

- 와이어 역타입(§11.D)은 양측 동일 사본 — `WireMsg` 한 정의에 옵셔널 필드 추가(정방향
  골든 라인 바이트 불변 유지가 머지 게이트).
- §3.5 변경 절차 동일: 필드 추가는 `#[serde(default)]` 만, 설계서 갱신 동반.

### 11.I 위험·재발 금지 (§11 한정)

- **히트테스트 ≠ paint**: 클릭 rect 가 그리기 rect 와 1px라도 다르면 오선택. → `hit_test`
  를 paint 와 **같은 모듈·같은 상수·같은 `s()`** 로 작성, column-major `cell_at` 만 사용.
- **외부클릭 소비**: 훅에서 `CallNextHookEx` 누락·`return 1` 금지 → 앱이 클릭 못 받음.
  반드시 비소비 패스쓰루.
- **IME 블로킹**: 역채널 ReadFile/마샬링은 worker·msg-only HWND 경유. TSF 스레드 wndproc
  적용은 엔진 락 짧게. 어떤 실패도 panic 금지 — 타이핑 영향 0.
- **컬러폰트 폴백**: D2D/DWrite 생성 실패 시 GDI 흑백으로 폴백(공백·패닉 금지).
- **stale 이벤트**: owner_hwnd+seq 불일치 역이벤트는 무시(포커스 전환 race).
- **LL훅 수명**: 표시 중에만 설치·hide 시 해제(상주 금지). 훅 누수 시 전 시스템 마우스
  지연 — Deactivate/shutdown 에서도 해제 보장.
- **MA_NOACTIVATE 유지**: 클릭으로 포커스 강탈 금지(§5.3).

---

## 부록 A — 참조 소스 좌표

| 무엇 | 어디 |
| --- | --- |
| 엔진 popup 상태 접근 | `src/input_engine/popup_dispatch.rs:244` `popup_state()` |
| view model 생성 | `src/popup/view_model.rs:82` `PopupState::view_model(home_row)` |
| home_row | `src/input_engine/engine.rs:266` `home_row_labels()` |
| column-major 인덱싱 근거 | `src/popup/popup_layout.rs:89-124` (`col*rows+row`), POPUP_SPEC §4.2 |
| rows=9 고정 정책 | `src/popup/popup_layout.rs:21-48`, POPUP_SPEC §4.4 |
| 트리거 시 북마크 플래그 | `src/input_engine/candidates.rs:63` `set_bookmark_flags` |
| Linux 와이어 형태(참조) | `unim-popup-types/src/lib.rs` `PopupRenderPayload` + `popup_render_flags` |
| 대체 대상 | `unim-tsf/src/popup_window.rs` (삭제), `key_handler.rs:474-508` drain, `text_service.rs:46,341,421,580` |
| DLL 로그 | `unim-tsf/src/register.rs:196` `dbg_log` (`UNIM_DEBUG_LOG=true`) — popup_ipc 에서 쓰려면 현행 `pub(crate)` 가시성으로 충분 |
| 색·레이아웃 기준 | POPUP_SPEC §3~§5, Linux `unim-popup-service/src/gtk_ui.rs` |
| **마우스 클릭 적용**(§11.G) | `src/popup/popup_keys.rs:141` `handle_click(row,col)→PopupKeyResult` |
| 클릭 확정 경로 | `src/input_engine/popup_dispatch.rs:190` `popup_select(idx)` (Enter 동일 경로, commit_buffer+HidePopup) |
| 페이지 클릭 | `src/input_engine/popup_dispatch.rs:263` `popup_change_page(PageDirection)` |
| 탭 클릭(이모지) | `src/input_engine/popup_dispatch.rs:389` `refresh_emoji_category_items(cat_index)` + `popup_state_mut` |
| 확장 토글(한자) | `src/popup/popup_keys.rs:311` `toggle_hanja_expanded()` via `popup_state_mut()` |
| 외부클릭 취소 | `src/input_engine/popup_dispatch.rs:225` `popup_cancel()` (원본 커밋+HidePopup) |
| 가변 popup 상태 | `src/input_engine/popup_dispatch.rs:249` `popup_state_mut()` |
| PageDirection 와이어값 | `src/input_engine/types.rs:34` `Prev=0,Next=1` |
| 마우스 확정 문서삽입 | `unim-tsf/src/composition.rs:395` `insert_text(ctx,tid,text)` (edit session) |
| OnKeyDown 컨텍스트 보관 | `unim-tsf/src/text_service.rs:392` `pic`(ITfContext) — `last_context` 캐시 위치 |
| 정방향 파이프 DUPLEX | `unim-popup-win/src/pipe_server.rs:19,109` (이미 `PIPE_ACCESS_DUPLEX`) |
| worker 송신 핸들 access | `unim-tsf/src/popup_ipc.rs:408` `try_connect` (GENERIC_WRITE→READ\|WRITE) |
| 렌더러 클릭 진입점 | `unim-popup-win/src/window.rs:286` `WM_LBUTTONDOWN`(V1 no-op 교체) |
