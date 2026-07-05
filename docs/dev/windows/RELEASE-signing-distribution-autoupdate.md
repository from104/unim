# UNIM Windows 배포 준비 — 코드 서명 · 배포 · 자동 업데이트

> 작성: 2026-07-06. 대상: UNIM Windows(TSF IME) MSI 배포.
> 현재 상태: MSI 미서명, 수동 배포(GitHub). 이 문서는 서명·배포·자동 업데이트 도입
> 방안과 **실행 체크리스트**를 정리한다. (2023~2026 코드서명 환경 변화 반영.)

---

## 0. 먼저 알아야 할 3가지 (결정에 직접 영향)

1. **Azure 서명은 한국 개인 개발자가 못 쓸 가능성이 큼.**
   Azure Artifact Signing(구 Trusted Signing)의 Public Trust 인증서는 **개인=미국·캐나다만**,
   **조직=미국·캐나다·EU·영국만** 지원(한국 미포함, 2026-07 기준). 한국이면 일반 CA 의 OV
   인증서 경로가 현실적. (지원 국가는 확장될 수 있으니 발주 전 재확인.)

2. **EV 인증서의 SmartScreen 이점이 사라짐.**
   2024년 3월 Microsoft Trusted Root 정책 변경으로 **EV·OV 가 동등하게** 다운로드 볼륨으로
   SmartScreen 평판을 쌓는다. EV 는 (a)게시자명 검증 표시 (b)**커널모드 드라이버 서명**에만
   유효한 추가 가치. **TSF IME 는 user-mode 라 EV 불필요 → OV 로 충분**(더 저렴).

3. **IME 자동 업데이트는 "알림형"이 안전.**
   IME DLL(`unim_tsf.dll`)은 텍스트 입력이 있는 **모든 프로세스에 로드**되므로 파일 교체 시
   그 프로세스들이 DLL 을 놓아야 한다(대개 재로그온/재부팅 필요). 완전 사일런트 자동교체는
   위험 → **알림 후 사용자가 설치·재로그온**하는 방식을 권장.

---

## 1. 코드 서명 (Code Signing)

### 1.1 왜 필요한가
- 미서명 실행/설치 파일은 SmartScreen 이 "알 수 없는 게시자" 경고 → 설치 이탈.
- TSF IME DLL 은 타 프로세스에 로드되므로 신뢰가 특히 중요(일부 보안 환경은 미서명 차단).

### 1.2 옵션 비교

| 옵션 | 비용 | 한국 | CI 자동화 | 하드웨어 | 비고 |
|---|---|---|---|---|---|
| **A. Azure Artifact Signing** | $9.99/월(5,000건)·$99.99/월(10만건) | ❌ 미지원 추정 | ◎ 최적 | 불요(클라우드) | 유료 Azure 구독 필수. 무료/체험 구독 불가 |
| **B. OV 인증서 (CA)** ⭐권장 | ~$200~400/년 | ✅ | ○ (클라우드HSM 선택 시) | USB 토큰 or 클라우드 HSM | SSL.com·Sectigo·DigiCert·GlobalSign |
| C. EV 인증서 | ~$300~600/년 | ✅ | ○ | HSM 필수 | IME 엔 과투자(커널 드라이버 아님) |

**권장: B(OV) + 클라우드 HSM 서명(예: SSL.com eSigner).**
- 이유: 한국 가능, EV 대비 SmartScreen 동등(2024.3~), user-mode 라 EV 불필요, 클라우드 HSM 이면
  CI 에서 USB 없이 서명 가능.

### 1.3 2023~2026 규정 변화 (중요)
- **2023.6.1~**: OV·EV 모두 **하드웨어 토큰(FIPS 140-2) 또는 클라우드 서명** 필수. 파일형
  `.pfx` 사설키 폐지 → CI 자동화하려면 **클라우드 HSM(eSigner류)** 을 골라야 USB 없이 서명.
- **2024.3~**: EV 의 SmartScreen 우대 폐지 → OV·EV 평판 동등(다운로드 볼륨 기반).
- **2026.3.1~**: 공개신뢰 코드서명 인증서 **유효기간 최대 458일**(CA/B Forum) → 갱신 주기 유의.

