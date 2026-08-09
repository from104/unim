# UNIM Windows — 웹조사 갭 목록 (다음 단계 웹조사 입력)

> 작성일 2026-06-19 · `_KNOWLEDGE_STATE.md` open 항목 중 **기존 문서로 답이 안 나오고 외부 1차소스가 필요한 것만** 추림.
> 순수 VM 실측으로만 풀리는 항목(O1 inline 실동작, O2 팝업 렌더, O4·O5 ATF 폴백 등)은 웹조사 대상 아님 — 여기서 제외.
> 각 갭: [조사 질문] · [우선순위] · [기대 1차소스]. needsWeb=true 였던 항목 중심.

---

## G1. CUAS가 composition을 live(GCS_COMPSTR)로 유지 vs terminate하는 정확한 상태머신 계약 — **P0**

**질문**: msctf.dll의 CUAS default text store가 TSF composition을 GCS_COMPSTR(미확정)로 유지하는 조건과 OnCompositionTerminated:IMMEDIATE를 발사하는 정확한 트리거는? fInterimChar=TRUE가 'interim character' 의미론으로 어떻게 WM_IME_COMPOSITION GCS_* 시퀀스로 매핑되는가? Win8 이후 CUAS가 'Vista+ always-on'인지, EnableCicero/CTF SFM 레지스트리 키가 실재하는지?

**기대 1차소스**:
- MS Learn: `learn.microsoft.com/windows/win32/tsf/` (CUAS·desktop-src/TSF 슬러그 — 기존 nn-msctf/ns-msctf가 404였으므로 `desktop-src/TSF/*` GitHub 경로로 재시도: `github.com/MicrosoftDocs/win32/tree/docs/desktop-src/TSF`)
- MS Learn TF_SELECTIONSTYLE Remarks(msctf.h) — fInterimChar 한국어 명시 재확인
- Wine `dlls/msctf/` + `dlls/imm32/` 소스(GCS_* 합성 경로), ReactOS `msctfime.ime`(@unimplemented 주의)
- katahiromz/ImeStudy, Mozc `tip_edit_session_impl.cc`(이미 인용, 절구조)

**메모**: 이론적 black box. VM Spy++ WM_IME_* diff(O1)와 상보. 웹조사로 계약 확정 못 하면 '경험적 확정(3 OSS + Remarks)'에 머묾 — 그것으로도 실용 충분, 이 갭은 엣지케이스 예측력용.

---

## G2. KakaoTalk/한컴 등 순수 IMM32 네이티브 앱에 TIP이 도달하는 표준 경로 — **P0**

**질문**: TSF 프로파일만 활성일 때 IMM32-only로 키를 소비하는 32비트 네이티브 앱(OnTestKeyDown/OnKeyDown 0회)에 한글 입력을 전달하는 정석은? (a) .ime를 같은 언어바 항목으로 듀얼 등록(CTF\Assemblies + Substitutes)하는 것이 실제 동작하는가, (b) Mozc는 이 케이스를 어떻게 처리/포기하는가, (c) WM_IME_REQUEST IMR_DOCUMENTFEED/IMR_RECONVERTSTRING이 ATF surrounding-text 채널로 동작하는 실제 앱 범위는?

**기대 1차소스**:
- MS Learn: IMM32 `imm-functions`, `WM_IME_REQUEST`(IMR_DOCUMENTFEED/IMR_RECONVERTSTRING), `ImmInstallIME`/`LoadKeyboardLayoutW`
- MS Learn CTF Assemblies/Substitutes: `learn.microsoft.com/windows-hardware/...` 또는 w8cookbook input-method 문서
- Mozc repo(`google/mozc`) win32/ime·win32/tip 디렉토리 — .ime 폐기 실증 + IMM32 read-side 폴백 코드
- SampleIME(MS) Register.cpp — Assemblies 등록 유무

**메모**: O4(유일한 구조적 ❌)의 해결 경로 결정. 듀얼모드 채택 여부(O9)와 직결.

---

## G3. Win11 24H2/25H2 Input Indicator anchor 회귀 + third-party SHOWNINTRAY 노출 조건 — **P1**

**질문**: Win11이 SHOWNINTRAY 플래그를 가진 third-party ITfLangBarItem을 새 작업표시줄 트레이/Input Indicator에 그리는 정확한 조건(추가 요구사항)이 MS 1차 문서에 있는가? 24H2/25H2 anchor 좌표 회귀의 공식 인지/수정 빌드는? 'compatible IME requirements'의 구체 체크리스트는?

**기대 1차소스**:
- MS Learn: `input-method-editor-requirements`(Custom IME requirements), w8cookbook `third-party-input-method-editors`
- MS Q&A / Feedback Hub: 25H2 Input Indicator UI bug 스레드
- GitHub `rime/weasel#1682`(작업표시줄 langbar item 영구 소멸 실사례) — 회귀 재현/우회
- 기존 404 회피: `github.com/MicrosoftDocs/win32` desktop-src/TSF langbar 슬러그 직접

