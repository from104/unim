# UNIM Windows MSI Installer

UNIM 0.3.0 부터 Windows MSI 빌드는 **GitHub Actions (windows-2022 + MSVC + WiX 3.x) 가 단일 정도(正道) 경로**이다. Linux 호스트의 `wixl` 트랙은 다음 사유로 폐기되었다:

1. **mingw GNU ABI ≠ Windows COM (MSVC) ABI** — `windows-rs` 가 가정하는 MSVC vtable 레이아웃과 mingw 빌드 산출물의 미세 불일치 가능성. TSF 인터페이스가 OS 와 협상 실패하면 IME 자체가 활성화되지 않는다.
2. **`wixl` 의 토큰 치환 한계** — `[#File.Id]` 같은 WiX 표준 토큰 처리가 누락된 케이스 보고. `InProcServer32` 에 raw 토큰이 박히면 COM 로더가 DLL 을 찾지 못한다.
3. **`wixl` 의 schema 부분 지원** — `SelfRegCost`, `Custom`, `ForceCreateOnInstall` 등 핵심 attribute 미지원으로 정적 레지스트리 + COM 자가 등록 이중 트랙 구현 불가.

## 파일 구조

```text
installer/
├── wix/
│   ├── unim.wxs                # WiX 3.x source (candle/light, MSVC 가정)
│   ├── gen-guids.sh            # globals.rs → generated/guids.wxi 단일 진실원
│   └── generated/
│       └── guids.wxi           # AUTO-GEN. globals.rs 변경 시 gen-guids.sh 재실행 후 커밋.
├── scripts/
│   ├── register-tsf.bat        # 수동 fallback (관리자 권한 regsvr32)
│   └── unregister-tsf.bat
├── assets/
│   └── unim.ico                # 선택. wxs 의 ARPPRODUCTICON 주석 해제 시 사용.
└── README.md                   # 본 문서
```

## 빌드 절차

### 표준 (GitHub Actions, 배포용)

1. Push to `main` / `develop` 또는 PR 생성, 또는 GitHub UI 에서 `Actions → Windows MSI → Run workflow` 수동 트리거.
2. 워크플로 (`.github/workflows/windows-msi.yml`) 가 다음을 실행:
   - `gen-guids.sh` 가 `globals.rs` 와 동기화돼 있는지 git diff 검사 (드리프트 시 실패).
   - `cargo check --workspace --target x86_64-pc-windows-msvc --locked`.
   - `cargo build --release --target x86_64-pc-windows-msvc -p unim-tsf -p unim-windows --locked`.
   - WiX 3.x (`candle` + `light -sval`) 로 `dist/unim-<version>-x64.msi` 생성.
3. Artifact `unim-<version>-x64-msi` 다운로드 → Windows 11 x64 VM 에서 설치 검증 (`docs/dev/windows/SMOKE_TEST.md`).

### 로컬 sanity (Linux 호스트)

배포용 산출물은 아니다. cross-compile 가능 여부만 확인한다.

```bash
# GUID/version 동기화
make wxi-guids

# 동기화 검증
make check-wxi-guids

# cargo check (mingw 또는 msvc 타깃)
make check-windows

# 풀빌드 (sanity 한정 — 산출물을 MSI 로 묶지 말 것)
make build-windows
```

## TSF 등록 — 이중 트랙

MSI 설치 시 **두 경로가 동시에 적용**된다 (둘 다 idempotent):

### (A) 정적 RegistryKey (wxs `<RegistryKey>`)

OS 가 TIP 메타데이터를 읽는 표준 위치를 채운다. 사용자 프로필 손상이나 sandbox 등으로 (B) 가 실패해도 IME 가 발견되는 보험.

| 키 경로 | 역할 |
|---|---|
| `HKCR\CLSID\{UNIM_CLSID}` | COM 클래스 이름 |
| `HKCR\CLSID\{UNIM_CLSID}\InProcServer32` | DLL 경로 + ThreadingModel=Apartment |
| `HKLM\SOFTWARE\Microsoft\CTF\TIP\{UNIM_CLSID}` | TIP 엔트리 |
| `…\LanguageProfile\0x00000412\{UNIM_PROFILE_GUID}` | 한국어 0x0412 프로필 (Enable=1) |
| `…\Category\Category\{CATGUID}\{UNIM_CLSID}` × 5 | 카테고리 등록 (MS SampleIME 패턴, 끝 GUID 가 CLSID) |
| `…\Category\Item\{CATGUID}\{UNIM_CLSID}` × 5 | 카테고리 아이템 |

