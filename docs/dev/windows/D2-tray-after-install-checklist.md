# D-2 — 설치/갱신 후 트레이(언어바) 메뉴 미표시: 원인 판정 + VM 검증 체크리스트

> 이슈: MSI 설치·갱신 직후 언어바/트레이에 UNIM 이 보이지 않는다(우클릭 메뉴 접근 불가).
> 본 문서는 (1) 실패 후보 3개의 코드 판정 근거, (2) 구현한 것/안 한 것, (3) VM 재현·검증
> 체크리스트를 담는다. "트레이 메뉴"는 이 앱에서 시스템 트레이 아이콘이 아니라 **TSF
> 네이티브 언어바(taskbar language bar)의 UNIM 버튼·우클릭 메뉴**를 가리킨다
> (`SMOKE_TEST.md` §5, 별도 트레이 앱은 이미 폐지됨).

## 1. 실패 후보 3개 판정

### (a) 사일런트 설치(UILevel<4)에서 위저드 CA 가 조건식에 걸려 스킵되는가 → **참, 그러나 mitigated**

- `installer/wix/unim.wxs` 451-461행:
  ```xml
  <Custom Action="LaunchSetupWizardFresh" After="LaunchPopupRenderer">
    NOT Installed AND NOT WIX_UPGRADE_DETECTED AND UILevel>=4
  </Custom>
  <Custom Action="LaunchSetupWizardUpgrade" After="LaunchPopupRenderer">
    WIX_UPGRADE_DETECTED AND UILevel>=4
  </Custom>
  ```
  429-436행 주석이 명문화: `msiexec /qn`(UILevel=2)·`/qb`(UILevel=3) 에서는 두 CA 모두
  **의도적으로** 스킵된다. 이 조건 자체는 버그가 아니라 설계(무인 설치에 GUI를 강제하지
  않음).
- 그러나 `msiexec /i ... /qn` 을 **직접** 부르는 `install.ps1` 은 이 스킵을 자체 보강한다
  — `install.ps1` 338행이 실제로 `/qn` 을 고정하고, 192-235행 `Invoke-PostInstallWizard`
  가 msiexec 종료 후(비승격 컨텍스트로 복귀한 상태에서) `unim-settings.exe --first-run`
  /`--whats-new` 를 **직접 spawn** 한다(506·511행이 성공/3010 두 경로 모두에서 호출).
  즉 **`install.ps1` 경유 설치는 (a) 로 인해 위저드가 안 뜨는 문제가 없다** — 이미 M-31
  로 막혀 있다(193-197행 주석에 이 정확한 인과관계가 기록돼 있음).
- **잔여 노출면**: `install.ps1` 을 거치지 않고 `msiexec /i unim.msi /qn` 을 직접(예:
  SCCM/GPO 무인 배포) 호출하는 경로는 이 보강이 없다 — 위저드가 영구히 안 뜨고, 뒤이어
  아래 (b)/(c) 의 자동 복구 수단도 Windows 에는 없다(`--first-run-if-needed` 로그인당
  autostart 는 **Linux 전용**, `.desktop` 파일로만 존재 — `unim-settings/data/
  io.github.from104.unim.FirstRun.desktop`; Windows 에는 대응하는 HKLM/HKCU Run 키나
  예약 작업이 `unim.wxs` 에 없다 — Run 키가 있는 건 `unim-popup-win.exe` 뿐, 268-283행).
  이번 구현 범위 밖(계약에 없음)이라 손대지 않았고, 아래 §3 잔여 리스크에 기록한다.

### (b) 위저드 미완주 시 트레이 등록이 안 남는 구조인가 → **참, 그리고 세미버 게이트가 영구화한다**

