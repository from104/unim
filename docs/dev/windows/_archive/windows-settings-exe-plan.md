# Windows 설정 UI 분리 계획 — DLL 내부 다이얼로그 → 별도 설정 exe

> 상태: **검토용 계획 (코드 미수정)** · 대상 브랜치: `feat/windows-msi-redesign`
> 작성 근거: 실제 소스 라인 대조 (2026-06-01). mozc/weasel 업계 표준 패턴 참고.

---

## 문제 요약

현재 `unim-tsf/src/settings_dialog.rs`(1631줄, 네이티브 Win32 4탭 다이얼로그)는 TSF DLL 내부에서
`show_settings_dialog()`로 직접 모달 창을 띄우고 **자체 `GetMessageW` 루프 + `EnableWindow(parent,false)`**
(`settings_dialog.rs:1614~`)를 돌린다. 이 루프가 호스트 앱의 TSF STA 입력 스레드 위에서 실행되어
호스트 메시지 펌프와 충돌 → 설정 창이 뜨지 않거나 입력이 잠긴다. 펌프를 호스트 프로세스 안에서
돌리는 한 근본 해결 불가.

---

## 목표 아키텍처

설정 UI를 **별도 exe** (`unim-tsf-settings.exe`)로 분리한다. DLL은 트리거만, exe는 UI 전담.
mozc(`mozc_tool.exe`를 `CreateProcess`), weasel(`WeaselDeployer.exe`를 `ShellExecute`)이 쓰는
업계 표준이다.

```
[경로 A] 랭귀지바/트레이 우클릭 메뉴 "설정"
   lang_bar.rs handle_menu_command(MENU_ID_SETTINGS)  ─┐
                                                        ├─► spawn_settings_exe()
[경로 B] Windows 설정 → 키보드 옵션 [속성]              │     (DLL 내부)
   fn_configure.rs ITfFnConfigure::Show(hwndparent) ───┘        │
                                                                │ CreateProcessW
                                                                ▼
                                            ┌──────────────────────────────────┐
                                            │  unim-tsf-settings.exe            │
                                            │  main() → show_settings_dialog()  │
                                            │  (자체 WNDCLASS + GetMessageW 루프 │
                                            │   — 자기 프로세스라 펌프 충돌 없음)│
                                            └──────────────────────────────────┘
                                                                │
                                       OK/적용 시 config.yaml + blacklist/userdict yaml 저장
                                                                │
       DLL OnSetFocus mtime reload (text_service.rs) ─► 포커스 전환 후 입력 시 반영
```

핵심 이점: ① 호스트 프로세스에서 modal UI를 그리지 않아 펌프 블로킹/STA 충돌 원천 제거,
② 모든 호스트 프로세스에 로드되는 DLL에서 1631줄 + commctrl 의존 제거(메모리·로드 비용↓),
③ exe는 단독 실행/디버깅 가능.

**UI 수단**: windows-rs 네이티브 유지(이미 0.62). 1631줄을 거의 그대로 이전 — 백지 재작성 없음.

---

## 코드 이전 매핑

`settings_dialog.rs`는 거의 전부 순수 Win32다. DLL 의존은 단 2종류뿐:

