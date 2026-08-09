# UNIM Linux 메이저 한글 입력기 종합 분석 보고서

- 작성일: 2026-07-06
- 브랜치: develop / HEAD `dc0f98a`
- 대상: UNIM (Rust 한글 IME) — Linux 스택: 공유 코어 `src/`(31,300줄) + `unim-dbus`/`unim-daemon`(중추) + 프런트엔드 7종(GTK3/4·Qt5/6·XIM·Wayland·GNOME 확장) + IBus 호환 레이어 + GUI 군(`unim-settings`·`unim-indicator`·`unim-popup-service`)
- 범위: 소스·아키텍처 / 입력 품질·앱 호환성 / 경쟁 IME 기능 격차 / UI·UX 미려함 / 접근성(장애인) / 안정성·배포·신뢰
- 방법: 13개 축의 파일:라인 대조 검증 분석을 종합. 본 보고서 작성 시 핵심 근거(`.github/workflows/` = `windows-msi.yml` 단 1개, `unim-cli/src/main.rs` commit_unit grep=0, `ignore_key_repeat` 집행이 `unim-tsf/src/text_service.rs`에만 존재하고 `unim-frontends/` grep=0, `gtk3/src/immodule.c:598`·`gtk4/src/immodule.c:678` bare `Alt_R` 스킵, `unim-settings/src/settings_dialog.rs:84 .search_enabled(false)`, `PKGBUILD:3 pkgver=0.3.0`, `popup_styles.generated.css:145 #1e1e2e` Mocha 하드코딩, workspace 0.3.63)를 실코드로 재확인함.
- 참고: 본 보고서는 자매편 `docs/references/research/2026-07-03-windows-major-ime-analysis.md`(Windows판)와 동일 골격을 미러하며, Windows판에 없는 §10 "Windows→Linux 포팅 백로그"를 신설했다. "새 코드"는 커밋 범위 `09269e0..HEAD`(84커밋, 2026-06-19~07-05)를 가리키며, 이 범위는 대부분 Windows 작업이고 Linux 프런트에 닿은 커밋은 실질 1건(`19aa31c` GNOME 아이콘)뿐이다.

---

## 1. Executive Summary

### 1.1 현 위치 한 문단 진단

UNIM Linux는 "단일 코어를 5개 입력 환경에 그대로 꽂는 아키텍처·문서·GTK4 단일화는 경쟁력 수준에 도달했으나, 제품으로서의 배포 파이프라인·접근성 런타임 집행·온보딩 관문이 통째로 비어 있어 '메이저 IME 인식'의 문턱을 넘지 못한" 상태다. 코어는 플랫폼 코드 0(`src/*.rs`에 `cfg(windows)` 전수 grep=0, r5a §1.4)으로 Windows 포팅에도 무수정 유지되고, 팝업은 daemon이 `PopupRender` view-model을 산출하고 렌더러는 소비만 하는 단일 SoT(POPUP_SPEC.md:630-739)이며, GTK4/libadwaita 단일화 + 프로세스 분리(설정/트레이/팝업) + deb `unim-desktop` 통합(`a44af7b`)까지 완결됐다. 경쟁 어디에도 없는 AutoTypeFix(양방향 한/영 오타 자동교정) + 학습형 억제 사전이라는 고유 무기, 그리고 하우스 규칙(수치=슬라이더) 100% 준수(SpinRow grep=0, r5b §1.2)와 접근성 그룹 신설이라는 장애 당사자 관점 UX도 갖췄다. 그러나 (1) Linux 빌드/테스트/deb CI가 전무(`.github/workflows` 실측 1개=`windows-msi.yml`)하고 외부 배포 채널(AUR·PPA·Flatpak·배포판 repo)이 0개(r5b §2.3-2.4)이며, (2) 접근성 설정(`ignore_key_repeat`·`toggle_announce_beep`)이 GUI·CLI에 **노출만 되고 Linux 런타임 집행이 없어**(설정해도 무효, r3 §4) 신뢰를 깨고, (3) 새 코드 84커밋 중 Linux 프런트 커밋이 1건뿐인 플랫폼 투자 비대칭(r5a §3.8)으로 Windows 신기능(word·프리셋·검색·undo·마법사)이 Linux에 미이식이며, (4) first-run 마법사·`doctor`가 부재해 온보딩이 "문서를 읽는 사용자"만 구제하고(r4 §④, r5b §3.2), (5) 한/영 토글키가 5개 프런트에서 `Alt_R` 하드코딩돼 config 권위를 계통적으로 위반(r5a §2.5-3)한다. 요약: **"코어·문서는 견고하나 배포·접근성 런타임·온보딩이 미완"**.

### 1.2 메이저화까지 핵심 갭 Top 5

| # | 갭 | 성격 | 근거 | 우선순위 |
|---|-----|------|------|----------|
| 1 | **Linux CI·외부 배포 채널 전무** → 품질 게이트가 로컬 `make test`뿐, 채택이 로컬 deb 빌드 전제 | 배포 위생(blocker) | `.github/workflows` 실측 1개(`windows-msi.yml`); `PKGBUILD:3 pkgver=0.3.0` 스테일; AUR/PPA/Flatpak 매니페스트 0건 (r5b §2.3-2.4) | **P0** |
| 2 | **접근성 설정 무효** → `ignore_key_repeat`·`toggle_announce_beep`가 GUI·CLI 노출됐으나 Linux 런타임 미소비(설정해도 no-op) | 접근성(blocker)+신뢰 | 집행 grep이 `unim-tsf/src/text_service.rs:902-917`에만; `unim-dbus`는 config get/set 브리지(`service.rs:764,1002`)만·런타임 억제 분기 0건, `unim-frontends` grep=0 (r3 §4; remaining-report:49) | **P0** |
| 3 | **온보딩 관문**(first-run 마법사·`doctor` 부재, im-config + 재로그인 수동) | 온보딩 관문(blocker) | `unim-settings`·`unim-gui-common`에 first-run/wizard grep=0; `doctor` grep=0 (r4 §④; r5b §3.2) | **P0~P1** |
| 4 | **토글키 config 미준수**(5프런트 `Alt_R` 하드코딩 스킵으로 config RightAlt 토글 Linux 절대 미동작) | 정확성·접근성 교차 | `gtk3:598`/`gtk4:678`/`qt5:306`/`qt6:290`/`gnome key_handler.js:19` (docs/dev/linux/toggle-key-frontend-config-bypass.md:15-25) | **P0(최소 패치)~P1(일원화 전체)** |
| 5 | **Windows 신기능 패리티 격차**(word 확정 단위·접근성 프리셋·설정 검색·undo·마법사 미이식) | 기능 격차 | commit_unit grep=0(Linux GUI); preset grep=0; `.search_enabled(false)` settings_dialog.rs:84 (r1 §④, r2 §③, r3 §5) | **P0~P1** |

교차절단 리스크(아키텍처): 새 코드에서 코어가 Windows 동기로 +1,200줄 변경됐고(`config.rs +376`·`input_engine/engine.rs +546`·`hangul/input_context.rs +310`, git diff --stat), 이 파일들은 Linux가 공유한다. 그런데 Linux 회귀 게이트는 로컬 `make test`뿐(§8, r5a §3.8, r5b §2.3)이라, CI 부재(갭 1)와 코어 공유가 곱해져 "Windows 작업이 Linux를 조용히 깨뜨려도 자동 검출 수단이 없다"는 구조적 리스크가 상존한다.

### 1.3 로드맵 한눈에

- **단기(0~3M, P0 중심):** Linux CI(build+test+deb) · 접근성 런타임 집행 2건(`ignore_key_repeat` 억제 · 한/영 전환 통지) · 토글키 bare `Alt_R` 스킵 제거 최소 패치(§3.4·프리셋 선행) · word 모드 "열린 뒷문" 가드 · 싱크 누락 2건(commit-unit CLI · bidirectional_combine Slint) 해소 · first-run 마법사 착수 · 접근성 프리셋 GTK 이식 · AUR/PPA 채널 · PKGBUILD/rpm 0.3.63 동기화.
- **중기(3~9M, P1):** word DBus API 확장(`replace_composition`) · 토글키 판정 데몬 일원화 전체 · Orca 후보 낭독(AT-SPI announce) · `unim-cli doctor` · 설정 검색·2계층 IA 이식 · 공유 코어 결손(§5.2④) 단어단위 한자 변환·범용 사용자사전 · `popup_font_scale`(P2).
- **장기(9M+, P2~P3):** Slint 설정앱 크로스플랫폼 공용화 재평가(Orca PoC 후) · 팝업 라이트/고대비 팔레트 · 공유 코어 결손(§5.2④) 옛한글·한자 빈도 학습 · 순수 Wayland 팝업 검증 · 스위치 스캐닝·half-QWERTY 미러.

---

## 2. 성숙도 스코어카드

성숙도 4단계: **초기**(핵심 기능만, 제품 미달) · **사용가능**(동작하나 마감·신뢰 부족) · **경쟁력**(오픈소스 상위, 일부 상용 대등) · **메이저급**(상용 IME 대등/우위).

> 용어 주의: 본 보고서에서 **'축'은 아래 스코어카드의 평가 단위(13개)**이고, **'섹션'은 문서 장(§1~§11)**이다 — 둘은 1:1 대응이 아니다. 특히 접근성은 **스코어카드 3개 축(⑩ 시각·⑪ 지체·⑫ 인지)** ↔ **문서 §7 단일 섹션(§7.1/7.2/7.3)**으로 매핑된다. 방법론·§11.3의 "13개 축"은 이 스코어카드 축 수를 가리킨다.

