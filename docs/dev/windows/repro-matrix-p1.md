# P1·실시간 reload 실측 절차 (wezterm/Telegram 조합 끊김 원인 격리)

> 목적: 지식베이스가 지목한 근본 원인(CUAS owner-side lifecycle 종료)을 **실측 로그**로 확정하고,
> A(display attribute 무음 실패) vs B(lifecycle 종료) 를 분리한다. P1 진단 로깅(`SetValue`/`GetValue`/
> `OnCompositionTerminated`)이 이 격리를 가능하게 한다.

## 0. 배포할 DLL
- 빌드 산출물: `target/x86_64-pc-windows-msvc/release/unim_tsf.dll` (MD5 `b4772d6…`, P1+실시간reload 포함)
- 설치 위치: `C:\Program Files\unim\unim_tsf.dll` (현재 `7de05b1…`, 06-02 = P1 이전 → 교체 필요)
- 진단 로그: `UNIM_DEBUG_LOG=true`(기본 ON) → 모든 호스트 앱이 **`%TEMP%\unim-tsf.log`** 에 PID 태그로 append.

## 1. 배포 (관리자 권한 — 사용자 직접)
DLL 은 실행 중인 앱(이미 IME 를 쓴 앱)에 로드돼 잠겨 있을 수 있으므로 **먼저 텍스트 앱을 모두 닫는다**
(wezterm·Telegram·메모장·설정앱). 그 뒤 아래를 `!` 로 실행(관리자 PowerShell UAC):

```
! powershell -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-Command','Copy-Item -Force \"C:\Users\USER\Desktop\work\unim\target\x86_64-pc-windows-msvc\release\unim_tsf.dll\" \"C:\Program Files\unim\unim_tsf.dll\"'"
```

- "사용 중" 오류 시: **로그오프 후 재로그인**(텍스트 앱 열기 전에 위 복사 먼저) 또는 **재부팅** 후 재시도.
- 또는 MSI 재빌드(버전 범프) 후 재설치.
- 교체 확인: 설치본 MD5 가 `b4772d6…` 와 같으면 성공.

## 2. 로그 초기화 (깨끗한 캡처)
배포 직후, 텍스트 앱 열기 **전에**:
```
! del "%TEMP%\unim-tsf.log"
```

## 3. 재현 매트릭스
각 셀에서 한글 입력 후 `%TEMP%\unim-tsf.log` 의 새 줄을 관찰. 앱마다 PID 가 달라 구분됨.

| 앱 \ 키 시퀀스 | ㄱ (자음 1) | 안녕 + Space | 안 + Backspace |
|---|---|---|---|
| **메모장** (IME-unaware, 대조군) | OK 기대 | OK 기대 | OK 기대 |
| **wezterm** | 끊김 의심 | 직전삭제/끊김 의심 | ? |
| **Telegram** | 직전삭제(#29743류) 의심 | 직전삭제 의심 | ? |

각 셀: ① 글자가 화면에 정상 조합되는가(육안) ② 로그에 어떤 줄이 찍히는가.

## 4. 로그 줄 해석 (P1 진단 신호)
- `set_composition_attribute: SetValue FAILED hr=...` → **A 확정**(attribute 자체가 안 박힘).
- `set_composition_attribute: GetValue MISMATCH ...` 또는 `GetValue failed` → **A 의심**(SetValue 는 OK 인데 range 에 안 남음 = CUAS 가 result-string 으로 오인 가능).
- (위 두 줄이 **없음** = attribute 정상) + `OnCompositionTerminated: IMMEDIATE -> fallback (... by_time=true ...)` → **B 확정**(lifecycle 종료, attribute 무죄).
- `known_cuas=true` 가 보이면 → 해당 창이 학습됨(2회 관찰 후), 타이밍 무관 폴백 진입(P1 캐시 동작 확인).
- 메모장에서 `IMMEDIATE` 가 **안** 떠야 정상(정식 text store 는 composition 유지).

## 5. 수집·분석
재현 후 `%TEMP%\unim-tsf.log` 내용을 공유(또는 경로 알려주면 직접 읽음).
- A 우세 → composition.rs 의 attribute 부여 경로 보강(P1 진단이 가리키는 지점).
- B 우세(예상) → CUAS lifecycle 대응(폴백 모드 품질 개선)이 정답. async(P2)는 무관 재확인.

## 6. 함께 검증되는 것
- **실시간 reload**: 메모장 입력 중 설정앱에서 자판 변경 → 포커스 전환 없이 다음 키에 반영되는지.
- **P1 HWND 학습 캐시**: wezterm 둘째 글자부터 `known_cuas=true` 로그.