| 현재 (DLL 내부) | 이전 후 (exe 내부) | 비고 |
|---|---|---|
| `crate::dll_instance()` × **18** (L167,184,293,312,333,356,380,400,426,446,946,1002,1055,1074,1088,1102,1116,**1592**) | `GetModuleHandleW(None)`(헬퍼 `get_instance()`) | exe는 자기 모듈 핸들이 곧 인스턴스. **L1592는 `show_settings_dialog` 본체의 메인 다이얼로그 `CreateWindowExW` hInstance — 빠뜨리면 안 되는 가장 중요한 치환 지점.** grep `-c` 확인 결과 정확히 18회 |
| `ID_BTN_SET_DEFAULT` / `crate::register::set_as_default()` — **4곳**: ① L105 상수 `const ID_BTN_SET_DEFAULT`, ② L770 `create_button(...)` 버튼 생성, ③ L1344 `match` 분기, ④ L1345 `crate::register::set_as_default()` 호출 | **확정: (a) 4곳 모두 제거** | **핵심: exe 크레이트에는 `register` 모듈이 없으므로 `crate::register::set_as_default` 호출이 남으면 컴파일 에러.** 버튼만 지우고 상수만 남기면 dead_code 경고. 기능은 트레이 메뉴 `MENU_ID_SET_DEFAULT`(lang_bar)와 중복이라 손실 없음 |
| `unim::config::*`, `unim::keystroke::profile`, `unim::typefix_blacklist`, `unim::typefix_userdict` | 그대로 — exe도 `unim = { path = ".." }` 의존 | Config 영속화는 코어 공유라 무변경 |
| 자체 `GetMessageW`/`IsDialogMessageW` 루프 (L1614~) | **그대로** | 이미 DialogBox 미사용·자체 루프라 exe `main()`으로 자연 이전 |
| `EnableWindow(parent,false)` + parent HWND 중심 정렬 (L1572~) | parent 없음 → 화면 중앙 정렬로 대체, `EnableWindow` 제거 | exe는 부모 프로세스를 disable할 권리/필요 없음 |
| INITCOMMONCONTROLSEX 초기화 (L1040) | 그대로 | commctrl 의존은 exe로 함께 이동 |

이전 방식(권장): `settings_dialog.rs`를 `unim-tsf/src/`에서 **새 exe 크레이트의
`src/settings_dialog.rs`로 파일 이동** + 18개 `dll_instance()`(L1592 메인 다이얼로그 포함)를
`GetModuleHandleW` 헬퍼로 치환 + `set_as_default` 4곳 완전 제거 + `main.rs` 추가.
unim-tsf에는 trigger spawn 헬퍼만 신규 작성.

---

## 핵심 설계 결정

### 1. 새 크레이트
- **이름**: `unim-tsf-settings` (주의: `unim-settings`는 **이미 존재**하는 GTK4/libadwaita Linux 크레이트 → 재사용 불가).
- **위치/형태**: 워크스페이스 루트 `unim-tsf-settings/`, `[[bin]] name = "unim-tsf-settings"`.
- **의존**: `unim = { path = ".." }` + `windows = { version = "0.62", features = [...] }`.
  GTK/DBus/tokio 의존 없음(순수 Win32). `panic = "abort"`는 워크스페이스 `[profile.release]`(Cargo.toml:62-63) 자동 상속.
- **Config 공유**: 코어 `unim::config::Config`를 그대로 사용. `Config::load_from_default_path()` /
  `save_to_default_path()` → `%APPDATA%\unim\config.yaml` (`src/config.rs:824-826,944`). blacklist/userdict yaml도 동일.

