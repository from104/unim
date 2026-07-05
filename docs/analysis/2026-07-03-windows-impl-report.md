# Windows 구현 세션 종합 검증 보고서 (2026-07-03)

브랜치: `feat/windows-msi-redesign`
세션 범위: `22be5c5`(v0.3.51) 이후 ~ `1d70351`(v0.3.54)
성격: 단기~중기 배포위생 · 접근성 · 코어 perf 일괄 구현

## 요약

- **랜딩: 13건 / 리버트: 0건 / 스킵: 0건**
- **device-QA 필요: 10건** (실기기·스크린리더·육안 확인 대기)
- 코어 회귀 테스트 **그린**: `cargo test -p unim` → 690 passed, 0 failed (+ doctest 19 passed, 1 ignored)
- 릴리스 범프 `1d70351` (0.3.54), 배포신뢰 사양서 `d6edccf` 별도 이관 완료

## 아이템별 결과

| 아이템 | 제목 | P | 결과 | 커밋 | verify | device-QA |
|---|---|---|---|---|---|---|
| I1 | 릴리스 로그 기본 OFF + 콘텐츠 마스킹 | P0 | 랜딩 | `408d3e2` | ok | 불요 |
| I2 | 무가드 .expect() 제거(호스트 abort 차단) | P0 | 랜딩 | `1981f0c` | ok | 필요 |
| I3 | ATF 핫패스 engine.reset() perf | P1 | 랜딩 | `cfc3719` | ok | 필요 |
| I4 | per-app 앱호환 config 외부화(word_mode_apps) | P1 | 랜딩 | `9edd33b` | ok | 필요 |
| I7 | 한/영 전환 능동 통지(NotifyWinEvent + 비프) | P1 | 랜딩 | `1df0ca5` | ok | 필요 |
| I5 | 팝업 렌더러 OS 테마·고대비 대응 | P0 | 랜딩 | `777b691` | ok | 필요 |
| I6 | 파괴적 삭제 확인 다이얼로그 + 되돌리기 토스트 | P0 | 랜딩 | `0c53ba1` | **concerns** | 필요 |
| I9 | 크래시 리포팅 + catch_unwind 착시 정리 | P1 | 랜딩 | `e5742ff` | ok | 필요 |
| I8 | 조합·후보 TSF UILess UIElement 접근성 노출 | P0 | 랜딩 | `e48e7db` | ok | 필요 |
| I11 | 후보 팝업 창 UIA 제공자(루트) | P1 | 랜딩 | `53f5e24` | ok | 필요 |
| I12 | 후보 팝업 캐럿 앵커링(+돋보기 추종) | P2 | 랜딩 | `862844a` | ok | 필요 |
| I10 | CI 테스트 게이트 | P1 | 랜딩 | `5b2d005` | ok | 불요 |
| DOC | 배포신뢰 후속 사양서(DEPLOY-TRUST-PLAN) | P0~P1 | 랜딩 | `d6edccf` | ok | 불요 |

> 참고: 세션 범위 내 선행 커밋 `d68f59e`(메이저 IME 분석 보고서), `d57ec4d`(langbar 시그니처 아이콘 + 이모지 팝업 정렬, 0.3.53)는 본 아이템 세트 이전 작업으로 위 표에서 제외.

## verify=concerns 주의: I6

I6는 기능은 랜딩됐으나 아래 코드 레벨 우려가 남아 있어 device-QA에서 우선 확인 대상:

- **5초 내 연속 삭제 2회**: `toast-visible`이 true→true라 Slint `if` 블록이 재생성되지 않아 기존 5초 타이머가 리셋되지 않음(스냅샷 자체는 최신 1개로 정상 덮어씀). 필요 시 `toast-nonce: int` 강제 리셋 도입 검토.
- 스크린리더/육안 실측으로 확인 다이얼로그 포커스·Esc/Enter 동작, 토스트 하단배치·5초 자동소멸, AT 라이브 리전 announce 확인 필요.
- 개별 삭제는 확인 다이얼로그 없이 되돌리기 가역성으로 3.3.4 충족(대량삭제만 확인). 개별에도 확인을 원하면 추가 배선 필요.

## device-QA 대기 체크리스트 (사용자 몫)

실기기 · 스크린리더 · 육안 확인이 필요한 항목. 자동 코드런으로 완결 불가.

### A. 스크린리더 (NVDA / Narrator)
- [ ] **I7** 한/영 토글 시 능동 낭독되는가 (NotifyWinEvent 대상=전경창 OBJID_CLIENT). 미흡 시 UIA 커스텀 프로퍼티/TTS 직접호출 대안 검토.
- [ ] **I8** (1)조합 중 음절 낭독 (2)한자/이모지/특수문자 후보 목록·선택 낭독 (3)페이지 전환 재낭독 (4)포커스전환 시 stale 미낭독.
- [ ] **I11** inspect.exe(Accessibility Insights)로 (1)팝업 창 존재+이름 낭독 (2)ControlType=목록 인식 (3)AutomationId 노출. 자식/이벤트 followup 완료 후 셀·선택 낭독 재검증.
- [ ] **I6** 확인 다이얼로그/토스트가 AT 라이브 리전으로 announce 되는가.