| # | 축 | 성숙도 | 핵심 병목 (한 줄) |
|---|-----|--------|-------------------|
| 1 | 3계층 아키텍처(Core→DBus→Frontend) | 경쟁력 | 키 hot-path DBus 왕복 비용(IME_BEHAVIOR.md:266-280), 팝업 렌더러 환경별 3분기 |
| 2 | GTK3/4 · Qt5/6 IM 모듈 | 경쟁력 | 토글키 `Alt_R` 하드코딩(config 미준수), preedit caret·word 미배선 |
| 3 | XIM 서버 | 사용가능 | ON-THE-SPOT commit 직후 preedit 1프레임 누락(미해결, xim-0.5.0 crate 근본), Chrome preedit SKIP |
| 4 | Wayland · GNOME 확장 · IBus 호환 | 사용가능~경쟁력 경계 | 순수 Wayland 팝업 미검증·KDE5 Wayland 미지원, content-purpose 미배선(Wayland state.rs:569·XIM·GNOME 3경로) |
| 5 | 입력 정확성(ATF·모아치기·word substrate) | 경쟁력 | word substrate가 코어만·Linux 게이트 부재, ATF `replace_composition` 미소비 |
| 6 | 경쟁 대비 기능 격차 | 경쟁력~메이저 진입 | 기능은 우위(ATF·즐겨찾기·자판 JSON), 결손은 기능이 아니라 유통(배포 채널 0) |
| 7 | 설정 GUI(`unim-settings`) | 경쟁력(메이저 미달) | 검색 OFF, 접근성 프리셋·undo·commit_unit 미노출(Windows Slint 대비 패리티 격차) |
| 8 | 팝업·인디케이터 | 사용가능~경쟁력 초입 | Mocha 하드코딩(라이트/고대비 무), 첫 팝업 1~2초 지연, Wayland ◀/▶ 클릭 불가 |
| 9 | 설치·최초설정·온보딩 | 초기(사용가능 미만) | first-run 마법사·`doctor` 전무, im-config+재로그인 수동, 외부 채널 0 |
| 10 | 시각장애·저시력 접근성 | 초기 | 팝업 셀 AT-SPI 무노출·announce 0(Orca 무낭독), 팝업 테마 하드코딩 |
| 11 | 지체·운동장애 접근성 | 초기(설정 노출만) | `ignore_key_repeat` 런타임 no-op(무효 스위치), 접근성 프리셋 GTK 부재 |
| 12 | 인지·범용 접근성(UD) | 사용가능 | 억제단어 삭제 confirm/undo 부재, 마법사·doctor 부재 |
| 13 | 안정성·배포·CI | 사용가능(deb만 프로덕션급) | Linux CI 0, 외부 채널 0, PKGBUILD/rpm 스테일 |

관찰: **코어·아키텍처·문서 축(1·5·6)은 경쟁력 이상**이나, **배포·온보딩 축(9·13)과 접근성 런타임 축(10·11)이 초기 단계**로 전체 성숙도를 끌어내린다. Windows판과 동형 진단이되, Linux는 "코어가 이미 받아놓은 접근성 자산(config 키·sticky 마스크·flatten 로직)을 프런트가 안 쓰는" 미소비형 격차가 특징이다 — 즉 신규 개발이 아니라 배선·집행이 관건이다.

---

## 3. 소스·아키텍처 분석

### 3.1 구조 개요

- **공유 코어 `src/`** (Linux·Windows 공유, 31,300줄 wc -l 실측): 입력 엔진(`input_engine/` 8,032), 한글 조합(`hangul/` 5,698), 자판(`keystroke/` 4,904), 팝업 상태기계(`popup/` 3,129), 이모지(`emoji/` 2,198), 설정(`config.rs` 2,195), 오타교정(`auto_typefix/` 1,726), 한자·특수문자(`hanja` 395·`special_chars` 285) (r5a §1.2 표).
- **중추 `unim-dbus`(6,682)·`unim-daemon`(1,230)**: `org.atit.unim.InputMethod` DBus 인터페이스 + Worker Thread 고립(엔진이 Send+Sync 문제로 별도 스레드, `engine_worker.rs` 1,918줄) (README.md:149; docs/dev/architecture/AGENTS.md:35-43).
- **프런트엔드 7종**: GTK3(C, `immodule.c` 1,050)·GTK4(C, 1,093)·Qt5(C++, 623)·Qt6(C++, 603)·XIM(Rust, 2,637)·Wayland input-method-v2(Rust, 1,965)·GNOME Shell 확장(JS, 4,006) + **IBus 호환 레이어**(`ibus_compat/` 1,126 — Flatpak/Snap 샌드박스 앱의 `im-ibus.so`가 UNIM을 IBus로 인식) (r5a §1.2-1.3).
- **GUI 군·부속**: `unim-popup-service`(팝업 단일 렌더러, 3,629)·`unim-settings`(Adw 설정, 2,002 = `settings_dialog.rs` 1,959 + main.rs 43)·`unim-indicator`(트레이, 167)·`unim-gui-common`(1,940)·`unim-keymap-studio`/`unim-typing-practice`(자판 도구)·`unim-cli`(2,065)·`unim-capi`(C-API FFI, 1,095) (r5a §1.2).

### 3.2 강점

- **단일 코어 5환경 공유:** "어떤 앱에서 쳐도 같은 조합 규칙·같은 한자 팝업·같은 자동 교정"(user-guide/README-ko.md:12). `src/*.rs`에 `cfg(windows)` 전수 grep=0, 플랫폼 cfg는 `src/build.rs:2-3` 단 1건 — Windows 포팅에도 코어 무수정 유지(r5a §1.4, §1.6).
- **팝업 단일 SoT:** daemon이 `PopupRender` view-model을 산출하고 렌더러(popup-service GTK4 / GNOME St 위젯)는 소비만 — 프런트엔드별 중복 렌더링 코드 제거 이력이 문서화됨(POPUP_SPEC.md:630-739, :632-636; AGENTS.md:64-73).
- **동작 명세의 문서화 수준:** `IME_BEHAVIOR.md`가 키 분류별 동작(§3)·포커스 시퀀스(§8.2-8.3)·이중 커밋 방지(§6)를 프런트엔드 공통 규격으로 못박고, 새 프런트엔드 체크리스트까지 보유(IME_BEHAVIOR.md:239-257). gedit "늘늘"·영문 Space 특례 등 사고가 명세로 승격됨(IME_BEHAVIOR.md:30-33, 46-49).
- **장수 프로세스 메모리 규율:** jemalloc 고정 + `MALLOC_ARENA_MAX=2` + 60초 malloc_trim + per-context HashMap 5종 일괄 해제 + zbus `object_server` remove 짝 강제 — RSS 2GB 사건 이후 회귀 금지 규칙(AGENTS.md:115-158).
- **테스트 하네스 + i18n 규율:** `tests/`에 toolkit별 수동/자동 테스트 앱 9종(gtk3/gtk4/qt5/qt6/xim/wayland/gnome/dbus + `test_ibus_compat.py`, r5a §1.4). `t!()` 호출 약 1,900여 회(대상 크레이트 src+unim-* 기준)로 CLI 도움말까지 번역(r5b §1.8).

### 3.3 약점·기술부채

| 약점 | 심각도 | 근거 |
|------|--------|------|
| 프런트 5곳 `Alt_R` 하드코딩 — config 유일 권위 원칙의 계통적 위반(RightAlt 토글 Linux 절대 미동작) | major | `gtk3:598`/`gtk4:678`/`qt5:306`/`qt6:290`/`gnome key_handler.js:19` (docs/dev/linux/toggle-key-frontend-config-bypass.md:15-25) |
| 키 hot-path DBus 왕복 — 모든 키가 ProcessKeyEvent RPC, GNOME은 call_sync 재진입을 키 큐로 우회 | major | IME_BEHAVIOR.md:266-280; AGENTS.md:85-87 |
| 팝업 렌더러 환경별 3분기 — X11 popup-service / GNOME Wayland extension St / 기타 Wayland layer-shell, 이중 표시 회귀 상존 | major | POPUP_SPEC.md:780-789; troubleshooting:537-539 |
| 레거시 dual-emit — PopupNavigate·HanjaBookmarkChanged·HanjaCandidatesReordered가 PopupRender와 병행 발행(0.2.x 호환 부채) | minor | POPUP_SPEC.md:731-739 |
| 코어의 유령 플랫폼 의존 — 루트 `Cargo.toml:18-20`이 `x11`·`libc`를 선언·`build.rs:2-3`이 `X11` 무조건 링크하나 `src/*.rs` 사용 grep=0 | minor | Cargo.toml:18-20; src/build.rs:2-3 (r5a §1.6) |
| 문서 드리프트 3건 — AGENTS.md:21 "unim-gui-gtk"(실제 unim-settings), POPUP_SPEC §1.1 자기모순, README.md:3 배지 0.3.0 vs 실제 0.3.63 | minor | r5a §1.6 |
| C-API in-tree 소비자 0 — `unim-capi`(1,095줄) 공개 API로만 유지 | minor | README.md:163 |

**정정(중요):** "코어에 플랫폼 의존성이 섞여 있다"는 인상은 오도다. 코어 위생은 실제로 양호(`cfg(windows)` grep=0, `cfg(unix)` cfg 분기도 `src/`엔 없음)하며, 유령 의존(`x11`/`libc`)은 코드 사용이 아니라 `Cargo.toml`/`build.rs` 선언 수준의 죽은 링크다(사용 grep=0). 즉 런타임 결함이 아닌 빌드 그래프 정리 대상이다.

### 3.4 프런트 5곳 `Alt_R` 하드코딩 — config 유일 권위 원칙의 계통적 위반 (심층)

메이저화를 가로막는 대표적 구조부채. UNIM의 설계 원칙은 "한/영 토글키는 config가 유일 권위"여야 하나, GTK3(`immodule.c:598`)·GTK4(`immodule.c:678`)·Qt5(`input_context.cpp:306`)·Qt6(`input_context.cpp:290`)·GNOME(`key_handler.js:19`) 5개 프런트가 bare `Alt_R`을 modifier로 간주해 **무조건 스킵**한다(docs/dev/linux/toggle-key-frontend-config-bypass.md:15-25). 문제는 두 층위다.

1. **기능 층위:** config에서 RightAlt를 한/영 토글로 지정해도, 프런트가 `Alt_R` 키 이벤트를 엔진에 전달하기 전에 스킵하므로 Linux에서 **절대 동작하지 않는다**. Windows(TSF)는 이 결함이 수정 완료됐으나 Linux는 추적·미해결 상태다(r5a §2.5-3).
2. **교차 층위:** §7의 접근성 프리셋 '한 손 사용'은 "비수정자 토글"을 포함하나 그 반대편인 RightAlt(수정자) 토글 자체가 무력화돼 있어, 프리셋을 Linux에 이식해도 토글 설정 축이 반쪽만 동작한다 — 접근성 기능이 하부 결함에 의해 무력화되는 전형이다.

**권고:** 프런트엔드는 raw 키 이벤트를 그대로 엔진/데몬에 전달하고, "이 키가 토글인가"의 판정을 데몬/코어(config 소비 지점)로 일원화한다. modifier 가드보다 `is_toggle_key` 판정을 앞세우는 것은 Windows에서 이미 검증된 패턴이다(config.rs의 sticky_toggle_mask 경로와 정합). (무엇: 토글 판정 SoT 일원화 / 왜: config 권위 회복 + §7 프리셋 이식의 선행조건 / 공수: M / 영향: high / **P1** 전체 일원화 — 단 **5프런트 bare `Alt_R` 스킵 제거 최소 패치는 §10-C 프리셋의 선행조건이라 단기 P0로 선행 편입**, 데몬 판정 일원화 전체는 중기 P1)

