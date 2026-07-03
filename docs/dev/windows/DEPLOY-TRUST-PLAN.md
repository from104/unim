# DEPLOY-TRUST-PLAN — 배포 신뢰 후속 사양서 (코드서명 · 온보딩 · 자동업데이트)

상태: **사양 확정 / 구현 착수 대기**
전제: 인증서 발급·실설치 QA·업데이트 서버 인프라가 필요하므로 자동 코드런으로는 완결 불가.
본 문서는 각 항목을 "파일:함수 단위 타깃 + 수용기준(acceptance)"으로 구체화하여 구현 착수 가능
수준으로 기술한다.

대상 브랜치: `feat/windows-msi-redesign` 이후. 세 항목은 상호 독립적으로 진행 가능하되,
**(a) 서명 → (c) 자동업데이트** 순서 의존이 있다(업데이트 검증이 서명을 전제).

---

## (a) Authenticode 코드서명 파이프라인

### a-1. 서명 대상 (전수)

CI(`.github/workflows/windows-msi.yml`)가 생산하는 산출물 전부:

| 산출물 | 경로 (CI 기준) | 비고 |
|---|---|---|
| `unim_tsf.dll` (x64) | `target/x86_64-pc-windows-msvc/release/` | TSF TIP — 모든 프로세스에 로드되므로 서명 필수 1순위 |
| `unim_tsf.dll` (x86) | `target/i686-pc-windows-msvc/release/` | WOW64 프로세스용 |
| `unim-tsf-settings.exe` | x64 release | Slint 설정 GUI |
| `unim-popup-win.exe` | x64 release | HKLM Run 상주 렌더러 — SmartScreen/AV 오탐 최다 예상 지점 |
| `unim_imm32.ime` (x64/x86) | rename 스텝 산출 | `.ime`도 PE — signtool 정상 서명 가능 |
| `unim-<ver>-x64.msi` | `dist/` | **바이너리 서명 후 MSI 패키징 후 MSI 자체 서명** (2단 서명) |

원칙: **서명 순서 = 내부 PE 전부 서명 → light.exe 패키징 → MSI 서명.**
MSI만 서명하고 내부 DLL 미서명이면 설치 후 TIP 로드 시점의 신뢰가 없다(특히
`unim_tsf.dll`은 브라우저·오피스 등 서드파티 프로세스에 로드되므로 EDR 정책상 미서명 DLL
차단 사례가 실존).

### a-2. 서명 서비스 선택

| | Azure Trusted Signing (권장) | SignPath.io (대안) |
|---|---|---|
| 비용 | ~$9.99/월 (Basic) | OSS 무료 티어 있음 (승인 심사) |
| 인증서 | 단기(72h) 인증서 자동 회전, EV 상당 신뢰 | OV/EV 선택 |
| CI 연동 | `azure/trusted-signing-action` 공식 액션 | SignPath GitHub 연동(아티팩트 업로드→서명→다운로드) |
| SmartScreen | Microsoft 자체 서비스라 평판 누적 유리 | OV는 평판 0에서 시작 |
| 요건 | Azure 테넌트 + 사업자/개인 신원검증 (개인은 3년 이상 이력 요구될 수 있음) | OSS는 프로젝트 심사 |

**결정 기준**: Azure 신원검증 통과 가능하면 Trusted Signing. 불가 시 SignPath OSS 신청.
두 경우 모두 아래 CI 스켈레톤의 "서명 스텝" 내용물만 교체되고 배선 위치는 동일하다.

### a-3. signtool 규격

```
signtool sign /fd SHA256 /td SHA256 /tr http://timestamp.acs.microsoft.com <file>
```

- `/fd SHA256`: 파일 다이제스트 SHA-256 (SHA-1 서명 금지 — Win10+ 불신).
- `/tr` + `/td SHA256`: **RFC 3161 타임스탬프 필수.** 미첨부 시 인증서 만료·회전(Trusted
  Signing은 72시간마다 회전) 즉시 서명 무효화됨. Trusted Signing은
  `timestamp.acs.microsoft.com`, SignPath/기타는 `http://timestamp.digicert.com` 사용.
