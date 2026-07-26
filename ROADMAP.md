# UNIM 프로젝트 로드맵

이 문서는 **UNIM** 프로젝트의 장기 목표와 개발 단계별 계획을 설명합니다.

## 🎯 핵심 목표

언어 상태 자동 감지 및 수동 텍스트 변환 기능을 갖춘, 하나로 통합된 크로스 플랫폼(Windows, macOS, Linux) 한국어 입력기 엔진(IME)을 구축하는 것입니다.

## 🛣️ 개발 단계

### 1단계: 기반 구축 및 리눅스 네이티브 (완료)

- [x] 한글 조합 로직을 갖춘 견고한 Rust 코어 라이브러리 개발.
- [x] 데이터 자산이 내장된 이식 가능한 `unim-cli` 구현.
- [x] `St.Clipboard`와 `Clutter`를 사용한 네이티브 GNOME Shell 확장 프로그램 개발.
- [x] 안정성을 위한 하이브리드 아키텍처(CLI + 네이티브 API) 적용.

### 2단계: 3계층 아키텍처 및 전체 프론트엔드 (완료)

- [x] **DBus 데몬 아키텍처**: `unim-daemon` + `unim-dbus` 기반 중앙 엔진 서비스 구축.
- [x] **GTK3/GTK4 IM 모듈**: C 언어 기반 IM Module 구현 (공통 코드 `gtk-common` 분리).
- [x] **Qt5/Qt6 플러그인**: C++ 기반 QPlatformInputContext 플러그인 구현 (공통 코드 `qt-common` 분리).
- [x] **XIM 프론트엔드**: Rust `xim` crate 기반 X11 XIM 서버 구현 (Over-The-Spot Preedit, 프로토콜 적합성 검증 완료).
- [x] **Wayland 프론트엔드**: `input-method-v2` + `virtual-keyboard-v1` 프로토콜 기반 구현 (KDE Plasma 지원).
- [x] **한자/특수문자 입력**: 모든 팝업이 `unim-popup-service` 단일 서비스로 중앙화 완료. GNOME Wayland만 Shell extension `popup_view.js`(St 위젯)로 자체 렌더, 그 외 환경(X11·기타 Wayland)은 popup-service(GTK4)가 전담. GTK/Qt IM 모듈의 임베디드 팝업 위젯은 제거됨.
- [x] **설정 도구**: GTK4 통합 설정 창 (`unim-settings-gtk`) + CLI (`unim-cli config`).
- [x] **시스템 트레이**: 트레이 인디케이터(`unim-indicator`) 별도 프로세스로 분리.

### 3단계: 문서화 및 안정화 (진행 중)

- [x] **컴포넌트별 SPEC.md 작성**: 12개 컴포넌트 기능 명세 문서화.
  - `src/`, `unim-capi/`, `unim-cli/`, `unim-daemon/`, `unim-dbus/`
  - `unim-frontends/gtk3/`, `gtk4/`, `qt5/`, `qt6/`, `xim/`, `wayland/`
- [x] **XIM 프로토콜 적합성 검증**: [XIM 사양](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html) 대비 3회 교차 검증 (11개 항목 적합).
- [x] **Wayland 프로토콜 참조 문서화**: `input-method-v2`, `virtual-keyboard-v1` 프로토콜 사양 참조 및 아키텍처 문서화.
- [x] **Wayland 키 반복(Key Repeat)**: `mio` + `timerfd` 기반 구현 완료 (`repeat.rs`).
- [x] **Wayland 한자/특수문자 팝업**: `zwp_input_popup_surface_v2` 기반 구현 완료 (`popup_surface.rs`, `popup_renderer.rs`).
- [ ] **Surrounding Text / Content Type**: Wayland 프로토콜 이벤트 활용 (Phase 4).
- [ ] **Debian 패키지 안정화**: 패키지 빌드/설치 프로세스 검증 및 개선.

### 3.5단계: UI 프런트엔드 분리 (Fcitx5 스타일)

엔진(daemon)과 UI(팝업/인디케이터/설정)를 DBus 시그널 기반으로 완전 분리, 툴킷별 네이티브 GUI 지원.