### 3.5 공유 코어 ↔ Linux 계약의 임피던스

코어 조합/자판 엔진 자체는 견고하나, 코어가 최근 Windows용으로 확장한 substrate에 대한 **Linux 프런트의 1급 대응이 미비**해 계약이 절반만 이어진다.

- **word 모드 substrate 미소비:** 코어에 `CommitUnit::{Syllable,Word,Smart}`와 누적 substrate가 들어왔고 데몬은 `commit_unit == Word`면 `set_accumulate_word(true)`를 주입한다(`engine.rs:252-253`). 그러나 `is_word_mode_app`/`set_word_mode` 게이트 호출처는 `unim-tsf`(Windows)뿐 — `unim-dbus`/`unim-daemon`/`unim-frontends` 호출 grep=0. Linux에선 Smart가 사실상 Syllable이다(r1 §④, r5a §3.5).
- **ATF `replace_composition` 미배선:** 코어는 `AutoTypeFixResult.replace_composition` bool을 산출하고 `engine_worker`가 `buf.word_mode = engine.is_word_mode()`까지 배선했으나(`engine_worker.rs:702-704`), DBus AutoTypeFix 시그널은 여전히 `(delete_chars, commit_text, preedit_text)` 3튜플뿐(`service.rs:258-266, 1980-1985`)이라 word 모드 치환 신호가 프런트에 도달하지 못한다(r1 §④-2).
- **repeat 플래그 미전달:** 코어·config에 `ignore_key_repeat`가 있으나 프런트가 OS 자동반복 여부를 엔진에 전달하지 않아 억제가 불가능(§7.2, r3 §5 P0-1).
- **content-purpose 미배선(3경로):** 비밀번호 필드 한글 차단이 순수 Wayland(`state.rs:569`)·XIM(`unim-frontends/xim/src/` purpose/content_type grep=0)·GNOME 확장(`unim_input_method.js:332` `vfunc_update_content_purpose`가 주석뿐인 빈 스텁, extension 전체 purpose 소비 0건) 3경로에서 미배선. 실배선은 GTK3/4 immodule(`immodule.c:808-847` `unim_dbus_set_content_type`)·Qt5/6(`input_context.cpp:468-480` `setContentType`)·ibus_compat뿐(r5a §2.1·§3.6).
- **ATF 핫패스 엔진 재생성(성능 부채):** ATF 적용 replay 경로가 교정마다 `*engine = InputEngine::new(&config)`를 호출(`engine_worker.rs:856` 순방향·`:888` 역방향)해 `InputEngine::new`(`engine.rs:185`)가 ≈6.45MB 한자사전(`HanjaDictionary::new()`; `src/data/hanja.txt` 실측 6,452,522B, 코드 주석의 6.76MB는 스테일)을 매번 재파싱한다. Windows는 이미 `engine.reset()`(`engine.rs:478` `pub fn reset`, 보존 목록 doc :469-477 — hanja_dict/북마크/카테고리 등 보존)로 전환해 이 재파싱을 제거했으나(`unim-tsf/src/auto_typefix.rs` I3-perf 주석), Linux 데몬만 옛 재생성 패턴을 계승 중이다 → §10-A 무재생성 전환 항목.

---

## 4. 입력 품질 & 앱 호환성

### 4.1 경로별 신뢰성 매트릭스

입력 경로 티어: **GTK-IM**(GTK IM Context 직결) / **Qt-IM**(QPlatformInputContext) / **XIM**(OVER/ON-THE-SPOT 구분) / **Wayland-IMv2**(wlroots/KDE) / **GNOME-TI3**(확장, text-input-v3) / **IBus-Compat**(샌드박스 에뮬레이션).

| 경로/대표앱 | 조합 렌더 | ATF | 팝업 | 비밀번호 purpose | word 모드 | 잔존 결함 |
|-------------|-----------|-----|------|------------------|-----------|-----------|
| GTK-IM (GTK3/4 앱, Electron, ghostty) | inline 정상 | delete_surrounding + Electron XTest 폴백 | popup-service GTK4 | ✓ 배선(immodule.c:808-847) | ✗(코어만) | 토글키 `Alt_R` 스킵 |
| Qt-IM (Kate, Krita, Qt6 앱) | inline 정상 | QInputMethodEvent | popup-service GTK4 | ✓(input_context.cpp:468-480) | ✗ | 토글키 `Alt_R` 스킵 |
| XIM (XTerm, WezTerm, Java/AWT) | OVER-THE-SPOT 정상 | N+1 BS synth 재구현 | popup-service GTK4 | **✗ 미배선(xim purpose grep=0)** | ✗ | **ON-THE-SPOT preedit 누락(미해결)**, Chrome preedit SKIP |
| Wayland-IMv2 (KDE Plasma6·Sway) | preedit_string 교체 | delete_surrounding_text | popup-service + layer-shell(미검증) | **✗ 미배선(state.rs:569)** | ✗ | 순수 Wayland 팝업 미검증 |
| GNOME-TI3 (GNOME Wayland 전앱) | Clutter InputMethod | vkbd BS+commit+preedit | extension St 위젯 | **✗ 미배선(unim_input_method.js:332 빈 스텁)** | ✗ | 같은 창 클릭 dismiss 휴리스틱(오탐 여지) |
| IBus-Compat (Flatpak/Snap 앱) | IBus 에뮬레이션 | 상동 | 상동 | (경로 의존) | ✗ | context_id 누수 방어를 DestroyContext에 의존 |

word 모드 컬럼이 전 경로 ✗인 것은 코어 substrate만 존재하고 어느 Linux 프런트도 게이트를 호출하지 않기 때문이다(r5a §3.5). 비밀번호 purpose 컬럼은 GTK3/4(`immodule.c:808-847` `unim_dbus_set_content_type`)·Qt5/6(`input_context.cpp:468-480` `setContentType`)만 실배선이고, **XIM**(`unim-frontends/xim/src/` purpose grep=0 — 히트 3건은 전부 "dual-purpose Hanja" 주석)·**GNOME**(`unim_input_method.js:332` `vfunc_update_content_purpose`가 빈 스텁·extension purpose 소비 0건)·**순수 Wayland**(`state.rs:569`) 3경로는 미배선이다. 표의 근거: r5a §2.1 표 + §3.6 배선 현황.

### 4.2 잔존 결함 목록

r5a §2.5 중 결함 10건을 전재하고, 정상 동작으로 문서화된 특이케이스 1건(XIM 이모지 팝업 ◀/▶×카테고리 탭 상호작용)은 결함이 아니므로 제외한 뒤, 그 자리에 kitty 검증 공백(r5a §2.3 유래, 아래 10번)을 정리해 싣는다.

1. **XIM ON-THE-SPOT commit 직후 preedit 누락(미해결)** — xim-0.5.0 crate `commit()`이 preedit_started 상태 미갱신이 근본 원인, crate fix 또는 프로토콜 시퀀스 재설계 필요(troubleshooting:566; handler.rs:378-).
2. **XIM AutoTypeFix Chrome preedit edge(알려진 SKIP)** — N+1 BS 재구현 후에도 Chrome XIM에서 preedit 미표시 케이스 잔존(troubleshooting:564; handler.rs:474, 909-952).
3. **토글키 config 미준수(추적·미해결)** — 5프런트 bare `Alt_R` 스킵(§3.4, toggle-key-frontend-config-bypass.md:15-25).
4. **순수 Wayland(비GNOME) 팝업 미검증** — wayland-backend 경로 이론상 동작·실검증 없음, KDE5 Wayland는 gtk4-layer-shell 패키지 부재로 미지원(POPUP_SPEC.md:785; README.md:101).
5. **Wayland ◀/▶ 마우스 클릭 불가(환경 의존)** — 컴포지터 IM popup 포인터 라우팅 의존, mutter 차단 → 키보드 ←/→ 대체(POPUP_SPEC.md:229).
6. **GNOME 같은 창 내 클릭 dismiss 휴리스틱** — cursor-jump 감지(100ms 무입력+50px)로 보완, 오탐/미탐 여지(POPUP_SPEC.md:567-573).
7. **터미널 preedit 미지원(AutoTypeFix)** — 세션 메모리(project_autotypefix_state)로만 추적, repo 문서(docs/user·docs/dev) 전수 grep에서 미기재 — **저장소 근거 부재라 재검증 필요(미검증)**.
8. **비밀번호 한글 차단 미배선(순수 Wayland·XIM·GNOME 3경로)** — content-type/purpose hint 미소비: 순수 Wayland(state.rs:569), XIM(`unim-frontends/xim/src/` purpose grep=0), GNOME(`unim_input_method.js:332` `vfunc_update_content_purpose` 빈 스텁·extension purpose 소비 0건). 실배선은 GTK3/4·Qt5/6·ibus_compat뿐.
9. **GNOME Wayland 팝업 이중 표시 가능성** — `Meta.is_wayland_compositor()` 감지 실패 시(troubleshooting:537-539).
10. **kitty 검증 공백** — docs·코드 전체 언급 grep=0, 검증되지 않음(미검증).
11. **첫 팝업 지연 1~2초** — D-Bus auto-activation lazy launching 비용(POPUP_SPEC.md:769).

### 4.3 자동적응 전략 (현재 vs 권고)

- **현재:** Windows는 CUAS 학습(`app_tiers.json` 영속화)으로 앱 능력을 동적 학습하나, Linux는 **정적 처방**이다. 유일한 자동화는 Flatpak override(`flatpak override --user`로 IM 환경변수를 비워 text-input-v3로 유도, `unim-daemon/src/main.rs:148-179`, r5a §2.2) — 타 IME에 없는 강점이나, 앱별 word/조합 능력의 동적 판정은 없다.
- **권고:**
  - 프런트 종류별(XIM/터미널 vs GTK/Qt/Wayland) capability 판정을 명문화 — word 모드 대상에서 XIM·터미널류를 명시 제외(wmux 교훈, r1 §⑤-10). (S / medium / **P2**, word 게이트 §10-A 선행)
  - Snap 자동화 부재(전역 override 메커니즘 없음, README §7)를 문서 처방이 아닌 lazy 감지+안내로 보강. (M / medium / **P2**)

---

## 5. 기능 격차 (경쟁 IME 대비)

대상: ibus-hangul · fcitx5-hangul · kime · nimf.

### 5.1 기능 대조표

> 아래 표는 저장소 자체 문서(faq/README-ko.md:8-27, Q1)를 재구성한 **프로젝트 자체 평가**이며, 경쟁 IME별 셀 값 자체는 **미검증**이다(r5b §4.1).