- 검증: `signtool verify /pa /all <file>` 가 모든 산출물에 대해 exit 0.

### a-4. CI 배선 스켈레톤 (`.github/workflows/windows-msi.yml`)

삽입 지점 2곳:

1. **"Rename IMM32 DLL → IME" 스텝(현재 104행) 직후, "Verify build artifacts" 이전**
   — PE 6종(unim_tsf.dll x64/x86, settings exe, popup exe, unim_imm32.ime x64/x86) 서명.
2. **"Build MSI (candle + light)" 스텝(현재 162행) 직후, integrity check 이전** — MSI 서명.

```yaml
# ── 삽입 1: PE 서명 (rename 스텝 직후) ──
- name: Sign binaries (Azure Trusted Signing)
  if: github.event_name != 'pull_request'   # PR 빌드는 비서명 (시크릿 미노출)
  uses: azure/trusted-signing-action@v0
  with:
    azure-tenant-id: ${{ secrets.AZURE_TENANT_ID }}
    azure-client-id: ${{ secrets.AZURE_CLIENT_ID }}
    azure-client-secret: ${{ secrets.AZURE_CLIENT_SECRET }}
    endpoint: https://eus.codesigning.azure.net/
    trusted-signing-account-name: ${{ secrets.TS_ACCOUNT }}
    certificate-profile-name: ${{ secrets.TS_PROFILE }}
    files-folder: target
    files-folder-filter: dll,exe,ime
    files-folder-recurse: true
    file-digest: SHA256
    timestamp-rfc3161: http://timestamp.acs.microsoft.com
    timestamp-digest: SHA256

# ── 삽입 2: MSI 서명 (light 직후) ── 동일 액션, files-folder: dist / filter: msi

# ── 삽입 3: 서명 검증 게이트 (artifact 업로드 직전) ──
- name: Verify signatures
  if: github.event_name != 'pull_request'
  shell: pwsh
  run: |
    $files = @(Get-ChildItem dist\*.msi) + @(Get-ChildItem `
      target\x86_64-pc-windows-msvc\release\unim_tsf.dll, ...)
    foreach ($f in $files) {
      $sig = Get-AuthenticodeSignature $f.FullName
      if ($sig.Status -ne 'Valid') { Write-Error "unsigned: $($f.Name)"; exit 1 }
    }
```

주의:
- `files-folder-filter`가 target 하위 의존 크레이트 빌드 부산물(빌드스크립트 exe 등)까지
  긁지 않도록 실제 구현 시 **명시적 파일 목록 방식**으로 좁힐 것(서명 호출 수 = 과금/시간).
- PR 빌드는 시크릿이 없으므로 `if:` 가드로 서명·검증 스텝 전체 스킵 — 기존 비서명 플로우
  그대로 통과해야 한다(회귀 금지).

### a-5. SmartScreen 평판

- OV 인증서는 서명해도 평판 0에서 시작 → 초기엔 "일반적으로 다운로드되지 않는 파일" 경고
  잔존. **다운로드 횟수 누적으로만 해소**(수주~수개월). EV/Trusted Signing은 즉시 통과에
  가깝다.
- 완화책: (1) 릴리스 노트에 SmartScreen 경고 안내 문구 — `doc-promo` 몫,
  (2) 동일 인증서 유지(재발급 시 평판 리셋), (3) MSI 파일명·ProductName 안정 유지.

### a-6. 수용기준 (a)

- [ ] main 브랜치 push 빌드 산출 MSI에 대해 `signtool verify /pa /all` exit 0.
- [ ] MSI 내부에서 추출한 6개 PE 전부 `Get-AuthenticodeSignature` Status=Valid.
- [ ] 서명 타임스탬프 존재(`TimeStamperCertificate` 비어있지 않음) — 인증서 만료 후에도 유효.
- [ ] PR 빌드는 서명 스텝 스킵되고 기존과 동일하게 성공.
- [ ] 클린 Win11 VM에서 MSI 더블클릭 시 UAC 다이얼로그에 "확인된 게시자: <이름>" 표시.

---

## (b) 최초설정 온보딩 (설치 → 즉시 사용 가능)

### b-0. 문제 정의 (닭-달걀)

per-machine MSI는 SYSTEM 컨텍스트라 HKCU/세션에 작용하는
`SetDefaultLanguageProfile`/`ActivateProfile`을 호출할 수 없다
(`unim-tsf/src/register.rs:214-218` 주석에 명문화). 현재 사용자는 설치 후 수동으로 설정 GUI
(`settings_dialog.rs:1398`)나 langbar 메뉴(`lang_bar.rs:680`)에서 `set_as_default()`를 눌러야
한다. **해소책 = 설치 종료 UI 체크박스 → 사용자 컨텍스트 헬퍼 실행.**

### b-1. WiX ExitDialog 체크박스

타깃: `installer/wix/unim.wxs` (현재 `<UI>` 섹션 없음 — 신설).

```xml
<Property Id="WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT"
          Value="UNIM을 기본 한국어 입력기로 설정하고 시작하기" />