5 개 카테고리: `TIP_KEYBOARD`, `DISPLAYATTRIBUTEPROVIDER`, `TIPCAP_UIELEMENTENABLED`, `TIPCAP_IMMERSIVESUPPORT`, `TIPCAP_SYSTRAYSUPPORT`.

### (B) DllRegisterServer (wxs `SelfRegCost="1"`)

`SelfRegCost` 가 `SelfReg` 테이블을 만들어 MSI 가 `InstallFinalize` 직전에 `DllRegisterServer` 를 호출. 이는 [`unim-tsf/src/register.rs::register_server()`](../unim-tsf/src/register.rs) 가 실행돼 다음 COM API 를 호출:

- `ITfInputProcessorProfiles::Register(&UNIM_CLSID)`
- `ITfInputProcessorProfiles::AddLanguageProfile(0x0412, …)`
- `ITfCategoryMgr::RegisterCategory(&UNIM_CLSID, &CAT_GUID, &UNIM_CLSID)` × 5

Microsoft 가 권장하는 정식 API 경로. 정적 레지스트리만으로는 OS TIP 캐시 무효화가 즉시 이루어지지 않는 사례가 있어 (B) 가 권장된다.

### 사용자 활성화 단계 (설치 후)

`설정 → 시간 및 언어 → 한국어 → 키보드 추가 → "UNIM Korean IME"`

이미 한국어가 설치된 시스템이라면 즉시 후보로 표시된다.

### 수동 fallback

UI 에 IME 가 보이지 않을 때:

```cmd
cd "C:\Program Files\UNIM"
register-tsf.bat   :: 관리자 권한, regsvr32 로 DllRegisterServer 재호출
```

## GUID / Profile 참조

| 항목 | 값 | 정의 위치 |
|---|---|---|
| `UNIM_CLSID` | `{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}` | `unim-tsf/src/globals.rs` |
| `UNIM_PROFILE_GUID` | `{B2C3D4E5-F6A7-8901-BCDE-F12345678901}` | `unim-tsf/src/globals.rs` |
| `UNIM_DISPLAY_ATTR_INPUT` | `{C3D4E5F6-A7B8-9012-CDEF-123456789012}` | `unim-tsf/src/globals.rs` |
| `UNIM_DISPLAY_ATTR_CONVERTED` | `{D4E5F6A7-B8C9-0123-DEF0-234567890123}` | `unim-tsf/src/globals.rs` |
| `UNIM_LANGBAR_ITEM_GUID` | `{E5F6A7B8-C9D0-1234-EF01-345678901234}` | `unim-tsf/src/globals.rs` |
| LangID | `0x0412` (Korean) | `unim-tsf/src/globals.rs` |
| UpgradeCode (MSI) | `{4F1A2B3C-5D6E-7F80-9A1B-2C3D4E5F6A7B}` | `installer/wix/unim.wxs` (Product 속성) |

**변경 시 절차**:

1. `unim-tsf/src/globals.rs` 수정.
2. `bash installer/wix/gen-guids.sh` 실행 → `installer/wix/generated/guids.wxi` 갱신.
3. 두 파일 모두 커밋. CI 는 `git diff --exit-code installer/wix/generated/` 로 드리프트 검사.

## 알려진 한계

1. **서명 미적용** — EV Code Signing 인증서 미보유. SmartScreen 경고는 사용자 우회 (`자세히 → 실행`) 가 필요. 정식 릴리스 시 별도 트랙.
2. **x86 미지원** — Platform=x64 only. 64-bit Windows 가 대상.
3. **ARM64 미지원** — windows-rs/MSVC ARM64 cross-build 검증 안 됨.
4. **AppX/MSIX 미지원** — Microsoft Store 배포 트랙은 별도.

## 검증 체크리스트

- `docs/dev/windows/SMOKE_TEST.md` — VM 설치 / 한글 입력 / 한자 변환 / 제거 매트릭스.
- `docs/dev/windows/MSI_DIAGNOSIS_TEMPLATE.md` — MSI 실패 시 채워 넣는 진단 양식.