### B. 라이트/고대비 육안
- [ ] **I5** 라이트 데스크톱(AppsUseLightTheme=1)에서 한자 compact·이모지 격자 팝업 Latte 팔레트 가독성/대비.
- [ ] **I5** 고대비 테마 4종에서 GetSysColor 팔레트·선택 링·헤더 밑줄 형태 단서.
- [ ] **I5** 색약/스크린리더 관점에서 선택 셀 링·활성 헤더가 색 없이도 구분되는가. (라이트 flash amber #df8e1d 대비 낮음 — 필요 시 텍스트 검정 고정 검토)
- [ ] **I7** toggle_announce_beep=true 시 한글 880Hz/영문 440Hz 비프 실청취, 연속 토글 스레드 누수·중첩음 확인.

### C. 캐럿 앵커링 / 돋보기 (I12)
- [ ] 실앱(메모장·크롬·Word)에서 팝업이 캐럿 하단에 뜨는가, 하단 근처에서 위로 플립되는가, Windows 돋보기 뷰포트를 추종하는가.
- [ ] GetTextExt 미지원/0 rect 앱(일부 터미널·레거시)에서 중앙 폴백 정상 동작.

### D. 조합/교정 실기 회귀
- [ ] **I3** ATF 순방향(영→한)·역방향(한→영) 교정 정확성 + 교정 직후 이어치기, reset() 확장이 팝업 닫힘/포커스아웃 회귀 없는지.
- [ ] **I2** 터미널(wezterm/wmux) 폴백 경로 앱에서 조합 시 오버레이 정상 표시(성공 경로 회귀 여부). 랭바 한/영 버튼 정상 표시 1회.
- [ ] **I4** config.yaml word_mode_apps 에 앱 추가(예 kakaotalk.exe) 후 Smart 모드 단어 전환, 기본값 winword/wmux 종전 동일 확인. Win32 다이얼로그(settings_dialog.rs) 신규 '단어 모드' 섹션 클리핑/오버플로, Slint SettingRow 렌더 확인.

### E. 크래시 리포팅 (I9)
- [ ] 호스트 앱(msctf/크롬/워드) 패닉 유발 시 `%APPDATA%\unim\crash\` 마커 파일 실제 생성, AppContainer(스티커메모) 토큰에서 폴백 `%TEMP%` 라도 남는지.

### F. MSI 설치
- [ ] MSI 재빌드 후 클린 설치 → TIP 로드 · 랭바 인디케이터 · 팝업 렌더러 상주 정상 여부(세션 변경분 반영 확인).

## 배포신뢰 후속 (이관 명시)

서명 · 온보딩 · 자동업데이트는 **인증서 발급/설치 QA/업데이트 인프라 등 사람+인프라 의존**이라 자동 코드런으로 완결 불가. 사양은 `docs/dev/windows/DEPLOY-TRUST-PLAN.md`(커밋 `d6edccf`, 275줄)로 이관됨.

선행 조건(사람 몫):
- (a) Azure Trusted Signing 신원검증 또는 SignPath OSS 승인 + GitHub 시크릿 등록.
- (b) `scripts/build-msi.bat`에 `-ext WixUIExtension` 플래그 추가는 스크립트 소유자 의뢰(편집 금지 제약). `set_as_default()`의 `unim-windows-common` 이동 리팩터(호출부 lang_bar.rs:680, settings_dialog.rs:1398 재수출 유지).
- (c) 자동업데이트는 (a) 완료 후 게시자 CN 확정돼야 착수 가능.

## 제외 항목 재확인

- **옛한글(고어 조합)**: 리눅스 + 코어 엔진 후속 과제. 이번 Windows 세션 범위 밖.
- **한자 단어 변환**: 후속 과제. 이번 세션 범위 밖.

## 코어 테스트 최종 그린

```
cargo test -p unim
test result: ok. 690 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (60.50s)
Doc-tests: 19 passed; 0 failed; 1 ignored (6.20s)
```

## 크로스-프런트엔드 / Linux 후속

- **I3** reset() perf: unim-dbus/src/engine_worker.rs:852,884 동일 `*engine = InputEngine::new(config)` 패턴 → 동일 최적화 검토(Linux/CI 몫). 확장된 reset() 팝업 초기화가 Linux 팝업 동기화와 상충 없는지 검증.
- **I4** word_mode_apps: 향후 Linux에 word 모드/commit_unit 도입 시 6지점(dbus Get/Set/ConfigChanged, GTK settings_dialog, GNOME gschema)으로 확장 필요. CLI interactive 메뉴 미노출은 curated subset 정책과 일관.
- **I7** GTK unim-settings build_accessibility_group + 신규 locale 키는 Linux CI 컴파일/렌더 검증 필요(Windows 빌드 불가). gschema 미변경.
- **I10** unim-keymap-common(gtk4/libadwaita)은 MSVC 크로스컴파일 불가로 테스트 게이트 제외 — 코어 로직 no-GUI 분리 시 Windows CI 포함 가능(별도 과제). CI 그린 여부는 GitHub Actions 원격 실측 필요.
- **I2/I3/I9** unim-imm32/unim-capi 등 타 프런트엔드 회귀는 해당 크레이트 빌드/테스트로 별도 확인 권장(이번 검증 범위=core+unim-tsf).

## 다음 세션 권장 순서

1. **device-QA 라운드 1 (P0 접근성)**: I8 → I11 → I5 → I6 을 NVDA/Narrator + 라이트/고대비로 실측. concerns 있는 I6(연속삭제 타이머) 우선.
2. **device-QA 라운드 2 (조합/교정)**: I3 ATF 회귀 → I2 폴백 오버레이 → I12 캐럿앵커/돋보기 → I7 낭독·비프.
3. **MSI 설치 검증** 후 크래시 리포팅(I9) 마커 생성 실측.
4. QA 결과 반영 후 **DEPLOY-TRUST-PLAN (a) 서명** 착수(인증서 신원검증 선행).
5. Linux 후속(I3 engine_worker reset, I4/I7 6지점 확장)은 CI/Linux 세션으로 분리.