<Property Id="WIXUI_EXITDIALOGOPTIONALCHECKBOX" Value="1" />  <!-- 기본 체크 -->
<UIRef Id="WixUI_Minimal" />

<CustomAction Id="RunFirstRun"
              FileKey="unim_tsf_settings_exe"
              ExeCommand="--firstrun"
              Return="asyncNoWait"
              Impersonate="yes" />   <!-- 핵심: 사용자 컨텍스트 -->
<UI>
  <Publish Dialog="ExitDialog" Control="Finish" Event="DoAction" Value="RunFirstRun">
    WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 and NOT Installed
  </Publish>
</UI>
```

- 빌드 배선: `candle`/`light`에 `-ext WixUIExtension` 추가 —
  `.github/workflows/windows-msi.yml`의 "Build MSI" 스텝(174·185행)과
  `scripts/build-msi.bat`(**편집 금지 파일** — 스크립트 소유자에게 WixUIExtension 플래그 추가
  의뢰; 본 계획의 유일한 .bat 변경 항목이며 별도 승인 게이트).
- `Impersonate="yes"` + ExitDialog 시점 실행 = UI 시퀀스는 설치 실행 사용자 권한으로 돌므로
  HKCU 접근 가능. **silent 설치(/qn)에서는 ExitDialog가 없어 실행되지 않음** — 의도된 동작
  (배포 관리자는 정책으로 처리), 문서화만 한다.

### b-2. `--firstrun` 헬퍼 (호스트: `unim-tsf-settings.exe`)

타깃: `unim-tsf-settings/src/main.rs:fn main()`(현재 160행) 초입에 인자 분기 추가.
신규 모듈 `unim-tsf-settings/src/firstrun.rs`.

동작 시퀀스 (`firstrun::run()`):

1. **사용자 활성 입력목록 추가** — 아래 2경로 순차 시도, 성공 1개면 충분:
   - 1순위: `InstallLayoutOrTip(L"0412:{CLSID}{ProfileGUID}", 0)` (`input.dll` 동적 로드,
     `windows` 크레이트 `LoadLibraryW`+`GetProcAddress` — 함수가 없는 구버전 OS면 2순위로).
     이 호출이 HKCU `Software\Microsoft\CTF\SortOrder\AssemblyItem` 및 언어 설정 UI 목록에
     프로필을 등록한다.
   - 2순위(폴백): HKCU `Software\Microsoft\CTF\Assemblies\0x00000412\{GUID…}` 직접 기록 —
     기존 수동 헬퍼(`%TEMP%\unim_fix.bat` 계열)에서 검증된 값 셋을 코드로 이식.
2. **기본 프로필 지정** — 기존 `unim-tsf/src/register.rs:set_as_default()`(현재 221행) 호출.
   settings exe는 `unim-tsf`를 lib으로 링크하지 않으므로 **함수를
   `unim-windows-common`으로 이동**(GUID 상수 `globals.rs`의 CLSID/ProfileGUID와 함께 재수출)
   하고 `register.rs`·`lang_bar.rs:680`·`settings_dialog.rs:1398`은 재수출 경유로 호출 유지
   (호출부 diff 최소화).
3. **3단계 미니 환영 창** (Slint, 설정 GUI 리소스 재사용):
   - ① "한/영 전환: 한/영 키 또는 오른쪽 Alt" (+ 설정된 토글키 실값 표시)
   - ② "한자 변환: 한자 키" + 팝업 스크린샷 1장
   - ③ "세부 설정은 시작 메뉴 → UNIM 설정" + [설정 열기] [닫기] 버튼
   - 실패 무해 원칙: 1·2단계가 에러여도 환영 창은 띄우고, 실패 항목은 창 안에 "수동 설정
     방법" 링크로 표기(설치 자체를 실패시키지 않는다 — `Return="asyncNoWait"`와 합치).

### b-3. 수용기준 (b)

- [ ] 클린 Win11 VM: MSI 설치 → 체크박스 기본 체크 → 마침 → **재로그인 없이** 아무
      메모장에서 즉시 UNIM으로 한글 입력 가능(Win+Space 목록에 UNIM 표시 + 기본 선택).
- [ ] 체크 해제 시 아무 HKCU 변경 없음(설치 전후 `reg export HKCU\...\CTF` diff 0).
- [ ] `/qn` silent 설치가 기존과 동일하게 성공(ExitDialog 부재로 firstrun 미실행).
- [ ] `unim-tsf-settings.exe --firstrun` 단독 재실행 시 멱등(중복 등록 없음, 에러 없음).
- [ ] 관리자 아닌 표준 사용자 설치 흐름(UAC 승격 후)에서도 HKCU 기록이 **승격 전 사용자**
      hive에 남는다 — Impersonate 검증 항목.
- [ ] 환영 창 3단계 텍스트 ko 우선, 스크린리더로 버튼 도달 가능.

---

## (c) 서명검증 자동업데이트

### c-1. 방식 결정: GitHub Releases API 폴러 (자체 구현, WinSparkle 비채택)

- WinSparkle은 C DLL 의존 + appcast XML 서버 필요 + DSA/EdDSA 별도 키 관리. 이미 (a)에서
  Authenticode를 확보하므로 **서명 검증을 Authenticode 게시자 검증으로 일원화**하는 편이
  의존성·키 관리 면에서 우월.
- 배포 채널 = GitHub Releases (별도 인프라 0). appcast가 필요해지면(다운로드 통계·단계적
  롤아웃) 그때 정적 JSON을 Releases 자산으로 추가하는 것으로 확장 가능 — 본 단계 범위 외.

### c-2. 상주 토대

이미 존재: `installer/wix/unim.wxs`의 HKLM Run 키(현재 259-264행)로
`unim-popup-win.exe`가 전 사용자 로그인 시 상주. **업데이트 폴러를 이 프로세스에 태운다**
(신규 상주 프로세스 추가 금지 — AV 오탐 표면 최소화).

타깃:
- 신규 크레이트 모듈 `unim-popup-win/src/updater.rs` — 폴러 스레드.
  `unim-popup-win/src/main.rs`의 초기화부(싱글턴 뮤텍스 획득 직후)에서 spawn.
- 공용 로직(버전 비교·서명 검증)은 `unim-windows-common/src/update.rs`에 두어 향후 설정
  GUI "지금 확인" 버튼(`unim-tsf-settings`)과 공유.

### c-3. 동작 사양

1. **폴링**: 기동 15분 후 1회 + 이후 24h 간격.
   `GET https://api.github.com/repos/<owner>/unim/releases/latest` (ETag 캐시, 타임아웃 10s,
   실패 시 조용히 다음 주기 — 오프라인 무해).