### 2. DLL이 exe를 찾는 방법 (가장 견고한 방식 선택)
**채택: DLL 경로 기준 같은 디렉토리.** 코드베이스에 이미 검증된 패턴 존재 —
`register.rs:26`가 `GetModuleFileNameW(Some(hmodule), ...)`로 DLL 풀패스를 얻는다.
MSI는 `unim_tsf.dll`과 `unim-tsf-settings.exe`를 **같은 `INSTALLDIR`(`%ProgramFiles%\UNIM`)에 설치**하므로
(wxs Directory `INSTALLDIR`, unim.wxs:45), DLL 경로의 파일명만 `unim-tsf-settings.exe`로 바꾸면 exe 절대경로 확정.
- 절차: `GetModuleFileNameW(dll_instance(), buf)` → 마지막 `\` 까지 자르고 `unim-tsf-settings.exe` 붙임.
- 레지스트리 설치경로 조회보다 견고(레지스트리 키 누락/regsvr32 단독설치에도 동작). weasel도 모듈 경로 기준.
- 폴백: 같은 폴더에 없으면 `ShellExecuteW`에 파일명만 넘겨 PATH 탐색(최후 수단).

### 3. 트리거 코드 전환 (경로 A·B 모두 spawn)
- **경로 A** `lang_bar.rs handle_menu_command` `MENU_ID_SETTINGS`:
  `show_settings_dialog(GetForegroundWindow())` 호출(**lang_bar.rs:524**) → `crate::spawn_settings()`로 교체.
- **경로 B** `fn_configure.rs ITfFnConfigure::Show`:
  `show_settings_dialog(hwndparent)` 호출(**fn_configure.rs:43**) → `crate::spawn_settings()`로 교체.
- **신규** `unim-tsf/src/` 에 `spawn_settings()` (예: `lib.rs` 또는 신규 `settings_launcher.rs`):
  - DLL 경로 → exe 경로 산출(설계 2),
  - `CreateProcessW`(권장, 핸들 회수·에러 코드 명확) 또는 `ShellExecuteW`(간단),
  - 실패 시 `MessageBoxW`로 사용자 안내(조용한 실패 금지).

### 4. 단일 인스턴스 가드
**필요(권장).** 메뉴/[속성]을 연타하면 설정창이 중복으로 뜬다. exe `main()` 진입 시
`CreateMutexW(name="Local\\unim-tsf-settings")` 후 `GetLastError()==ERROR_ALREADY_EXISTS`면
기존 창을 `FindWindowW`(WND_CLASS_NAME) → `SetForegroundWindow`로 끌어올리고 즉시 종료.
weasel/mozc도 동일 패턴.

### 5. exe 진입점
`main.rs`의 `fn main()`에서 단일 인스턴스 체크 후 `show_settings_dialog()` 호출.
`show_settings_dialog`의 자체 `GetMessageW` 루프가 그대로 모달 역할(L1614~) — 루프 종료 시 프로세스 종료.
부모 HWND 인자 제거(또는 `HWND(0)`로 호출하고 내부에서 중앙 정렬 분기).

---

## 5지점 Config 동기화 / Linux 회귀 / MSI·CI 영향

### 5지점 Config 동기화 — **비해당**
설정 **항목 추가·삭제 없음**. UI를 옮기는 리팩터링일 뿐 config 스키마 불변.
엔진(`src/config.rs`)·GUI(`unim-gui-gtk`)·CLI(`unim-cli config`) 어느 곳도 건드리지 않는다.
(만약 이 작업 중 항목을 추가하려는 충동이 생기면 → 별도 작업으로 분리, 6지점 싱크 규칙 적용)

### Linux(GTK/Qt/XIM) 회귀 — **영향 0**
- 코어 `src/` **무수정**. `settings_dialog.rs`는 `#[cfg(windows)]` 경로의 Windows 전용 파일이고
  Linux 빌드에 포함되지 않는다.
- unim-tsf에서 1631줄 제거는 **cdylib(DLL) 내부 변화**일 뿐, 코어 라이브러리 API 불변.
- 새 exe 크레이트는 워크스페이스 멤버지만 Windows 타깃에서만 빌드(아래 CI 항목). Linux `cargo build --workspace`에
  새 크레이트가 끌려오지 않도록 **windows-only 의존**으로 격리하거나, Linux CI의 빌드 대상에서 제외.
  - **확정 형태**: `main.rs`를 `#![cfg(windows)]`(파일 전체 inner attribute)로 가리면 **비-Windows에서 bin
    target에 `main` 함수가 사라져 "main 함수 없음" 에러**. `cargo build --workspace`(Linux)는 members에
    포함된 이 크레이트를 **반드시 컴파일**하므로, 정확히 아래 형태로 명세한다:
    ```rust
    #[cfg(windows)]
    fn main() { /* 단일 인스턴스 + show_settings_dialog */ }
    #[cfg(not(windows))]
    fn main() {}   // 비-Windows 폴백 — 워크스페이스 빌드 깨짐 방지
    ```
  - `windows` 의존은 `[target.'cfg(windows)'.dependencies]`로 격리(비-Windows에서 끌려오지 않게).
  - `settings_dialog` 모듈도 `#[cfg(windows)] mod settings_dialog;`로 가드.

### MSI 변경 (`installer/wix/unim.wxs`)
- `INSTALLDIR` 아래 새 컴포넌트 추가(unim.wxs:60 `UnimTsfDll` 컴포넌트 옆, 기존 네이밍 관례 일치):
  ```xml
  <Component Id="UnimTsfSettingsExe" Guid="<신규-GUID>" Win64="yes">
    <File Id="unim_tsf_settings_exe" Name="unim-tsf-settings.exe"
          Source="$(var.WIN_OUT_DIR)\unim-tsf-settings.exe" KeyPath="yes" />
  </Component>
  ```