| 기능 | ibus-hangul | fcitx5-hangul | kime | nimf | **UNIM** | 격차 |
|------|:----:|:----:|:----:|:----:|:----:|------|
| GTK3/4·Qt5/6·XIM 네이티브 | ✓ | ✓ | ✓(Wayland △) | ✓ | **✓ 전부** | 대등 |
| Wayland input-method-v2 | ✓(IBus) | ✓ | △ | ✓ | **✓** | 대등 |
| GNOME Shell 직접 통합 | ✓(IBus) | △ | ✗ | ✗ | **✓(자체 확장)** | 우위 |
| 양방향 한↔영 오타 자동교정 | ✗ | ✗ | ✗ | ✗ | **✓(AutoTypeFix, 고유)** | **UNIM 우위** |
| 한자 9/81칸 그리드·즐겨찾기 | △/✗ | △/✗ | ✗ | △/✗ | **✓** | 우위 |
| 사용자 자판 JSON(inherits+rule_sets) | ✗ | △ | △ | ✗ | **✓** | 우위 |
| 접근성 옵션(반복 억제·전환 통지) | ✗ | ✗ | ✗ | ✗ | **부분(노출만·런타임 무효)** | 명목 우위 |
| 배포판 공식 repo 수록 | ✓ | ✓ | 부분(AUR, 미검증) | 부분(포크, 미검증) | **✗(0건)** | **결정적 결손** |
| CI·자동 배포 | (상류 유지) | (상류 유지) | ✓(GitHub, 미검증) | ✗ | **✗(Linux CI 0)** | **결정적 결손** |
| first-run 마법사 | ✗ | ✗ | ✗ | ✗ | **✗** | 대등(공통 부재) |

### 5.2 결정적 격차 상세 (근거)

- **격차는 기능이 아니라 유통이다.** 기능 축에서 UNIM은 AutoTypeFix·즐겨찾기·자판 JSON 단독 우위이나(faq/README-ko.md Q1 하단 요약), 결정적 결손은 **배포 채널 0**이다: ibus-hangul·fcitx5-hangul은 모든 주요 배포판 공식 저장소에 존재하고 `im-config`/`im-chooser`가 기본 인지하지만, UNIM은 공식 repo·PPA·Flatpak 매니페스트가 없어(find 실측, r5b §2.4) 로컬 deb 빌드가 전제다(최대 채택 장벽, 미검증 딱지: 배포판 수록 현황은 일반 지식).
- **한국어 전용의 전환 비용:** IBus/Fcitx5는 CJK+다국어 엔진 호스트라 배포판 기본 스택으로 선택되나, UNIM은 한국어 특화라 기존 IM 프레임워크 제거("IBus 제거 필수", FAQ Q2)라는 파괴적 전환 비용이 발생(r5b §4.2, 미검증).
- **kime 대비:** 같은 Rust·한국어 전용 포지션. kime은 GitHub 배포·AUR 수록(미검증)로 유통에서 선행. UNIM 차별점은 AutoTypeFix·팝업 UX·GNOME 확장·자판 도구 등 기능 폭(FAQ 표 기준 kime은 Wayland △·GNOME ✗).
- **공유 코어 결손 4건은 Linux에도 동일 적용(교차 참조):** Windows판 §5.1-5.2가 공유 코어(`src/`) 근거로 '결정적 결손'으로 판정한 4건은 플랫폼 공유 코어라 Linux에도 그대로 적용된다 — ① **옛한글/고어**(코어가 현대 자모만·U+AC00 완성형만 생성, `hangul/char.rs:60-67`·`hangul/jamo/cho.rs:60-91`), ② **한자 단어 단위 변환**(`input_engine/candidates.rs:22-29`가 `preedit.chars().last()` 마지막 1음절만 대상, 사전 다음절 키 275,021줄 존재하나 `search_word` 경로 부재), ③ **범용 사용자 낱말/상용구 사전**(`typefix_userdict.rs:199` `is_ascii_alphabetic()`로 비영문 거부 → 한글/한자/구절 등록 차단), ④ **한자 후보 사용빈도 자동 학습**(수동 북마크만). §5.1 표는 Windows 경쟁제품(MS/날개셋/새나루) 기준이라 이 4행을 옮기지 않았으나, 특히 옛한글은 ibus-hangul(libhangul)이 지원하므로 **Linux 경쟁 구도(ibus-hangul·fcitx5-hangul)에서는 오히려 더 뚜렷한 격차**다. 우선순위는 Windows판 §5.3과 동일(단어단위 한자·범용 사용자사전 P1, 빈도 학습·옛한글 P2)이며, 이를 §1.3·§9.1 매트릭스·§9.2(중기 P1: 단어단위 한자·사용자사전 / 장기 P2: 옛한글·빈도학습)에 배치한다. ※ Windows판의 5번째 결손 ITfFnReconversion(재변환)은 TSF 고유라 공유 코어 이슈가 아니므로 제외.

### 5.3 권고

| 권고 | 무엇/왜 | 공수 | 영향 | P |
|------|---------|------|------|---|
| AUR/PPA 채널 개설 | 최대 채택 장벽이 유통. deb 완성도(10패키지) 위에 AUR PKGBUILD·Ubuntu PPA를 얹으면 즉효. §8 CI와 연계 | M | high | **P0** |
| Flatpak 배포 채널 검토 | 샌드박스 IME는 난제이나 데스크톱 앱 배포 표준. 현 Flatpak override 자동화 자산 활용 | L | medium | **P2** |
| kime 대비 포지셔닝 문서화 | 같은 Rust·한국어 전용 경쟁자 대비 차별점(AutoTypeFix·접근성) 명시 | S | low | **P2** |

---

## 6. UI/UX 미려함

### 6.1 설정 GUI (`unim-settings` GTK4 + libadwaita)

**강점:** `Adw.PreferencesWindow` + Page/Group/Row 5페이지(일반/오타교정/억제단어/사용자사전 + GNOME Shell 조건부, settings_dialog.rs:88-128), 시스템 테마 자동 추종(`StyleManager::set_color_scheme(Default)`, :71). 하우스 규칙(수치=슬라이더) **완전 준수** — `SpinRow`/`SpinButton` grep=0(4크레이트), 수치는 전부 `gtk4::Scale` + tick 마크(build_int_scale_row :791, r5b §1.2). **접근성 그룹 신설**(:727-780, 새 코드 c9f6b5d). 저장 즉시 D-Bus `SetConfigYaml` fire-and-forget으로 데몬 능동 전파(settings_dbus.rs:14-19) — Windows의 패시브 mtime 감지보다 우위. 억제단어 **3-상태 그룹 분리 뷰**(임시/확정/비활성 별도 그룹 + 카운트 + 행별 버튼, :1481-1493, :1543+) + 파일 mtime 폴링 자동 새로고침(:1427-1439)은 Windows Slint의 StandardListView(행내 버튼 불가)보다 표현력 우위(r2 §③).

**약점:**

| 약점 | 심각도 | 근거 |
|------|--------|------|
| 설정 항목 검색 OFF — Adw 기본 검색을 의도적으로 끔 | major | `.search_enabled(false)` settings_dialog.rs:84 |
| 접근성 프리셋 2종·ATF 강도 3프리셋·2계층 IA 부재(Windows Slint에만) | major | preset grep=0; GTK는 전 항목 평면 노출(945-1235) |
| 파괴적 삭제 confirm/undo 부재 — Toast는 저장 피드백 전용 | major | settings_dialog.rs:1371-1466(confirm/스냅샷 0건) |
| `commit_unit` 콤보 미노출 + CLI ConfigKey 부재(싱크 누락) | major | commit_unit grep=0(GTK); unim-cli/src/main.rs grep=0 |
| `bidirectional_combine` Slint 미노출(역방향 싱크 누락) | minor | slint:307 주석뿐; GTK·CLI엔 있음(settings_dialog.rs:387, unim-cli/src/main.rs:455-456 `KoreanBidirectionalCombine`) |
| 브랜드 디자인(사이드바+카드+그라디언트) 대비 순정 Adwaita | minor | Windows Slint slint:26-51 vs GTK 순정 |

**재설계 제안:** → §10-B로 위임(GTK 유지 + Windows 신기능 이식 단기 노선, Slint 공용화는 Orca PoC 후 재평가).

### 6.2 팝업/인디케이터 (`unim-popup-service` · `unim-indicator`)

**강점:** 팝업 3종(한자 9/81칸·특수문자·이모지) 단일 SoT + 무포커스 설계(포커스 강탈 방지, hanja.rs:76 set_focusable(false)) + 환경별 렌더러 분기 문서화(POPUP_SPEC.md:780-789). 트레이는 초경량(unim-indicator 167줄) + ksni StatusNotifierItem, 상태 아이콘 색 비의존('가'/'A' 글리프+툴팁). GNOME 확장 단색 SVG 아이콘 셋(새 코드 19aa31c, HIG symbolic 방향).

**약점:**

| 약점 | 심각도 | 근거 |
|------|--------|------|
| 팝업 팔레트 단일(Catppuccin Mocha) — 라이트/고대비 미추종. 단 `POPUP_SPEC.md:364-405`(§5)가 이 팔레트를 '공통 디자인 시스템' **규범 명세**로 못박은 상태이므로, 하드코딩 '결함'이 아니라 접근성 관점의 **스펙 개정 사안**(개정+사용자 승인 선행) | major | `POPUP_SPEC.md:364-405`; `popup_styles.generated.css:145 #1e1e2e`; extension stylesheet.css:29-30,58 |
| 첫 팝업 지연 1~2초(D-Bus auto-activation lazy) | minor | POPUP_SPEC.md:769 |
| Wayland ◀/▶ 마우스 클릭 불가(mutter 라우팅 차단) | minor | POPUP_SPEC.md:229 |

**재설계 제안:** → §7.1·§10-C(팝업 라이트/고대비 팔레트 분기).

### 6.3 설치/온보딩

§8(안정성·배포)·§10-D(마법사)와 공유하므로 여기서는 요약만 둔다(중복 회피). 핵심은 (1) first-run 마법사·`doctor` 전무, (2) im-config + 재로그인 100% 수동, (3) 외부 배포 채널 0, (4) PKGBUILD/rpm 스테일 — 온보딩이 "문서를 읽는 사용자"만 구제하는 구조다(r4 §④, r5b §3.2). 따라서 마법사(§10-D)는 감지→명령 안내가 아니라 **감지→원클릭 적용**(`im-config -n unim` 실행 버튼 + 로그아웃 유도)으로 설계해야 이 구조를 실제로 닫는다.

---

## 7. 접근성 (장애인 기능) — 최우선 강조 섹션

