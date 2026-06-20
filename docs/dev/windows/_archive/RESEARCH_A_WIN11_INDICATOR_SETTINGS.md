# RESEARCH A — Win11 입력 표시기(가/A) 표시 설정 + 레거시 언어바 활성화 메커니즘

조사일: 2026-05-30 / 대상: Win11 22H2~24H2 / 대상 IME: UNIM (third-party TSF)

> 표기 규칙: [근거] = 1차/신뢰출처 확인됨, [확인 필요] = 단일/약한 출처라 실기 검증 필요.

---

## 질문 1 — Win11 설정의 입력 표시기 표시/숨김 옵션

### 1-A. 고급 키보드 설정 (Advanced keyboard settings)
경로: **설정 > 시간 및 언어 > 입력(Typing) > 고급 키보드 설정(Advanced keyboard settings)**
또는 검색창에 "Advanced keyboard settings" / "고급 키보드 설정".

해당 화면의 핵심 항목:
- **"사용 가능한 경우 바탕 화면 언어 모음 사용" (Use the desktop language bar when it's available)** — 체크하면 레거시 데스크톱 언어바가 켜짐. [근거: 다수 일치]
- **"언어 모음 옵션" (Language bar options)** — 위 체크 후 활성화. 클릭 시 레거시 "텍스트 서비스 및 입력 언어(Text Services and Input Languages)" 대화상자 열림. [근거]
- **"앱 창마다 다른 입력 방법을 사용하도록 허용"(Let me set a different input method for each app window)** — 표시기 직접 토글은 아님.

동작 차이:
- 체크 OFF(기본): 모던 시스템 트레이 입력 표시기(KOR/ENG, 한/영)만 사용.
- 체크 ON: 데스크톱 언어바(floating/docked)가 추가로 그려짐. third-party IME의 langbar item이 여기 그려질 수 있음.

### 1-B. 작업 표시줄 시스템 트레이
경로: **작업표시줄 우클릭 > 작업 표시줄 설정 > "기타 시스템 트레이 아이콘(Other system tray icons)"**. [근거]
- 입력 표시기는 **입력 방법(키보드 레이아웃/언어)이 2개 이상**일 때 자동 표시. [근거: 다수 일치]
- 1개뿐이면 트레이에 아예 안 뜸 → **UNIM의 핵심 의심 원인** (아래 체크리스트 참고).

---

## 질문 2 — 레거시 데스크톱 언어바 활성화

1-A의 "바탕 화면 언어 모음 사용" 체크 → 언어바 출현. "언어 모음 옵션" 대화상자(언어 모음 탭)에서 상태 선택: [근거]
- **바탕 화면에 떠 있음 (Floating On Desktop)**
- **작업 표시줄에 도킹 (Docked in the taskbar)**
- **숨김 (Hidden)**
부가: "작업 표시줄에 추가 언어 모음 아이콘 표시", "언어 모음에 텍스트 레이블 표시" 체크박스.

언어바는 ctfmon 기반으로 그려지며, third-party TSF IME가 등록한 langbar item(ITfLangBarItem)을 표시할 수 있음. 변경 후 **로그아웃/재로그인 또는 재부팅**이 필요한 경우 있음. [근거]

---

## 질문 3 — 관련 레지스트리 키

### `HKCU\Software\Microsoft\CTF\LangBar` [근거 확정]
- **ShowStatus** (DWORD): **`0`=바탕화면 floating, `3`=숨김, `4`=작업표시줄 docked**.
  (renenyffenegger.ch 레지스트리 노트로 확정. 일부 튜토리얼이 3/4를 뒤바꿔 적는 경우 있으니 이 매핑 기준.)
- **Label** (DWORD): `0`=텍스트 레이블 표시, `1`=숨김. [확인 필요: 단일 출처]
- **ExtraIconsOnMinimized** (DWORD): 추가 아이콘 표시 여부. [확인 필요: 단일 출처]
- **TransparentLevel / Transparency** (DWORD): floating 언어바 투명도. [확인 필요]

### `HKLM\SOFTWARE\Microsoft\CTF` 및 TSF 등록
입력 표시기에 IME가 뜨려면 TSF 등록의 **카테고리/LanguageProfile**이 올바라야 함. (MEMORY: UNIM v0.3.0에서 Categories·LanguageProfile 누락 결함 있었음 → 직접 연관) [확인 필요: 카테고리별 정확 효과]

> **[근거: MS Learn — IME requirements]** Win8 이후 모던 환경에서는 "IME 아이콘을 호스팅할 언어바가 없다(no language bar to host IME icons)". 대신 **입력 표시기(Input Indicator)**가 시스템 트레이에 떠서 **현재 실행 중인 IME의 브랜딩 아이콘**만 표시한다. 즉 트레이 "가/A"는 레거시 langbar가 아니라 **입력 표시기가 IME LanguageProfile의 아이콘**을 그리는 것 → UNIM의 LanguageProfile 아이콘/등록이 정확해야 트레이에 정상 표시됨.

### `HKCU\Control Panel\International\User Profile`
설치된 입력 프로필 목록 보관. 표시기 토글 직접 값은 미확인. [확인 필요]

---

## 질문 4 — 그룹 정책

언어바 관련 gpedit 정책 존재(예: 사용자 구성 > 관리 템플릿 > ... 언어바 끄기/켜기 류). 정확한 정책 경로/이름은 1차 확인 실패. [확인 필요]

---

## 질문 5 — ctfmon.exe / TextInputManagementService

- **ctfmon.exe** (CTF Loader): 언어바·입력 표시기·대체 입력(필기/음성/전환) 렌더링 주체. 미실행 시 언어바/표시기 안 뜸. `Win+R > ctfmon.exe`로 수동 기동 가능. [근거]
- **TextInputManagementService** (구 TabletInputService 계열): 텍스트 입력 관리 서비스. 비활성화 시 입력 관련 기능(표시기 포함) 오동작 가능. 자동 시작이어야 함. [근거]

---

## 결론 — "가/A 인디케이터를 켜는 OS측 조건" 체크리스트

> **결정적 발견 [근거: MS Learn IME requirements]**: 모던 Win10/11에는 IME 아이콘을 호스팅하는 언어바가 없고, 트레이 **입력 표시기**가 IME 브랜딩 아이콘 + 모드 아이콘을 그린다. 단, **"호환(compatible) IME"에만** 브랜딩/모드 아이콘이 표시된다. **호환되지 않는 IME는 브랜딩 아이콘이 안 뜨고, 입력 표시기가 대신 "언어 약어(language abbreviation)"만 표시**한다. 또한 IME 아이콘은 **.ico 단독 파일이 아니라 DLL/EXE 리소스에 저장**해야 한다. → UNIM이 "가/A" 브랜딩 아이콘 대신 "KO/한국어 약어"만 뜨거나 아예 안 뜬다면, LanguageProfile의 아이콘 등록(DLL 리소스 경로/인덱스) 또는 호환성 카테고리 등록이 빠진 것이 직접 원인일 가능성이 큼.

표시기가 뜨려면 아래가 **모두** 충족되어야 함:
1. [ ] 입력 방법이 **2개 이상** 등록 (단일 IME만 있으면 트레이 표시기 자체가 안 뜸 — 가장 흔한 원인).
2. [ ] UNIM TSF가 **카테고리 + LanguageProfile**까지 정상 등록 (MEMORY의 v0.3.0 결함 영역).
3. [ ] LanguageProfile의 **아이콘이 DLL/EXE 리소스로** 등록됨 (.ico 단독 X). 누락 시 약어만 표시.
4. [ ] **ctfmon.exe** 실행 중.
5. [ ] **TextInputManagementService** 자동 시작/실행 중.
6. [ ] 트레이 표시기를 쓸 거면 언어바 체크 OFF; 레거시 langbar item을 그릴 거면 "바탕 화면 언어 모음 사용" ON + ShowStatus=0 또는 4 (3=숨김 피할 것).

---

## 사용자가 지금 당장 토글/확인할 항목

### 설정 경로
- 설정 > 시간 및 언어 > 입력 > 고급 키보드 설정 → "사용 가능한 경우 바탕 화면 언어 모음 사용" 체크 → "언어 모음 옵션"에서 floating/docked 선택.
- 작업표시줄 우클릭 > 작업 표시줄 설정 > 기타 시스템 트레이 아이콘에서 입력 관련 토글 확인.

### reg query 명령 (PowerShell)
```powershell
reg query "HKCU\Software\Microsoft\CTF\LangBar"
reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP" /s
reg query "HKCU\Control Panel\International\User Profile" /s
Get-Service TextInputManagementService | Format-List Status,StartType
Get-Process ctfmon -ErrorAction SilentlyContinue
```

### 레거시 언어바 강제 켜기 (검증용)
```powershell
reg add "HKCU\Software\Microsoft\CTF\LangBar" /v ShowStatus /t REG_DWORD /d 0 /f   # 0=floating
# 적용: 로그아웃/재로그인 또는 ctfmon 재시작
```

---

## 출처
- MS Learn — Default Input Profiles for Windows Language Packs (관련 배경, 표시기 직접문서 아님).
- ElevenForum/TenForums "Enable or Disable Language Bar and Input Indicator" 튜토리얼 시리즈 (ShowStatus/Label 값, 설정 경로).
- 다수 웹검색 일치 결과: Advanced keyboard settings 경로, 작업표시줄 "기타 시스템 트레이 아이콘", ctfmon/TextInputManagementService 역할, 2개 이상 입력방법 조건.

## 미확보/추가 조사 필요
- `CTF\LangBar` 값 의미의 MS **1차** 문서.
- 그룹 정책 정확 경로/이름.
- third-party TSF가 트레이 표시기에 뜨기 위한 **카테고리 GUID별 정확한 효과** (SampleIME 표준 8종과 표시기 관계).