- `<Feature Id="Complete">`(unim.wxs:215~)에 **`<ComponentRef Id="UnimTsfSettingsExe" />`** 추가
  (unim.wxs:223 `ComponentRef Id="UnimTsfDll"` 옆). **Component Id 와 ComponentRef Id 가 정확히 일치해야
  light.exe 링크 성공**(불일치 시 LGHT0094). File Id `unim_tsf_settings_exe` 와도 네이밍 일관.
- **GUID — gen-guids.sh 수정 불필요(확인 완료)**: `gen-guids.sh`는 `globals.rs`에서 **TSF 고정 const만**
  추출한다(`UNIM_CLSID`, `UNIM_PROFILE_GUID`, `UNIM_DISPLAY_ATTR_*`, LangID, Version → `guids.wxi`의
  `<?define ...?>`). **WiX `<Component Guid>` 값은 gen-guids.sh가 생성하지 않으며**, 기존 컴포넌트
  (`UnimTsfDll` Guid `6B7C8D9E-...`, `RegisterScripts` `8D9E0F1A-...` 등)는 **unim.wxs에 하드코딩된 리터럴**이다.
  따라서 새 컴포넌트는 **새 리터럴 GUID를 unim.wxs에 직접 박으면 끝** — gen-guids.sh/guids.wxi 무수정.
  단, build-msi.bat:22-26 / GHA의 `git diff --exit-code installer/wix/generated/guids.wxi` drift 검사는
  globals.rs를 안 건드리면 통과하므로 영향 없음.
- (선택) 시작 메뉴 바로가기를 exe로 추가 가능(`StartMenuShortcuts`).
- **WiX candle 함정**(메모리 project_windows_msi_ci_gotchas): XML 주석 내 `--` 금지(CNDL0104). 새 주석 작성 시 주의.

### CI / 빌드 통합 — **5지점에 `-p unim-tsf-settings` 추가**
새 exe는 `cargo build`의 명시 패키지 목록에 빠지면 빌드/패키징 누락된다. 다음 모두 갱신:
1. `.github/workflows/windows-msi.yml` — `cargo check` 스텝의 `-p unim -p unim-capi -p unim-tsf`에 추가.
2. 같은 파일 `cargo build (release)` 스텝의 `-p unim-tsf`에 추가.
3. 같은 파일 "Verify build artifacts"의 `for f in unim_tsf.dll` 목록에 `unim-tsf-settings.exe` 추가.
4. `Makefile:382` `WIN_CRATES := -p unim -p unim-capi -p unim-tsf` 에 `-p unim-tsf-settings` 추가
   → `make check-windows`/`build-windows` cross-compile sanity에 자동 포함.
5. `scripts/build-msi.bat:30` `cargo build -p unim-tsf ...`에 `-p unim-tsf-settings` 추가.
- **windows-msi.yml 트리거 함정**(메모리): `feat/*` push는 트리거 안 됨(main/develop만). PR로 검증해야 CI 동작.

---

## 구현 단계 (Phase별)

> reviewer 확정 순서. **각 Phase 독립 커밋. 롤백은 Phase 3 트리거 커밋 revert.**

### Phase 1 — 새 크레이트 골격 (Linux 빌드 무파괴 즉시 검증)
- 신규 `unim-tsf-settings/Cargo.toml`(name, `[[bin]]`, `unim = { path = ".." }`,
  `[target.'cfg(windows)'.dependencies] windows = "0.62"`).
- 루트 `Cargo.toml` `[workspace] members`에 `"unim-tsf-settings"` 추가.
- `src/main.rs`에 **확정형 cfg 가드 main** 작성: `#[cfg(windows)] fn main(){…}` + `#[cfg(not(windows))] fn main(){}`.
- **검증**: `cargo build --workspace`(Linux)가 깨지지 않는지 **즉시 확인**(members 포함 크레이트는 무조건 컴파일됨).

