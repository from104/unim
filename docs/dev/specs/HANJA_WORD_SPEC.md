# UNIM 한자 단어 입력 설계서 (HANJA_WORD_SPEC)

> 이 문서는 한자 팝업의 변환 **대상**을 "preedit 마지막 음절" 에서
> **방금 입력한 어절(대상①)** 과 **앱 선택 영역(대상②)** 으로 넓히는 기능의 규격이다.
> 팝업의 모양·키·즐겨찾기·DBus 시그니처는 [`POPUP_SPEC.md`](POPUP_SPEC.md) 를 그대로 따르며,
> 이 문서는 그 위에 "무엇을 대상으로 삼고, 확정 시 앱 텍스트를 어떻게 바꾸는가" 만 정의한다.
> `POPUP_SPEC.md` 본문은 수정하지 않았다 — 필요한 조항 개정은 §5 "POPUP_SPEC 개정안(파일 반영 승인 대기)" 에 문구 그대로 두었다.

- 상태: **PM 대행 결정 반영 — 기현님 최종 확인 대기** (구현 전, develop `ba64255` 기준 설계). §0 의 Q1~Q9 는 기현님 부재중 PM 대행이 결정했고 코드·문서는 그 결정대로 **바로 구현한다**(본문은 결정이 스며든 상태). 기현님 승인이 남은 것은 `POPUP_SPEC.md` **파일 자체의 수정**(§5 반영, PLAN U14b)뿐이며, 승인 후 반영하고 이 문서의 상태를 갱신한다.
- 관련 문서: `POPUP_SPEC.md`(팝업 규격), `unim-dbus/SPEC.md`(DBus), `ROADMAP.md` ① 항목, 사용자 매뉴얼 §4.2.

---

## 0. 결정 항목 (한눈에)

기현님 부재중 **PM 대행 결정**(2026-09-17) — **기현님 최종 확인 대기**. 구현자는 '결정' 열대로 진행하며 본문은 결정이 반영된 상태다. 기현님이 뒤집으면 §8 이력에 행을 추가하고 본문을 되돌린다.

| # | 항목 | 본문 기본값 | 대안 | **결정** (PM 대행 결정, 기현님 최종 확인 대기) |
|---|---|---|---|---|
| Q1 | §5 POPUP_SPEC 개정안(규칙 2·4·5·10·11, §9.2 예외) | 채택 | 문구 수정 요청 | **코드와 문서는 대상② 를 구현·서술한다** — 기현님 요청 원문에 "선택한 단어" 가 명시돼 있어 기능 자체는 결정된 것으로 본다. 승인 대기로 남는 것은 `POPUP_SPEC.md` **파일 수정(U14b)뿐**. 대상① 판 2벌 문구·`Resolve::None` 스텁 같은 승인 전 조건부 장치는 두지 않는다 |
| Q2 | 출력 형식의 괄호 문자 | 반각 `(` `)` — `대한민국(大韓民國)` | 전각 `（）` | **반각 `(` `)`** |
| Q3 | 대상② 확정 시 선택 앞뒤에서 제외한 공백을 되붙임(§2.4.3) | 되붙임(문서 보존) | 공백 소실 수용 | **되붙임** |
| Q4 | GTK4·Qt 의 "빈 소비 키" 선택 삭제 동작 변경(§4.5) — 선택한 채 한자키·한/영 전환키를 눌러도 선택이 남는다 | 채택(D8 충족 필수) | — | **채택** |
| Q5 | 다음절 후보의 글자별 뜻 합성 표시(§2.5.3) | 포함 | 제외(뜻 빈 채 출시) | **포함 — U2b 필수** |
| Q6 | 대상② 불일치 시 접근성 비프(§2.9) | 포함(옵션 ON 일 때만) | 제외 | **해당 없음** — Q8(a) 채택으로 "선택은 했는데 아무 팝업도 안 뜨는" 무음 실패 경로가 사라져 비프를 낼 상황이 없다. `pending_ui_feedback`·`UiFeedback`·`announce_hanja_nomatch` 는 만들지 않는다 |
| Q7 | 단어 일치 시 음절만 바꾸는 길이 없음. 사전 다음절 27.5만 항목이라 2음절 이상 한자어는 거의 일치 → "한국"+F9 에서 國 단독 후보 경로가 사실상 사라진다(Esc 뒤 재차 F9 는 idle→이모지) | 수용(v1 은 단어 후보만) | (a) 팝업 중 한자키 재타 = 접미 1자 축소(v2 축소 키를 v1 로 당김, 추가 키 0) / (b) 단어 후보 뒤에 음절 후보 이어붙이기(`hanja_candidates` 타입 변경 파급, 별도 설계) | **(a) 채택** — 팝업 중 한자키 재타 = target 접미 1자 축소(§2.5.5). 대상① 전용(대상② 는 선택 전체 치환이라 축소 없음), 길이 1 이면 현행 동작 유지 |
| Q8 | 대상② 불일치 시 동작(D6) — 초안은 "팝업 없이 키만 소비". select-on-focus 위젯(GtkEntry 기본)·사전 일치 한글이 자동 선택된 검색창에서는 사용자가 선택한 적 없어도 idle 한자키가 침묵하고 이모지 경로가 영구 차단된다(Esc → 래퍼 게이트로 선택 유지 → 재차 F9 → 같은 결과). 반면 툴킷 관례상 어떤 커밋도 선택을 치환하며(타이핑과 동일, Ctrl+Z 복구 가능) | D6(무동작) | (a) 불일치 시 종전 idle 동작(이모지 팝업)으로 폴백 — 사전 일치 한글 선택만 한자 팝업 / (b) 최소 타협: 한자 팝업 안에서 한자키 재타 → 이모지 팝업 전환 | **(a) 채택** — 사전 정확 일치 한글 선택만 한자 팝업, 그 외는 종전 idle 동작(이모지 팝업). D6 무동작·`NoMatchSelection`·불일치 비프 폐기 |
| Q9 | XIM·surrounding 미지원 Wayland 앱에서 마우스 커서 이동 뒤 대상① 확정이 무관 텍스트를 비가역 삭제할 수 있다(§2.2.1 주석 — 리셋도 검증도 없음). "헤더 확인 후 Esc" 는 상시 비용 | (a) XIM: **IM 이 유발하지 않은 스팟 갱신**(idle 이고, 직전 commit/preedit_draw 뒤 IM 이 기대하지 않은 `XNSpotLocation` 보고 — 보조로 y 변화·x 감소는 무조건) 시 `DbusRequest::Reset` 1회 송신, idle 구간당 1회 디바운스(`handler.rs:918` 스팟 보고 지점). **주의**: Reset 은 `reset_engine_and_capture_commit`(chord 강제 flush + 한자/preedit 캡처 + `InputEngine::new` = 사전 ≈6.45MB 재파싱, `engine_worker.rs:733-757`)이라 값싸지 않다 — 오탐 비용 = 재파싱 1회 + 단음절 퇴화(안전 방향), 디바운스가 상한. 재파싱이 실측에서 문제되면 엔진 `recent_clear` 만 하는 경량 RPC `ClearRecentSyllables` 로 교체(v1.1 후보) | (b) Wayland: `SurroundingText` 이벤트가 한 번도 오지 않은 컨텍스트에서는 `committed>0` 대상① 을 끄고 preedit 내부 일치만 허용(D1 "Linux 전 프런트 대상①" 을 surrounding 미지원 앱에 한해 축소) / (c) 둘 다 미적용 | **(a) 포함**(계획 그대로, U5) **+ (b) 채택** — 신규 RPC 없이 엔진 `surrounding_seen: bool` 로 "surrounding 을 준 적 있는 컨텍스트는 반드시 검증(빈 텍스트 = 실패)", Wayland 프런트는 미수신 컨텍스트에 `SetSurroundingText("",0,0)` 마커를 보내 `committed>0` 대상① 을 퇴화시킨다(§2.2.3·§4.2). GTK3/4·Qt·GNOME·TSF 는 이미 surrounding 을 보내므로 자동 적용(TSF 는 조합 중에도 읽도록 §4.4 보강) |

---

## 1. 목적·용어

### 1.1 목적

현재 한자키는 `hanja_target` 이 preedit 마지막 한 음절이라 `대한민국` 을 `大韓民國` 으로 한 번에 바꿀 수 없다(`ROADMAP.md` ①). 사전(`src/data/hanja.txt`)에는 다음절 항목이 27만 5천 개 이미 있고 `HanjaDictionary` 는 문자열 키 `HashMap` 이라 `대한민국` 조회가 그대로 동작한다. 이 문서는 **새 키·새 모드 없이** 기존 2키 시퀀스(한자키 → 숫자)로 단어를 바꾸는 규격을 정한다.

### 1.2 용어

| 용어 | 정의 |
|---|---|
| **대상①** | 사용자가 방금 입력한 어절. 음절 확정 모드에서 "대한민" 이 이미 앱에 확정되고 "국" 이 조합 중일 때 "대한민국". 단어 확정 모드에서는 preedit 누적분 안의 어절 |
| **대상②** | 앱에서 선택(selection)된 한글 단어 |
| **최근 확정 음절 버퍼** (`recent_syllables`) | 엔진이 이 컨텍스트의 앱에 **커밋한** 텍스트 중 끝에서 이어지는 한글 완성 음절(U+AC00–D7A3)열, 최대 17자. preedit 은 포함하지 않는다 |
| **target / 사전 키** (`hanja_target`) | 사전 조회 문자열 = 팝업 헤더 표시 = 즐겨찾기 키. 대상①에서 "대한민국" |
| **확정 접두 길이** (`hanja_committed_chars`) | target 중 이미 앱에 확정돼 있어 확정 시 지워야 하는 글자 수. "대한민국" 에서 3 |
| **재커밋 텍스트** (`hanja_recommit`) | 취소·포커스 이탈 시 앱에 되돌려 줄 텍스트 = 팝업 진입 당시 preedit 전체. 대상②는 빈 문자열 |
| **커밋 접두/접미** (`hanja_commit_prefix/suffix`) | 확정 문자열 앞뒤에 그대로 붙일 문자열. 단어 모드의 사전 비일치 접두("오늘"), 대상②의 선택 앞뒤 공백 |
| **교체 채널** | 이미 앱에 나간 글자를 지우고 새 텍스트를 넣는 경로. AutoTypeFix 의 `AutoTypefixApply(delete_chars, commit_text, preedit_text)` 시그널을 그대로 재사용한다 |
| **출력 형식** (`hanja_output_format`) | 확정 문자열 서식: `漢字` / `한자(漢字)` / `漢字(한자)` |