- `unim-settings/src/wizard.rs`:
  - `WIZ_DEFAULT_IME` 페이지는 `is_new(WIZ_INTRODUCED) && !platform::wizard_is_default_ime()`
    일 때만 노출된다(107-109행). `기본 입력기 지정`(`set_as_default()`)은 **오직 [마침]
    클릭 시**(229-264행)에만 실행된다 — X 로 닫으면 절대 실행되지 않는다.
  - Windows 는 `record_seen_on_close`(301-310행)가 **무조건 true** — X 로 닫아도
    `WizardSeenVersion` 이 현재 버전으로 즉시 기록된다(Linux FirstRun 만 예외로 seen 을
    보류해 다음 로그인 재출현을 보장함, 287-296행 주석).
  - 그 결과: Windows 사용자가 첫 위저드를 **완주하지 않고 닫으면**, `WizardSeenVersion`
    이 이미 현재 버전으로 박히고, 다음에(수동 재실행이든 다음 업데이트의 `--whats-new`
    든) `is_new(WIZ_INTRODUCED)` 는 `parse_semver(introduced) > parse_semver(seen)` 로
    평가되어 **false** 가 된다(86-91행) → `WIZ_DEFAULT_IME` 페이지가 **다시는 뜨지
    않는다**, `wizard_is_default_ime()` 가 여전히 false 인데도. 트레이/언어바에 UNIM 이
    영구히 안 뜬 채로 "완료 기록"만 남는 조용한 실패 모드.
  - **판정**: (b) 는 사실이며, 단순 "미완주 = 이번 회차만 미등록"이 아니라 **미완주가
    향후 모든 위저드 실행에서 등록 기회 자체를 지워버리는 구조적 결함**이다.
- 이번 구현 범위(계약 2)는 "각 후보 판정"만 요구했으므로 **이 결함 자체는 고치지
  않았다** — 별도 이슈로 분리해 `is_new(WIZ_INTRODUCED)` 와 `!wizard_is_default_ime()`
  의 AND 결합을 재검토할 것을 권고(§3 잔여 리스크).

### (c) ActivateProfile 성공해도 이미 실행 중인 explorer.exe 세션엔 반영 안 되는가 → **참 — 이번 구현의 대상**

- `unim-tsf/src/register.rs` 214-218행(및 `unim-windows-common/src/ime.rs` 26-30행에
  동일 주석의 복제)이 이미 명문화: `SetDefaultLanguageProfile`/`ActivateProfile` 은
  **HKCU·현재 세션**에 작용하고, `TF_IPPMF_FORSESSION` 은 그 호출을 실행한 **프로세스의
  세션**(마법사 exe 프로세스)에 반영된다 — 하지만 언어바를 그리는 것은 **이미 그 세션에서
  로그온 때부터 떠 있던 explorer.exe** 다. explorer.exe 는 새로 등록된 TIP/프로필을
  스스로 재조회하지 않으므로, ActivateProfile 이 성공(`Result::Ok`)해도 그 explorer.exe
  가 그리는 언어바에는 반영되지 않는다.
- **답**: 재로그인, 또는 explorer.exe 재시작(껐다 켬 — 새로 뜬 explorer.exe 는 그 시점의
  등록 상태를 다시 읽는다). 강제 자동화는 금지(§계약 2) — 열려 있는 탐색기 창이 전부
  닫히는 부작용이 있어 사용자 동의가 필수.

## 2. 구현한 것

- `unim-windows-common/src/ime.rs::restart_explorer()` — `taskkill /F /IM explorer.exe`
  후 `explorer.exe` 재기동. 옵트인 전용, 마법사가 스스로 호출하지 않는다.
- `unim-settings/src/platform/{windows,linux,mod}.rs::wizard_restart_explorer()` —
  3-way 함수 표면(Windows 실구현·Linux/fallback no-op)에 새 항목 추가.
- `unim-settings/ui/settings.slint` — 완료 페이지(kind==5)에 `wiz-platform-windows &&
  wiz-whats-new` 게이트 카드 신설: "탐색기가 자동으로 안 읽을 수 있다" 안내(=재로그인
  고지) + "지금 탐색기 재시작" 버튼(옵트인, 클릭 전엔 아무 것도 실행되지 않음).
  신규 설치(FirstRun)는 계약 2가 "갱신 케이스"로 범위를 명시해 대상에서 제외했다 —
  동일 근본원인이 FirstRun 에도 있으므로 후속 확장 후보(§3).
- `unim-settings/src/wizard.rs` — `wiz-platform-windows` 게이트 설정(`cfg!` 기반, Linux
  는 항상 false → 렌더 트리 불변) + 콜백 배선.
- **폐기한 접근**: `docs/dev/windows/DEPLOY-TRUST-PLAN.md` §b-1 의 `WixUI_Minimal` +
  `WIXUI_EXITDIALOGOPTIONALCHECKBOX` ExitDialog 체크박스 안은 **구현하지 않았다** —
  `installer/wix/unim.wxs` 에는 애초에 `<UI>`/`UIRef`/`WixUI*` 블록 자체가 없어(grep 결과
  0건) 그 다이얼로그가 존재하지 않는다. 대신 이미 있는 `--whats-new` CA(444-449·
  458-460행)가 여는 마법사 완료 페이지에 안내를 이관했다.