### Phase 2 — 다이얼로그 코드 이전 (DLL 의존 제거)
- `unim-tsf/src/settings_dialog.rs` → `unim-tsf-settings/src/settings_dialog.rs`로 이동.
- **18× `crate::dll_instance()`(L1592 메인 다이얼로그 포함) → `get_instance()` 헬퍼(`GetModuleHandleW(None)`)로 치환.**
- **`set_as_default` 4곳 완전 제거**: L105 상수, L770 `create_button`, L1344 match 분기, L1345 `crate::register::set_as_default()` 호출.
  → **`register` 의존 제거가 목적**(exe엔 register 모듈 없음 — 남기면 컴파일 에러).
- `show_settings_dialog`: parent 인자 제거/무시, `EnableWindow` 제거, 화면 중앙 정렬.
- `main.rs`: 단일 인스턴스 mutex → `show_settings_dialog()` 호출.
- **검증**: `cargo build -p unim-tsf-settings --target x86_64-pc-windows-msvc` 단독 빌드 통과.

### Phase 3 — DLL 트리거 전환 (1631줄 제거 확인)
- `unim-tsf/src/`에 `spawn_settings()` 추가(DLL→exe 경로 산출 + `CreateProcessW` + 실패 `MessageBoxW`).
- 경로 A `lang_bar.rs:524`·경로 B `fn_configure.rs:43` → `crate::spawn_settings()` 호출로 교체.
- `settings_dialog` 모듈 선언 제거(`lib.rs:29 pub mod settings_dialog;`), 죽은 import 정리.
- **검증**: `cargo build -p unim-tsf --target msvc`로 DLL 빌드 통과 + 1631줄 제거 확인.

### Phase 4 — MSI / 빌드 / CI 통합
- `unim.wxs`에 **Component Id `UnimTsfSettingsExe`(신규 리터럴 GUID) + File `unim_tsf_settings_exe` +
  Feature 내 `<ComponentRef Id="UnimTsfSettingsExe" />`** 추가 (gen-guids.sh 무수정 — MSI 섹션 참조).
- **CI 5지점** `-p unim-tsf-settings` 갱신(상단 CI 항목 1~5).
- **검증**: `make check-windows`(Linux sanity) 통과. PR 올려 `windows-msi.yml` 그린 확인(feat push는 트리거 안 됨 → PR 필수).

---

## 리스크 & 롤백 경로

| 리스크 | 영향 | 완화 |
|---|---|---|
| exe 경로 탐색 실패(설치 위치 비표준/regsvr32 단독) | 설정창 안 뜸 | DLL 경로 기준 1차 + PATH/ShellExecute 폴백 + 실패 MessageBox |
| `ITfFnConfigure::Show` 명세(닫힐 때까지 반환 금지) 위반 | OS가 동기 완료 가정 시 오동작 | mozc/weasel도 spawn 후 즉시 반환이 실무 표준 — 미해결 항목 참조. 1차 동일 채택 |
| 새 크레이트가 Linux `--workspace` 빌드 깨뜨림 | Linux CI 적색 | cfg(windows) 의존 격리 + `#![cfg(windows)]` 빈 크레이트 폴백 |
| MSI 컴포넌트 GUID drift / candle 주석 `--` | MSI 빌드 실패 | gen-guids.sh 선실행·커밋, 주석 `--` 금지 |
| `set_as_default` 버튼 제거로 기능 후퇴 | 사용자 편의 저하 | 트레이 메뉴 `MENU_ID_SET_DEFAULT`(lang_bar)와 기능 중복 → 제거해도 손실 없음. **확정: 제거** |
| exe↔DLL 버전 스큐(부분 업데이트) | 설정 필드 불일치 가능 | MSI 단일 패키지로 함께 설치/제거 → 정상 경로에선 동기. 차기 인지 항목(현 범위 무관) |
| 단일 인스턴스 누락 | 설정창 중복 | named mutex + FindWindow/SetForegroundWindow |

**롤백**: 4 Phase 모두 `feat/windows-msi-redesign` 위 커밋 단위 분리. 문제 시 Phase 3 트리거 커밋만
revert하면 in-DLL 다이얼로그로 즉시 복귀(`settings_dialog.rs`를 unim-tsf에 잔류시키는 중간안도 가능).
코어·Linux 무수정이라 롤백 표면 최소.