> UNIM Linux의 접근성은 "config·GUI 노출은 이번 새 코드로 생겼으나 **런타임 집행이 통째로 비어 있는**" 상태다. `unim-settings`에 접근성 그룹(`toggle_announce_beep`·`ignore_key_repeat` 스위치)이 신설됐고 3지점 config 싱크도 끝났으나, 실제 억제·통지 로직은 `unim-tsf`(Windows) 전용이라 **Linux에서는 설정해도 무효(no-op)**다 — `unim-dbus`는 config get/set 브리지(`service.rs:764,1002`)만 있고 런타임 억제 분기는 0건, `unim-frontends` 소비 grep=0(재확인). 팝업 접근성도 창 레벨 라벨 3종에 그쳐 Orca가 후보 내용을 낭독하지 못한다. 접근성은 메이저 IME의 법적·윤리적 필수요건이자, 기현(뇌병변 사지마비: 오른발 마우스·입 젓가락 타이핑) 직결 영역이다. 이 섹션을 가장 두껍게 다룬다.

### 7.1 시각장애·저시력 (Screen reader / Low vision)

#### 7.1.1 현 상태

**강점(전제 충족):**
- 팝업 3종에 AT-SPI **창 레벨 라벨** 부여("한자 후보"/"이모지 선택"/"특수문자 선택", hanja.rs:153-154·emoji.rs:271-272·special.rs:179-180) — 창 존재는 Orca에 노출됨.
- libadwaita 설정 GUI는 위젯 내재 AT-SPI 노출·키보드 내비를 공짜로 얻음(adw::PreferencesGroup/SwitchRow, r3 §4).
- **뜻 병기 flatten 로직**(`unim-tsf/src/ui_element.rs:70-99`의 candidate snapshot)은 `unim::popup` 코어 타입 기반이라 **Linux 재사용 가능** — 단 코드 소재는 unim-tsf(Windows 크레이트)로 '코어에 들어온 자산'이 아닌 **이식 참조**다(r3 §5).

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| 팝업 셀 단위 AT-SPI 무노출 + 라이브 리전 announce 0 → Orca가 후보 내용·선택·페이지 무낭독(창 라벨만) | **blocker(시각장애 사용자군의 팝업 기능 한정)** | `popup/*`에 update_property(Label) 3건이 전부; announce/relation grep=0 |
| 팝업 라이트/고대비 미추종 — Mocha CSS 하드코딩 | major | `popup_styles.generated.css:145 #1e1e2e`; extension stylesheet.css:29-30 |
| GNOME extension 팝업 accessible 0건 — St 위젯 reactive/can_focus만, accessible-name 없음 | major | popup_view.js:79,237-240,342-345; accessible grep=0 |
| 한/영 전환 AT-SPI 능동 통지 전무(Orca 낭독 불가) | major | NotifyWinEvent 상당 Linux 측 grep=0 |

#### 7.1.2 신규 기능 제안 (구체)

1. **[P1] Orca 후보 낭독** — GTK4 4.14+ `gtk::Accessible::announce()`(라이브 리전)로 표시/페이지/선택 변경 시 스냅샷 문자열 낭독. `ui_element.rs:70-99`의 뜻 병기 flatten 로직을 재사용하고, 셀 위젯별 accessible label 병행(hanja.rs:316-355). POPUP_SPEC 무포커스(hanja.rs:76) 불변 — 포커스 이동형 AT 패턴 금지. (M / high / **P1**) 표준: AT-SPI2, Orca.
2. **[P2] 팝업 라이트/고대비 2벌** — `popup_styles` CSS를 adw StyleManager dark/high-contrast 신호로 2벌 분기, extension stylesheet는 St 테마 클래스 분기. **선행조건: `POPUP_SPEC.md:364-405`(§5 색상표) 단일 팔레트 규범을 라이트/고대비 2벌로 개정 + 사용자 명시 승인**(하우스 규칙: POPUP_SPEC 절대 준수, 변경 시 승인 필수). (M / high / **P2**) 표준: 데스크톱 고대비 테마.
3. **[P1] 한/영 전환 AT-SPI announce** — §7.2 P1-3과 동일 항목(비프+낭독). (S / high / **P1**)

### 7.2 지체·운동장애 (Motor / Physical disability) — 기현 직결·최상단 배치

#### 7.2.1 현 상태

**강점(전제 충족):**
- 코어 `sticky_toggle_mask`는 press_key 경로 마스킹이라 **Linux도 자동 수혜**(engine.rs:156, 363-373) — 고정키 래치 Shift가 쌍자음에 자연 반영.
- '세벌식 순아래' keymap이 `accessibility`·`noshift` 태그 보유(ko_3bul_noshift.json:8-10) — 코어 자산, 플랫폼 공통.
- 모아치기 기본 OFF·opt-in이라 기본 한글 입력은 타이밍 의존 0(WCAG 2.2 부합).

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| `ignore_key_repeat` Linux 런타임 무효 — GUI/CLI 노출됐는데 미집행(=신뢰 파괴) | **blocker** | 집행이 `text_service.rs:902-917`(Windows)만; `unim-dbus`는 config 브리지(`service.rs:764,1002`)뿐·억제 분기 0건, `unim-frontends` grep=0 (remaining-report:49) |
| 프런트 게이트 sticky 정렬(peek_sticky_masked_modifiers) Linux 미적용 | major | 사용처 `key_handler.rs:80`(Windows) 유일; unim-dbus grep=0 |
| 토글키 하드코딩과 교차 — RightAlt 토글 Linux 미동작이 접근성 프리셋 무력화 | major | §3.4; toggle-key-frontend-config-bypass.md:15-25 |
| 접근성 프리셋('한 손'/'넉넉한 타이밍')·순아래 추천 배지·모아치기 카드 GTK 부재 | major | `settings_dialog.rs:727-780`(build_accessibility_group)은 토글 2개뿐; preset grep=0 |

#### 7.2.2 신규 기능 제안

1. **[P0] `ignore_key_repeat` Linux 런타임 집행** — XKB detectable autorepeat / GTK·XIM 이벤트의 repeat 판별로 프런트에서 repeat 플래그를 엔진에 전달, `engine_worker` 분기(`text_service.rs:902-917` 이식). 단 DBus `process_key_event`(`service.rs:1739`) 계약에 repeat 필드가 없어 **시그니처 확장(7프런트+ibus_compat 공통 계약 변경) 또는 프런트 로컬 억제 택일 결정**이 필요하고, Wayland는 `unim-frontends/wayland/src/repeat.rs`가 반복을 자체 생성하므로 자체 억제가 최저비용 경로다(파일 단위 분해는 §10-C P0-1). 입 젓가락 타이핑 = 키 홀드가 길어 자동반복 연타·토글 진동이 직접 타격이고, GTK 설정에 스위치가 이미 보이는데 무효라 신뢰 문제까지 겹친다. (M~L / critical / **P0**, 계약 변경 감안 상향) 표준: XKB autorepeat, 데스크톱 필터키.
2. **[P0] 접근성 프리셋 2종 GTK 이식 + 순아래 추천 배지** — '한 손 사용'/'넉넉한 타이밍' 로직이 `unim-tsf-settings/src/main.rs:805-872`에 이미 완성돼 있고 config 조작이라 toolkit-free다. `settings_dialog.rs:727 build_accessibility_group` 확장으로 재사용. '넉넉한 타이밍'(ATF 판정시간 최대+반복 억제)은 기현의 입력 리듬에 정확히 부합. **선행조건: §3.4 토글키 하드코딩 수정**('한 손 사용'의 비수정자/RightAlt 토글이 무력화되지 않도록) — 5프런트 bare `Alt_R` 스킵 제거 **최소 패치를 단기 P0로 선행 편입**하고 데몬 판정 일원화 전체는 중기 P1로 둔다. (S~M / high / **P0**)
3. **[P1] 한/영 전환 능동 통지(비프 + AT-SPI announce)** — 비프는 GTK `gdk::Display::beep` 또는 libcanberra, 낭독은 인디케이터/extension에서 AT-SPI announce(GNOME은 `Main.osdWindowManager` 병행 검토). 모드 오인→오타→재입력은 타이핑 비용이 큰 기현에게 배액 손해, config 키는 이미 존재. (S / high / **P1**)
4. **[P1] 프런트 게이트 sticky 정렬** — `engine_worker` 단축키 게이트 앞에 `peek_sticky_masked_modifiers`(engine.rs:363) 적용, unim-imm32 동일. 코어 API 완성 상태라 게이트 한 줄 정렬. (S / medium / **P1**)
5. **[P2] `popup_font_scale`(클릭 타깃 확대)** — 오른발 마우스 = 클릭 타깃이 클수록 유리. config 신설이라 6지점 동기화 필요(설정 원칙). **선행조건: `POPUP_SPEC.md:383-397`(§5.2)이 폰트를 고정 px로 명세하므로 폰트 명세 개정 + 사용자 승인.** GUI 노출 시 수치 입력은 `gtk::Scale` 슬라이더(하우스 규칙, SpinRow 금지). 한자/팝업 선택마다 매번 걸리는 항목이라 실사용 영향이 커 **중기 P2로 상향**한다(구 P3에서 재평가 반영). (M / medium / **P2**)

### 7.3 인지·범용 (Cognitive / Universal Design)

#### 7.3.1 현 상태

**강점:** 설정 각 행 평문 서브타이틀·툴팁, libadwaita 접근성 배선, i18n(ko/en 전 컴포넌트 t!() 약 1,900여 회), 억제단어 3-상태 뷰의 명료성(r5b §1.8, r2 §③).

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| 억제단어(블랙리스트) 삭제 confirm/undo 부재 → 학습 데이터 비가역 손실 | major | `settings_dialog.rs:1371-1466`(confirm/스냅샷 없음) |
| first-run 마법사·`doctor` 부재 → 온보딩·자기진단 경로 없음 | major | first-run/doctor grep=0 (r4 §④; r5b §3.2) |
| 언어 ko/en 한정(제3 언어 없음) | minor | locales ko/en 2종뿐(r5b §1.8) |

#### 7.3.2 신규 기능 제안

1. **[P1(confirm)/P2(undo)] 억제단어 삭제 confirm + undo** — `settings_dialog.rs:1371-1466`에 `adw::AlertDialog` **삭제 confirm(공수 S)은 단기 P1로 분리 상향**(오른발 마우스 오클릭 한 번에 학습 데이터 비가역 손실, 복구는 재학습=추가 타이핑이라 비용 대비 방지 효과 큼), undo(삭제 직전 스냅샷/Toast, Windows I6 `main.rs:469` 패턴)는 P2 유지. WCAG 3.3.4. (S~M / medium / **P1~P2**)
2. **[P0] first-run 마법사** — §10-D로 위임(unim-settings 통합, `--first-run/--whats-new`). 온보딩 blocker이므로 §9.2 P0 정의·§10-D와 일치하게 P0로 통일. (M / critical / **P0**)
3. **[P2] `unim-cli doctor`** — troubleshooting §1 진단 절차 자동화(state frontends·env·IM 캐시·데몬 생존). (M / high / **P2**)