### 1.3 v1 범위

| 항목 | v1 | 근거 |
|---|---|---|
| 대상① | Linux 6 프런트엔드(GTK3/4·Qt5/6·XIM·Wayland·GNOME) 전부 + Windows TSF. 단 surrounding 을 준 적 있는 컨텍스트에서 접두 검증이 실패하면(빈 surrounding 포함 — Wayland 미수신 마커, [Q9](b)) `committed>0` 대상① 은 단음절로 퇴화한다(§2.2.3) | 교체 채널을 6종이 이미 구독하고 있다. TSF 는 코어를 직접 링크해 자동 상속 |
| 대상② | GTK4·Qt5/6·GNOME 확장·Wayland(배선 신설)·TSF. **GTK3·XIM 미지원**(종전 idle 동작 유지) | GTK3 는 anchor 를 전달할 vtable 이 없고(`gtk3/src/immodule.c:1220-1222` anchor=cursor), XIM 은 surrounding 개념이 없다 |
| 출력 형식 | 단음절 변환에도 동일 적용 | 서식 조립이 `select_hanja` 한 곳이라 키보드·마우스·pull 경로 자동 동기 |
| Windows | TSF 구현 + cross-compile(`make check-windows`) 검증만. IMM32 **코드 무변경** — 코어 호스트 능력 플래그(`hanja_word_replace_capable`, 기본 **false**, §2.2.2)가 대상① 교체 경로를 끄므로 IMM32·`unim-capi` 는 종전 단음절 동작·바이트 동일 | 이 환경에서 런타임 불가. IMM32 는 `press_key` 뒤 commit/preedit 만 드레인하고 팝업 액션·교체 페이로드를 드레인하지 않는다(`unim-imm32/src/input.rs:110-112,:181`, `lib.rs:224`) — 플래그 없이는 RecentWord 확정이 "국" 을 소실시킨다 |
| 커밋 | 하지 않음(승인 후) | — |

---

## 2. 동작 명세

### 2.1 한자키 분기

한자키(`hanja_keys`, 기본 `Hanja`·`F9`)는 종전대로 `press_key()` 언어 분기 직전에서 처리한다(POPUP_SPEC §9.2 v3.2). 분기만 넓어진다.

| 상태 | 앱 선택 영역 | 동작 |
|---|---|---|
| 한자 팝업 열림 | 무관 | 대상①(`RecentWord`/`WordBuffer`)이고 target 이 2자 이상이면 **접미 1자 축소**(§2.5.5, [Q7](a)). 그 외(길이 1·대상②)는 현행(push: 팝업 미지원 키 재처리 → 취소 후 idle 이모지 / pull: 같은 팝업 재발행) |
| 조합 중(preedit 있음) | 무시 | 대상① 시도(§2.2, §2.3). 다음절 일치가 없으면 종전(마지막 음절) — 바이트 동일 |
| idle | 있음 · 사전 정확 일치 한글 | 대상② 팝업(§2.4) |
| idle | 있음 · 불일치/비한글/19자 이상/공백만 | **이모지 팝업** — 선택이 없는 것과 동일하게 종전 idle 동작으로 폴백([Q8](a)). 선택 텍스트는 건드리지 않는다(§4.5) |
| idle | 없음 | 이모지 팝업(종전 v3.2) |

pull 경로(Qt·XIM 의 `GetHanjaCandidates`)는 `start_hanja_conversion` 을 직접 부르므로 같은 판정을 거친다. 후보가 없으면(idle 불일치 포함) 두 프런트는 특수문자 → `ProcessKey` 폴백으로 위 표에 도달하므로 push/pull 결론이 일치한다.

> 불일치 선택에서 이모지로 폴백하는 이유([Q8](a)): 어떤 커밋도 선택 영역을 치환하는 것이 툴킷 관례(타이핑과 동일, Ctrl+Z 복구)이고, 초안의 D6 무동작을 유지하면 select-on-focus 위젯(GtkEntry 기본)·사전 일치 한글이 자동 선택된 검색창에서 사용자가 선택한 적 없어도 idle 한자키가 침묵하고 이모지 경로가 영구 차단된다(Esc → 래퍼 게이트로 선택 유지 → 재차 F9 → 같은 결과). 따라서 **사전에 정확히 있는 한글 선택만** 한자 팝업이고 나머지는 전부 종전 idle 동작이다. 이모지를 확정하면 위젯이 선택을 그 이모지로 치환한다 — 매뉴얼에 한 줄 안내(PLAN §7.2). 이모지 팝업이 열리기만 하고 확정하지 않으면 선택은 남는다(§4.5 래퍼 게이트).

### 2.2 대상① — 음절 확정 모드

#### 2.2.1 최근 확정 음절 버퍼

- 엔진(`InputEngine`) 소유. 한글 완성 음절만, 상한 17자(사전 최장 키 18자 − preedit 1자). 초과 시 앞에서 버린다.
- **채움**: 이번 키 처리로 앱에 커밋된 문자열 가운데 완성 음절을 뒤에 붙인다. chord(모아치기) idle 만료로 흘러나온 음절도 포함한다. **한자키 자체가 확정한 음절도 포함한다** — chord 모드에서 한자키 분기의 `finalize_chord_buffer()` 가 직전 음절("민")을 commit_buffer 로 내리므로, `start_hanja_conversion()` 전에 그 델타를 버퍼에 흡수해야 target 풀이 "대한민"+"국" 이 된다(안 하면 풀 "대한"+"국" → "한국" 일치·접두 1 → XIM 에서 '민' 오삭제).
- **리셋 조건**(하나라도 해당하면 전부 비운다):

| 조건 | 예 |
|---|---|
| 비한글 커밋 | 공백·구두점·숫자·영문·특수문자·이모지·한자 확정·분해 자모 |
| 앱으로 통과한 키 | Enter·Tab·Escape·방향키·Home/End/PageUp/PageDown·Delete·Insert·F키·Ctrl/Alt/Super 조합 단축키 (수정자 단독 키는 제외) |
| Backspace 통과(조합 없음) | 앱이 커서 앞 1자를 지운 것으로 보고 버퍼 끝 1자 **pop**. 선택 영역이 있으면 전부 비움 |
| 한/영 전환 | 토글키·auto-english·AutoTypeFix 자동 전환·`SetGlobalMode` |
| 팝업 확정/취소/미지원 키 재처리 | 한자·특수문자·이모지 팝업 공통 |
| `Reset`/`FocusOut`/`engine.reset()` | 마우스 클릭(GTK/Qt/GNOME 은 Reset 을 보낸다), 포커스 이동, AutoTypeFix 교정 후 reset |
| 비밀번호/PIN 필드 진입 | fail-closed: 진입 시 비우고, 차단 중에는 채우지 않는다 |
| 설정 리로드로 한글 컨텍스트 재구성 | 자판이 바뀌면 문맥 무의미 |

- Wayland·XIM 은 마우스 클릭 시 Reset 을 보내지 않는다(Wayland `state.rs` 에는 `DbusRequest::Reset` 송신이 없고, XIM 은 `XResetIC`·ATF 완료 시뿐). 이 경우의 드리프트는 §2.2.3 정합성 검증(Wayland, surrounding 을 주는 앱)과 XIM 의 **스팟 점프 Reset**([Q9](a))으로 줄이고, 남는 경우는 팝업 헤더 확인 후 Esc — 취소는 접두를 건드리지 않으므로 문서 손상은 없다. surrounding 을 주지 않는 Wayland 앱의 잔여 위험은 [Q9](b).
  - 스팟 점프 판정 기준: XIM 핸들러는 commit/preedit_draw 직후 `expect_spot_update = true` 를 세우고 스팟 보고(`handler.rs:918`)에서 소비한다. **기대 없이 도착한 스팟 갱신 = 사용자 커서 이동**으로 보고, 로컬 preedit 이 없으면(idle) `Reset` 1회. 보조로 직전 커밋 스팟 대비 y 변화·x 감소는 무조건 Reset. idle 구간당 1회 디바운스(`reset_sent` 플래그, 다음 commit/preedit_draw 에서 해제). 앱은 커밋마다 스팟을 전진 보고하므로(폰트 폭은 IM 이 모름) "같은 줄 짧은 후퇴 클릭" 을 거리 임계로는 잡을 수 없다 — 그래서 기준을 거리가 아니라 "IM 이 유발했는가" 로 둔다. 오탐(스크롤·expose 보고)은 Reset 비용(엔진 재생성 = 사전 재파싱, chord 강제 flush) + 단음절 퇴화라 안전 방향. XIM 은 surrounding 이 없어 §2.2.3 검증도 건너뛰므로 미탐(false negative)이 곧 오삭제다 — 기준의 보수성이 우선.
- 채움 훅(`recent_push_char`)은 비밀번호 차단 중이면 push 대신 비운다 — chord idle 만료 타이머 경로는 키 래퍼를 거치지 않으므로 훅 자체가 fail-closed 여야 한다(§2.8).

#### 2.2.2 target 결정

1. preedit 의 마지막 글자가 미완성 자모(초성 등)이면 종전 규칙(마지막 글자, 초성이면 특수문자 전환).
2. `pool = 버퍼 + preedit`. `pool` 의 접미 가운데 길이 `min(18, len)` 부터 **2** 까지 내림차순으로 사전에 있는 첫 문자열을 target 으로 삼는다(최장 접미).
   - 접미가 preedit 보다 길면(버퍼 글자를 포함) → 대상①. `확정 접두 길이 = 접미 길이 − preedit 길이`. §2.2.3 검증을 통과해야 한다.
   - 접미가 preedit 안에서 끝나면(모아치기 등 preedit 이 2자 이상인 드문 경우) → 확정 접두 0, 접미 앞의 preedit 은 커밋 접두로 보존.
3. 일치가 없으면 종전(마지막 음절, 확정 접두 0) — 종전과 바이트 동일.
4. **호스트 능력 플래그** `hanja_word_replace_capable: bool`(엔진 필드, 기본 **false**, setter `set_hanja_word_replace_capable`): 교체 채널(§4.1 페이로드)을 드레인하는 호스트만 true 로 켠다 — Linux 데몬(엔진 생성 헬퍼)·TSF(`text_service.rs:187/:463` 생성 직후). false 면 2 의 탐색에서 버퍼 글자를 포함하는 접미(접미 길이 > preedit 길이)를 건너뛰어 대상① 이 preedit 내부 일치로 제한된다. IMM32(`unim-imm32`)·`unim-capi` 소비자는 페이로드를 드레인하지 않으므로 기본값 그대로 종전 단음절·바이트 동일(§1.3). 기본을 false 로 두는 이유: 드레인하지 않는 호스트에서 켜지면 확정 시 조합 음절이 소실되고 페이로드가 영구 미드레인으로 남는다(fail-closed). `unim-capi` 는 setter 1개를 export 한다.

