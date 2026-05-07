# UNIM Windows MSI Installer

Linux 호스트에서 cross-compile한 UNIM 산출물(`unim-windows.exe` + `unim_tsf.dll`)을 단일 `.msi`로 패키징한다.

## 산출물 구조

```
installer/
├── wix/
│   └── unim.wxs              # WiX 3 source (wixl 호환)
├── scripts/
│   ├── register-tsf.bat      # 수동 TSF 프로필 등록 (관리자 권한)
│   └── unregister-tsf.bat    # 수동 TSF 프로필 해제
├── assets/
│   └── unim.ico              # ARP 아이콘 (placeholder, 별도 생성 필요)
└── README.md                 # 본 문서
```

## 빌드 도구

### Linux (권장: 크로스 빌드)

```bash
sudo apt install msitools     # wixl, msiinfo 등 제공
```

`wixl`은 WiX 3 schema의 핵심 부분(Product/Component/File/Registry/Shortcut/Feature/MajorUpgrade)을 지원하며, 일부 고급 기능(SDK CustomAction, Burn bundle 등)은 미지원.

### Windows (대안)

```cmd
# WiX Toolset 3.x
candle.exe -arch x64 -out unim.wixobj installer\wix\unim.wxs
light.exe  -out unim-0.2.0-x64.msi unim.wixobj
```

또는 `cargo install cargo-wix` 후 `cargo wix --target x86_64-pc-windows-msvc`.

## 빌드 절차

```bash
# 1) Windows release 빌드 (산출물: target/x86_64-pc-windows-gnu/release/{unim-windows.exe, unim_tsf.dll})
make build-windows

# 2) MSI 생성
make msi

# 결과
ls -lh dist/unim-0.2.0-x64.msi
```

`make msi`는 다음을 수행:
1. `make build-windows` 의존성 (산출물 없으면 자동 빌드)
2. `installer/assets/unim.ico` 존재 검증 (없으면 placeholder 생성)
3. `wixl -v -a x64 -D WIN_OUT_DIR=... -o dist/unim-0.2.0-x64.msi installer/wix/unim.wxs`
4. `msiinfo`로 결과 검증

## TSF 등록 흐름 (완전 정적)

MSI 설치 시 **모든 TSF 등록이 레지스트리만으로 처리**된다 (regsvr32 호출 없음). 총 20개 RegistryKey 행:

1. **COM CLSID** (HKCR\CLSID\{CLSID}) — DllGetClassObject 진입점
2. **InProcServer32** — DLL 경로 + Apartment threading
3. **TIP entry** (HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID})
4. **LanguageProfile** (LangID `0x0412` Korean) — `Description="UNIM Korean IME"`, `Enable=1`
5. **5개 Category** (TIP_KEYBOARD, DISPLAYATTRIBUTEPROVIDER, UIELEMENTENABLED, IMMERSIVESUPPORT, SYSTRAYSUPPORT) — Category/Item 양쪽 키
6. **Start Menu** — `UNIM Korean IME` 실행, `Uninstall UNIM` 제거

설치 후 사용자 액션:
- **Settings → Time & Language → Language → 한국어 → Add a keyboard → UNIM Korean IME**

(이미 한국어가 추가되어 있으면 즉시 후보로 표시됨)

수동 fallback (UI에서 안 보일 때):
```cmd
cd "C:\Program Files\UNIM"
register-tsf.bat   :: 관리자 권한 (DllRegisterServer 호출)
```

## CLSID / Profile GUID (참조)

| 항목 | 값 |
|---|---|
| `UNIM_CLSID` | `{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}` |
| `UNIM_PROFILE_GUID` | `{B2C3D4E5-F6A7-8901-BCDE-F12345678901}` |
| `UNIM_DISPLAY_ATTR_INPUT` | `{C3D4E5F6-A7B8-9012-CDEF-123456789012}` |
| `UNIM_DISPLAY_ATTR_CONVERTED` | `{D4E5F6A7-B8C9-0123-DEF0-234567890123}` |
| `UNIM_LANGBAR_ITEM_GUID` | `{E5F6A7B8-C9D0-1234-EF01-345678901234}` |
| LangID | `0x0412` (Korean) |
| UpgradeCode (MSI) | `{4F1A2B3C-5D6E-7F80-9A1B-2C3D4E5F6A7B}` |

원본: `unim-tsf/src/globals.rs`. **변경 시 wxs도 함께 갱신**해야 한다.

## 알려진 한계

1. **MSVC vs GNU**: 현재 cross-compile은 mingw GNU. MSVC ABI 권장 시 별도 CI(Windows runner)에서 빌드.
2. **아이콘 placeholder**: `installer/assets/unim.ico`는 32x32 PNG 또는 정식 .ico로 교체 필요.
3. **서명 미적용**: SmartScreen 경고 회피하려면 EV Code Signing 인증서로 `signtool sign` 필요(릴리스 단계 과제).
4. **SelfReg deprecated**: Microsoft 권장은 정적 레지스트리. 단, TSF profile registration은 API 호출이라 SelfReg 외 깔끔한 대안 없음.