- [x] **unim-gui 모듈 분리**: DBus, 트레이, UI 모듈 분리 완료.
- [x] **unim-gui-common 크레이트**: DBus 통신 + 트레이 등 공통 로직 추출 완료.
- [x] **unim-gui-gtk 전환**: `unim-gui-common` 의존으로 전환 완료.
- [x] **unim-gui-qt 신규 구현**: cxx-qt 기반 Qt6 네이티브 GUI 구현 완료.
- [x] **Debian 패키지 재구성**: 9개 바이너리 패키지로 분할 (`unim-common` / `unim-im-gtk` / `unim-im-qt` / `unim-xim` / `unim-wayland` / `unim-gui-gtk` / `unim-gui-qt` / `unim-gnome` / `unim` 메타). GUI 두 개 공존 허용(Conflicts 불필요), `unim-gnome`은 `unim-gui-gtk`를 Depends로 강제. `apt install unim` 한 줄로 full stack 설치.

### 3.7단계: 자판 프로필 v1 (완료)

자판 정의를 하드코딩 Rust const에서 **자기 완결 v1 JSON**으로 이관. 사용자 자판(`~/.config/unim/layouts/*.json`) + 상속(`inherits`) + 선택형 규칙 세트(rule_sets) 지원. Phase 6단계 엔진 재설계(v2)의 데이터 기반을 마련.

- [x] **v1 스키마 정의** (`docs/dev/plans/LAYOUT_PROFILE_V1.md`): schema_version, metadata(다국어), inherits, combinations(자기 완결), rule_sets, active_rule_sets. v0 하위 호환 자동 승격.
- [x] **Phase 1·2 — 로더·빌더·Composer 통합**: `src/keystroke/profile/` 하위에 schema/loader/builder/localized 신설. `HangulComposer{2,3}Bul::new_with_profile` + v0→v1 동일 결과 regression.
- [x] **Phase 3 — 레지스트리·상속·핫리로드**: `ProfileRegistry`(내장 + `~/.config/unim/layouts` 통합 네임스페이스, 사용자 우선), `inherit::resolve`(재귀 해석 + 순환 탐지 + layer-merge), 디렉토리 mtime 기반 자동 재스캔.
- [x] **Phase 4 — Config·CLI·DBus·엔진 연결**: `korean.custom_layout`(`Option<String>`)·`korean.active_rule_sets`(`Vec<String>`) 필드 5-point 싱크. `unim-config layout list/describe/validate` 서브커맨드. `InputEngine::new`가 ProfileRegistry를 거쳐 효과적 프로필을 로드, 실패 시 enum 경로 폴백.
- [x] **Phase 5 — GTK GUI**: settings_dialog의 한국어 자판 ComboRow가 모든 한국어 프로필(내장 + 사용자) 표시. 선택 시 규칙 세트 SwitchRow가 동적 재구성.
- [x] **Phase 6 — 내장 10종 v1 이관**: `docs/references/keymaps/*.json` 9종 + 신규 `ko_3bul_qwerty` 1종을 `src/keystroke/keymap/`로 이관. 기존 Rust const와 동일 `CombinedJamoMap` 산출 (behavior-preserving, regression test로 고정).
- [x] **Phase 7 — 문서·마이그레이션 공지**: 본 섹션 + CHANGELOG Added 블록 + README 간단 안내.
- [x] **키맵 도구 제공**: 자판을 눈으로 보고(보기), 편집하고(키맵 스튜디오), 익히는(타자 연습) GTK4 도구 3종 제공. 키맵 스튜디오·타자 연습은 5행 키보드 위젯을 공유. 키맵 스튜디오는 헤더 3단 드롭다운(언어 › 출처 › 자판) + 4탭 구성, 빌트인 보호 / 사용자 자판 저장 정책.

### 4단계: 자동 상태 전환 (지능화)