2. **버전 비교**: `tag_name`(`v0.3.x`) vs `env!("CARGO_PKG_VERSION")`. semver 파싱 실패 시
   중단(다운그레이드 제안 금지).
3. **다운로드**: 자산 중 `unim-<ver>-x64.msi` 1개를 `%LOCALAPPDATA%\UNIM\updates\`로.
   Content-Length 검증 + 릴리스 자산에 함께 올리는 `<msi>.sha256`(CI가 생성) 대조.
4. **서명 검증 (게이트, 필수)**: `WinVerifyTrust(WINTRUST_ACTION_GENERIC_VERIFY_V2)` 성공
   **그리고** 리프 인증서 게시자 CN이 상수 `EXPECTED_PUBLISHER`와 일치(§a의 인증서 CN을
   `unim-windows-common/src/update.rs` 상수로 고정). 둘 중 하나라도 실패 → 파일 삭제 + 구조
   로그(`dbg_log`) 기록 + 해당 버전 재시도 금지 마킹. **CN 고정(pinning)이 핵심** — "아무나
   서명된 MSI"가 아니라 "우리가 서명한 MSI"만 통과.
5. **UX**: 자동 설치 금지. 팝업 렌더러의 기존 알림 표면으로 "UNIM <ver> 업데이트 가능 —
   설치하려면 클릭" 토스트 → 클릭 시 `msiexec /i <검증된 경로> /passive` 실행(UAC는 OS 몫).
   MSI MajorUpgrade가 구버전 제거를 처리. TSF DLL이 각 앱에 로드 중이므로 MSI의 기존
   파일-사용중 처리(재부팅 요구) 플로우를 그대로 따른다 — 강제 종료 시도 금지.
6. **설정 배선**: `auto_update_check: bool` (기본 true) — 설정 GUI 토글 +
   HKCU `Software\atit.org\UNIM\AutoUpdateCheck`(기존 `PREF_SUBKEY` 재사용,
   `register.rs:252` 패턴 준용). Windows 전용 키이므로 Linux 6지점 동기화 대상 아님.

### c-4. CI 추가분

`.github/workflows/windows-msi.yml`에 릴리스 태그 push 시: MSI의 `sha256` 파일 생성 +
`gh release upload` (기존 artifact 업로드 스텝과 별개, `if: startsWith(github.ref,
'refs/tags/v')`).

### c-5. 수용기준 (c)

- [ ] 구버전 설치 VM에서 신버전 릴리스 공개 후 24h 내(또는 재로그인 15분 후) 토스트 표출.
- [ ] 토스트 클릭 → UAC → 업데이트 완료 → 설정 GUI "정보" 탭 버전이 신버전.
- [ ] **비서명/타서명 MSI를 릴리스 자산으로 바꿔치기한 모의 공격 시 설치 제안이 절대 뜨지
      않고** 파일이 삭제된다 (CN pinning 검증 — 테스트는 로컬 mock 서버로 수행 가능).
- [ ] 오프라인/GitHub 5xx 상황에서 사용자 가시 에러 0, 입력 지연 0 (폴러는 별도 스레드,
      hot-path 무접점).
- [ ] 설정 토글 OFF 시 네트워크 요청 0 (프로세스 재시작 없이 즉시 반영은 비요구 — 다음
      기동부터 적용이면 합격).
- [ ] `cargo test -p unim-windows-common`에 버전 비교·CN 파싱 유닛테스트 추가, all-pass.

---

## 구현 순서 제안 및 선행 조건

| 순서 | 항목 | 선행 조건 (사람 몫) |
|---|---|---|
| 1 | (a) 서명 | Azure Trusted Signing 계정/신원검증 **또는** SignPath OSS 승인 + GitHub 시크릿 등록 |
| 2 | (b) 온보딩 | 없음 — 즉시 구현 가능 (클린 VM QA만 필요). scripts/build-msi.bat 플래그 추가는 소유자 의뢰 |
| 3 | (c) 자동업데이트 | (a) 완료 + 게시자 CN 확정 + GitHub Releases 채널 운영 개시 |

(b)는 서명과 독립이므로 병행 가능. (c)의 CN 상수는 (a)의 실제 인증서 발급 후에만 확정된다.