예: 버퍼 "대한민" + preedit "국" → "대한민국"(확정 접두 3). 버퍼 "뷁" + "국" → "국". 플래그 false + 버퍼 "대한민" + "국" → "국".

#### 2.2.3 정합성 검증(안전망)

버퍼는 키 입력만 보므로 마우스 편집·앱 자동교정·메뉴 Undo 뒤에는 앱 문서와 어긋날 수 있다. 잘못 지우는 것이 최악이므로, **앱이 준 surrounding text 가 있으면** 지울 접두를 대조한다.

- `before` = surrounding 의 커서 앞 텍스트, `prefix` = 지울 접두("대한민"), `pre` = preedit("국").
- 통과 조건: `before` 가 `prefix` 로 끝남(GTK/Qt/Wayland/GNOME: surrounding 은 preedit 을 포함하지 않는다) **또는** `before` 가 **비어 있지 않고** `prefix` 보다 짧으며 그 접미임(앱이 잘라 보고한 경우). **TSF 에서만** 추가로 `prefix+pre` 로 끝남 / 그 잘림 접미(조합 텍스트가 문서 안에 있다) — 엔진 플래그 `surrounding_includes_preedit: bool`(기본 false, TSF 가 `set_surrounding_text` 호출 옆에서 true) 이 true 일 때만 이 두 절을 평가한다. 프런트 구분 없이 `prefix+pre` 절을 허용하면 Wayland/GNOME 클릭 드리프트(버퍼 "대한민" 잔류 → 문서의 기존 "…대한민국" 뒤 클릭 → "국" 입력)에서 `before`="…대한민국" 이 `full` 로 끝나 검증이 뚫리고 확정 시 '한민국' 이 지워진다(Wayland 는 클릭 시 Reset 을 보내지 않으므로 검증이 유일한 안전망).
- surrounding 이 비어 있으면(XIM·미지원 앱) 검증 없이 통과. surrounding 은 있는데 `before` 가 비어 있으면(커서가 0·줄 첫머리, 줄 단위로 보고하는 위젯) **실패** — 빈 문자열은 모든 문자열의 접미이므로 잘림 절을 공허하게 통과시키면 안 된다(줄 첫머리에서 앞 줄 끝·개행을 지우는 사고).
- 커서 오프셋이 문자 길이를 **넘으면 검증 실패**(fail-closed). 정상 문자 오프셋은 절대 길이를 넘지 않으므로 초과는 단위 불일치(GNOME 확장은 Mutter 오프셋을 단위 변환 없이 전달, Qt focus-in 은 UTF-16 단위)의 증거다 — 클램프로 삼키면 "그럴듯한 오답" 이 된다(§2.4.1). 오프셋은 검증 없는 IPC 입력이라 패닉 방지는 유지한다(슬라이스 전 범위 검사).
- 실패 시 종전(마지막 음절)으로 **조용히 퇴화** — 안전한 방향으로만 실패한다.

### 2.3 대상① — 단어 확정 모드(`commit_unit=Word`, word-mode 앱)

- 앱에 나간 글자가 없으므로 확정 접두 0. `preedit` 전체의 최장 사전 접미를 target 으로, 접미 앞의 나머지("오늘"+"대한민국" 의 "오늘")는 커밋 접두로 보존해 확정 시 **접두+한자를 한 번에 커밋**한다(부분 커밋 API 신설 없음).
- 일치가 없을 때의 폴백(마지막 음절)에서도 접두를 보존한다. **종전 결함 수정**: 현행은 preedit 이 2자 이상일 때(단어 모드의 누적 preedit, 음절 모드의 모아치기 preview 등) 마지막 음절만 바꾸거나 취소하면 마지막 글자 앞 preedit 이 사라진다(`candidates.rs:150-156` 전체 clear + `popup_dispatch.rs:228` target 만 복원). 개정 후는 확정 시 접두+한자, 취소 시 preedit 전체 재커밋 — 따라서 "종전과 바이트 동일" 불변식은 **preedit 1자**일 때만 성립한다(§2.6).

### 2.4 대상② — 선택 영역

#### 2.4.1 판정

`SetSurroundingText(text, cursor, anchor)` 로 받은 값에서 **`max(cursor, anchor)` 가 문자 길이를 넘으면 선택 없음으로 거부**(로그 1회, 패닉 없음)하고, 그 외 `min < max` 이면 선택이 있다. 오프셋이 텍스트 길이를 넘는 입력은 실재한다 — GNOME 확장은 Mutter 오프셋을 단위 변환 없이 전달하고 Qt focus-in 은 UTF-16 단위를 보낸다. 범위 검사 없이 슬라이스하면 `a > b` 패닉으로 단일 워커 스레드가 죽어 모든 컨텍스트가 멈추지만, **클램프로 삼켜서도 안 된다**: 바이트 오프셋 "대한민국" 의 "대한" 선택 = (0,6) 을 (0,4) 로 클램프하면 target "대한민국" 팝업 → 확정 "大韓民國" 이 실제 선택 "대한" 만 치환 → "大韓民國민국". 초과 자체가 단위 불일치의 증거이므로 거부가 맞다(GNOME 실측에서 바이트로 확인되면 확장 쪽에서 문자 단위로 변환). `typefix_convert` 의 슬라이스(`surrounding.rs:197-199`, 현행 `end.min(len)` 클램프)도 같은 판정으로 통일한다. 선택 문자열의 앞뒤 공백을 제외한 부분이 **(a) 비어 있지 않고 (b) 전부 완성 음절이며 (c) 18자 이하이고 (d) 사전에 정확히 있을 때**만 그 문자열을 target 으로 삼는다. 하나라도 어긋나면 팝업 없이 키를 소비한다(§2.1).

#### 2.4.2 확정

확정 접두 0 인 일반 커밋(`CommitText`/`commit_buffer`). 위젯이 선택 영역을 커밋 문자열로 치환한다(GtkText·QLineEdit 은 툴킷 소스로 확인, 그 밖의 위젯은 실측 항목 §6.5).

#### 2.4.3 공백 보존 [Q3]

위젯은 **선택 전체**(공백 포함)를 치환하므로, 판정에서 제외한 앞뒤 공백을 커밋 문자열에 되붙인다. `" 대한민국 "` 선택 → `" 大韓民國 "`.

#### 2.4.4 조합 중이면 선택 무시

preedit 이 있으면 대상① 이 우선한다(모호성 제거).

### 2.5 후보·팝업·즐겨찾기

#### 2.5.1 후보와 정렬 — 종전 유지
`hanja_dict.search(target)` 순서(사전 등장순) → 즐겨찾기 stable sort 우선 → 9개/페이지(`HANJA_PAGE_SIZE`). 동음 다중 항목(國家/國歌)은 각각 별개 후보.

#### 2.5.2 즐겨찾기 — 종전 유지
키는 `(target, 한자)` 문자열 쌍. 단어도 그대로("대한민국", "大韓民國"). 서식과 무관하게 한자 원문으로 저장하므로 출력 형식을 바꿔도 즐겨찾기는 유지된다. 단어 즐겨찾기는 정확히 같은 단어 팝업에서만 재사용된다(매뉴얼에 명시).

#### 2.5.3 다음절 후보의 뜻 표시 (선택) [Q5]
다음절 항목의 뜻은 거의 비어 있거나 표제어를 되풀이한다(`hanja.txt:57963` `대한민국:大韓民國:대한민국`, `:28636` `국가:國家:`). 9개 목록에서 國家/國歌 를 구분할 단서가 없으므로, 단음절 항목의 뜻(`국:國:나라 국`)으로 만든 역색인으로 글자별 뜻을 `·` 로 이어 표시한다(예: `나라 국 · 집 가`). 원본 뜻이 있고 표제어와 다르면 원본을 쓴다. 역색인 키 `(한자, 단어 속 음)` 이 두음법칙·다독음(예: `역사:歷史` 의 歷 은 단음절 표제어가 "력")에서 빠지면 **한자 단독 키(첫 등장 뜻)로 폴백**해 빈칸을 남기지 않는다. 사전(`dict.rs`)만 바뀌고 렌더러는 무수정.

#### 2.5.4 헤더 표시
헤더 `「대한민국」 → 한자` 는 문자열 연결이라 무수정. 8~18자 target(2,124건)에서 팝업 폭을 넘을 수 있어 세 렌더러(popup-service·GNOME·Windows)에 헤더 ellipsize 를 넣는다. Windows compact 한자 열은 고정 90px 이라 다글자 후보가 뜻 열과 겹치므로 실측 폭으로 바꾼다(compact 리스트는 설계서상 이미 가변 폭). **expanded 9×9 격자의 셀 폭은 고정 유지**(`popup-renderer-design.md:422` "popup 폭 고정 정책" — 페이지마다 폭이 요동하면 안 된다) + 셀 텍스트 `DT_END_ELLIPSIS`(전체 한자는 헤더가 담당, 설계서 :425 와 정합). 설계서 §3 은 동결 문서라 compact 열 폭 문구를 함께 갱신한다.

### 2.6 확정

| 대상 | 커밋 문자열 | 채널 |
|---|---|---|
| 대상①(음절 모드, 확정 접두 N>0) | `서식(target, 한자)` | **교체**: `AutoTypefixApply(delete_chars=N, commit_text, preedit_text="")` — 응답 preedit "" 로 조합 음절을 먼저 지우고, 시그널로 접두 N자 삭제 → 커밋 |
| 대상①(단어 모드) | `커밋 접두 + 서식(target, 한자)` | 일반 커밋 |
| 대상② | `앞 공백 + 서식(target, 한자) + 뒤 공백` | 일반 커밋(위젯 치환) |
| 단음절(종전) | `커밋 접두 + 서식(target, 한자)` | 일반 커밋 — 서식이 `漢字` 이고 **preedit 이 1자**이면 종전과 바이트 동일. preedit 2자 이상은 §2.3 결함 수정으로 접두가 보존된다(의도된 변경) |