---

## Windows VM 검증 체크리스트

- [ ] 랭귀지바/트레이 우클릭 메뉴 "설정" → exe 설정창 정상 표시 (경로 A)
- [ ] Windows 설정 → 한국어 → 키보드 옵션 [속성] → 설정창 표시 (경로 B, 카테고리 키 함정 확인)
- [ ] 설정 변경 → 저장 → **포커스 전환 후(다른 창 클릭 등) 다음 입력 시** 엔진 반영 확인
      (OnSetFocus mtime reload — 포커스 전환 시에만 발화. 기존 동작과 동일, 회귀 아님)
- [ ] 설정창 떠 있는 동안 **호스트 앱(메모장/워드/브라우저) 입력·펌프 안 멈춤** (핵심 회귀 검증)
- [ ] 메뉴/[속성] 클릭 → exe spawn cold-start 지연 체감(수백 ms 수준 허용 가능한지) 확인
- [ ] 메뉴/[속성] 연타 → 설정창 1개만, 기존 창 포그라운드로 (단일 인스턴스)
- [ ] 억제 단어/사용자 사전 탭 편집 → 별도 yaml 즉시 저장 확인
- [ ] DLL에서 1631줄 제거 후에도 한/영 전환·조합·랭귀지바 정상 (DLL 회귀)
- [ ] 설정 exe를 INSTALLDIR에서 직접 더블클릭 실행 → 독립 동작 (단독 디버깅)
- [ ] **Linux 회귀 0**: `cargo build --workspace`(Linux) 통과, GTK 설정/XIM/Qt 정상

---

## 미해결 / 불확실

1. **`ITfFnConfigure::Show` 명세 vs spawn 관행**: MS 문서상 Show는 다이얼로그가 닫힐 때까지
   반환하지 않는 것이 원칙(모달). 하지만 mozc·weasel 모두 외부 도구를 spawn한 뒤 `S_OK` 즉시 반환하며,
   실제 Windows 설정 UI는 이를 문제삼지 않는다(비동기 spawn이 사실상 업계 관행). **1차 채택: spawn 후 즉시 반환.**
   VM에서 [속성] 경로의 실동작 확인 필요.
2. **`set_as_default` 처리 — 확정: (a) 4곳 제거**. (b 코어 추출은 `register::set_as_default`가 TSF
   COM `ITfInputProcessorProfileMgr`을 써 "코어에 플랫폼 의존 금지" 위반이라 부적합, c DLL export 호출은
   과한 복잡도.) 트레이 메뉴 `MENU_ID_SET_DEFAULT`와 기능 중복이라 손실 없음. **사용자 승인 완료.**
3. **카테고리 키 함정**: `register.rs`는 `RegisterCategory` 미호출(msctf NULL deref 회피),
   카테고리 키는 `unim.wxs:114-118` 등 static 블록에만 존재. regsvr32 단독 설치 시 경로 B [속성] 버튼
   자체가 안 뜰 수 있음 → 이는 **본 분리 작업과 독립한 기존 이슈**. exe 분리가 이 문제를 악화시키진 않으나
   경로 B 검증 시 MSI 설치 경로로 테스트해야 함.
4. **exe 경로 탐색**: DLL 경로 기준(채택)이 1차. 비표준 설치 케이스 대비 폴백 정책 VM 확인.
5. **새 크레이트의 Linux 워크스페이스 빌드 격리 — 형태 확정**: `#[cfg(not(windows))] fn main(){}` 폴백으로
   비-Windows 컴파일을 통과시킨다(`#![cfg(windows)]` 파일 전체 가드는 main 부재 에러이므로 금지).
   stub main이 빈 함수 경고를 내면 `#[allow(...)]` 부착. `cargo build --workspace`(Linux)로 실제 확인.
6. **exe↔DLL 버전 스큐**(차기 인지): 부분 업데이트 시 설정 필드 불일치 가능. MSI 단일 패키지로 함께
   설치/제거되므로 정상 경로에선 비발생. 현 범위 무관, 향후 자동 업데이트 도입 시 재검토.