### 7.4 접근성 종합

접근성은 UNIM Linux 메이저화의 **가장 저평가된 리스크이자 가장 높은 레버리지 영역**이다. 결정적 사실은 Windows 라운드에서 **코어에 이미 들어온 자산**(`ignore_key_repeat`/`toggle_announce_beep` config+테스트, `sticky_toggle_mask`, 순아래 keymap 태그)을 Linux가 **공짜로 받아놓고 안 쓰는** 상태라는 점이다(단 `ui_element.rs:70-99` flatten 로직만은 코어가 아니라 unim-tsf 소재·`unim::popup` 코어 타입 기반이라 재사용 가능한 **이식 참조**로 구분) — 즉 신규 개발이 아니라 런타임 집행·배선만 붙이면 된다. P0 두 건(7.2-1 `ignore_key_repeat` 집행, 7.2-2 프리셋 이식)만으로 "설정했는데 무효"라는 신뢰 문제를 닫고, 저비용 P1인 한/영 전환 통지(7.2-3)를 더하면 무시각·모드오인 사각지대까지 함께 해소한다. POPUP_SPEC의 무포커스(hanja.rs:76)·환경별 분기 구조를 깨지 않는 announce 방식이어야 함은 모든 제안의 불변 제약이다.

---

## 8. 안정성·배포·신뢰

### 8.1 강점

- **Debian 패키징 — 가장 완성된 채널(10 바이너리 패키지)** — unim-common/im-gtk/im-qt/xim/wayland/desktop/keymap-studio/typing-practice/gnome/meta, `unim-desktop` 통합(구 4패키지 Replaces/Breaks, 새 코드 a44af7b). prerm 데몬 pkill + chrpath로 RUNPATH 제거(lintian-clean, r5b §2.1).
- **메모리 규율** — jemalloc·malloc_trim·일괄 해제(§3.2, AGENTS.md:115-158).
- **man 5종** — unim.1/unim-cli.1/unim-indicator.1/unim-settings.1/unim-popup-service.1(Makefile install-core, r5b §2.2).
- **문서 품질** — 한/영 쌍 user-guide 487줄·troubleshooting 610줄·faq 325줄, "60초 첫 입력" 동선(r5b §3.1).
- **panic=abort 전역 프로필** — 루트 `Cargo.toml:66-70`이 `panic="abort"`를 release·dev 전 프로필에 적용(1차 사유는 Windows TSF COM 경계 UB 회피, 주석 명시). Linux 장수 데몬·XIM·Wayland 프런트도 패닉 시 언와인딩 없이 즉시 abort하고 복구는 DBus auto-activation에 의존 — 크래시 격리는 되나 가시화 수단은 로그뿐이라 §1.2 코어 공유 리스크의 이면(r5a §1.6).

### 8.2 약점

| 약점 | 심각도 | 근거 |
|------|--------|------|
| Linux CI 전무 — 빌드/테스트/deb 생성 자동화 0, 품질 게이트가 로컬 make test뿐 | **blocker** | `.github/workflows` 실측 1개(`windows-msi.yml`); r5b §2.3 |
| PKGBUILD 0.3.0 스테일 + rpm 구식 분리형 구조 | major | `PKGBUILD:3 pkgver=0.3.0`(워크스페이스 0.3.63); rpm spec이 desktop 통합 미반영(r5b §2.4) |
| Flatpak·PPA·AUR 부재(채택 관문) | major | 매니페스트 find 0건(r5b §2.4) |
| first-run 마법사·`doctor` 부재 | major | first-run/doctor grep=0(r4 §④, r5b §3.2) |
| README 배지 0.3.0 드리프트 | minor | README.md:3 vs 실제 0.3.63(r5a §1.6) |
| `debs/`에 0.3.0-1 로컬 산출물 트리 잔존 | minor | 배포가 사실상 "로컬 make deb" 흐름(r5b §2.4) |

### 8.3 권고

| 권고 | 무엇/왜 | 공수 | 영향 | P |
|------|---------|------|------|---|
| Linux CI(build+test+deb) | Windows는 push+PR MSVC 빌드+MSI 자동인데 Linux는 0. `windows-msi.yml` 병렬로 cargo build/test + `make deb` 게이트 신설. 코어 공유(§1.2 리스크) 회귀 자동 검출 | S~M | critical | **P0** |
| PKGBUILD·rpm 0.3.63 동기화 또는 퇴역 명시 | 스테일 채널이 "지원되는 척"하는 것이 더 위험 | S | high | **P0~P1** |
| AUR/PPA 채널 | 최대 채택 장벽 해소(§5.3) | M | high | **P1** |
| Flatpak 채널 검토 | 데스크톱 배포 표준, 샌드박스 override 자산 활용 | L | medium | **P2** |
| `unim-cli doctor` | troubleshooting 진단 자동화(§7.3, r5b §3.2) | M | high | **P1** |

---

## 9. 우선순위 로드맵

### 9.1 Impact × Effort 매트릭스

```
 영향
critical │ [P0] ignore_key_repeat 집행(M~L)  [P0] Linux CI(S~M)
         │ [P0] 접근성 프리셋 이식(S~M)      [P0] first-run 마법사(M)
         │ [P0] word 뒷문 가드(S)
─────────┼──────────────────────────────────────────────────────
  high   │ [P0] 싱크누락 2건(S)              [P1] Orca 후보낭독(M)
         │ [P1] 한/영 전환통지(S)            [P1] word DBus 확장(M)
         │ [P1] sticky 게이트정렬(S)         [P1] 토글키 일원화(M)
         │ [P0] AUR/PPA 채널(M)              [P1] doctor(M)
─────────┼──────────────────────────────────────────────────────
 medium  │ [P2] 억제단어 undo(S~M)           [P2] Slint 공용화 재평가(L)
         │ [P2] kime 포지셔닝 문서(S)        [P2] 팝업 라이트/고대비(M)
         │ [P2] popup_font_scale(M)          [P2] Flatpak 채널(L)
─────────┼──────────────────────────────────────────────────────
  low    │ [P2] Win32 폴백 퇴역(M)           [P3] 스캐닝·half-QWERTY(L)
         │                                   [P3] 순수 Wayland 검증(L)
         └──────────────────────────────────────────────────────
            S            M            L           XL   공수
```

> **§5.2④ 공유 코어 결손 배치(매트릭스 반영):** [P1] 한자 단어단위 변환(`search_word`, M/high) · [P1] 범용 사용자 낱말·상용구 사전(M/high) · [P2] 옛한글/고어 입력(L/medium) · [P2] 한자 후보 사용빈도 자동 학습(M/medium). 위 격자에는 지면상 생략했으며 시간 구획 배치는 §9.2를 따른다.

### 9.2 시간 구획

> **우선순위 정의(일관화):** **P0 = 채택·온보딩·접근성·배포위생 차단요소(blocker)** 또는 **의도치 않은/자동 경로의** 사용자 데이터 비가역 손실 방지 — 막히면 '메이저 IME 인식' 자체가 불가한 항목. **P1 = blocker는 아니나 메이저 패리티에 필수인 기능-격차·구조 경화.** 여기에 기현의 접근성 최우선 원칙(입 젓가락 타이핑·오른발 마우스 직결 항목 상향)을 가중한다. **Orca 후보 낭독**은 §7.1.1에서 **시각장애 사용자군의 팝업(한자·이모지·특수문자) 기능에 한정된 blocker**로 명시하되, 로드맵 시기는 **P1**로 둔다 — 근거: 한글 조합·커밋·설정 등 UNIM 핵심 동작은 libadwaita 위젯 내재 AT-SPI로 Orca에서 이미 낭독되므로 전체 채택이 차단되지 않고(데이터 손실도 없음), 팝업 후보 낭독은 보조 기능의 접근성 패리티 항목이다(§9.1의 high 행과 일관). 반면 지체 계열 `ignore_key_repeat` 집행은 "노출된 설정이 무효"라는 신뢰 파괴이므로 P0로 상향한다. 한편 **자동 경로의 비가역 손실**(word 모드 뒷문 가드 = 존재하지 않는 확정문 자동 삭제, §10-A P0)은 P0로 거르되, **사용자의 명시적 클릭 경로**인 억제단어 삭제도 차등하되, 오른발 마우스 오클릭 빈발 프로파일(오클릭 한 번=학습 데이터 비가역 손실)을 감안해 **삭제 confirm은 단기 P1로 상향**하고 undo(스냅샷 복구)만 P2로 둔다(§7.3.2-1).

#### 단기 (0~3M) — 배포 게이트·접근성 집행·차단요소 제거

- **P0(blocker):** Linux CI(build+test+deb) · **접근성: `ignore_key_repeat` Linux 런타임 집행**(7.2-1) · **토글키 5프런트 bare `Alt_R` 스킵 제거 최소 패치**(§3.4 — 프리셋 선행조건, 데몬 판정 일원화 전체는 중기 P1 유지) · **접근성: 프리셋 2종 GTK 이식**(7.2-2, 위 토글키 최소 패치 선행) · word 모드 "열린 뒷문" 가드(§10-A) · 싱크 누락 2건(commit-unit CLI · bidirectional_combine Slint) · **first-run 마법사 착수**(§10-D, 온보딩 blocker=P0).
- **P0~P1:** AUR/PPA 채널 개설(채택 관문) · PKGBUILD·rpm 0.3.63 동기화 또는 퇴역.
- **P1:** **접근성: 한/영 전환 능동 통지**(7.2-3) · sticky 게이트 정렬(7.2-4) · 설정 검색 활성화(`.search_enabled(true)`) · **억제단어 삭제 confirm**(7.3-1의 confirm만, `adw::AlertDialog`·공수 S — 오클릭 비가역 손실 방지).

#### 중기 (3~9M) — 기능 패리티·구조 경화·접근성 확장

- **P1:** word DBus API 확장(`replace_composition` + FocusIn 게이트, §10-A) · 토글키 판정 데몬 일원화 전체(§3.4, 단기 최소 패치 후속) · **접근성: Orca 후보 낭독**(7.1-1) · `unim-cli doctor` · 설정 2계층 IA 이식(§10-B; undo는 P2 분리, §7.3.2-1) · 마법사 페이지별 감지 헬퍼+원클릭 적용(§10-D) · **공유 코어 결손(§5.2④): 한자 단어단위 변환(`search_word`)·범용 사용자 낱말/상용구 사전**(Windows판 §5.3 공동).
- **P2:** **접근성: 팝업 라이트/고대비 팔레트(7.1-2), 억제단어 삭제 undo(7.3-1, confirm은 단기 P1), `popup_font_scale`(7.2-5, POPUP_SPEC §5.2 폰트 명세 개정+승인 선행)** · Flatpak 채널 검토 · Win32 폴백 다이얼로그 퇴역(GUI 3벌→2벌, §10-B).