**메모**: 해소(GetIcon E_FAIL+아이콘 임베드는 이미 코드 완료 — _KNOWLEDGE_STATE §5) 적용 후에도 floating 글리프 미표시면 OS 제약인지 판별. 미표시 확정 시 별도 `unim-tray.exe`(Track B) 결정 입력.

---

## G4. CTF\LangBar ShowStatus 값 의미·그룹정책·빌드별 매트릭스 — **P1**

**질문**: `HKCU\Software\Microsoft\CTF\LangBar\ShowStatus`(DWORD)의 0/3/4 정확 의미(0=floating, 3=숨김, 4=docked 추정의 1차 확인), 그룹정책 경로, Win11 23H2 vs 24H2 vs 25H2별 third-party langbar 노출 차이는?

**기대 1차소스**:
- MS Learn / Group Policy 레퍼런스(Text Services Framework, 언어바 정책)
- (커뮤니티 출처 ElevenForum/TenForums는 3/4 뒤바꿔 적는 사례 있음 — 1차 교차검증 필요)
- renenyffenegger.ch 레지스트리 노트(2차, 보조)

**메모**: 사용자에게 레거시 langbar 강제 켜기 안내 시 값 어긋남 방지. VM reg add로 실측 병행.

---

## G5. ATTR_TARGET_CONVERTED 보장 + 한자 후보 선택 시 GCS_COMPATTR 계약 — **P2**

**질문**: 한국어 한자 후보 선택 중 ATTR_TARGET_CONVERTED(GCS_COMPATTR)가 보장되는가? MS 한국어 IME는 별도 후보창을 띄우며 composition을 비우는데(Scintilla #2392 'no composition string or target'), 일본어식 in-composition target-clause 하이라이트 모델이 한국어에 성립하는가?

**기대 1차소스**:
- MS Learn: `WM_IME_COMPOSITION` GCS_COMPATTR / `ImmGetCompositionString` ATTR_* 값 정의
- GitHub Scintilla #2392(no composition string or target) — 실증
- (확정은 실기기 GCS_COMPATTR dump 필요 — 웹은 계약 확인까지)

**메모**: 팝업 도메인 H2(UILess) 및 한자 팝업 attribute 렌더 정확성과 연결. 우선순위 낮음(렌더러가 중앙 자체표시라 영향 제한적).

---

## G6. .ime PE 표시명 리소스(-1/-1000) + 코드서명 요구 범위 — **P2**

**질문**: 'Layout Display Name'=@...,-1 가 참조하는 string resource를 .ime PE에 STRINGTABLE로 넣는 정석은? Win10/11에서 unsigned .ime/.dll/.msi가 거부되는 정확한 환경(UWP/protected-process/특정 AV/기업정책)은?

**기대 1차소스**:
- MS Learn: keyboard layout 'Layout Display Name' 레지스트리 + `@dll,-id` 리소스 규약, `STRINGTABLE`/rc.exe
- MS Learn 코드서명: Authenticode/IME 로드 정책, AppContainer/protected process 요구
- saenaru/NavilIME repo의 .rc/STRINGTABLE 예시

**메모**: O8/O12 흡수. 표시명은 cosmetic, 코드서명은 LOW(blocker A 확정 후에만 의심).

---

## 갭 우선순위 요약

| ID | 갭 | 우선 | needsWeb 근거 도메인 |
|---|---|---|---|
| G1 | CUAS 상태머신 계약 | P0 | 도메인1·4·5 (msctf black box) |
| G2 | 순수 IMM32 네이티브 TIP 도달 경로 | P0 | 도메인3 (KakaoTalk/한컴 순방향 ATF) |
| G3 | Win11 트레이 SHOWNINTRAY 노출 조건·회귀 | P1 | 도메인6 (404 슬러그 미확보) |
| G4 | ShowStatus 값·정책 매트릭스 | P1 | 도메인6 (커뮤니티 출처 의존) |
| G5 | ATTR_TARGET_CONVERTED/한자 GCS_COMPATTR | P2 | 도메인4 |
| G6 | .ime 표시명 리소스·코드서명 범위 | P2 | 도메인3 |

> **다음 단계 권고**: G1·G2를 묶어 deep-research 1회(MS Learn desktop-src/TSF + Wine msctf + Mozc win32/tip 교차). G3·G4는 Win11 트레이 트랙으로 묶음. G5·G6은 .ime 트랙 후순위. **단, O1·O2(VM 실측)가 웹조사보다 정보량 큼 — 웹조사와 병행/선행 권장.**