## 3. 잔여 리스크 (이번 범위 밖 — 후속 이슈 후보)

1. **(a) 잔여 노출면**: `install.ps1` 을 거치지 않는 순수 `msiexec /qn` 무인 배포는
   위저드가 영구히 안 뜬다. Windows 에 `--first-run-if-needed` 급의 로그인당 재시도
   게이트가 없다(Linux 만 `.desktop` autostart 로 존재).
2. **(b) 구조적 결함**: 위저드 미완주(X 닫기) 시 seen 버전이 즉시 기록되어 향후
   `WIZ_DEFAULT_IME` 페이지가 다시 뜨지 않는다 — `wizard_is_default_ime()` 실측과
   무관하게 `is_new(WIZ_INTRODUCED)` 게이트만으로 억제되는 것이 원인.
3. **FirstRun(신규 설치)에도 (c) 와 동일한 explorer 미반영 문제가 있다** — 계약이
   "갱신 케이스"로 범위를 좁혔을 뿐 근본원인은 동일. 안내 카드 게이트를
   `wiz-whats-new` 없이 `showed_default_ime`(위저드가 실제로 기본입력기 페이지를
   보여준 실행)만으로 여는 확장을 고려할 것.
4. **set_as_default 제거 결정과의 정합**(계약 3, 보고만): "Windows 설정 UI 별도 exe
   분리" 결정(계획 PASS, 미구현)은 DLL 내부 modal(옛 Win32 설정창)을 별도 exe 로
   대체하며 `set_as_default` 를 그 자리에서 제거하는 방향이었다. 현재 코드는 아직
   그 이전 상태 그대로다 — `unim-tsf/src/lang_bar.rs:792`(언어바 우클릭
   "기본 입력기로 설정" 메뉴)와 `unim-tsf/src/settings_dialog.rs:1461`(옛 내장 Win32
   다이얼로그의 `ID_BTN_SET_DEFAULT` + `MessageBoxW`)가 **여전히**
   `crate::register::set_as_default()` 를 DLL STA 스레드에서 직접 호출한다. 이번 D-2
   변경은 `unim-settings.exe`/`unim-windows-common` 쪽에만 손을 대 이 두 호출부와
   충돌하지 않는다 — 단, 그 분리 결정이 실제 구현될 때 이 두 호출부도 함께 정리돼야
   한다(별도 작업).

## 4. VM 검증 체크리스트

전제: `SMOKE_TEST.md` 의 표준 절차(설치→레지스트리→TIP 발견)를 먼저 통과한 뒤 아래를
추가로 수행한다. Windows 11 x64 VM, 클린 스냅샷 권장.

### 4.1 재현 매트릭스 — "위저드 완주 여부 × UILevel"

| UILevel 경로 | 위저드 완주(마침 클릭) | 위저드 미완주(X 닫기) | 위저드 미표시(CA 스킵) |
|---|---|---|---|
| `/qn`(UILevel=2, `install.ps1` 경유) | ☐ 언어바에 UNIM 노출 | ☐ WizardSeenVersion 기록되는지 확인, 다음 실행에서 기본입력기 페이지 재출현 여부 | 해당 없음(install.ps1 이 항상 실행) |
| `/qn`(UILevel=2, `msiexec` 직접) | 위저드 자체가 안 뜬다 → | | ☐ **예상 실패 케이스** — 언어바에 UNIM 미노출, 수동 복구 수단 없음(§3-1) 확인 |
| `/qb`(UILevel=3) | 좌동(위저드 CA 미실행) | | ☐ 좌동 |
| 더블클릭(UILevel=5) | ☐ 언어바 즉시 노출(재로그인 불필요 여부 확인 — (c) 재현 지점) | ☐ 완료 페이지 안내 카드 노출 확인(--whats-new 갱신 시나리오만) | 해당 없음 |
| 갱신(`-Update`, `install.ps1`) | ☐ 완료 페이지에 D-2 안내 카드 + "지금 탐색기 재시작" 버튼 노출·클릭 후 언어바 갱신 확인 | ☐ X 닫기 후 언어바 상태(구버전 그대로인지) | — |

각 셀: `☐ OK` / `☐ FAIL: 사유` 로 갱신. UILevel=5(더블클릭)+완주 조합이 "재로그인 없이
즉시 반영"의 최선 케이스이므로 이게 실패하면 (c) 가 FirstRun 에도 재현됨을 뜻한다.