- 서식 조립 규칙: 한글 부분은 `target`(사전 키). 뜻은 넣지 않는다. 커밋 접두/접미는 서식 대상이 아니다.
- 확정 후 버퍼와 한자 상태는 모두 비운다.

### 2.7 취소·포커스 이탈

| 경로 | 동작 |
|---|---|
| Escape / 팝업 미지원 키 재처리 | `hanja_recommit` 을 커밋(대상①: "국" 또는 단어 모드 preedit 전체, 대상②: 없음) → 팝업 닫기 → 미지원 키는 이어서 재처리 |
| `CancelHanja` RPC(팝업 밖 클릭·Wayland deactivate 등) | 같은 텍스트를 데몬이 `CommitText` 로 발행 |
| `FocusOut`/`Reset` RPC | 엔진 재생성 전에 같은 텍스트를 캡처해 커밋 |
| TSF 포커스 이탈(`OnSetFocus`) | 조합 텍스트는 앱이 `OnCompositionTerminated` 로 문서에 남기므로 재커밋하지 않는다(중복 삽입 방지). 오버레이 조합 앱(`composition_unsupported`)에서는 옛 컨텍스트에 삽입을 시도하고 실패하면 로그. 한자 상태 정리는 `engine.reset()` 분기와 "전이 포커스 reset 스킵(조합 보존)" 분기 **양쪽 공통**으로 실행한다(`text_service.rs:1904-1930` 스킵 분기도 `popup_ipc.hide()` 는 실행하므로 엔진만 hanja_mode 로 남으면 다음 키가 팝업 dispatch 로 샌다). 스킵 분기·정상 앱은 `cancel_hanja` + 조합 텍스트 keep 확정(materialize), 재커밋·삽입 시도 없음 |

원칙: **이미 확정돼 앱에 있는 접두와 선택 영역은 절대 건드리지 않는다.**

### 2.8 비밀번호 게이트(fail-closed)

- 비밀번호/PIN 필드(`content_purpose.should_block_hangul()`)에서는 **진입 시** 버퍼와 기존 surrounding(`surrounding_text/cursor/anchor`)을 비우고, 이후 채우지도 저장하지도 않는다. 종전 `set_surrounding_text`(`surrounding.rs:71-76`)는 **새 값**만 거부·클리어하고 차단 분기(`:38-47`)는 잔류를 지우지 않는다 — 목적 통지보다 먼저 도착한 선택 스냅샷(Qt mid-focus 에코모드 전환 `:319-336`)이 남아 목적 게이트가 없는 `typefix_convert`(`:183`)에 노출된다. 차단 분기 첫 줄 정리에 surrounding 3필드 클리어를 포함한다.
- pull 경로(`GetHanjaCandidates`)는 `press_key` 의 영문 강제를 거치지 않으므로 `start_hanja_conversion` 첫머리에서 차단한다(종전엔 없던 게이트).
- 프런트 이중 게이트(Qt): `m_contentPurpose` 가 Password/PIN 이면 `update()` 재질의 결과 대신 `("",0,0)` 만 전송한다 — 엔진이 버리더라도 비번 평문을 DBus 로 실어 보내지 않는다(TSF `key_handler.rs:417 atf_active` 식 프런트 게이트 관례).
- 로그에는 target **길이만** 남긴다: 종전 `candidates.rs:37-42` `"한자 후보 발견: '{}' -> {} 개"` 는 target 을 평문으로 찍는데, target 이 최대 18자 타이핑 텍스트로 넓어지고 `UNIM_DEVELOP=1` 이면 `~/.unim-log/…` 파일에 영속한다(`logging.rs:82-88,:115-118`). XIM·터미널은 목적 통지가 없어 비번 조각이 남을 수 있다 → `'{}'` 를 `{}자` 로 바꾼다. 신규 로그("선택 단어 한자 불일치", "후보 없음")는 텍스트 없음 유지.
- 팝업이 열린 채 비밀번호 목적이 도착하면 팝업을 닫고 **재커밋하지 않는다**(비번 필드에 원문 재삽입 금지). XIM 은 목적 통지 자체가 없어 구조적 잔여(종전 preedit 노출과 동급).
  - 순서: 한자 상태 정리(버퍼 clear + 팝업 취소로 `korean_context`·`preedit_cache` 비움)는 `set_content_purpose` 차단 분기의 **첫 줄**이어야 한다. 종전 코드는 같은 분기에서 `flush_preedit()` 을 먼저 부르는데(`surrounding.rs:38-47`), 한자 팝업은 preedit 을 유지한 채 열리므로 정리가 그 뒤에 오면 "국" 이 `commit_buffer` 로 흘러 다음 키 처리에서 비번 필드에 실린다.
  - 데몬 배선: `SetContentType` 요청은 응답 채널이 없고 워커에는 시그널 발행 수단이 없으므로, 요청에 `oneshot` 응답(팝업을 닫았는지 bool)을 추가하고 `service.rs` 가 응답을 받아 `HidePopup` 을 발행한다(빈 텍스트 `redirect_commit_and_hide("")` = HidePopup 만). 이것이 없으면 후보 창이 비번 필드 위에 다음 팝업 RPC 때까지 남는다.

### 2.9 실패 피드백 (선택) [Q6]

선택은 했는데 사전에 없으면 팝업이 안 뜬다. 접근성 옵션 `toggle_announce_beep` 이 켜져 있을 때만 짧은 비프를 내고, 데몬 로그 `ENGINE "선택 단어 한자 불일치"` 를 남긴다. 기본 무음이라 회귀 0.

### 2.10 프런트엔드별 지원 매트릭스

| 프런트엔드 | 대상① | 대상② | 비고 |
|---|---|---|---|
| GTK4 | ✔ | ✔ | 선택 삭제 래퍼 게이트(§4.5) |
| GTK3 | ✔ | ✘(idle=이모지) | anchor 미전달 |
| Qt5/6 | ✔ | ✔ | surrounding 을 `update(Qt::InputMethodQueries)` 시 갱신(커스텀 `hanja_keys` 포함, **빈 텍스트도 전송**, 비번 필드는 `("",0,0)` — §2.8) + 래퍼 게이트(§4.5). 한자키 시점 재질의만으로는 부족: `input_context.cpp:381` 은 F9·Hangul_Hanja 하드코딩 pull 경로라 다른 `hanja_keys` 는 `processKey`(:408) → 엔진 분기로 가는데, surrounding 은 focus-in 1회(:554-564, 비어 있으면 미전송)뿐이라 QLineEdit Tab 포커스의 전체 선택 스냅샷이 stale 로 남아 `has_selection()` 오판 → 커서에 엉뚱한 단어 삽입 또는 이모지·한자 모두 침묵 |
| XIM | △(알려진 결함) | ✘(idle=이모지) | ⚠️ **대상① 교체 실측 실패(2026-09 docker L3)** — 교체 채널(자가 주입 BackSpace N+1, `handler.rs` `PopupEvent::AutoTypeFix`)에서 앱이 BS 1개만 처리한 뒤 확정이 먼저 도착하고 남은 BS 가 확정 한자를 지운다(`대한민국`→`대한大韓`). AutoTypeFix 역방향과 같은 경로라 함께 조사 대상, `tests/harness/scenarios/hanja_word.json` `known_fail.xim` 에 표기. 교체 도착 시 preedit 표시 선클리어(마우스 확정 잔상 방지 — `handle_popup_event` 에는 `user_ic` 가 없으므로 `CommitText` 분기처럼 `last_focused_ic_info` 로 IC 를 재구성해 `server.preedit_draw(&mut ic, "")`, NOTHING/POSITION IC 는 PeWindow 숨김 경로. 구현 비용이 크면 선클리어를 포기하고 "마우스 확정 시 잔상 잔여(키보드 확정은 정상)" 로 비고 정정). 스팟 점프 Reset([Q9](a)) |
| Wayland | ✔ | ✔(배선 신설) | 바이트 판별(§4.2) + `SurroundingText` → `SetSurroundingText` |
| GNOME 확장 | ✔ | ✔(실측 항목) | 교체 전 preedit 선클리어(`UnimInputMethod.clearPreedit()`, `unim_input_method.js:728`) + `SPEC.md` §2.6·시그널 표·`vfunc_set_surrounding` 행 갱신. `onAutoTypeFix`(`extension.js:284`) 는 `_hasFocus` 게이트로 교체 프레임을 무음 드롭한다 — St 팝업 클릭 시 `_hasFocus` 유지 여부는 §6.5 실측 항목(GTK4 `is_focused` 와 같은 성격) |
| Windows TSF | ✔ | ✔ | cross-compile 검증만(§4.4). 엔진 생성 직후 `set_hanja_word_replace_capable(true)` |
| IMM32 | 대상① 미지원(단음절 유지) | ✘ | 코드 무변경. 교체 채널·팝업 액션 드레인이 없으므로 코어 플래그 기본 false 로 종전 바이트 동일(§2.2.2 4). `unim-capi` 소비자도 동일 |

---

## 3. 설정 `hanja_output_format`

### 3.1 필드·값·기본

- 위치: `engine.korean.hanja_output_format` (`KoreanConfig`, `commit_unit` 옆). 근거: 단축키류(`hanja_keys`)는 `EngineConfig` 최상위, 조합·변환 동작류(`commit_unit`, `word_mode_apps`)는 `KoreanConfig` — 출력 서식은 후자.
- 값(serde 태그 = variant 이름, `CommitUnit` 관례): `Hanja`(기본, `漢字`, 종전 동작) / `HangulHanja`(`한자(漢字)`) / `HanjaHangul`(`漢字(한자)`). CLI 값: `hanja` / `hangul-hanja` / `hanja-hangul`.
- 구 config.yaml 은 필드 없음 → 기본 `Hanja`. `KoreanConfig` 는 `KoreanConfigCompat` 을 거쳐 역직렬화되므로(`config.rs:605`) **Compat 에도 필드를 넣어야** YAML 값이 살아남는다.
- 핫리로드: 비파괴 setter(조합 끊김 없음). 리로드 fingerprint 에는 넣지 않는다(AutoTypeFix 핫키 선례).

### 3.2 동기화 지점