- [ ] **문맥 감지**: 현재 입력 필드 상태나 언어 문맥을 감지하는 방법 연구.
- [x] **자동 교정 엔진 (AutoTypeFix)**: 실시간 오타 감지 구현. forward(영→한: `gksrmf` → `한글`), reverse(한→영: `ㅈㅐㅍㅁ` → `wave`) 양방향 지원. XIM·GTK3/4·Qt5/6·Wayland·GNOME Shell 전 프론트엔드 통합. (`src/auto_typefix.rs`)
- [x] **사용자 학습 — 억제 사전(Blacklist)**: 롤백 관측(BS + 모드 전환) + 재시도 시점 자동 등록 방식으로 "원치 않는 교정" 단어를 Tentative로 학습. GUI에서 Confirm 시 Confirmed, 시간 만료 시 Inactive. `~/.config/unim/typefix-blacklist.yaml`에 저장, 데몬 mtime 핫리로드. (`src/typefix_blacklist.rs`)
- [ ] **사용자 학습 — 양성 사전**: 사용자별 타이핑 패턴 기반의 *긍정적* 로컬 사전(오타 교정 promotion)은 미구현. 현재 Blacklist는 교정 제외만 담당.

### 5단계: 크로스 플랫폼 확장

- [ ] **입력 컨텍스트 통합**: 단순 "변환 도구"에서 완전한 입력기(IME) 서비스로 진화 (리눅스용 `ibus`, `fcitx5` 연동).
- [ ] **크로스 플랫폼 지원**: Windows(TSF) 및 macOS용 네이티브 백그라운드 서비스 및 연동 방안 조사.

> **메모 — `unim-capi` 위치**: 현재 UNIM 내부 컴포넌트는 모두 DBus 또는 Rust API를 직접 사용해 unim-capi를 링크하는 in-tree 소비자가 없습니다(프런트엔드의 capi 링크 의존도 해제됨). unim-capi는 외부 프로그램이 UNIM 코어를 임베딩하기 위한 **공개 C API**로 유지하며, 공개 헤더 `unim.h`는 빌드 시 Rust 표면과의 드리프트를 자동 검사합니다. 위 크로스 플랫폼 임베딩의 토대가 됩니다.

### 6단계: 엔진 재설계 (고급 한글 입력 기법 지원)

현재 UNIM의 한글 엔진은 **정적 키맵 + 하드코딩 오토마타** 구조라 아래 기능들을 표현할 수 없다. 복벌식·갈마들이 조사(`docs/references/research/복벌식·갈마들이 조사.md`)와 순아래받침 규칙 조사(`docs/references/research/순아래받침_규칙.md`)에서 드러난 공통 결론: **낱자에 "어디서 왔는지" 정보가 붙어야 하고, 키 해석이 컴포저 상태에 접근할 수 있어야 한다**. 이 두 전제를 도입하는 엔진 리팩터가 아래 모든 항목의 선행 조건이다.

- [ ] **낱자 provenance 태깅**: `Jamo` 표현을 `(kind, source_key)` 튜플로 확장해 같은 ㅗ/ㅜ라도 어느 키에서 왔는지 구별. 세벌식 390의 `9`-ㅜ, `/`-ㅗ 이중모음 전용 역할, 복벌식 자동 판정의 근거가 되는 날개셋문자 64-bit 토큰 개념(연구 문서 §4.1)에 대응.
- [ ] **문맥 의존 키 해석 (글쇠 수식 최소 집합)**: 키→자모 매핑이 컴포저 상태(`has_cho`/`has_jung`/`has_jong`/`syllable_empty`)를 조회할 수 있도록 predicate 엔진 도입. 두벌식 `/`, 세벌식 390 `/` 같은 적응형 글쇠(연구 문서 §4.2) 지원. 날개셋의 Turing-complete 수식 전면 이식은 별도.
- [ ] **자판 프로필 v2**: v1(`docs/dev/plans/LAYOUT_PROFILE_V1.md`)에서 유보한 provenance + predicate 필드를 스키마에 추가. 세벌식 390 원본 규약을 있는 그대로 재현.
- [ ] **모아치기 (stroke replay)**: 낱자 입력 순서가 바뀌어도 재배열해 한 음절로 조합. 안마태 자판 등 순서 자유 자판 지원.
- [ ] **복벌식**: 어절 첫 타자의 손 위치(좌/우)로 두벌식·세벌식 자동 전환(연구 문서 §5.2). 어절 단위 버퍼 + 첫 낱자 provenance가 전제.
- [ ] **옛한글**: U+1100 확장 블록 낱자, 방점, 합용병서 지원. Jamo enum 확장 + 고급 문자 생성기.
- [ ] **초·종성 공유 결합 규칙 (shared combination)**: 날개셋 §5.1의 "종성이 초성 결합 규칙을 중첩 적용" 동작을 엔진 레벨에서 직접 표현. v1의 `share_cho_jong` 플래그는 복제 수준에 그침.
- [ ] **날개셋 `.ist` 수입기** (별도 바이너리 `unim-import-nalgaeset`): XML로 내보낸 `.ist`만 읽어 UNIM v2 프로필로 변환. 바이너리 `.ist`는 비지원(연구 문서 §7).