### 4.2 D-3 배너로 스테일 DLL 확정 절차

D-3(스테일 DLL 배너, 별도 이슈)이 아직 구현 전이라면 아래 수동 절차로 대체 확인한다.

1. 갱신 설치 **직후**, 재로그인/탐색기 재시작 **전에**:
   ```powershell
   Get-Process explorer | ForEach-Object {
       (Get-Process -Id $_.Id -Module | Where-Object {$_.ModuleName -match 'unim_tsf'}).FileVersion
   }
   reg query "HKCR\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\InProcServer32" /ve
   (Get-Item "C:\Program Files\UNIM\unim_tsf.dll").VersionInfo.FileVersion
   ```
2. 세 값(explorer 에 로드된 모듈 버전 / 레지스트리가 가리키는 경로 / 디스크 파일의
   실제 버전)을 비교한다. explorer 로드 버전이 디스크 파일 버전보다 낮으면 **스테일
   DLL 확정** — Windows Installer 가 파일 사용 중이라 교체를 예약했거나(재부팅 대기)
   구버전이 여전히 메모리에 상주 중인 것.
3. D-3 배너가 구현되면 이 3단계를 배너의 "구버전 감지" 로직과 대조해 배너가 실제
   스테일 상태를 정확히 판별하는지 검증한다(1·2 단계가 배너의 ground truth 역할).

### 4.3 CUAS 부류 판별

언어바 갱신 확인에 쓰는 타깃 앱이 CUAS-aware 인지에 따라 결과가 갈릴 수 있다
(`repro-matrix-p1.md`, `research-korean-tsf-imes.md` 참조 — CUAS 는 owner-side
composition lifecycle 을 관리하는 msctf 브리지).

- ☐ 메모장/Edge 주소창(표준 CUAS 경로, `WM_IME_*` 메시지 기반) — 언어바 상태와 별개로
  입력 자체는 되는지 1차 확인.
- ☐ Windows Terminal/VS Code(ConPTY, PR#4796 규칙 — TSF inline 지원) — 언어바 아이콘
  갱신 여부가 메모장과 다르면 CUAS 유무가 D-2 재현에 영향을 준다는 신호.
- ☐ conhost(순수 콘솔, CUAS 제외 경로) — 언어바 반영 여부가 GUI 앱과 다르면 기록.
- 판별 기준: `_WEAKLINK_RESOLUTION.md` §"③ 기본입력" 표의 conhost vs
  Terminal/VSCode 분리와 동일한 방식으로 결과를 두 군으로 나눠 기록한다.

### 4.4 tmux 확인 (Ctrl+B 프리픽스 미충돌)

WSL 또는 Windows Terminal 안에서 `tmux` 세션을 열고:

- ☐ `Ctrl+B` 가 UNIM 의 어떤 트리거 키(한자/특수문자/토글)와도 충돌하지 않고 tmux
  prefix 로 정상 전달되는지 확인(`Ctrl+B` 이후 `%`/`"` 로 pane 분할까지).
- ☐ tmux pane 안에서 한/영 토글 후 한글 입력이 정상인지(언어바 상태와 pane 내부 IME
  상태가 어긋나지 않는지) 확인 — D-2 재현 중 언어바가 갱신되지 않은 상태에서 tmux
  pane 진입 시 한/영 전환 자체가 막히는지가 핵심 관찰 포인트.

### 4.5 트레이(언어바) 우클릭 확인

- ☐ 언어바의 UNIM 버튼을 우클릭 → `한/영 전환` / `기본 입력기로 설정` / `설정 열기`
  3항목이 모두 보이는지(`SMOKE_TEST.md` 5.4 와 동일 기준).
- ☐ D-2 재현 상태(언어바 미노출)에서는 애초에 우클릭할 대상이 없음을 확인 —
  "메뉴가 비어 있다/일부만 보인다"가 아니라 "UNIM 항목 자체가 언어바에 없다"임을
  스크린샷으로 구분해 기록(오분류 방지 — 후자만 D-2, 전자는 별개 결함).
- ☐ §4.1 매트릭스의 "지금 탐색기 재시작" 버튼 클릭 후, 같은 우클릭 메뉴가 나타나는지
  재확인(수정 성공 기준).

## 5. 결과 보고

`MSI_DIAGNOSIS_TEMPLATE.md` 를 복사한 진단서에 §4 결과를 첨부하고, "5. 트레이/설정 UI"
행에 이 문서의 §4.1 매트릭스 결과 요약을 함께 적는다.