### 1.4 서명 대상 (순서 중요)
1. `unim_tsf.dll` (x64) — `target\x86_64-pc-windows-msvc\release\`
2. `unim_tsf.dll` (x86/i686) — `target\i686-pc-windows-msvc\release\` (WOW64/카톡 등 32비트 호스트)
3. `unim-tsf-settings.exe`
4. `unim-popup-win.exe`
5. **MSI** (`dist\unim-<ver>-x64.msi`) — 위 4개를 먼저 서명 후 패키징하고, 마지막에 MSI 서명

> `scripts\build-msi.bat` / `.github/workflows/windows-msi.yml` 의 candle/light **전에** DLL·EXE 서명,
> light **후에** MSI 서명을 삽입한다. (서명 스텝은 인증서 확보 후 추가.)

### 1.5 SmartScreen 평판
- 서명해도 신규 게시자는 **초기 다운로드 구간에 경고**가 뜰 수 있고, 클린 설치 다운로드가 쌓이며
  사라진다(즉효 아님). EV 라도 즉시 무경고는 아님(2024.3 정책 후).

---

## 2. 배포 (Distribution)

| 채널 | 역할 | 작업량 | 비고 |
|---|---|---|---|
| **GitHub Releases** | 주 채널 | 소 | 서명 MSI 첨부. 태그→빌드→서명→릴리스 CI 자동화 |
| **winget** | 발견성·업데이트 | 중 | microsoft/winget-pkgs 에 매니페스트 PR. `winget install/upgrade` |
| **웹사이트(atit.org)** | 직접 다운로드 | 소 | 최신 MSI 링크 |
| Scoop / Chocolatey | 선택 | 소 | 커뮤니티 패키지 매니저 |
| MS Store | 비권장 | 대 | TSF+MSIX 제약 큼 |

- **winget**: `wingetcreate` 툴로 매니페스트 생성·제출·업데이트 자동화. YAML 매니페스트를
  microsoft/winget-pkgs 에 PR → 자동 검증 통과 시 등재. PackageIdentifier 예: `atit.UNIM`.
- Linux(deb/rpm/AUR)는 별도 파이프라인(기존).

---

## 3. 자동 업데이트 (Auto-update)

### 3.1 전략 (2단계)
1. **1차 — winget 위임**: winget 등록만 하면 사용자가 `winget upgrade`(또는 자동 업그레이드
   설정)로 갱신. **추가 개발 0**. 가장 먼저 확보.
2. **2차 — 인앱 알림형**: 설정앱/팝업서비스가 GitHub Releases API(`/repos/from104/unim/releases/latest`)로
   최신 태그 vs 현재 버전(`CARGO_PKG_VERSION`) 비교 → "업데이트 있음" 표시 → 다운로드+설치
   안내(+**재로그온 필요** 고지). 이미 구현한 설치 마법사의 **`--whats-new`** 모드와 자연 연결.

### 3.2 왜 사일런트 자동교체를 피하나
- IME DLL 이 로드 중이면 MSI 가 파일 교체를 **다음 재부팅으로 예약**하거나 실패. 사용자 모르게
  교체하면 입력 중 IME 가 불안정해질 수 있음 → 사용자가 시점을 통제하는 알림형이 안전.

### 3.3 프레임워크 후보
- **직접 구현(권장)**: GitHub Releases API 폴링 + 알림 UI. IME 특성상 통제 쉬움. Rust 로 설정앱/
  서비스에 통합.
- WinSparkle: appcast(XML) 기반, MSI 친화 C 라이브러리. 표준적이나 IME 재시작 통제엔 커스텀 필요.

---

## 4. ✅ 실행 체크리스트 (우선순위)

### A. 결정 (먼저)
- [ ] 서명 방식: **OV 인증서 + 클라우드 HSM(eSigner)** 확정 — 단, Azure Artifact Signing 한국
      지원 여부 먼저 확인(지원되면 월 $9.99 A안이 더 저렴·CI 편함)
- [ ] 발급 주체: **개인 vs 사업자** — 개인 OV 는 CA 마다 검증 요건 상이(일부는 사업자 요구)

### B. 서명 셋업
- [ ] CA 선택·주문(예: SSL.com eSigner), 신원 검증 통과(**수일~수주 소요**)
- [ ] 서명 자격증명을 GitHub Actions Secrets 등록(클라우드 서명 API 키/계정)
- [ ] CI 에 서명 스텝 추가: DLL(x64·x86)·EXE 2종 → 패키징 → MSI 순
- [ ] 서명 검증 확인(`signtool verify /pa`), SmartScreen 실측

### C. 배포 셋업
- [ ] GitHub Releases 자동화 워크플로우(태그 push → 서명 MSI 첨부)
- [ ] winget 매니페스트 작성·PR(`atit.UNIM`)
- [ ] atit.org 다운로드 페이지 갱신

### D. 자동 업데이트
- [ ] winget 등록으로 1차 확보
- [ ] 인앱 버전 체커 구현 여부 결정 → 하면 설정앱에 통합(`--whats-new` 연동)

---

## 5. 🤝 Claude 가 도울 수 있는 것 (코드/설정)
- CI 서명 스텝 + GitHub Releases 릴리스 워크플로우 작성 (인증서 확보 후)
- winget 매니페스트(YAML) 생성
- **인앱 업데이트 체커**(GitHub Releases API → 알림 → 마법사 `--whats-new` 연동) 설계·구현
- 서명/SmartScreen 검증 절차 문서화

## 6. 🚀 인증서 확보 전에 미리 착수 가능한 것
아래는 **서명 인증서 없이도** 지금 만들 수 있어, 인증서 도착 즉시 릴리스 가능:
1. **winget 매니페스트** 초안(서명 후 URL/해시만 갱신)
2. **인앱 업데이트 체커**(GitHub Releases API 버전 비교 + 알림 UI)
3. **GitHub Releases CI 워크플로우**(서명 스텝만 나중에 삽입)

---

## 참고 (2026-07 확인)
- Azure Artifact Signing 가격: https://azure.microsoft.com/en-us/pricing/details/artifact-signing/
- Trusted Signing 개인 개발자 오픈(미/캐): https://techcommunity.microsoft.com/blog/microsoft-security-blog/trusted-signing-is-now-open-for-individual-developers-to-sign-up-in-public-previ/4273554
- EV vs OV / SmartScreen 변화(SSL.com): https://www.ssl.com/faqs/which-code-signing-certificate-do-i-need-ev-ov/
- Windows 코드서명 옵션(Microsoft Learn): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- winget 매니페스트 제출(Microsoft Learn): https://learn.microsoft.com/en-us/windows/package-manager/package/repository
- wingetcreate: https://github.com/microsoft/winget-create