| # | 지점 | 파일 |
|---|---|---|
| 1 | 코어 enum·필드·Default·**Compat 브리지** | `src/config.rs` |
| 2 | 엔진 캐시 + 비파괴 setter | `src/input_engine/engine.rs` |
| 3 | 데몬 리로드 루프 setter 호출 | `unim-dbus/src/engine_worker.rs` |
| 4 | DBus YAML/JSON | serde 자동. 레거시 `get_config/set_config` 는 `commit_unit` 선례대로 생략 — 따라서 L3 하네스가 `commit_unit`/서식을 바꾸려면 `harness.py` 의 레거시 `set_config`(`SetConfig` 키 API)가 아니라 `GetConfigYaml → 키 패치 → SetConfigYaml` 헬퍼(§6.3)를 써야 한다 |
| 5 | CLI `config set/show` + 로케일 8키 + **`unim-cli/SPEC.md` §2.3 `config set` 키 표** | `unim-cli/src/main.rs`, `unim-cli/locales/{ko,en}.yml`, `unim-cli/SPEC.md`(`:76-92` 표에 `hanja-output-format \| hanja, hangul-hanja, hanja-hangul` 행 — 표는 이미 `commit-unit`·`word-mode-apps` 도 빠진 드리프트 상태라 `commit-unit \| syllable, word, smart` 백필은 선택) |
| 6 | GTK 설정 ComboRow + 로케일 + 병합 화이트리스트 | `unim-settings-gtk/…`, `unim-gui-common/src/settings_helpers.rs` |
| 7 | Slint 설정 ComboBox + 바인딩 + 병합 + **en `.po`** | `unim-settings/ui/settings.slint`, `unim-settings/src/main.rs`, `unim-settings/translations/en/LC_MESSAGES/unim-settings.po`(`build.rs:13` 번들 — `@tr` 문자열은 .po 에 없으면 영어 로케일에서 한국어 노출) |
| 8 | Windows 레거시 Win32 모달 콤보 | `unim-tsf/src/settings_dialog.rs`(`fn_configure.rs:45`·`lang_bar.rs:799` 가 호출하는 활성 UI). `ID_CMB_COMMIT_UNIT` 선례는 const(:68)·create(:693)·읽기(:1248) **3지점** + 저장 역변환 |
| 9 | 사용자 매뉴얼 §4.2·§5.1(GUI 투어 row)·§7.2(CLI 예시)·이모지 문장(:690) + **조합 확정 단위 절의 CLI 블록·YAML 샘플(ko `:590-593`/`:605-609`, en `:588-591`/`:603-606` — ko/en 1:1)** + Windows 사용안내 idle 표 + FAQ Q28 확인 + **루트 `README.md`**(한자키 3분기 표 `:207-213`·"한자키 한 번 + 숫자 한 번" `:217-219`·영문 요약 `:33` — `make help-html` 입력이 아니라 자동 반영되지 않는다, `tools/gen-help/src/main.rs:4-8`) | `docs/user/user-guide/README{-ko,}.md`, `docs/user/UNIM-Windows-사용안내.md`, `docs/user/faq/README{-ko,}.md`, `README.md` |
| 10 | 오프라인 도움말 HTML 재생성(`make help-html`, CI 게이트 `make check-help-html` — 빌드 의존성 아님) | `help/unim-help-{ko,en}.html`, `help/windows/unim-help-{ko,en}.html`. Linux 프런트 표는 `<!-- @platform:linux -->` 마커로 감싸 Windows 판에 새지 않게 |
| 11 | 코어 엔진 `src/SPEC.md` — §2.1 `InputEngine` 필드(`:111-114` `hanja_target` 주석을 "사전 키(어절 가능)" 로 + 신규 필드 8개), §2.3 `press_key` 파이프라인(`:135-176` — `press_key` 래퍼 `recent_track_after_key` 와 Hanja 분기 3갈래 대상①/②/이모지), §3.2 열거형(`:305` `HanjaOutputFormat`) | `src/SPEC.md`(CONTRIBUTING.md:66-68 즉시 반영 규칙) |
| — | GNOME gschema·`unim-capi`(setter 1개 export 만, §2.2.2 4) | 무변경 |

### 3.3 UI 문구 (ko / en)

| 키 | ko | en |
|---|---|---|
| `row_hanja_output_format` | 한자 출력 형식 | Hanja Output Format |
| `row_hanja_output_format_subtitle` | 漢字 · 한자(漢字) · 漢字(한자) — 한자 팝업에서 확정할 때 넣는 형식 | 漢字 · 한자(漢字) · 漢字(한자) — text inserted when you confirm a hanja candidate |
| `row_hanja_output_format_tooltip` | 「한자만」은 漢字만 넣습니다. 「한글(한자)」는 대한민국(大韓民國)처럼 한글 뒤에 괄호로 한자를, 「한자(한글)」는 大韓民國(대한민국)처럼 한자 뒤에 괄호로 한글을 붙입니다. 한 글자 변환과 단어 변환 모두 같은 형식을 씁니다. | "Hanja only" inserts 漢字 alone. "Hangul(Hanja)" inserts 대한민국(大韓民國); "Hanja(Hangul)" inserts 大韓民國(대한민국). Applies to both single-syllable and word conversion. |
| `hanja_output_format_hanja` | 한자만 (漢字) | Hanja only (漢字) |
| `hanja_output_format_hangul_hanja` | 한글(한자) — 한자(漢字) | Hangul(Hanja) — 한자(漢字) |
| `hanja_output_format_hanja_hangul` | 한자(한글) — 漢字(한자) | Hanja(Hangul) — 漢字(한자) |
| `row_hanja_keys_subtitle` (개정) | 쉼표로 구분 (예: Hanja, F9) — 조합 중이면 어절/음절 한자 변환, 한글 단어를 선택한 뒤 누르면 그 단어 변환, 그 외에는 이모지 팝업 | Comma separated (e.g. Hanja, F9) — converts the word/syllable being typed, converts a selected Hangul word, otherwise opens the emoji popup |
| `row_hanja_keys_tooltip` (개정) | 한자/특수문자 팝업을 띄울 키. 예: Hanja, F9. 한글 조합 중: 방금 친 어절(예: 대한민국)이 한자 사전에 있으면 단어 후보, 없으면 마지막 음절 후보. 앱에서 한글 단어를 선택한 채 누르면 그 단어의 한자 후보(GTK4·Qt·GNOME·Wayland·Windows). 조합·선택이 모두 없으면 이모지 팝업. | Keys that open the Hanja/special-char popup, e.g. Hanja, F9. While composing: word candidates for the word you just typed (e.g. 대한민국) if it is in the dictionary, otherwise last-syllable candidates. With a Hangul word selected in the app: candidates for that word (GTK4, Qt, GNOME, Wayland, Windows). Idle with no selection: emoji popup. |
| CLI `help_ck_hanja_output_format` | 한자 출력 형식 (hanja, hangul-hanja, hanja-hangul). 漢字 / 한자(漢字) / 漢字(한자) | Hanja output format (hanja, hangul-hanja, hanja-hangul): 漢字 / 한자(漢字) / 漢字(한자) |
| CLI `hanja_output_format_label` / `_note` | 한자 출력 형식 / (단음절·단어 변환 공통) | Hanja output format / (applies to syllable and word conversion) |
| CLI `error_invalid_hanja_output_format` | 잘못된 한자 출력 형식: %{value}. 가능한 값: %{allowed} | Invalid hanja output format: %{value}. Allowed: %{allowed} |
| CLI `hanja_output_format_changed` | 한자 출력 형식을 '%{fmt}'(으)로 변경했습니다. | Hanja output format set to '%{fmt}'. |
| Slint `@tr("한자 출력 형식")` (title·accessible-label) | 한자 출력 형식 | Hanja Output Format |
| Slint `@tr(description)` | 漢字 · 한자(漢字) · 漢字(한자) — 한자 팝업에서 확정할 때 넣는 형식 | 漢字 · 한자(漢字) · 漢字(한자) — text inserted when you confirm a hanja candidate |
| Slint 옵션 라벨 | 코어 `display_name()`(한국어 리터럴) 그대로 — `commit_unit` 선례와 동일한 **알려진 갭**(영어 로케일에서 「한자만/한글(한자)/한자(한글)」 한국어 노출). .slint 쪽 `@tr` 3개 매핑은 선례 정정과 함께 별도 | — |
| Slint `@tr("한글을 한자로 변환할 때 사용할 키입니다.")` (한자 변환 키 row description 개정, `settings.slint:810` + `.po:244` msgstr) | 조합 중이면 어절/음절 한자 변환, 한글 단어를 선택한 뒤 누르면 그 단어 변환, 그 외에는 이모지 팝업을 여는 키입니다. | The key that converts the word/syllable being typed, converts a selected Hangul word, and otherwise opens the emoji popup. |

GTK en 로케일 문자열은 `Hanja Output Format`(대문자 F) — CHANGELOG 영문 항목의 UI 이름도 이것과 일치시킨다.

**Q1 승인 전 문구(대상② 서술 제외)**: `row_hanja_keys_*`(GTK)·Slint 한자 변환 키 description·CHANGELOG Added 1행 후반부·매뉴얼 선택 변환 소절·루트 README 선택 행은 대상②(선택 변환)를 확정 서술하므로 Q1 게이트에 묶인다. 승인 전에 U9/U14 가 착수하면 다음 2벌 문구 중 **대상① 판**을 쓴다 — GTK subtitle: "쉼표로 구분 (예: Hanja, F9) — 조합 중이면 방금 친 어절/음절 한자 변환, 그 외에는 이모지 팝업" / "Comma separated (e.g. Hanja, F9) — converts the word/syllable being typed, otherwise opens the emoji popup"; tooltip 은 위 개정 문구에서 "앱에서 한글 단어를 선택한 채 …" 문장을 뺀 것; Slint description: "조합 중이면 어절/음절 한자 변환, 그 외에는 이모지 팝업을 여는 키입니다." / "The key that converts the word/syllable being typed and otherwise opens the emoji popup." Q1 거절 시 GTK ko/en 로케일·Slint .po·CHANGELOG·매뉴얼이 존재하지 않는 기능을 안내하지 않도록 하는 장치다. GNOME 사용자는 prefs.js 리다이렉트로 Slint 앱만 보므로 Slint 행을 빼면 두 GUI 안내가 갈린다.

---

## 4. 교체 프로토콜

### 4.1 `AutoTypefixApply` 재사용 (Linux 데몬)