### 7단계: 입력 방식·플랫폼 확장 (구상 — 미착수)

아래 6개는 아직 계획 단계도 아닌 **아이디어 백로그**다. 각 항목의 "선행"은 착수 전에
반드시 끝나 있어야 하는 것, "난점"은 그 항목이 실패한다면 십중팔구 여기서 실패한다는 지점이다.
규모는 전부 주 단위 이상이며, 순서는 우선순위가 아니다.

- [ ] **① 한자 단어 단위 변환**: 현재는 `hanja_target` 이 한 음절(또는 현재 preedit)이라 `대한민국`을
  `大韓民國`으로 한 번에 바꿀 수 없다. **유리한 조건 — 사전 데이터는 이미 있다**:
  `src/data/hanja.txt`(libhangul 유래) 30만 3천 행 중 **다음절 항목이 27만 5천 개**이고
  `HanjaDictionary` 는 한글 문자열 키의 `HashMap` 이라 `entries.get("대한민국")` 이 그대로 동작한다.
  단어 누적 버퍼(`InputContext::word_buffer`, `commit_unit = Word`)도 이미 존재한다.
  - 필요한 일: 변환 대상 결정(어절 최장일치 → 실패 시 음절 폴백), 이미 **커밋된** 단어를 뒤에서
    변환하는 경로(`surrounding_text` + `smart_backspace` 인프라 재사용, 한컴·MS IME 방식),
    후보 랭킹(현재는 사전 등장 순), 팝업의 다중 음절 표시(`POPUP_SPEC.md` 준수).
  - 난점: "어디까지가 한 단어인가" 판정. 조사가 붙은 `대한민국은` 에서 `대한민국`만 잘라내려면
    최장일치 + 조사 목록이 필요하고, 잘못 자르면 사용자가 매번 범위를 고쳐야 해 오히려 느려진다.

- [ ] **② macOS 입력기**: InputMethodKit(`IMKServer`/`IMKInputController`) 기반 네이티브 IME.
  DBus 없이 in-process 구조라 **Windows TSF 포팅과 같은 형태** — 코어는 그대로 두고 프런트엔드만 새로.
  - 선행: `unim-capi`(현재 `pub extern` 58개, `unim.h` 드리프트 가드 있음)가 Swift/ObjC 에서
    필요한 표면을 다 덮는지 점검. 덮지 않으면 capi 확장이 1순위 작업.
  - 난점: 코드 서명·공증(Apple Developer 계정 필수)과 `~/Library/Input Methods` 번들 배포 체계.
    빌드도 macOS 머신이 있어야 한다(현재 개발 환경엔 없음 — CI 러너로 우회 가능한지 확인 필요).
    후보창은 `IMKCandidates` 를 쓸지 자체 `NSPanel` 을 그릴지 결정 필요(POPUP_SPEC 대응).

- [ ] **③ Android · iOS 입력기**: 데스크톱 포팅과 **성격이 다르다** — 화면 자판 UI 자체를 우리가 그려야 하고,
  그 자판이 곧 제품의 얼굴이 된다. ⑥ 한손 자판과 사실상 한 몸.
  - Android: `InputMethodService` (Kotlin) + JNI ↔ Rust 코어(`cargo-ndk`).
  - iOS: Keyboard Extension(`UIInputViewController`) + Rust staticlib. **앱 확장 메모리 한도**(수십 MB)가
    실질 제약이라 ⑤ 예측 모델과 동시 적재는 어려울 수 있다. "전체 접근 허용" 없이 동작하는 범위 설계 필요.
  - 난점: 두 플랫폼 모두 스토어 심사·서명 체계가 필요하고, 데스크톱과 공유 불가능한 UI 코드가
    대량으로 생긴다. 유지보수 대상이 사실상 두 개 늘어난다.