#### 장기 (9M+) — 확장·플랫폼

- **P2:** Slint 설정앱 크로스플랫폼 공용화 재평가(Orca/accesskit PoC 통과 후, §10-B) · **공유 코어 결손(§5.2④): 옛한글/고어 입력·한자 후보 사용빈도 자동 학습**.
- **P3:** 스위치 스캐닝·half-QWERTY 미러 · 순수 Wayland(비GNOME) 팝업 검증 · kitty 검증 공백 해소.

### 9.3 접근성 로드맵 명시 배치

| 시기 | 접근성 항목 | P |
|------|-------------|---|
| 단기 | `ignore_key_repeat` 집행(7.2-1), 토글키 최소 패치(§3.4·프리셋 선행), 프리셋 GTK 이식(7.2-2), 한/영 전환 통지(7.2-3), sticky 게이트 정렬(7.2-4), 억제단어 삭제 confirm(7.3-1) | P0~P1 |
| 중기 | Orca 후보 낭독(7.1-1), 팝업 라이트/고대비(7.1-2), 억제단어 undo(7.3-1), `popup_font_scale`(7.2-5), doctor(7.3-3) | P1~P2 |
| 장기 | 스위치 스캐닝·half-QWERTY, 순수 Wayland 검증 | P3 |

---

## 10. Windows→Linux 포팅 백로그 (신설 — 4대 영역)

> Windows 라운드(`09269e0..HEAD`)에서 신설된 4대 기능 영역을 Linux로 이식하는 파일 단위 작업 목록. 각 영역은 우선순위 분해(P0~P2) + 공수/영향 + 선행조건 + 파일 단위 작업항목으로 구성한다. 코어는 대부분 **수정 불필요**(이미 공유·테스트 랜딩)이며, 작업의 무게중심은 프런트/데몬 배선과 GUI 이식이다. 단, 이식 참조로 지목한 Windows 원본(접근성 프리셋 로직·`ui_element` flatten·`ignore_key_repeat` 분기·reset 후 1키 재보장 등)은 **커밋·유닛테스트는 완료됐으나 런타임 실측이 0(VM 검증 대기, r3 §2)**이므로, 이식 시 원본 패턴의 실기 검증을 병행해야 한다.

### 10-A 단어 단위 입력 (commit_unit / word preedit) — 근거: r1 전체

| P | 항목 | 공수/영향 | 파일 단위 작업 |
|---|------|-----------|----------------|
| **P0** | **뒷문 가드**: config에 `commit_unit: Word` 수기 기재 시 데몬이 누적을 켜는데(engine.rs:252-253) ATF는 `replace_composition` 미소비 → delete_chars가 존재하지 않는 확정문 삭제 가능(데이터 손실, r1 §⑤-1) | S / critical | `unim-dbus/src/engine_worker.rs:947-959`에 word 모드 시 ATF apply 억제(또는 delete_chars 0 강제) 단기 가드 |
| P1 | DBus AutoTypeFix 시그널 `replace_composition` 확장 + word replay | M / high | `service.rs:258-266, 1980-1985` 시그널 4튜플화; `engine_worker.rs:188-202` 전달; `unim-tsf/src/auto_typefix.rs:429-455` 상응 replay를 Linux wrapper에. **재생성 패턴 계승 금지**: Windows replay는 `engine.reset()` 기반이므로 Linux의 `InputEngine::new` 재생성(engine_worker.rs:856·888)을 그대로 옮기지 말 것(아래 무재생성 전환 행과 연동) |
| P1 | ATF replay 무재생성 전환(핫패스 성능·§3.5) | S / high | `engine_worker.rs:856·888`의 `*engine = InputEngine::new(&config)`를 `engine.reset()`(engine.rs:478 `pub fn reset`, 보존 목록 doc :469-477 — hanja_dict/북마크/카테고리 등 보존)로 교체, 교정마다 ≈6.45MB 한자사전 재파싱 제거. Windows `auto_typefix.rs` I3-perf 패턴과 정합(코어 API 1급 대응 완료 상태) |
| P1 | FocusIn 게이트(TSF reapply_word_gate 등가) | M / high | `engine_worker.rs:59-60(extract_app_id), :208-253` 활용해 `is_word_mode_app`→`set_word_mode`; reset 후 1키 재보장 패턴 동일 채택(r1 §⑤-3) |
| P1 | `word_mode_apps` Linux 형식 결정 | S / medium | `src/config.rs:519-527` — Windows exe명("winword.exe")과 WM_CLASS/app-id 불일치. Linux 기본 후보(LibreOffice Writer 등) 정책 결정. **XIM·터미널류 제외 원칙**(wmux 교훈, r1 §⑤-10) |
| P1 | 설정 6지점 확장 | M / medium | dbus Get/Set/ConfigChanged + unim-settings 콤보 + gschema + **CLI `commit-unit` 신설**(현 부재, unim-cli/src/main.rs grep=0) — impl-report:98 |
| P2 | 프런트 preedit caret 정책 + 한자팝업×word 검증 + word×chord 가드 | M / medium | gtk3/gtk4 immodule.c cursor index 끝-추종, qt-common, wayland는 자연 호환(r1 §④-3); `candidates.rs:22-29` 경로 테스트 — **word 다음절 preedit이 생겨도 한자 변환은 여전히 마지막 1음절만 대상**(단어단위 변환 `search_word` 경로 부재, §5.2 ②와 상호작용, 미검증); **word×chord(모아치기) 동시 설정 동작 미정의**(r1 §⑤-4: `input_context.rs`가 word 모드를 sequential 전용으로 두어 chord 교차는 명시적 범위 밖 — Linux는 모아치기가 GTK 노출이라 `commit_unit=Word`+chord 조합이 실제 발생 가능) 검증·가드. ※ `commit()` 반환은 word 모드에서 best-effort 마지막 char 계약(input_context.rs:353-355, r1 §⑤-6) |

- 선행조건: **P0 가드가 모든 것에 선행**. GTK/Qt/Wayland 먼저, XIM은 명시적 비대상. 코어는 수정 불필요(r1 §④ "100% 재사용").

### 10-B 설정 GUI (Slint 개편의 Linux 이식) — 근거: r2 전체

| P | 항목 | 공수/영향 | 파일 단위 작업 |
|---|------|-----------|----------------|
| **P0** | 싱크 누락 2건 해소(하우스 3지점 원칙 위반 현행) | S / high | `unim-cli/src/main.rs`에 commit-unit ConfigKey; `settings.slint:307` bidirectional_combine 노출(또는 미노출 사유 문서화) |
| P1 | 단기 노선 (a): GTK 유지 + Windows 신기능 이식 | S~M / high | `settings_dialog.rs:84` search_enabled(true); 프리셋 로직 코어/unim-gui-common 하향(원본 unim-tsf-settings/src/main.rs:269-279, 805-872 — toolkit-free); confirm=adw::AlertDialog(:1371-1466); ATF 2계층 IA(:945-1235 재구성) — **undo(main.rs:469 실체는 '마지막 삭제 1건 복원')는 P2 분리(§7.3.2-1·§10-C P2)** |
| P2 | Slint 공용화 (b)는 **Orca/accesskit PoC 통과 후 재평가** | L / medium | 리스크: AT-SPI 미검증·Skia 패키징·SetConfigYaml 배선(settings_dbus.rs:14-19)·GSettings 페이지 이관(r2 §④-b) |
| P2 | Win32 폴백 다이얼로그 퇴역(GUI 3벌→2벌) | M / medium | `unim-tsf/src/settings_dialog.rs` 1,694줄 — Windows 측이나 드리프트 원천이라 병기 |

- 선행조건: (b)는 (a) 완료 + Orca PoC 후. i18n 이중 체계(.po vs rust-i18n .yml) 드리프트 감시 항목 명기(r2 §⑤-8).

### 10-C 접근성 (본 보고서 §7과 교차 인덱스) — 근거: r3 §5

| P | 항목 | 공수/영향 | 파일 단위 작업 |
|---|------|-----------|----------------|
| **P0-1** | `ignore_key_repeat` Linux 런타임 | M~L / critical (계약 변경 감안 상향) | ① **DBus 계약 확장**: `unim-dbus/src/service.rs:1739 process_key_event` 시그니처에 repeat 플래그 필드 추가(7프런트+ibus_compat 공통 계약이라 파급 큼) — **대안: 프런트 로컬 억제(계약 무변경)와 택일 결정 필요**. ② **프런트별 repeat 감지 지점**: gtk3/gtk4 `immodule.c`(GdkEventKey), qt-common(QKeyEvent `isAutoRepeat`), xim `handler.rs`(XKB detectable autorepeat), GNOME `key_handler.js`(Clutter repeat). ③ **Wayland 특례**: `unim-frontends/wayland/src/repeat.rs`는 input-method-v2 특성상 키 반복을 **자체 생성**하므로 'OS 자동반복 판별'이 아니라 자체 반복 억제가 최저비용 경로(계약·프런트 우회) — 별도 취급. 억제 로직 자체는 `text_service.rs:902-917` 이식 |
| **P0-2** | 프리셋 2종 GTK + 순아래 배지 | S~M / high | `settings_dialog.rs:727` build_accessibility_group 확장; 로직은 10-B 코어 하향분 재사용. **선행조건: §3.4 토글키 5프런트 bare `Alt_R` 스킵 제거 최소 패치(단기 P0로 선행 편입, 데몬 일원화 전체는 중기 P1)** |
| P1-3 | 한/영 전환 통지(비프 + AT-SPI announce) | S / high | gdk beep 또는 libcanberra; GNOME은 indicator.js/extension에서 announce·OSD 병행 |
| P1-4 | sticky peek 게이트 정렬 | S / medium | engine_worker 단축키 게이트 앞 `peek_sticky_masked_modifiers`(engine.rs:363); unim-imm32 동일 |
| P1-5 | Orca 후보 낭독(I8 등가) | M / high | GTK4 `Accessible::announce()` + `ui_element.rs:70-99` flatten 재사용 + 셀 accessible label(hanja.rs:316-355). POPUP_SPEC 무포커스 불변 |
| P2 | 팝업 라이트/고대비(I5 등가) / 억제단어 삭제 confirm(단기 P1)+undo(P2)(I6 등가) / `popup_font_scale`(클릭 타깃 확대) | M · S~M · M | popup_styles CSS 2벌 분기(adw StyleManager)·extension stylesheet — **선행조건: `POPUP_SPEC.md:364-405`(§5 색상표) 개정 + 사용자 승인**; settings_dialog.rs:1371-1466; **`popup_font_scale`은 `POPUP_SPEC.md:383-397`(§5.2) 폰트 명세 개정+승인 선행·6지점 신설·`gtk::Scale` 슬라이더** |
| P3 | 스위치 스캐닝·half-QWERTY 미러 | L | 양 플랫폼 공통 신규 — 후속 설계(팝업 미의존 접근성 확장) |