- 신규 시그널·신규 `PopupAction`·RPC 시그니처 변경은 **없다**. `SelectHanja(u)->s` 는 그대로 서식 적용 문자열을 반환한다.
- 키보드 확정(`ProcessKeyEvent`): 엔진이 확정 접두가 있을 때만 out-of-band 페이로드 `HanjaReplacement{delete_chars, preedit_chars, text}` 를 남기고, 데몬이 이를 기존 `EngineResponse.auto_typefix` 필드에 실어 응답 후 `AutoTypefixApply(delete_chars, text, "")` 를 발행한다. 응답의 preedit 은 "" (프런트가 조합 음절을 먼저 지운다).
- 마우스 확정(`SelectHanja` RPC): 데몬 내부 응답을 `SelectHanjaOutcome{None, Commit(text), Replace{delete_chars,text}}` 로 구분해 `Replace` 면 `CommitText` 대신 `AutoTypefixApply` 를 popup-owner path 로 발행하고 `HidePopup` 을 잇는다.
- 상호배제: 팝업 확정 프레임에서는 AutoTypeFix 검사가 게이트로 건너뛰므로 두 교체가 같은 프레임에 겹치지 않는다. AutoTypeFix 모드 전환 시의 Global 카테고리 전파 블록은 한자 교체 프레임에서 발화하지 않도록 게이트한다.
- 소비 후 버퍼 폐기: ATF 블록(`engine_worker.rs:1338-1345`)은 `popup_action.is_none()` 게이트로 확정 프레임을 통째로 건너뛰어 `KeystrokeBuffer` 의 `push`/`update_on_commit`/`clear` 도 실행되지 않는다. RecentWord 확정 후 문서는 "大韓民國"(4자)인데 버퍼는 11키·`committed_chars=3`·`has_preedit=true` 로 남고 역방향 `delete_chars = committed_chars + has_preedit`(`src/auto_typefix/reverse.rs:13`) 가 이 stale 값을 쓴다 → 한자 교체 프레임(`ProcessKeyEvent` 의 `hanja_replaced`, `SelectHanja` RPC 경로 모두)에서 `keystroke_buffers.remove(&context_id)`(모드 전환 선례 `:1290`), TSF 는 `state.buf.clear()`(`auto_typefix.rs:221` 선례).
- 시그널 순서: 동기 RPC 응답(preedit "") → 메인루프에서 시그널 수신(AutoTypeFix 역방향과 동일).
- GTK4 `on_auto_typefix` 는 `!unim->is_focused` 면 드롭한다(`immodule.c:475`) — 종전 `on_commit_text`(:451-461) 에는 없던 조건. 마우스 확정(SelectHanja)을 교체 채널로 옮기면 팝업 클릭이 포커스를 뺏는 환경에서 교체가 조용히 드롭되고 HidePopup 은 이미 발행돼 "국" 이 유실될 수 있다. **프런트 우회는 넣지 않는다(현행 가드 유지)**: `preedit_text=="" && delete_chars>0` 는 역방향 AutoTypeFix 프레임과 같은 시그니처라(`engine_worker.rs:1713-1716` — preedit "" + delete>0 + ASCII commit) 우회가 역방향 ATF 의 포커스 가드까지 풀고, 포커스가 다른 창으로 넘어간 뒤 `delete_surrounding` 이 실패(Electron 등)하면 `:488-519` XTest BackSpace 폴백이 **현재 포커스 창**에 실제 키로 들어간다(현행 '조용히 드롭' 은 안전). popup-service 클릭이 GTK4 포커스를 뺏는 것으로 §6.5 실측에서 확인되면 popup-service 쪽(X11 override-redirect / layer-shell `keyboard_interactivity=none`)에서 고친다. 꼭 프런트 우회가 필요해지면 판별자를 한자 전용(commit_text 에 ASCII 도 한글도 아닌 문자 포함 — §4.2 와 동일)으로 두고 XTest 폴백 경로는 우회에서 제외한다. GNOME `onAutoTypeFix` 의 `_hasFocus` 게이트(`extension.js:284`)도 같은 성격 — St 팝업 클릭 시 유지 여부 실측(§6.5), 유지 안 되면 같은 원칙(렌더러 쪽 수정 우선).
- `SetContentType` 응답 채널(§2.8): `EngineRequest::SetContentType` 에 `response: oneshot::Sender<bool>` 추가, `service.rs set_content_type` 이 await 후 true 면 `redirect_commit_and_hide("")`. 프런트 쪽 DBus 시그니처는 불변(`SetContentType(u)` 호출 방식).
- AutoTypeFix 순방향 교정 뒤 버퍼 시드: 데몬 순방향 적용은 `engine.reset()` 뒤 마지막 음절만 replay 하므로(`engine_worker.rs:1555-1562`) 교정된 접두 "대한민" 이 버퍼에 없다. **replay 뒤 `engine.clear_commit()`(`:1600`) 직후**에 `engine.recent_clear(); for c in fix.commit_text.chars() { engine.recent_push_char(c) }` 로 시드한다 — 데몬 스스로 "replay 에서 commit 이 발생할 수 있다" 고 전제하고 `clear_commit` 을 부르므로, reset 직후·replay 전에 시드하면 replay 중 한글 commit 델타가 래퍼 (4) 로 시드 위에 추가 push 돼 surrounding 없는 XIM/Wayland 에서 검증 없이 과다 삭제될 수 있다. replay 뒤 시드는 replay 부작용과 무관하게 결정적이다. 영타→한글 교정 직후 한자키는 상용 경로라 v1 포함.

### 4.2 Wayland 바이트 판별

`zwp_input_method_v2.delete_surrounding_text` 는 **바이트** 단위인데 현행 휴리스틱은 "commit 첫 글자가 한글이면 삭제 대상은 ASCII(1B)" 로 판정한다(`state.rs:255-268`). `한자(漢字)` 형식은 첫 글자가 한글이라 3자 삭제가 1/3 만 지워진다.

- 해결: 휴리스틱 **앞에** 판별자 추가 — `commit_text` 에 ASCII 도 한글(음절·자모)도 아닌 문자가 하나라도 있으면 한자 교체로 보고 `before_bytes = delete_chars × 3`. 대상① 의 삭제 대상은 구성상 **항상 완성 음절(UTF-8 3B)** 이라 추정이 아닌 확정값이다.
- 회귀 0 근거: AutoTypeFix 의 commit_text 는 한글(순방향) 또는 ASCII(역방향) 뿐이라 판별자와 서로소 → 기존 경로 바이트 동일.
- `SurroundingText` 이벤트(`state.rs:626-628` 현재 드롭)는 `done` 에서 바이트→문자 오프셋으로 바꿔 `SetSurroundingText` 로 전달한다(대상② 감지·§2.2.3 검증 용도, 바이트 계산에는 쓰지 않는다). deactivate 시 빈 값으로 stale 제거.

### 4.3 XIM Reset 부작용 — 무해 증명

XIM 은 교체 완료 후 preedit 이 비어 있으면 `Reset` RPC 를 보낸다(`handler.rs:1103-1108`). 그 시점 엔진은 이미 확정을 마쳐 한자 모드 아님·preedit/commit_buffer 비어 있음·페이로드 drain 됨 → `reset_engine_and_capture_commit` 의 캡처가 전부 `None` → 커밋 메아리 없음 → 엔진 재생성(모드 보존). 버퍼가 비는 것은 "확정 후 리셋" 과 같은 결과. 유일한 경합(N+1 BS 완료 전 다음 자모 입력)은 종전 AutoTypeFix 역방향과 동일한 창이라 새 위험이 아니다. 회피 불필요.

### 4.4 Windows TSF

- 코어 상속: 버퍼·target·서식·페이로드는 코어에 있으므로 자동.
- 확정 2지점(키보드 `handle_key_down`, 마우스 `apply_reverse_event`): 팝업 액션 drain 직후 페이로드를 drain 해 `end_composition_keep_text`(`composition.rs:570`, 조합 "국" 을 문서 텍스트로 materialize) → `replace_surrounding(span = delete_chars + preedit_chars, text, "")`(`composition.rs:663`) — 역방향 AutoTypeFix 와 같은 레시피(CUAS 앱의 clear 무효 회피). 오버레이 조합 앱은 preedit 이 문서에 없으므로 `span = delete_chars` 만.
- `apply_reverse_event` 반환형은 `()` 유지: 교체는 항상 `preedit_text=""` 라 `PhaseSplit`/`SynthHeadTail` 이 발생하지 않는다(`key_handler.rs:443-444` 주석과 동일 조건). 단 **시그니처는 2개 늘어난다**: 현행 매개변수(`key_handler.rs:1130-1140` engine·config·comp_mgr·popup·context·tid·comp_sink·env·last_owner·last_seq)에는 오버레이 상태가 없어 공용 교체 함수를 부를 수 없다 — `composition_unsupported: bool`, `preedit_win: &mut Option<PreeditWindow>` 를 추가하고 호출부(`text_service.rs:2436`)가 `ctx.composition_unsupported.load(SeqCst)`(:80)·`&mut *ctx.preedit_window.lock().unwrap()`(:64) 을 기존 락 순서(engine → config → composition_mgr → popup_ipc → last_context) **뒤**에 취득해 전달한다(구현자는 `preedit_window` 를 먼저 잡고 engine 을 잡는 경로가 없는지 `rg preedit_window.lock unim-tsf/src` 로 확인).
- surrounding 갱신(Q9): 한자키를 누르는 매 순간 조합 중이든 idle 이든 `read_hanja_surrounding` 으로 문서(또는 조합 중이면 조합 포함) 커서 앞 텍스트를 읽어 `set_surrounding_text` 에 저장한다(읽기 실패·선택 없음은 빈 값으로 stale 제거). `engine.set_surrounding_includes_preedit(doc_composition)` 을 매번 재설정한다 — `doc_composition` 은 조합이 활성이고 오버레이 조합(`composition_unsupported`)이 아닐 때만 true(오버레이 앱은 조합 텍스트가 문서에 없어 켜면 클릭 드리프트가 검증을 뚫는다, §2.2.3). 대상② 확정은 `insert_text`(`composition.rs:639`, 선택 치환) 경로 — `replace_surrounding` 은 `Collapse(TF_ANCHOR_START)` 때문에 선택 앞 글자를 지우므로 쓰지 않는다.
- `OnSetFocus` 캡처 래퍼(종전 버그): 한자 팝업 중 포커스 이탈 시 bare `engine.reset()` 이 상태를 버렸다. 정상 앱은 조합을 문서 텍스트로 확정(keep) 후 상태 정리, 오버레이 앱은 복구 시도 후 로그(§2.7).
- 렌더러(`unim-popup-win`): 헤더 `DT_END_ELLIPSIS`, compact 한자 열 실측 폭, expanded 셀은 **폭 고정 + 셀 텍스트 ellipsis**(§2.5.4). wire 프로토콜 무변경. 동결 설계서 `docs/dev/windows/popup-renderer-design.md`(:416 compact 열 폭·:422 격자) 문구 갱신.
- 검증: `make check-windows` + `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu`(WIN_CRATES 밖). 런타임은 VM 대기.