- [ ] **④ 음성 입력**: 접근성 관점에서 이 목록 중 사용자 체감이 가장 클 수 있는 항목.
  - 설계 결정 3가지: (a) **온디바이스 전용**(whisper.cpp 계열)인가 클라우드 허용인가 —
    UNIM 의 프라이버시 원칙상 온디바이스 기본이 자연스럽다, (b) 결과를 preedit 으로 넣어 수정 가능하게
    할지 바로 커밋할지, (c) 구두점·편집 명령("지우기", "쉼표")을 받는 명령 모드를 둘지.
  - 선행: 스트리밍 부분 결과를 IME 프로토콜에 태우는 방식 정리(각 프런트엔드의 preedit 갱신 빈도 제약),
    트리거 방식(핫키 vs 토글 vs 상시 대기).
  - 난점: 한국어 모델의 정확도·지연·모델 크기 삼각 절충. 그리고 상시 대기 시 마이크 점유 정책.

- [ ] **⑤ 단어·문장 예측 후보창 (경량 LLM)**: 다음 단어/문장 후보를 후보창에 제시.
  - **선행(필수) — 비밀번호 필드 억제**: `content_purpose` 게이트(2026-07-26 완료)가 예측 경로에도
    적용되어야 한다. 예측은 입력 문맥을 모델에 넣는 기능이라, 이 게이트 없이는 켜서는 안 된다.
    민감 필드에서는 예측·학습 양쪽 모두 정지가 기본값.
  - 기존 자산: `surrounding_text`(문맥), `typefix_userdict`·`typefix_blacklist`(로컬 학습 선례와 저장 규약),
    팝업 서비스(후보창 렌더).
  - 난점: **지연 예산**. 타이핑 흐름을 끊지 않으려면 키 입력당 수십 ms 안에 후보가 나와야 하는데,
    이는 모델 크기의 상한을 정해버린다. n-gram/사전 기반 1차 후보 + 소형 LM 재순위 같은 하이브리드가
    현실적일 가능성이 크다. 메모리·배터리(모바일에서는 ③과 충돌), 학습 데이터의 로컬 보관 원칙도 함께.

- [ ] **⑥ 한손 입력 자판 (천지인 · 나랏글 등)**: 모바일(③)의 전제이자, 데스크톱에서도 한손 사용자에게 의미 있는 항목.
  - **선행(필수) — 자판 프로필 v2**(6단계). 현재 v1 스키마는 정적 키→자모 매핑이라 이 자판들의 핵심 동작을
    표현할 수 없다: 천지인의 **같은 키 반복 타 순환**(ㄱ→ㅋ→ㄲ)과 `ㅣ·ㅡ` 조합 모음, 나랏글의
    **획추가·쌍자음 변형 키**(현재 조합 중인 낱자에 연산을 가하는 방식). 둘 다 6단계의
    provenance 태깅 + 컴포저 상태 접근 predicate 가 있어야 자연스럽게 표현된다.
  - 추가로 필요한 것: **multi-tap 타이머**(같은 키 재타 vs 다음 글자 시작을 시간으로 가르는 확정 규칙).
    기존 `chord_window_ms`(모아치기)와 개념이 비슷하나 의미가 반대라 별도 설계가 필요하다.
  - 난점: 타이머 기반 확정은 프런트엔드마다 키 이벤트 타이밍 보장이 달라(특히 XIM·원격 세션)
    동작이 갈릴 수 있다. 타이머 없는 확정 방식(다음 키가 오면 즉시 확정)과의 선택지 비교가 선행되어야 한다.

> **의존 관계 요약**: ⑥ ← 6단계(프로필 v2) / ③ ← ⑥ / ⑤ ← content_purpose 게이트(완료) /
> ②·③ ← `unim-capi` 표면 점검. ①은 다른 항목에 의존하지 않아 **단독 착수가 가능한 유일한 항목**이고,
> 사전 데이터가 이미 있어 이 목록에서 투입 대비 효과가 가장 좋다.