### 10-D 마법사 + 매뉴얼 — 근거: r4 전체, r5b §3

| P | 항목 | 공수/영향 | 파일 단위 작업 |
|---|------|-----------|----------------|
| **P0** | first-run 마법사 unim-settings 통합(온보딩 blocker 해소) | M / critical | `--first-run/--whats-new` 인자 + adw::NavigationView; `is_new_since/parse_semver`(unim-tsf-settings/src/main.rs:390-404 순수 함수)를 unim-gui-common으로 이동; seen = XDG 상태 파일(`~/.local/state/unim/wizard-seen`) |
| P1 | 페이지별 감지 헬퍼 + **원클릭 적용** | M / high | 언어팩→im-config 활성 감지(25_unim.rc:9-15 env 검사); **'im-config 적용' 버튼이 `im-config -n unim`(사용자 레벨·`~/.xinputrc` 기록·root 불필요)을 마법사에서 직접 실행**하고, 완료 후 **로그아웃/세션 재시작 유도 버튼**까지 플로우에 포함(터미널 명령 표시는 폴백만 — 입 젓가락 타이핑 사용자 부담 회피); 기본입력기→`gnome-extensions info`/env; 딥링크 `xdg-open` gnome-control-center(실패 무해 패턴 ime.rs:125 동일) |
| P1 | 기동 트리거(lazy 감지) | S~M / medium | postinst GUI 불가(root·무DISPLAY, r4 §④-3) → unim-settings 기동 시 검사 + daemon 첫 기동 desktop notification 권유 조합 |
| P1 | `unim-cli doctor` | M / high | troubleshooting §1 진단 절차 자동화(state frontends·env·IM 캐시·데몬 생존) — r5b §3.2 |
| P2 | 매뉴얼 보강 4건 | S~M / medium | ① Linux user-guide에 "오탐 해소 ①~⑦ 처방전" 장 미러(Windows판 L73-103) ② 0.3.63 신기능·버전 스탬프 반영(현 0.3.0 기준) ③ 마법사 사용법 장(양 플랫폼 공통 미기재) ④ Windows판 영문판 신설(ko/en 쌍 관례 정합) |

---

## 11. 부록

### 11.1 근거 파일:라인 인덱스 (핵심)

**공유 코어 `src/`:**
- `src/config.rs:52-94, 519-527, 589-594` — CommitUnit·word_mode_apps·commit_unit 브리지
- `src/config.rs:879-906` — `toggle_announce_beep`·`ignore_key_repeat` config 키(+테스트 :1499-1569)
- `src/input_engine/engine.rs:156, 252-253, 363-373, 397-431` — sticky_toggle_mask, set_accumulate_word, peek_sticky_masked_modifiers, set/is_word_mode
- `src/hangul/input_context.rs:195-205, 216-238, 337-370` — word compose 라우팅·backspace 키재생·display/commit
- `src/auto_typefix/mod.rs:57-63` — `replace_composition` 필드; forward.rs:130-134·reverse.rs:89-92
- `src/keystroke/keymap/ko_3bul_noshift.json:8-10` — 순아래 accessibility/noshift 태그
- `src/build.rs:2-3` · 루트 `Cargo.toml:18-20` — 유령 x11/libc 링크(사용 grep=0)

**DBus·데몬:**
- `unim-dbus/src/engine_worker.rs:59-60, 188-202, 208-253, 702-704, 947-959` — extract_app_id, ATF 전달, PerApp, buf.word_mode 배선
- `unim-dbus/src/service.rs:258-266, 1980-1985` — AutoTypeFix 3튜플 시그널
- `unim-daemon/src/main.rs:148-179, 347-357` — Flatpak override 자동화, IBus 호환 활성

**프런트엔드 7종:**
- `unim-frontends/gtk3/src/immodule.c:598, 808-847` · `gtk4/src/immodule.c:678` — Alt_R 스킵, purpose 배선
- `unim-frontends/qt5/src/input_context.cpp:306, 468-480` · `qt6/src/input_context.cpp:290` — Alt_R 스킵, purpose
- `unim-frontends/xim/src/handler.rs:378-, 474, 909-952` — ON-THE-SPOT 누락, N+1 BS
- `unim-frontends/wayland/src/state.rs:569` — content-type 미사용
- `unim-gnome-extension/key_handler.js:19` · `popup_view.js:79, 237-240` — Alt_R 스킵, 팝업 accessible 0
- `docs/dev/linux/toggle-key-frontend-config-bypass.md:15-25` — 5프런트 하드코딩 추적

**GUI 군·배포:**
- `unim-settings/src/settings_dialog.rs:84, 387, 727-780, 1371-1466` — search OFF, bidirectional, 접근성 그룹, 삭제(confirm 없음)
- `unim-gui-common/src/settings_dbus.rs:14-19` — SetConfigYaml 능동 전파
- `unim-popup-service/src/popup/hanja.rs:76, 153-154, 316-355` · `popup_styles.generated.css:145` — 무포커스, 창 라벨, Mocha 하드코딩
- `unim-cli/src/main.rs` — commit_unit grep=0(싱크 누락)
- `.github/workflows/windows-msi.yml`(유일) · `PKGBUILD:3 pkgver=0.3.0` · `debian/control`(10패키지)

**명세·문서:**
- `docs/dev/specs/POPUP_SPEC.md:229, 364-405, 567-573, 630-739, 769, 780-789` — Wayland ◀/▶ 클릭 불가, §5 Catppuccin Mocha 규범 팔레트·폰트, dismiss 휴리스틱, 팝업 단일 SoT, 첫 팝업 지연, 렌더러 3분기
- `docs/dev/architecture/IME_BEHAVIOR.md:30-33, 46-49, 239-257, 266-280` — gedit "늘늘"·영문 Space 특례, 새 프런트 체크리스트, 키 hot-path DBus 왕복
- `docs/dev/architecture/AGENTS.md:21, 35-43, 64-73, 85-87, 115-158`(루트 `AGENTS.md`는 6줄 redirect stub이므로 반드시 이 경로로 대조) — 구명칭 드리프트, DBus 구조, 팝업 SoT, call_sync 키 큐, 메모리 규율
- `docs/user/troubleshooting`(README-ko):537-539·564·566 — GNOME Wayland 이중 표시, Chrome preedit SKIP, XIM ON-THE-SPOT 누락
- `docs/user/faq/README-ko.md`(Q1-Q2, :8-27) · `docs/user/user-guide/README-ko.md:12` — 자사 비교표(경쟁 셀 미검증), 단일 코어 서술
- `docs/dev/linux/toggle-key-frontend-config-bypass.md:15-25` — 5프런트 Alt_R 하드코딩 추적(프런트엔드 그룹에도 수록)

**Windows 원본(이식 참조):**
- `unim-tsf/src/text_service.rs:902-917` — ignore_key_repeat 집행
- `unim-tsf-settings/src/main.rs:269-279, 390-404, 805-872` — ATF 프리셋, is_new_since/parse_semver, 접근성 프리셋
- `unim-tsf/src/ui_element.rs:70-99` — 후보 낭독 flatten 로직
- `unim-windows-common/src/ime.rs:38-186` — 마법사 감지 헬퍼

### 11.2 참고 표준·경쟁제품

**표준/API:**
- 입력: XIM(OVER/ON-THE-SPOT) · GTK IM Context · Qt QPlatformInputContext · Wayland input-method-unstable-v2 / text-input-v3 · IBus DBus 인터페이스
- 접근성: AT-SPI2 · Orca · GTK4 `Accessible::announce()`(라이브 리전) · GNOME HIG · WCAG 2.2(1.4.3/1.4.11/2.1.4/3.3.4) · XKB detectable autorepeat · 데스크톱 고대비/필터키
- 배포: Debian Policy(postinst/prerm) · im-config · XDG(Base Directory/autostart/state) · systemd user · AUR PKGBUILD · Flatpak/Snap
- 국제화: rust-i18n · gettext(.po) · GSettings/gschema

**경쟁제품:**
- ibus-hangul(IBus 기반, 배포판 기본 수록) · fcitx5-hangul(Fcitx5 호스트) · kime(Rust·한국어 전용, GitHub/AUR 유통) · nimf(단일 코어 다중 프론트, 포크 유지) — 자사 비교표(faq/README-ko.md Q1)는 프로젝트 자체 평가·경쟁 셀 미검증.

### 11.3 검증 메모

본 보고서는 13개 축의 파일:라인 대조 검증 분석을 종합했으며, 작성 시 다음 핵심 근거를 실코드로 재확인함: `.github/workflows/` = `windows-msi.yml` 단 1개(Linux CI 부재), `unim-cli/src/main.rs`에서 commit_unit grep=0(commit-unit ConfigKey 부재), `ignore_key_repeat` 집행이 `unim-tsf/src/text_service.rs`에만 존재하고 `unim-frontends/` grep=0(Linux 런타임 no-op), `gtk3/src/immodule.c:598`·`gtk4/src/immodule.c:678` bare `Alt_R` 스킵, `unim-settings/src/settings_dialog.rs:84 .search_enabled(false)`, `PKGBUILD:3 pkgver=0.3.0`(워크스페이스 0.3.63과 스테일), `popup_styles.generated.css:145 #1e1e2e`(Mocha 하드코딩), HEAD `dc0f98a`. 원 인상 대비 주요 정정: (1) "코어에 플랫폼 의존성이 섞였다"는 오도 — `cfg(windows)` grep=0, 유령 x11/libc는 사용 grep=0의 죽은 링크(§3.3). (2) "Linux에 접근성 옵션이 있다"는 절반의 진실 — GUI·CLI 노출과 3지점 config 싱크는 완료됐으나 **런타임 집행이 없어 설정해도 무효**(§7.2). (3) "unim-gui-gtk" 구명칭은 문서 드리프트 — 실제는 unim-settings + unim-indicator + unim-popup-service 분리(r2 §③). (4) 터미널 preedit 이슈·kitty 검증은 저장소 문서 근거 부재라 **재검증 필요(미검증)**로 표기(§4.2). (5) 경쟁 IME 비교표는 자사 문서 기반이라 경쟁 셀 값 자체는 미검증(§5.1).