### 4.5 GTK4·Qt 선택 삭제 래퍼 게이트 [Q4]

GTK4(`immodule.c:1061`)와 Qt(`input_context.cpp:446`)는 엔진 결과가 `consumed` 이기만 하면 앱 선택 영역을 먼저 지운다. 이대로면 한자키(GTK4) 또는 팝업 중 Esc(Qt)가 **선택을 지워** 취소해도 원문이 사라지고, 불일치 선택 + 한자키(소비, 팝업 없음)도 텍스트를 조용히 지운다.

- 개정: `consumed && (commit 비어있지 않음 || preedit 비어있지 않음)` 일 때만 선택을 지운다.
- 효과: 확정 키(commit 있음)에서는 종전대로 삭제→커밋(치환), 팝업 열림·내비·Esc·한/영 전환 같은 "아무것도 넣지 않는 소비 키" 에서는 선택이 남는다. 바뀌는 기존 동작은 "선택한 채 한/영 전환키를 누르면 선택이 지워지던 것" 뿐이며 의도된 동작이 아니다. L3 회귀 케이스로 고정한다.

---

## 5. POPUP_SPEC 개정안 (승인 대기) [Q1]

승인 시 아래 문구를 `POPUP_SPEC.md` 에 그대로 반영한다. 조항 번호는 현행 `POPUP_SPEC.md`(v3.3) 기준.

### 5.1 §3.7 동작 규칙 — 2번 교체

> 2. **대상**: 다음 순서로 결정한다.
>    - (a) 조합 중이고 preedit 의 마지막 글자가 완성 음절이면: 음절 확정 모드에서는 「엔진이 기억하는 최근 확정 한글 음절(최대 17자) + preedit」, 단어 확정 모드에서는 「preedit 전체」의 접미 가운데 한자 사전에 있는 **가장 긴** 문자열(2자 이상, 최대 18자). 예: "대한민"+"국" → "대한민국". 없으면 preedit 의 마지막 음절(종전 규칙, 예: "국").
>    - (b) 조합 중이고 마지막 글자가 미완성 자모이면: 종전 규칙(마지막 글자, 초성이면 규칙 7 특수문자 전환).
>    - (c) 조합 중이 아니고 앱 선택 영역(`SetSurroundingText` 의 `cursor_pos != anchor_pos`)이 있으면: 선택 텍스트(앞뒤 공백 제외)가 전부 완성 음절이고 18자 이하이며 사전에 정확히 있을 때 그 문자열. 조건을 만족하지 않으면 어떤 팝업도 띄우지 않고 키만 소비한다.
>    - 최근 확정 음절 버퍼는 비한글 확정(공백·구두점·영문·숫자·특수문자·이모지·한자 포함)·Enter·커서 이동·Backspace 통과(끝 1자 제거)·한/영 전환·포커스 이탈·Reset·비밀번호 필드·팝업 확정/취소 시 비워진다.
>    - (a)에서 접미가 이미 확정된 글자를 포함하면(음절 확정 모드) 그 글자 수를 "확정 접두 길이" 로 기억한다. 단어 확정 모드에서 접미 앞의 나머지 preedit 은 확정 시 그대로 함께 커밋한다.

### 5.2 §3.7 동작 규칙 — 4번 교체

> 4. **선택 시**: `SelectHanja(globalIndex)` → 엔진이 **출력 형식 설정(`hanja_output_format`: 漢字 / 한자(漢字) / 漢字(한자))을 적용한 문자열** 반환(단음절 변환에도 동일 적용) → 프론트엔드가 커밋. 대상에 확정 접두가 포함되면(규칙 2(a) 음절 확정 모드) 데몬은 `CommitText` 대신 `AutoTypefixApply(delete_chars=접두 글자 수, commit_text=서식 문자열, preedit_text="")` 를 popup-owner path 로 발행하고 프론트엔드는 AutoTypeFix 와 동일한 삭제→커밋을 수행한다. 규칙 2(c) 는 `delete_chars=0` 커밋이며 위젯이 선택 영역을 치환한다(판정에서 제외한 앞뒤 공백은 커밋 문자열에 되붙인다).

### 5.3 §3.7 동작 규칙 — 5번 교체

> 5. **취소 시**: `CancelHanja()` → 엔진이 팝업 진입 때 preedit 에서 내려간 텍스트(음절 확정 모드: 마지막 음절, 단어 확정 모드: 누적 preedit 전체, 선택 영역: 없음)를 반환 → 프론트엔드가 그대로 커밋(없으면 커밋 없음) → 팝업 닫기. 이미 확정돼 앱에 있는 접두와 선택 영역은 건드리지 않는다.

### 5.4 §3.7 동작 규칙 — 10번 신설

> 10. **선택 영역 변환 지원 범위**: 규칙 2(c) 는 선택 영역을 `SetSurroundingText` 로 전달하는 프론트엔드(GTK4·Qt5/6·GNOME Shell 확장·Wayland·Windows TSF)에서만 동작한다. GTK3·XIM 은 선택 정보를 전달하지 못하므로 종전 idle 동작(이모지 팝업)을 유지한다. 선택이 있으나 사전에 없으면 어떤 팝업도 띄우지 않는다.

### 5.5 §9.2 idle Hanja 키 dispatch 정책 — 인용문 교체

> **idle Hanja 키 dispatch 정책 (v3.4)**: Hanja 키는 `input_category` 와 무관하게
> `press_key()` 의 언어 분기 직전에 처리. 조합 중이면 한자 변환(§3.7 규칙 2(a)(b)).
> preedit/조합 idle 이면 (i) 앱 선택 영역이 있을 때 §3.7 규칙 2(c) 선택 단어 변환을 시도하고
> 사전 일치가 없으면 아무 팝업도 띄우지 않는다, (ii) 선택 영역이 없으면 emoji popup 트리거(종전 v3.2 동작).
> 종전엔 `process_korean_key` 안에 있어 영문 모드 첫 Hanja 키가 not_consumed 로 떨어져 무시되던 회귀가 있었다.

### 5.6 §2.4 DBus 메서드 정리 — 행 주석 보강

> `GetHanjaCandidates` 의 `target` 은 다음절 어절일 수 있다. `SelectHanja` 의 반환 `s` 는 출력 형식이 적용된 문자열이며, 확정 접두가 있으면 데몬이 `CommitText` 대신 `AutoTypefixApply(u,s,s)` 를 발행한다. `CancelHanja` 는 규칙 5 의 텍스트를 커밋한다. 시그니처 변경 없음.

### 5.7 §11 변경 이력 — 행 추가

> | (승인일) | **v3.4** | **한자 단어 변환 — §3.7 규칙 2 대상 확장(최근 확정 음절+조합 최장 접미·단어 모드 preedit 접미·앱 선택 영역), 규칙 4/5 확정·취소 페이로드 개정, 규칙 10 선택 변환 지원 범위, §9.2 idle 정책에 선택 영역 예외. 확정 시 `AutoTypefixApply` 를 접두 교체 채널로 재사용, `SelectHanja` 반환 문자열에 출력 형식 설정(`hanja_output_format`) 적용. 헤더 ellipsize·Windows compact 한자 열 동적 폭. 별도 규격: `HANJA_WORD_SPEC.md`** |

### 5.8 `unim-dbus/SPEC.md` 보강 (승인 불필요 — 신규 기능 문서화)

- `:254` `GetHanjaCandidates`: "target 은 다음절 어절일 수 있다".
- `:255` `SelectHanja`: "반환은 출력 형식 적용 문자열. 확정 접두가 있으면 `CommitText` 대신 `AutoTypefixApply(u,s,s)`(preedit_text="") 가 popup-owner path 로 발행된다".
- `:256` `CancelHanja`: "팝업 진입 때 내려간 preedit 부분만 커밋(선택 영역 대상은 커밋 없음)".
- `:285` `AutoTypefixApply`: "한자 단어 교체에도 사용 — 그때 `delete_chars`=확정 접두 글자 수, `preedit_text=""`".
- `:264` `SetSurroundingText`: "한자 선택 변환(대상②) 감지·접두 정합성 검증에도 사용. Wayland 는 `done` 마다, Qt 는 `update()` 마다(변경 시, 빈 텍스트 포함), TSF 는 idle 한자키 시점에 전송".

### 5.9 POPUP_SPEC 반영 절차

Q1 승인 → §5.1~5.7 문구를 `POPUP_SPEC.md` 에 그대로 반영(v3.4) → 이 문서 상태를 "승인·반영" 으로 갱신. `CONTRIBUTING.md` 팝업 변경 6지점 체크리스트에 `POPUP_SPEC.md` 가 들어 있으므로 별도 작업 단위(PLAN U14b)로 잡는다. §9.2 정책 이탈(idle+선택 → 이모지 미발동)과 규칙 10 은 **Q1 승인 전에는 코드 착수하지 않는다**(엔진 U8 의 `selection_target` 분기·데몬 U11). 문서·UI 문구 중 대상② 서술(GTK `row_hanja_keys_*`·Slint 한자 변환 키 description·CHANGELOG Added 1행 후반부·매뉴얼 선택 변환 소절·루트 README 선택 행)도 같은 게이트 — 승인 전에는 §3.3 의 "Q1 승인 전 문구(대상① 판)" 를 쓴다.

---

## 6. 테스트 요구

### 6.1 L1 단위 (`src/input_engine/tests_hanja_word.rs` 신규)
두벌식 키열 대한민국 = `E O G K S A L S R N R`(11키: 대=E,O / 한=G,K,S / 민=A,L,S / 국=R,N,R). 필수 케이스:

| 영역 | 케이스 |
|---|---|
| 대상① | 정상("대한민"+"국" → target 대한민국·확정 접두 3), 불일치 폴백("뷁"+"국" → "국"), 미완성 자모 폴백, 확정 페이로드(`Some{3,1,"大韓民國"}`·commit_buffer 비어 있음·HidePopup), 키보드/직접 호출 두 경로, **호스트 플래그 false(기본)에서 "대한민"+"국" → target "국"·페이로드 None·commit_buffer "國"(IMM32/capi 바이트 동일)**, **chord 모드 "대한민" 확정 + chord "국" 대기 + 한자키 → target "대한민국"·확정 접두 3**(한자키 자체가 확정한 음절 흡수) |
| 정합성 검증 | surrounding 비어 있음(통과), GTK 형("…대한민")·잘린 형("민")(통과), 불일치("xyz")(→ "국"), **`surrounding="대한민국 만세", cursor=0`(before 빈 문자열 → "국" 퇴화)**, **TSF 형("…대한민국"): `surrounding_includes_preedit=false`(기본) + surrounding "대한민국"·cursor 4·prefix "대한민"·pre "국" → 실패(→ "국" 퇴화, Wayland/GNOME 클릭 드리프트) / 같은 입력 + 플래그 true → 통과**, **cursor > 길이 → 실패** |
| 버퍼 | 상한 17, Backspace pop(선택 있으면 clear), chord idle flush 반영(비번 차단 중이면 push 안 됨), 수정자 단독 키 비리셋 |
| 오프셋 방어 | `set_surrounding_text("가나", 5, 3)` 뒤 한자키 → 패닉 없이 선택 없음(범위 초과 거부 + a≥b 조기 반환); **`("대한민국", 0, 12)` → 선택 없음·팝업 없음(단위 불일치 거부, 클램프로 "대한민국" 팝업이 뜨면 실패)**; `typefix_convert` 도 동일 거부 |
| 팝업 중 키 | PageDown·즐겨찾기 토글·expand 뒤 버퍼 **clear**(래퍼 (2) `was_popup` — target·committed 는 진입 시 확정이라 무해) |
| 리셋 조건 | Space·Enter·Tab·Escape·Left·Home·Delete·Ctrl+A·토글키·영문자·특수문자 자모·쉼표/숫자/기호 등 비한글 커밋 각 지점·이모지/특수문자/한자 확정·`reset()`·`set_input_category`·비밀번호 진입 |
| 단어 모드 | "오늘대한민국" → target 대한민국·접두 "오늘"·확정 "오늘大韓民國"·취소 "오늘대한민국"; 불일치 폴백에서 접두 보존 |
| 대상② | 정상(선택 "대한민국")·공백 보존(`" 대한민국 "` → `" 大韓民國 "`)·거부 4종(불일치·비한글·19자·공백만)·선택 없음 idle → 이모지·조합 중 선택 무시·취소 시 커밋 없음 |
| 서식 | 3종 × 단음절/단어; config serde(필드 없는 YAML → `Hanja`, `HanjaHangul` 파싱이 Compat 경유로 살아남음) |
| 즐겨찾기 | 단어 키 토글·`HanjaCandidatesReordered.target=="대한민국"` |
| 비밀번호 | 진입 시 버퍼 비움, pull 경로 차단, 팝업 열림 → `set_content_purpose(Password)` → `commit_buffer.is_empty() && !is_hanja_mode()` 단언(재커밋 없음 = 종전 `flush_preedit` 보다 먼저 정리), **`set_surrounding_text("대한민국",0,4)` → `set_content_purpose(Password)` → `!has_selection() && surrounding_text().0.is_empty()`**(진입 시 잔류 제거) |
| 회귀 | `cargo test --workspace` 전량(AutoTypeFix `tests.rs`·`tests_atf_hotkey`·`tests_scenarios`·`tests_popup_change_page`) 경고 0 |

### 6.2 L2 DBus (`tests/unim-test-dbus`)
evdev 키열 `18 24 34 37 31 30 38 31 19 49 19` → Hanja(123). (1) `GetHanjaCandidates` target=="대한민국" → `AutoTypefixApply` 스트림 구독 후 `SelectHanja(0)` → `(3, "大韓民國", "")`. (2) `SetSurroundingText("대한민국 만세",0,4)` → Hanja → target=="대한민국" → `SelectHanja(0)` → `CommitText "大韓民國"`. (3) `SetSurroundingText("abc def",0,3)` → Hanja → 후보 없음·이모지 팝업 시그널 미발행. (4, 선택) `SetConfigYaml` 로 `HangulHanja` → "대한민국(大韓民國)". (5) 팝업 열림 → `SetContentType(Password)` → `HidePopup` 시그널 수신·`CommitText` 미발행. (6) AutoTypeFix 순방향 교정(영타 "eogksals" → "대한민") 직후 "국"+Hanja → target=="대한민국"(버퍼 시드).

### 6.3 L3 Xvfb 하네스 (`tests/harness/scenarios/hanja_word.json`)
(1) `hanja-word-syllable`: 11키 → `{committed:"대한민", preedit:"국"}` → F9 → 1 → `{committed:"大韓民國", preedit:"", rendered:"大韓民國"}`. (2) `hanja-word-selection`(GTK4 앱 필드): 11키 → space → Left → shift+Home → F9 → 1. (3) `selection-kept-on-consumed-key`(래퍼 게이트 회귀): `abc` → 선택 → 한/영 전환키 → committed "abc" 유지 → F9 → "abc" 유지. (4, 선택) `hanja-word-custom-key`(Qt 앱, `hanja_keys` 에 F9 아닌 키): 11키 → 커스텀 한자키 → 팝업 헤더 "대한민국"(stale 선택 스냅샷 없이 `update()` 갱신 경로 확인). `commit_unit`/서식 자동 적용 확장은 선택 — 채택 시 `harness.py` 에 `set_config_field(path, value)`(`GetConfigYaml` → 키 패치 → `SetConfigYaml`, 종료 시 복원) 헬퍼가 **필수**(레거시 `SetConfig` 는 두 키를 모르므로 layout 패턴 복제는 조용히 실패).

### 6.4 Windows
`make check-windows` 경고 0 + `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu`. VM 검증 항목(이 환경 불가): 확정 2지점 교체, CUAS(synth) 다글자 삭제, 오버레이 폴백, `insert_text` 선택 치환, `OnSetFocus` 래퍼, 렌더러 폭.

### 6.5 실측 항목(리포 밖)
Mutter `vfunc_set_surrounding` 호출 빈도·오프셋 단위(바이트/문자 — 바이트면 확장 쪽에서 문자 단위 변환, 엔진은 초과 오프셋을 거부한다 §2.4.1), GtkTextView·St.Entry·Chromium(text-input-v3)·Electron 의 commit 시 선택 치환 여부, Qt UTF-16 오프셋의 서로게이트 처리, 앱별 surrounding 보고 지연, popup-service 클릭 시 GTK4 `is_focused` 유지 여부(X11 override-redirect / layer-shell keyboard_interactivity — §4.1), GNOME St 팝업 클릭 시 `UnimInputMethod._hasFocus` 유지 여부(`extension.js:284` 게이트 — §4.1), XIM 스팟 점프 Reset 의 실제 비용(사전 재파싱 시간·클릭 뒤 첫 키 지연 — 문제되면 경량 RPC, Q9(a)).

---

## 7. v2 메모

- **target 축소/확장 키** [Q7]: 팝업 중 한자키 재타 → 접미 1자 축소(대한민국 → 한민국 → 민국 → 국), Shift+한자키 → 확장. v1 은 단어 일치 시 단어 후보만 보인다.
- **조사 분리**: "대한민국은" 에서 "대한민국" 만 — 조사 목록 + 최장일치. v1 은 "조사를 붙이기 전에 한자키" 를 매뉴얼에 안내.
- **후보 랭킹**: 사전 등장순이 빈도순이라는 보장이 없다. 즐겨찾기로 보완, 빈도 데이터는 v2.
- ~~AutoTypeFix 순방향 교정 뒤 버퍼 시드~~ → v1 로 당김(§4.1, U11 1줄).
- ~~XIM 커서 점프 감지~~ → v1 [Q9](a) 로 당김(Reset 송신 — 엔진 재생성·chord 강제 flush 동반, idle 구간당 1회 디바운스). 버퍼만 비우는 경량 RPC `ClearRecentSyllables`(엔진 `recent_clear` 만, `unim-dbus/SPEC.md` 1행)는 Reset 비용이 실측에서 문제될 때 v1.1 — "Reset 이 값싸다" 는 전제는 성립하지 않으므로 '불필요' 로 단정하지 않는다.
- **XIM 대상②**: X11 PRIMARY 선택 우회.
- **기능 on/off 설정**: 두지 않는다 — "다음절 일치 없음 = 종전" 이 곧 안전망이고, 스위치는 설정 동기 지점 9곳과 테스트 매트릭스를 두 배로 만든다. 필요해지면 `KoreanConfig.hanja_word_conversion: bool` 을 §3.2 표대로 추가.

---

## 8. 변경 이력

| 날짜 | 버전 | 변경 내용 |
|---|---|---|
| 2026-09-17 | v1-draft | 초안 — 세 설계안(엔진·UX·플랫폼) 심사 종합, POPUP_SPEC 개정안 §5 수록, 승인 대기 |
| 2026-09-17 | v1-draft2 | 검증 반영 — 선택 오프셋 양끝 클램프(패닉 방지), `before` 빈 문자열 검증 실패, 비번 게이트 순서·데몬 HidePopup 응답 채널, TSF OnSetFocus 양 분기, 바이트 동일 불변식을 preedit 1자로 한정, ATF 순방향 시드·XIM 스팟 Reset v1 승격, Windows 격자 폭 고정 유지, 동기 지점 표 확장(Slint .po·help-html·매뉴얼 범위), §5.9 반영 절차, Q7 대안·Q8·Q9 신설 |
| 2026-09-17 | v1-draft3 | 2차 검증 반영 — 호스트 능력 플래그 `hanja_word_replace_capable`(기본 false, IMM32/capi 회귀 차단), chord 한자키 확정 음절 흡수, 선택 오프셋 초과를 클램프 대신 **거부**(단위 불일치 fail-closed), `prefix+pre` 검증 절을 TSF 전용 플래그로 한정, 비번 진입 시 surrounding 잔류 제거·Qt 프런트 이중 게이트·로그 길이만, GTK4 `is_focused` 우회 철회(역방향 ATF 시그니처 동일·XTest 폴백 위험)·GNOME `_hasFocus` 실측 항목, XIM 스팟 점프 판정 기준("IM 이 유발하지 않은 갱신"+디바운스)·Reset 비용 정정, ATF 순방향 시드를 replay 뒤로, 한자 교체 후 KeystrokeBuffer 폐기, TSF `apply_reverse_event` 시그니처 2개 추가, Qt surrounding 을 `update()` 갱신으로, 동기 지점에 `src/SPEC.md`·`unim-cli/SPEC.md`·루트 README·매뉴얼 ko/en 1:1 추가, Q1 승인 전 2벌 문구·Slint 한자 키 row |
