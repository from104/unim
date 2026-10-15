# UNIM 한자 단어 입력 설계서 (HANJA_WORD_SPEC)

> 이 문서는 한자 팝업의 변환 **대상**을 "preedit 마지막 음절" 에서
> **방금 입력한 어절(대상①)**, **앱 선택 영역(대상②)**, **커서가 걸친 단어(대상③, v1.6)** 로 넓히는 기능의 규격이다.
> 팝업의 모양·키·즐겨찾기·DBus 시그니처는 [`POPUP_SPEC.md`](POPUP_SPEC.md) 를 그대로 따르며,
> 이 문서는 그 위에 "무엇을 대상으로 삼고, 확정 시 앱 텍스트를 어떻게 바꾸는가" 만 정의한다.
> `POPUP_SPEC.md` 본문은 수정하지 않았다 — 필요한 조항 개정은 §5 "POPUP_SPEC 개정안(파일 반영 승인 대기)" 에 문구 그대로 두었다.

- 상태: **PM 대행 결정 반영 — 기현님 최종 확인 대기** (구현 전, develop `ba64255` 기준 설계). §0 의 Q1~Q9 는 기현님 부재중 PM 대행이 결정했고 코드·문서는 그 결정대로 **바로 구현한다**(본문은 결정이 스며든 상태). 기현님 승인이 남은 것은 `POPUP_SPEC.md` **파일 자체의 수정**(§5 반영, PLAN U14b)뿐이며, 승인 후 반영하고 이 문서의 상태를 갱신한다.
- **v1.6 상태(2026-10-12)**: **기현님 승인(2026-10-12)·구현 반영(미커밋, 실기·L3 확인 전).** 설계(대상③ "커서가 걸친 단어", 대상 우선순위, 선택 출처 신뢰 등급, 거부 플래그, PRIMARY 퇴역)는 기현님 확정(2026-10-12). v1.6 에서 바뀐 절: 상태·§0 Q8·Q10~Q12·§1.2·§1.3·§2.1·§2.4.6·**§2.4b(신설)**·§2.5.5·§2.5.6·§2.6·§2.7·§2.8·§2.10·§2.10.1·§3.3·§4.1·§4.4·§4.5·**§4.6(신설)**·§5.10·§6·§7·§8. 같은 날 `POPUP_SPEC.md` v3.7(§3.7 규칙 2·4·5·10·11·12, §9.2)·`unim-gnome-extension/SPEC.md` §2.8 도 함께 고쳤다.
- 관련 문서: `POPUP_SPEC.md`(팝업 규격), `unim-dbus/SPEC.md`(DBus), `ROADMAP.md` ① 항목, 사용자 매뉴얼 §4.2.

---

## 0. 결정 항목 (한눈에)

기현님 부재중 **PM 대행 결정**(2026-09-17) → **기현님 최종 승인(2026-09-26, 전 항목 결정대로)**. 구현자는 '결정' 열대로 진행하며 본문은 결정이 반영된 상태다. 기현님이 뒤집으면 §8 이력에 행을 추가하고 본문을 되돌린다.

| # | 항목 | 본문 기본값 | 대안 | **결정** (PM 대행 결정 → 2026-09-26 기현님 승인) |
|---|---|---|---|---|
| Q1 | §5 POPUP_SPEC 개정안(규칙 2·4·5·10·11, §9.2 예외) | 채택 | 문구 수정 요청 | **코드와 문서는 대상② 를 구현·서술한다** — 기현님 요청 원문에 "선택한 단어" 가 명시돼 있어 기능 자체는 결정된 것으로 본다. `POPUP_SPEC.md` **파일 수정(U14b)** 은 2026-09-26 승인·반영(v3.4, §5). 대상① 판 2벌 문구·`Resolve::None` 스텁 같은 승인 전 조건부 장치는 두지 않는다 |
| Q2 | 출력 형식의 괄호 문자 | 반각 `(` `)` — `대한민국(大韓民國)` | 전각 `（）` | **반각 `(` `)`** |
| Q3 | 대상② 확정 시 선택 앞뒤에서 제외한 공백을 되붙임(§2.4.3) | 되붙임(문서 보존) | 공백 소실 수용 | **되붙임** |
| Q4 | GTK4·Qt 의 "빈 소비 키" 선택 삭제 동작 변경(§4.5) — 선택한 채 한자키·한/영 전환키를 눌러도 선택이 남는다 | 채택(D8 충족 필수) | — | **채택** |
| Q5 | 다음절 후보의 글자별 뜻 합성 표시(§2.5.3) | 포함 | 제외(뜻 빈 채 출시) | **포함 — U2b 필수** |
| Q6 | 대상② 불일치 시 접근성 비프(§2.9) | 포함(옵션 ON 일 때만) | 제외 | **해당 없음** — Q8(a) 채택으로 "선택은 했는데 아무 팝업도 안 뜨는" 무음 실패 경로가 사라져 비프를 낼 상황이 없다. `pending_ui_feedback`·`UiFeedback`·`announce_hanja_nomatch` 는 만들지 않는다 |
| Q7 | 단어 일치 시 음절만 바꾸는 길이 없음. 사전 다음절 27.5만 항목이라 2음절 이상 한자어는 거의 일치 → "한국"+F9 에서 國 단독 후보 경로가 사실상 사라진다(Esc 뒤 재차 F9 는 idle→이모지) | 수용(v1 은 단어 후보만) | (a) 팝업 중 한자키 재타 = 접미 1자 축소(v2 축소 키를 v1 로 당김, 추가 키 0) / (b) 단어 후보 뒤에 음절 후보 이어붙이기(`hanja_candidates` 타입 변경 파급, 별도 설계) | **(a) 채택** — 팝업 중 한자키 재타 = target 접미 1자 축소(§2.5.5). 대상① 전용(대상② 는 선택 전체 치환이라 축소 없음), 길이 1 이면 현행 동작 유지 |
| Q8 | 대상② 불일치 시 동작(D6) — 초안은 "팝업 없이 키만 소비". select-on-focus 위젯(GtkEntry 기본)·사전 일치 한글이 자동 선택된 검색창에서는 사용자가 선택한 적 없어도 idle 한자키가 침묵하고 이모지 경로가 영구 차단된다(Esc → 래퍼 게이트로 선택 유지 → 재차 F9 → 같은 결과). 반면 툴킷 관례상 어떤 커밋도 선택을 치환하며(타이핑과 동일, Ctrl+Z 복구 가능) | D6(무동작) | (a) 불일치 시 종전 idle 동작(이모지 팝업)으로 폴백 — 사전 일치 한글 선택만 한자 팝업 / (b) 최소 타협: 한자 팝업 안에서 한자키 재타 → 이모지 팝업 전환 | **(a) 채택** — 사전 정확 일치 한글 선택만 한자 팝업, 그 외는 종전 idle 동작(이모지 팝업). D6 무동작·`NoMatchSelection`·불일치 비프 폐기. **v1.6 갱신(Q10)**: 불일치 선택은 그대로 이모지다 — 선택이 있는 채 대상③ 을 확정하면 앱이 그 선택까지 치환하므로 대상③ 으로 넘어가지 않는다. 선택이 **없는** idle 만 대상③ → 없으면 이모지로 바뀐다(§2.1) |
| Q9 | XIM·surrounding 미지원 Wayland 앱에서 마우스 커서 이동 뒤 대상① 확정이 무관 텍스트를 비가역 삭제할 수 있다(§2.2.1 주석 — 리셋도 검증도 없음). "헤더 확인 후 Esc" 는 상시 비용 | (a) XIM: **IM 이 유발하지 않은 스팟 갱신**(idle 이고, 직전 commit/preedit_draw 뒤 IM 이 기대하지 않은 `XNSpotLocation` 보고 — 보조로 y 변화·x 감소는 무조건) 시 `DbusRequest::Reset` 1회 송신, idle 구간당 1회 디바운스(`handler.rs:918` 스팟 보고 지점). **주의**: Reset 은 `reset_engine_and_capture_commit`(chord 강제 flush + 한자/preedit 캡처 + `InputEngine::new` = 사전 ≈6.45MB 재파싱, `engine_worker.rs:733-757`)이라 값싸지 않다 — 오탐 비용 = 재파싱 1회 + 단음절 퇴화(안전 방향), 디바운스가 상한. 재파싱이 실측에서 문제되면 엔진 `recent_clear` 만 하는 경량 RPC `ClearRecentSyllables` 로 교체(v1.1 후보) | (b) Wayland: `SurroundingText` 이벤트가 한 번도 오지 않은 컨텍스트에서는 `committed>0` 대상① 을 끄고 preedit 내부 일치만 허용(D1 "Linux 전 프런트 대상①" 을 surrounding 미지원 앱에 한해 축소) / (c) 둘 다 미적용 | **(a) 포함**(계획 그대로, U5) **+ (b) 채택** — 신규 RPC 없이 엔진 `surrounding_seen: bool` 로 "surrounding 을 준 적 있는 컨텍스트는 반드시 검증(빈 텍스트 = 실패)", Wayland 프런트는 미수신 컨텍스트에 `SetSurroundingText("",0,0)` 마커를 보내 `committed>0` 대상① 을 퇴화시킨다(§2.2.3·§4.2). GTK3/4·Qt·GNOME·TSF 는 이미 surrounding 을 보내므로 자동 적용(TSF 는 조합 중에도 읽도록 §4.4 보강) |
| Q10 | (v1.6) idle 한자키 + 선택 없음 | 이모지(v3.2~v3.6) | — | **기현님 확정 2026-10-12 — 대상③ "커서가 걸친 단어"**(§2.4b). 대상 우선순위: 조합 중 → 대상① / idle + 신뢰 가능한 선택 → 대상② / idle + 선택 없음 → 대상③ / 그래도 없으면 이모지. ibus-hangul·fcitx5-hangul·macOS 한국어 입력기·한글 워드프로세서·Word 의 "커서 위치 단어 변환" 과 같은 계열. 선택 방식 대비 단어마다 선택 키 k개(Shift+←×k, 보통 2~4)를 아낀다 |
| Q11 | (v1.6) GNOME 확장·GTK3 모듈의 PRIMARY **내용** 읽기(v1.2~v1.5 대상② 보강: 인접/유일 판정·키보드 길이 추적) | 유지 | — | **퇴역**(§2.10.1). PRIMARY 는 내용을 읽지 않고 **변경 신호**로만 쓴다(거부 플래그 입력, §2.4b.7). LibreOffice 가 PRIMARY 를 첫 읽기 시점에 굳히고(swdtflvr.cxx:496-532) editeng 은 키보드 선택을 반영하지 않아(impedit.cxx:1764-1767) 내용 추정이 글자 유실을 낳았다 |
| Q12 | (v1.6) 선택 출처 신뢰 등급 | — | — | **App-pull·Widget 만 신뢰**, App-push(text-input 경유)는 신선할 때만, Guess(PRIMARY 내용 추정)는 쓰지 않는다(§2.4.6) |

---

## 1. 목적·용어

### 1.1 목적

현재 한자키는 `hanja_target` 이 preedit 마지막 한 음절이라 `대한민국` 을 `大韓民國` 으로 한 번에 바꿀 수 없다(`ROADMAP.md` ①). 사전(`src/data/hanja.txt`)에는 다음절 항목이 27만 5천 개 이미 있고 `HanjaDictionary` 는 문자열 키 `HashMap` 이라 `대한민국` 조회가 그대로 동작한다. 이 문서는 **새 키·새 모드 없이** 기존 2키 시퀀스(한자키 → 숫자)로 단어를 바꾸는 규격을 정한다.

### 1.2 용어

| 용어 | 정의 |
|---|---|
| **대상①** | 사용자가 방금 입력한 어절. 음절 확정 모드에서 "대한민" 이 이미 앱에 확정되고 "국" 이 조합 중일 때 "대한민국". 단어 확정 모드에서는 preedit 누적분 안의 어절 |
| **대상②** | 앱에서 선택(selection)된 한글 단어. v1.6 부터 신뢰 가능한 출처(§2.4.6)가 보고한 선택만 |
| **대상③** (v1.6) | **커서가 걸친 단어** — idle·선택 없음일 때 surrounding 에서 커서 앞뒤로 이어진 한글 음절 구간 안의, 커서를 포함하거나 커서에 닿는 사전 단어(§2.4b). 예: `대한민국│` · `대한│민국` · `만세 │대한민국` 모두 "대한민국"(입력란 맨 앞 `│대한민국` 은 제외 — §2.4b.2) |
| **a / b** (v1.6) | 대상③ 확정 때 커서 기준으로 지울 글자 수 — a = 커서 앞, b = 커서 뒤. `대한│민국` 이면 a=2·b=2 |
| **선택 출처 등급** (v1.6) | anchor 를 누가 언제 보고했는가: App-pull · Widget · App-push · Guess(퇴역). §2.4.6 |
| **거부 플래그** (v1.6, `selection_possible`) | "앱이 IM 에 알리지 않은 선택이 있을 수 있음". 서 있으면 대상③ 을 쓰지 않는다. §2.4b.7 |
| **낡음** (v1.6, `surrounding_stale`) | 마지막 surrounding 보고 뒤에 커서·텍스트를 바꿀 수 있는 사건이 있었음. 서 있으면 대상②·③ 을 쓰지 않는다. §2.4b.6 |
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
| 대상② | GTK3(v1.4, 위젯 조회만)·GTK4·Qt5/6·TSF. GNOME 확장·순수 Wayland 는 앱이 anchor 를 보내는 경우(GTK4·Qt6·Chromium text-input-v3)에 한해, 신선할 때만(§2.4.6). **XIM 미지원**(종전 idle 동작 유지) | GTK3 는 `gtk_im_context_set_surrounding` 에 anchor 인자가 없지만(`gtk3/src/immodule.c` `set_surrounding` 은 anchor=cursor), v1.4 부터 한자키 직전에 **위젯(GtkEditable·GtkTextView)에 직접 조회**해 선택을 복원하고 `SetSurroundingText` 를 동기로 보낸다(§2.10·§2.4.5, 기현님 승인 2026-10-05). v1.4~v1.5 의 "같은 프로세스 PRIMARY" 폴백은 v1.6 에서 퇴역(§2.10.1). XIM 은 surrounding 개념이 없고 선택 전달 수단이 없다 |
| 대상③ (v1.6) | surrounding 을 주고 확정 채널(§4.6 `ReplaceSurroundingText`)을 처리하며 선택을 신뢰할 수 있거나(App-pull·Widget) 거부 신호를 보고하는(GNOME 확장) 경로: GTK3 모듈(GtkEditable·GtkTextView)·GTK4 모듈·Qt5/6·GNOME 확장(삭제 글자 수 실측 §6.5 (10) 뒤 활성)·TSF. **순수 Wayland 는 보류**(포인터·PRIMARY 신호 수단 없음 — §2.10, 미결), XIM·IMM32·X11 Chromium(GTK IM)·터미널은 surrounding 이 없어 불가 | 대상이 surrounding 에서 직접 나오므로 "버퍼와 문서 대조" 가 필요 없다. 남는 위험은 IM 이 모르는 선택과 낡은 surrounding 뿐이고 둘 다 이모지로 가는 쪽으로 막는다(§2.4b.6·§2.4b.7) |
| 출력 형식 | 단음절 변환에도 동일 적용 | 서식 조립이 `select_hanja` 한 곳이라 키보드·마우스·pull 경로 자동 동기 |
| Windows | TSF 구현 + cross-compile(`make check-windows`) 검증만. IMM32 **코드 무변경** — 코어 호스트 능력 플래그(`hanja_word_replace_capable`, 기본 **false**, §2.2.2)가 대상① 교체 경로를 끄므로 IMM32·`unim-capi` 는 종전 단음절 동작·바이트 동일 | 이 환경에서 런타임 불가. IMM32 는 `press_key` 뒤 commit/preedit 만 드레인하고 팝업 액션·교체 페이로드를 드레인하지 않는다(`unim-imm32/src/input.rs:110-112,:181`, `lib.rs:224`) — 플래그 없이는 RecentWord 확정이 "국" 을 소실시킨다 |
| 커밋 | 하지 않음(승인 후) | — |

---

## 2. 동작 명세

### 2.1 한자키 분기

한자키(`hanja_keys`, 기본 `Hanja`·`F9`)는 종전대로 `press_key()` 언어 분기 직전에서 처리한다(POPUP_SPEC §9.2 v3.2). 분기만 넓어진다.

v1.6 대상 우선순위(위에서 먼저 맞는 행): **조합 중 → 대상① / idle + 신뢰 가능한 선택 → 대상② / idle + 선택 없음 → 대상③ / 그래도 없으면 이모지.**

| 상태 | 앱 선택 영역 | 동작 |
|---|---|---|
| 한자 팝업 열림 | 무관 | 대상①(`RecentWord`/`WordBuffer`)이고 target 이 2자 이상이면 **접미 1자 축소**(§2.5.5, [Q7](a)). 대상③(`SurroundingWord`)이면 **다음 후보**(§2.4b.4), 마지막 후보 다음이면 이모지 팝업으로 전환. 그 외(길이 1·대상②)는 현행(push: 팝업 미지원 키 재처리 → 취소 후 idle 재판정 / pull: 같은 팝업 재발행) |
| 조합 중(preedit 있음) | 무시 | 대상① 시도(§2.2, §2.3). 다음절 일치가 없으면 종전(마지막 음절) — 바이트 동일. 대상③ 은 보지 않는다 |
| idle | 있음(신뢰 등급 충족·신선) · 사전 정확 일치 한글 | 대상② 팝업(§2.4) |
| idle | 있음(신뢰 등급 충족·신선) · 불일치/비한글/19자 이상/공백만 | **이모지 팝업**([Q8](a)). 대상③ 으로 가지 않는다 — 선택이 있는 채 확정하면 앱이 선택까지 치환한다. 선택 텍스트는 건드리지 않는다(§4.5) |
| idle | 보고된 선택 있음 · 낡음(§2.4b.6) | **이모지 팝업** — 낡은 선택으로 대상을 잡지 않는다 |
| idle | 없음 · 대상③ 조건 충족(§2.4b.1) | **대상③ 팝업**(v1.6) |
| idle | 없음 · 대상③ 불가(커서 양옆이 한글 아님·사전 일치 없음·거부 플래그·낡음·능력 미선언·surrounding 없음) | 이모지 팝업(종전 v3.2) |

pull 경로(Qt·XIM 의 `GetHanjaCandidates`)는 `start_hanja_conversion` 을 직접 부르므로 같은 판정을 거친다. 후보가 없으면(idle 불일치 포함) 두 프런트는 특수문자 → `ProcessKey` 폴백으로 위 표에 도달하므로 push/pull 결론이 일치한다. 대상③ 후보 소진 뒤 pull 폴백이 대상③ 을 다시 시작하지 않도록 엔진이 소진 표시를 남긴다(§2.4b.4).

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
- 통과 조건: `before` 가 `prefix` 로 끝남(GTK/Qt/Wayland/GNOME: surrounding 은 preedit 을 포함하지 않는다) **또는** `before` 가 **비어 있지 않고** `prefix` 보다 짧으며 그 접미임(앱이 잘라 보고한 경우). **TSF 선언 또는 관측 학습**(v1.5)일 때만 추가로 `prefix+pre` 로 끝남 / 그 잘림 접미(조합 텍스트가 문서 안에 있다 — 잘림 접미 절은 `before` 가 `pre` 보다 **길 때만**: `before == pre` 는 앱이 preedit 만 보고한 것(문단·필드 처음)이라 recent 가 문서에 없고 `full` 의 접미로서 공허 통과하기 때문) — 엔진 플래그 `surrounding_includes_preedit: bool`(기본 false, TSF 가 `set_surrounding_text` 호출 옆에서 true) **또는** 학습 상태 `learned_includes_preedit`(아래) 가 true 일 때만 이 두 절을 평가한다. 프런트 구분 없이 `prefix+pre` 절을 허용하면 Wayland/GNOME 클릭 드리프트(버퍼 "대한민" 잔류 → 문서의 기존 "…대한민국" 뒤 클릭 → "국" 입력)에서 `before`="…대한민국" 이 `full` 로 끝나 검증이 뚫리고 확정 시 '한민국' 이 지워진다(Wayland 는 클릭 시 Reset 을 보내지 않으므로 검증이 유일한 안전망).
- **preedit 포함 관측 학습(v1.5, 엔진 `learned_includes_preedit`)**: LibreOffice(VCL 편집기)는 text-input surrounding 에 preedit 을 포함해 보고한다(들쭉날쭉 — 어떤 보고는 미포함). 선언 플래그가 없는 Linux 경로에서 `before` 의 `prefix+pre` 절이 막혀 대상이 1자로 강등되던 문제를, `set_surrounding_text` 가 **저장값은 그대로 두고 관측만** 해 학습한다. 증거 = `cursor==anchor` 인 조합 중 보고 두 건이 ① 중간 확정 없이(`recent_*` 변경 시 관측 해제) ② preedit 끝 글자가 서로 다르고 ③ 각 `before` 가 그 preedit 으로 끝나며 ④ `before` 에서 preedit 을 뺀 앞부분과 그 길이가 같다. preedit 미포함 앱은 preedit 이 바뀌어도 `before` 가 변하지 않으므로 오탐이 없고, 모아치기처럼 끝 글자가 같은 변화는 증거가 아니다. 학습은 FocusIn·엔진 재생성(FocusOut·Reset)에서 해제되고 `reset()` 에서는 보존(관측만 비움)한다. 보강: 학습 모드에서 조합 중이 아니고 `cursor==anchor` 인 보고의 `before` 가 최근 버퍼로 끝나지 않으면 `recent_clear` — 늦은 보고 드리프트로 엉뚱한 글자를 지우지 않는다(실패 방향 = 1자). 통과 조건의 `before`-끝 일치 절은 학습 후에도 유지(preedit 빠진 보고 통과). 백스페이스 판정·`selection_target`·typefix 는 저장값만 읽으므로 영향이 없다. 로그는 길이·개수·이유 코드만(§2.8).
- surrounding 이 비어 있으면(XIM·미지원 앱) 검증 없이 통과. surrounding 은 있는데 `before` 가 비어 있으면(커서가 0·줄 첫머리, 줄 단위로 보고하는 위젯) **실패** — 빈 문자열은 모든 문자열의 접미이므로 잘림 절을 공허하게 통과시키면 안 된다(줄 첫머리에서 앞 줄 끝·개행을 지우는 사고).
- 커서 오프셋이 문자 길이를 **넘으면 검증 실패**(fail-closed). 정상 문자 오프셋은 절대 길이를 넘지 않으므로 초과는 단위 불일치(GNOME 확장은 Mutter 오프셋을 단위 변환 없이 전달, Qt focus-in 은 UTF-16 단위)의 증거다 — 클램프로 삼키면 "그럴듯한 오답" 이 된다(§2.4.1). 오프셋은 검증 없는 IPC 입력이라 패닉 방지는 유지한다(슬라이스 전 범위 검사).
- 실패 시 종전(마지막 음절)으로 **조용히 퇴화** — 안전한 방향으로만 실패한다.

### 2.3 대상① — 단어 확정 모드(`commit_unit=Word`, word-mode 앱)

- 앱에 나간 글자가 없으므로 확정 접두 0. `preedit` 전체의 최장 사전 접미를 target 으로, 접미 앞의 나머지("오늘"+"대한민국" 의 "오늘")는 커밋 접두로 보존해 확정 시 **접두+한자를 한 번에 커밋**한다(부분 커밋 API 신설 없음).
- 일치가 없을 때의 폴백(마지막 음절)에서도 접두를 보존한다. **종전 결함 수정**: 현행은 preedit 이 2자 이상일 때(단어 모드의 누적 preedit, 음절 모드의 모아치기 preview 등) 마지막 음절만 바꾸거나 취소하면 마지막 글자 앞 preedit 이 사라진다(`candidates.rs:150-156` 전체 clear + `popup_dispatch.rs:228` target 만 복원). 개정 후는 확정 시 접두+한자, 취소 시 preedit 전체 재커밋 — 따라서 "종전과 바이트 동일" 불변식은 **preedit 1자**일 때만 성립한다(§2.6).

### 2.4 대상② — 선택 영역

#### 2.4.1 판정

`SetSurroundingText(text, cursor, anchor)` 로 받은 값에서 **`max(cursor, anchor)` 가 문자 길이를 넘으면 선택 없음으로 거부**(로그 1회, 패닉 없음)하고, 그 외 `min < max` 이면 선택이 있다. 오프셋이 텍스트 길이를 넘는 입력은 실재한다 — GNOME 확장은 Mutter 오프셋을 단위 변환 없이 전달하고 Qt focus-in 은 UTF-16 단위를 보낸다. 범위 검사 없이 슬라이스하면 `a > b` 패닉으로 단일 워커 스레드가 죽어 모든 컨텍스트가 멈추지만, **클램프로 삼켜서도 안 된다**: 바이트 오프셋 "대한민국" 의 "대한" 선택 = (0,6) 을 (0,4) 로 클램프하면 target "대한민국" 팝업 → 확정 "大韓民國" 이 실제 선택 "대한" 만 치환 → "大韓民國민국". 초과 자체가 단위 불일치의 증거이므로 거부가 맞다(GNOME 실측에서 바이트로 확인되면 확장 쪽에서 문자 단위로 변환). `typefix_convert` 의 슬라이스(`surrounding.rs:197-199`, 현행 `end.min(len)` 클램프)도 같은 판정으로 통일한다. 선택 문자열의 앞뒤 공백을 제외한 부분이 **(a) 비어 있지 않고 (b) 전부 완성 음절이며 (c) 18자 이하이고 (d) 사전에 정확히 있을 때**만 그 문자열을 target 으로 삼는다. 하나라도 어긋나면 선택이 없는 것과 같이 종전 idle 동작(이모지 팝업)으로 폴백한다(§2.1, [Q8](a)).

#### 2.4.2 확정

확정 접두 0 인 일반 커밋(`CommitText`/`commit_buffer`). 위젯이 선택 영역을 커밋 문자열로 치환한다(GtkText·QLineEdit 은 툴킷 소스로 확인, 그 밖의 위젯은 실측 항목 §6.5).

#### 2.4.3 공백 보존 [Q3]

위젯은 **선택 전체**(공백 포함)를 치환하므로, 판정에서 제외한 앞뒤 공백을 커밋 문자열에 되붙인다. `" 대한민국 "` 선택 → `" 大韓民國 "`.

#### 2.4.4 조합 중이면 선택 무시

preedit 이 있으면 대상① 이 우선한다(모호성 제거).

#### 2.4.5 순서 불변식 (v1.4)

**선택은 한자키를 처리하기 전에 엔진에 반영되어야 한다.** 엔진은 한자키 시점의 `SetSurroundingText` 값만 읽는다. 프런트가 선택(anchor)을 `SetSurroundingText` 로 보낸 직후 `GetHanjaCandidates`/`ProcessKeyEvent` 를 부르는데 두 호출이 엔진 채널에 **도착 순서대로** 들어가지 않으면 낡은 선택으로 대상이 잡힌다(2026-10-05 실측: GTK4 에서 앱 선택 "서사" 의 target 이 낡은 부분 선택으로 "사" 1자가 되어, 확정 시 앱 선택 "서사" 전체가 치환되며 '서' 가 유실됐다). 이를 두 층으로 보장한다.

- **데몬**: `org.atit.unim.InputContext` 인터페이스는 메서드별 태스크를 만들지 않고 **직렬로 처리**한다(zbus 4.4 의 반전된 `spawn` 속성 — `unim-dbus/SPEC.md` §4). 같은 연결에서 응답 없이 보낸 `SetSurroundingText` 가 이후 호출보다 먼저 엔진 채널에 들어간다.
- **GTK3 프런트**: 한자키 직전 복원은 `SetSurroundingText` 를 **동기**(`g_dbus_connection_call_sync`)로 보낸다. 실패하면 선택 복원을 포기하고 종전 동작으로 간다.
- (v1.6) 대상③ 의 힌트(`SurroundingHint`)·능력(`SetClientCapabilities`)도 같은 InputContext 직렬 처리에 기대어 한자키보다 먼저 엔진에 반영된다(§4.6).

#### 2.4.6 선택 출처 신뢰 등급 (v1.6, [Q12])

| 등급 | 출처 | 경로 | 대상② | 거부 플래그(§2.4b.7) |
|---|---|---|---|---|
| **App-pull** | 한자키 시점에 앱에 물어 받은 anchor | GTK4 모듈(매 키 `retrieve-surrounding` → GtkText·GtkTextView 의 `set_surrounding_with_selection`), Qt5/6(`ImAnchorPosition` 질의), TSF(`ITfContext::GetSelection`) | 사용 | 판정에 쓰지 않음 — 선택이 있으면 anchor 로 보고된다 |
| **Widget** | 같은 프로세스 위젯 직접 조회 | GTK3 모듈 GtkEditable·GtkTextView(§2.10) | 사용 | 쓰지 않음 |
| **App-push** | 앱이 text-input-v3 로 밀어 보낸 anchor(컴포지터 경유, 지연·누락 가능) | GNOME 확장, 순수 Wayland | **신선할 때만**(§2.4b.6) | **씀** — GTK3·LibreOffice·Firefox 는 선택이 있어도 anchor=cursor 를 보낸다(gtk-3-24 imwayland.c:893-901, LO gtkframe.cxx:6068-6084, Firefox IMContextWrapper.cpp L2026-2046) |
| ~~Guess~~ | PRIMARY 내용으로 추정한 선택 | (v1.2~v1.5 GNOME 확장·GTK3 폴백) | **퇴역 — 쓰지 않는다**(§2.10.1) | — |
| 없음 | surrounding 자체가 없음 | XIM, X11 Chromium/Electron GTK IM(`SetSurroundingText(){}` 빈 함수), IMM32, 터미널 | ✘ | — |

- 등급은 와이어에 싣지 않는다. 엔진은 컨텍스트의 **능력 비트**(§4.6 `SetClientCapabilities`, TSF 는 엔진 setter)로 안다: `SELECTION_RELIABLE` 이 있으면 App-pull/Widget, 없으면 App-push. 능력을 선언하지 않은 컨텍스트(구 프런트·외부 클라이언트)는 대상③ 이 꺼지고 대상② 는 v1.5 와 같다(바이트 동일).
- GTK3 모듈은 포커스 위젯이 GtkEditable·GtkTextView 가 아니면(직접 조회 불가) 한자키 직전에 `SurroundingHint(SELECTION_UNKNOWN)` 을 보내 **그 한자키에 한해** 거부 플래그를 세운다 — 그 위젯의 선택은 IM 이 모른다.
- 출처를 와이어에 싣는 통합 계약(`Snapshot{text, cursor, anchor, sel_src, preedit}` + 단일 `Replace` 명령)은 v2 메모(§7).

### 2.4b 대상③ — 커서가 걸친 단어 (v1.6, [Q10])

선택 없이 커서만 단어에 두고 한자키를 누르면 그 단어를 바꾼다. 선택 방식(nav + Shift+←×k + 한자키 + 숫자 = **nav + k + 2** 키)보다 단어마다 k키(보통 2~4)가 줄어 **nav + 2** 키가 되고, 마우스 드래그도 필요 없다. 가장 긴 일치가 원하는 것보다 길면 한자키를 더 누른다(§2.4b.4).

#### 2.4b.1 발동 조건
한자키를 눌렀을 때 아래가 **모두** 참일 때만 대상③ 을 찾는다. 하나라도 거짓이면 이모지(§2.1).

1. idle — preedit 없음·조합 중 아님·팝업 없음(팝업 중 한자키는 §2.4b.4).
2. 컨텍스트 능력: `REPLACE_SURROUNDING` 이 있고, `SELECTION_RELIABLE` 또는 `POINTER_HINTS` 가 있다(§4.6). TSF 는 엔진 setter 로 `REPLACE_SURROUNDING | SELECTION_RELIABLE`.
3. surrounding 이 있다(`surrounding_seen`, 비어 있지 않음). 오프셋이 범위 안이다(§2.4.1 의 초과 거부 규칙 그대로). 선택이 없다(`cursor == anchor`).
4. 신선하다 — `surrounding_stale == false`(§2.4b.6).
5. 거부 플래그가 없다 — `selection_possible == false`(§2.4b.7). App-pull·Widget 컨텍스트는 이 조건을 보지 않는다(단 그 한자키에 `SELECTION_UNKNOWN` 이 왔으면 본다).
6. 비밀번호/PIN 이 아니다(§2.8 — 차단 중에는 surrounding 자체를 비운다).
7. 후보가 1개 이상이다(§2.4b.2).

#### 2.4b.2 구간 추출과 후보 집합
- `text` = surrounding(문자 단위), `c` = 커서.
- **구간** `[s, e)`: `c` 에서 왼쪽으로 한글 완성 음절(U+AC00–D7A3)이 이어지는 데까지가 `s`, 오른쪽으로 이어지는 데까지가 `e`. 자모·공백·문장부호·숫자·영문·한자·이모지·줄바꿈에서 멈춘다. `s == e`(커서 양옆이 모두 한글 음절이 아님)면 대상③ 없음 → 이모지. 앱이 surrounding 을 잘라 보고했으면 잘린 끝에서 멈춘다(보이지 않는 글자는 후보에 넣지 않는다 — 실패 방향은 더 짧은 후보).
- **후보**: `s ≤ i ≤ c ≤ j ≤ e`, `1 ≤ j − i ≤ 18`(`HANJA_MAX_KEY_CHARS`), `text[i..j]` 가 사전에 있는 모든 `[i, j)`. 곧 커서를 포함하거나(`i < c < j`) 커서에 닿는(`j == c` 또는 `i == c`) 사전 단어다. 1음절도 후보다(단음절 사전 항목).
- **LibreOffice 방어**: `c == 0` 이면 커서 뒤에만 있는 후보(`i == c`)를 버린다 — 곧 `c == 0` 에서는 대상③ 이 없다. LibreOffice(VCL gtk3/gtk4)는 선택이 있으면 surrounding 을 "선택 문자열, cursor 0" 으로 보고하고(edtwin.cxx:6952-7004), 선택이 있을 때 `DeleteSurroundingText` 를 거부한다(edtwin.cxx:7044). 이때 대상③ 을 확정하면 삭제는 무시되고 커밋이 선택 전체를 치환해 글자가 사라진다. 대가: 문단·입력란 맨 앞에 커서를 두고 바로 뒤 단어를 바꾸는 경우만 막힌다(단어 끝으로 커서를 옮기면 된다).
- 최근 확정 음절 버퍼(`recent_syllables`)는 쓰지 않는다 — 대상은 surrounding 에서만 나온다.

#### 2.4b.3 후보 순서 (확정)
정렬 키(앞 기준이 우선):

| 순위 | 기준 | 비고 |
|---|---|---|
| 1 | 길이 내림차순 | 가장 긴 사전 단어가 첫 팝업 |
| 2 | 같은 길이면 커서에서 **끝나는** 것(`j == c`) | 방금 친 단어·커서 앞 단어 — ibus-hangul·fcitx5-hangul·macOS 의 "커서 앞 단어" 관례 |
| 3 | 같은 길이면 커서에서 **시작하는** 것(`i == c`) | |
| 4 | 같은 길이면 커서를 **가로지르는** 것(`i < c < j`), 그 안에서는 시작이 왼쪽인 것(`i` 오름차순) | |

예(**실제 사전 `src/data/hanja.txt` 기준**, 2026-10-12 확인 — 국어교육·국어·교육·**어교**(魚膠)·국·어·교·육·대한민국·대한·민국·**한민**·대·한·민 이 있고 국어교·어교육·대한민·한민국 은 없다. L1 시험(§6.1)이 실제 사전으로 이 표를 단언하므로 사전을 바꾸면 이 표와 매뉴얼 §4.2·FAQ Q36·POPUP_SPEC 규칙 11 예를 함께 고친다):

| 문서와 커서 | 후보 순서 — 한자키 재타마다 다음 | a / b |
|---|---|---|
| `국어교육│` | 국어교육 → 교육 → 육 → (이모지) | 4/0 → 2/0 → 1/0 |
| `국어│교육` | 국어교육 → 국어 → 교육 → 어교 → 어 → 교 → (이모지) | 2/2 → 2/0 → 0/2 → 1/1 → 1/0 → 0/1 |
| `국│어교육` | 국어교육 → 어교 → 국어 → 국 → 어 → (이모지) | 1/3 → 0/2 → 1/1 → 1/0 → 0/1 |
| `대한│민국` | 대한민국 → 대한 → 민국 → 한민 → 한 → 민 → (이모지) | 2/2 → 2/0 → 0/2 → 1/1 → 1/0 → 0/1 |
| `│국어교육` (c=0) | 대상③ 없음 → 이모지 | LibreOffice 방어 |
| `국어교육 │` | 대상③ 없음 → 이모지 | 커서 앞이 공백, 뒤가 끝 |
| `abc│국어` | 국어 → 국 → (이모지) | 0/2 → 0/1 |

#### 2.4b.4 한자키 재타 — 다음 후보, 소진하면 이모지
- 대상③ 팝업에서 한자키(Ctrl/Alt/Super 없음)를 다시 누르면 §2.4b.3 순서의 다음 후보로 팝업을 갱신한다(target·a·b·즐겨찾기 키가 함께 바뀐다). 대상① 의 접미 축소([Q7](a), §2.5.5)를 대상③ 에서는 이 순환이 대신한다.
- 마지막 후보에서 한 번 더 누르면 한자 팝업을 닫고(커밋 없음) **이모지 팝업**을 연다. 처음 후보로 돌아가지 않는다 — 한자키만으로 이모지에 닿는 길을 남긴다.
- 후보 목록은 첫 한자키 때 한 번 계산해 팝업이 닫힐 때까지 고정한다.
- pull 경로(Qt 하드코딩 F9 → `GetHanjaCandidates`)도 같은 순서로 다음 후보를 돌려준다. 소진하면 빈 후보를 돌려주며 엔진이 `surrounding_word_exhausted` 를 세워, 이어지는 `ProcessKey` 폴백 한자키가 대상③ 을 다시 시작하지 않고 이모지로 가게 한다. 이 표시는 그 다음 키 처리·surrounding 수신·포커스 변화에서 내린다.

#### 2.4b.5 확정·취소
- **확정**: 커밋 문자열 = `서식(target, 한자)`(§2.6). 구간에 공백이 없으므로 대상② 의 공백 되붙임은 없다. 커서 기준 **앞 a자·뒤 b자 삭제 + 커밋** 을 한 묶음으로 요구한다 — 데몬은 시그널 `ReplaceSurroundingText(a, b, text, expect)`(§4.6)를 popup-owner path 로 발행하고, TSF 는 문서 범위 `[c−a, c+b)` 를 치환한다(§4.4). **앱의 선택 치환에 기대지 않는다** — "선택이 없다" 를 전제로 한 명시 삭제이며, §2.4b.6·§2.4b.7 이 그 전제를 지킨다.
- 프런트가 삭제 실패를 알 수 있으면(GTK `gtk_im_context_delete_surrounding` 이 FALSE, TSF 범위 획득 실패) **커밋하지 않는다** — 문서 무변경, 로그 `reason=delete_failed`. AutoTypeFix 의 XTest BackSpace 폴백은 대상③ 에 쓰지 않는다(커서 뒤 b자를 지울 수 없고, 폴백이 필요한 앱은 surrounding 도 믿기 어렵다). Qt(`setCommitString` 의 치환 범위)·GNOME(`Clutter.InputMethod.delete_surrounding`)·Wayland 는 실패를 알 수 없다 — 잔여 위험(§2.10).
- **프로토콜별 실패 의미(v1.6 검토 보강)** — "실패를 안다" 의 범위가 다르다:

  | 경로 | 삭제 단위·범위 | 알 수 있는 실패 | 알 수 없는 실패의 결과 |
  |---|---|---|---|
  | GTK3/GTK4 `gtk_im_context_delete_surrounding(ctx, -a, a+b)` | 문자, 커서 기준 offset·n_chars. 커서 뒤 삭제 가능 | 위젯 핸들러가 없거나 FALSE 반환(LibreOffice VCL 은 선택이 있으면 FALSE — edtwin.cxx:7044) | GtkEntry·GtkText·GtkTextView 는 범위를 다시 검사하지 않고 TRUE 를 반환한다 — TRUE 는 "처리함" 이지 "a+b 자를 정확히 지움" 이 아니다. 신선도 게이트(§2.4b.6)와 매 키 pull 이 전제 |
  | Qt `QInputMethodEvent::setCommitString(text, -a, a+b)` | UTF-16 단위(완성 음절은 BMP 라 1자=1단위), 커서 기준 | 없음(이벤트에 반환값 없음) | 치환 범위를 무시하는 사용자 위젯이면 원문 뒤에 한자가 **중복 삽입**(유실 아님). LibreOffice qt VCL 은 `deleteReplacementText` 결과를 확인만 하고 커밋은 그대로 한다 — 선택이 있으면 anchor 로 보고되므로(App-pull) 대상③ 에 오지 않는다 |
  | GNOME `delete_surrounding(-a, a+b)` → mutter → text-input-v3 `delete_surrounding_text(before, after)` | mutter 가 **자기 캐시 surrounding** 으로 문자→바이트 변환, 앱 툴킷이 다시 바이트→문자 | 없음(text-input-v3 에 결과 이벤트 없음) | 앱 쪽 변환이 어긋나면 과소·과다 삭제. 과다 삭제는 글자 유실이므로 **§6.5 (10) 실측 전에는 GNOME 경로 대상③ 을 켜지 않는다**(GSettings `surrounding-word-replace`, 기본 false — 실측 절차는 §6.5 (10) 아래 "실측 절차"). GNOME 의 AutoTypeFix 는 `delete_surrounding` 이 아니라 **가상 키보드 BackSpace 연타**(`vkbd.backspaceMultiple`)라서 이 경로의 실측 기록으로 **갈음할 수 없다** — `delete_surrounding` 자체를 별도로 실측해야 한다 |
  | 순수 Wayland `zwp_input_method_v2.delete_surrounding_text(3a, 3b)` + `commit_string` + `commit` | 바이트, 완성 음절 3B 고정 | 없음 | 보류(§2.10) |
  | TSF `ITfRange` `ShiftStart(−a)`·`ShiftEnd(+b)` → `SetText` | 문자(UTF-16) | Shift 가 돌려준 실제 이동량이 a·b 와 다름, 범위 획득 실패 | — (아래 검증으로 막는다) |
  | XIM·IMM32 | — | — | 대상③ 없음 |

- **TSF 확정 전 내용 대조**: TSF 는 엔진과 같은 프로세스라 target(한글)을 안다. 편집 세션 안에서 범위 `[c−a, c+b)` 의 텍스트를 읽어 target 과 다르면 커밋하지 않는다(`reason=verify_mismatch`). 프로토콜 변경 없이 되는 fail-closed 검사다.
- **DBus 프런트의 내용 대조(채택·구현)**: `ReplaceSurroundingText` 는 지울 한글 원문 `expect_text: s`(항상 비어 있지 않음)를 싣는다. pull 프런트(GTK3/4 모듈·Qt)는 시그널 처리 시점에 surrounding 을 다시 물어(`gtk_im_context_get_surrounding`·`ImSurroundingText`) `text[c−a..c+b] == expect_text` 일 때만 삭제·커밋한다(`reason=verify_mismatch`) — 낡음 게이트가 놓친 커서 이동을 마지막에 잡는다. GNOME 확장은 mutter 캐시만 있어 대조하지 않는다(`expect_text` 는 받되 무시). 프런트는 시그널 페이로드 타입이 `(uuss)` 가 아니면(구버전 데몬 등) 타입 문자열만 로그하고 무시한다 — 문서 무변경.
- 키보드 확정(`ProcessKeyEvent`)·마우스 확정(`SelectHanja`) 모두 같은 채널을 쓴다. 응답 preedit 은 "" 이다(idle 이라 원래 없다).
- 확정 뒤: 한자 상태·후보 목록·`recent_syllables` 를 비우고, `surrounding_stale = true`(커밋했으므로), 거부 플래그 해제(팝업 확정), AutoTypeFix `KeystrokeBuffer` 폐기(§4.1 과 같은 이유).
- **취소**(Esc·`CancelHanja`·포커스 이탈·미지원 키): 재커밋 없음(`hanja_recommit = ""`), 문서 무변경. 미지원 키는 이어서 재처리한다.

#### 2.4b.6 신선도(낡음) 게이트
a·b 는 보고된 surrounding 의 커서 기준이다. 앱의 실제 커서가 보고 뒤에 움직였으면 엉뚱한 글자가 지워진다. 그래서 **마지막 surrounding 보고 뒤에 커서·텍스트를 바꿀 수 있는 사건이 하나라도 있었으면** 대상②·③ 을 쓰지 않는다(이모지).

| 낡음 설정(`surrounding_stale = true`) | 낡음 해제 |
|---|---|
| 엔진이 커밋을 냈다(일반 커밋·교체·취소 재커밋 모두) | `SetSurroundingText` 수신(값이 같아도) |
| 엔진이 소비하지 않고 앱으로 넘긴 키(수정자 단독 키 제외) | |
| `SurroundingHint` 의 KEY·POINTER_PLAIN·POINTER_SELECT 수신(PRIMARY_CHANGED·SELECTION_UNKNOWN 은 낡음 아님) | |
| FocusIn(새 입력란의 값이 아직 없다) | |

- pull 프런트(GTK3/4 모듈은 매 키 `retrieve-surrounding`, TSF 는 한자키 시점 읽기)는 한자키 직전에 새 값을 보내므로 늘 신선하다. **Qt 는 한자키 시점 재질의 결과를 변경 여부와 무관하게 보낸다**(현행 "변경 시만 전송" 이면 아무것도 바꾸지 않은 통과 키 뒤에 낡음이 남아 이모지로 떨어진다 — 안전 방향이지만 혼란).
- push 경로(GNOME 확장)는 앱이 보고를 늦추거나 빠뜨린다(§2.10 — Shift+Home 미보고, 마우스 선택 지연). 보고가 오지 않으면 낡음이 남아 이모지로 간다. 앱이 평이동·클릭 뒤에 곧바로 보고하는지는 §6.5 실측 항목이다(보고하지 않는 앱에서는 대상③ 이 드물게 뜬다).
- 낡음·거부 플래그·소진 표시는 컨텍스트 상태로 **엔진 재생성(Reset·FocusOut 재생성)을 넘어 이월**한다(입력 모드 보존과 같은 방식) — GNOME·GTK·Qt 는 클릭 때 Reset 을 보내므로 재생성에서 초기화하면 "클릭 → 한자키" 가 늘 이모지가 된다. 초기화는 FocusIn 에서만 한다.

#### 2.4b.7 거부 플래그 — "선택 가능성"
App-push 컨텍스트(GNOME 확장)에서는 앱에 선택이 있어도 anchor=cursor 로 보고될 수 있다(GTK3·LibreOffice·Firefox, §2.4.6). 그 상태에서 대상③ 을 확정하면 a/b 삭제와 별개로 커밋이 앱의 선택까지 치환해 글자가 사라진다. 그래서 선택이 생겼을 수 있는 사건 뒤에는 대상③ 을 쓰지 않고 이모지로 간다. 잘못 서도 이모지로 갈 뿐 글자는 잃지 않는다(fail-closed).

| 구분 | 사건 | 엔진이 아는 경로 |
|---|---|---|
| **설정** | Shift+탐색키(←→↑↓·Home·End·PgUp·PgDn, Ctrl+Shift+탐색 포함) | 엔진이 받은 키 / 확장 `SurroundingHint(KEY)` |
| 설정 | Ctrl+A | 같음 |
| 설정 | Shift+클릭 | 확장 `SurroundingHint(POINTER_SELECT)` |
| 설정(보강) | 마우스 드래그(눌린 채 4px 초과 이동)·더블/세 번 클릭(직전 눌림 500ms 이내) | 확장 `POINTER_SELECT` |
| 설정 | mutter PRIMARY `owner-changed`(`Meta.Selection`, `SELECTION_PRIMARY`) — **내용은 읽지 않는다** | 확장 `SurroundingHint(PRIMARY_CHANGED)` |
| 설정(보강) | FocusIn — 돌아온 창에 예전 선택이 남아 있을 수 있다 | 엔진 |
| 설정(그 한자키 한정) | GTK3 모듈이 포커스 위젯을 직접 조회할 수 없음 | GTK3 모듈 `SurroundingHint(SELECTION_UNKNOWN)` |
| **해제** | Shift 없는 탐색키(평이동 — Ctrl+←/→ 단어 이동 포함) | 엔진 / 확장 `KEY` |
| 해제 | 텍스트 입력(엔진이 커밋을 냄 — 앱은 선택을 그 입력으로 치환한다)·Enter·Tab·BackSpace·Delete·Ctrl+X/V/Z/Y | 엔진 / 확장 `KEY` |
| 해제 | 한자·특수문자·이모지 팝업 확정 | 엔진 |
| 해제 | Shift 없는 단일 클릭(캐럿만 놓음) | 확장 `POINTER_PLAIN` |
| 바꾸지 않음 | 수정자 단독 키, 한자키, 팝업 밖 Esc | — |

- App-pull·Widget 컨텍스트는 플래그를 판정에 쓰지 않는다(선택이 있으면 anchor 로 보고된다). `SELECTION_UNKNOWN` 만 그 한자키에 한해 본다.
- 사건 도착 순서가 뒤바뀌어(예: Shift+→ 의 PRIMARY 변경이 평이동 뒤에 늦게 도착) 플래그가 다시 서도 이모지로 갈 뿐이다.
- 판정은 엔진 한 곳(SoT)에서 한다. 확장은 키를 분류하지 않고 **우회한 키의 keyval·state** 와 **포인터 사건 종류**만 보고한다(§4.6) — 키 분류 표가 엔진과 확장에 두 벌 생기지 않게.
- 대상② 와의 관계: 신선한 App-push 선택이 보고돼 있으면 플래그와 무관하게 §2.1 의 선택 행(대상② 또는 불일치 이모지)으로 간다.

#### 2.4b.8 LibreOffice
- **경로별 정리**: GNOME 확장 경로(gtk3/gtk4 VCL, Wayland 세션 기본) = 대상③ ✔(아래 방어 아래). X11 세션의 GTK3 모듈 경로 = 문서 창이 GtkEditable·GtkTextView 가 아니라 `SELECTION_UNKNOWN` → **대상③ ✘**(이모지 — 대화상자의 GtkEntry 필드는 ✔). qt5/qt6/kf VCL = App-pull 이라 ✔. GTK3 모듈 경로는 `delete_surrounding` 이 선택 시 FALSE 를 돌려주고 `c==0` 방어도 있어 이론상 풀 수 있으나, 실측 전까지 막아 둔다(§0 결정 쟁점).
- 선택이 있으면 surrounding = "선택 문자열, cursor 0"(edtwin.cxx:6952-7004) → §2.4b.2 의 `c == 0` 방어로 대상③ 이 생기지 않는다. 선택을 만드는 키·포인터·PRIMARY 변경은 거부 플래그도 세운다(이중 방어). Calc 셀·Impress(editeng)는 키보드 선택을 PRIMARY 에 반영하지 않지만(impedit.cxx:1764-1767) 키 사건으로 플래그가 선다.
- preedit 포함 보고와 관측 학습(§2.2.3)은 조합 중의 일이라 idle 의 대상③ 과 겹치지 않는다. 학습은 대상① 용으로 그대로 둔다. 확정 직후의 늦은 보고(preedit 이 섞인 낡은 값)는 §2.4b.6 이 막는다 — 커밋으로 낡음이 서고, 새 보고가 와야 풀린다.
- 실측 대기: GNOME 경로에서 Writer 의 surrounding 이 선택·커서 변경 직후 곧바로 오는지(§6.5).

#### 2.4b.9 surrounding 이 없는 경로
터미널·XIM·X11 Chromium/Electron(GTK IM)·IMM32·VTE 는 대상③ 이 없다. 조합 중 대상①(방금 입력한 단어, 버퍼 기반 §2.2)만 있고 idle 한자키는 이모지(종전)다.

### 2.5 후보·팝업·즐겨찾기

#### 2.5.1 후보와 정렬 — 종전 유지
`hanja_dict.search(target)` 순서(사전 등장순) → 즐겨찾기 stable sort 우선 → 9개/페이지(`HANJA_PAGE_SIZE`). 동음 다중 항목(國家/國歌)은 각각 별개 후보.

#### 2.5.2 즐겨찾기 — 종전 유지
키는 `(target, 한자)` 문자열 쌍. 단어도 그대로("대한민국", "大韓民國"). 서식과 무관하게 한자 원문으로 저장하므로 출력 형식을 바꿔도 즐겨찾기는 유지된다. 단어 즐겨찾기는 정확히 같은 단어 팝업에서만 재사용된다(매뉴얼에 명시).

#### 2.5.3 다음절 후보의 뜻 표시 (선택) [Q5]
다음절 항목의 뜻은 거의 비어 있거나 표제어를 되풀이한다(`hanja.txt:57963` `대한민국:大韓民國:대한민국`, `:28636` `국가:國家:`). 9개 목록에서 國家/國歌 를 구분할 단서가 없으므로, 단음절 항목의 뜻(`국:國:나라 국`)으로 만든 역색인으로 글자별 뜻을 `·` 로 이어 표시한다(예: `나라 국 · 집 가`). 원본 뜻이 있고 표제어와 다르면 원본을 쓴다. 역색인 키 `(한자, 단어 속 음)` 이 두음법칙·다독음(예: `역사:歷史` 의 歷 은 단음절 표제어가 "력")에서 빠지면 **한자 단독 키(첫 등장 뜻)로 폴백**해 빈칸을 남기지 않는다. 사전(`dict.rs`)만 바뀌고 렌더러는 무수정.

#### 2.5.4 헤더 표시
헤더 `「대한민국」 → 한자` 는 문자열 연결이라 무수정. 8~18자 target(2,124건)에서 팝업 폭을 넘을 수 있어 세 렌더러(popup-service·GNOME·Windows)에 헤더 ellipsize 를 넣는다. Windows compact 한자 열은 고정 90px 이라 다글자 후보가 뜻 열과 겹치므로 실측 폭으로 바꾼다(compact 리스트는 설계서상 이미 가변 폭). **expanded 9×9 격자의 셀 폭은 고정 유지**(`popup-renderer-design.md:422` "popup 폭 고정 정책" — 페이지마다 폭이 요동하면 안 된다) + 셀 텍스트 `DT_END_ELLIPSIS`(전체 한자는 헤더가 담당, 설계서 :425 와 정합). 설계서 §3 은 동결 문서라 compact 열 폭 문구를 함께 갱신한다.

#### 2.5.5 팝업 중 한자키 재타 — 접미 축소 [Q7](a)

대상①(`RecentWord`/`WordBuffer`)로 연 한자 팝업에서 한자키를 다시 누르면 target 을 더 짧은 접미 가운데 사전에 있는 가장 긴 것(1자면 마지막 음절)으로 옮겨 팝업을 다시 띄운다 — "대한민국" → (한민국 없음) → "민국" → "국". 확정 접두 길이도 함께 줄며, 남은 접두는 원래 접두의 접미라 §2.2.3 검증을 그대로 만족한다. 대상③(v1.6)은 축소 대신 후보 순환(§2.4b.4)이다. 대상②(선택 전체 치환)·target 1자·Ctrl/Alt/Super 조합이면 축소하지 않고 현행 경로(push: 팝업 미지원 키 재처리 / pull: 같은 팝업 재발행)로 간다. push(`press_key`)·pull(`start_hanja_conversion`) 두 경로가 같다(`hanja_word.rs` `shrink_hanja_target`).

#### 2.5.6 단어 팝업은 1×9 목록 고정 (POPUP_SPEC v3.5 §3.7 규칙 12)

대상(target)이 2음절 이상이면(대상①·②·③ 공통) 9×9 확장 그리드를 쓰지 않는다. 대상③ 의 순환(§2.4b.4)으로 1음절 후보가 되면 그 팝업은 단음절이라 확장할 수 있다. 단어 후보는 다글자라 고정 폭 셀 격자에 맞지 않고 한 화면 9개로 충분하다. `.`(Period)·`TogglePopupExpand`·⊞ 클릭은 no-op(키는 소비)이고 `expand_visible=false` 라 세 렌더러가 ⊞ 를 그리지 않는다. 숫자 1~9·↑↓·페이지 이동·Home/End 는 compact 경로 그대로다. Q7(a) 접미 축소로 target 이 1자가 되면 새 단음절 `PopupState` 가 만들어져 확장이 되살아난다. 구현: `PopupState::hanja_word`(`new_hanja_with_top_row` 에서 `target.chars().count() >= 2`) — `toggle_hanja_expanded` 는 `hanja_word` 면 no-op, `view_model` 은 `expand_visible: !hanja_word`. 렌더러·DBus 시그니처·설정은 무변경.

### 2.6 확정

| 대상 | 커밋 문자열 | 채널 |
|---|---|---|
| 대상①(음절 모드, 확정 접두 N>0) | `서식(target, 한자)` | **교체**: `AutoTypefixApply(delete_chars=N, commit_text, preedit_text="")` — 응답 preedit "" 로 조합 음절을 먼저 지우고, 시그널로 접두 N자 삭제 → 커밋 |
| 대상①(단어 모드) | `커밋 접두 + 서식(target, 한자)` | 일반 커밋 |
| 대상② | `앞 공백 + 서식(target, 한자) + 뒤 공백` | 일반 커밋(위젯 치환) |
| 대상③ (v1.6) | `서식(target, 한자)` | **커서 기준 교체**: `ReplaceSurroundingText(a, b, text, expect)`(§4.6) — 앞 a자·뒤 b자 삭제 후 커밋. 삭제 실패를 알면 커밋하지 않는다(§2.4b.5). TSF 는 범위 `[c−a, c+b)` 치환 |
| 단음절(종전) | `커밋 접두 + 서식(target, 한자)` | 일반 커밋 — 서식이 `漢字` 이고 **preedit 이 1자**이면 종전과 바이트 동일. preedit 2자 이상은 §2.3 결함 수정으로 접두가 보존된다(의도된 변경) |

- 서식 조립 규칙: 한글 부분은 `target`(사전 키). 뜻은 넣지 않는다. 커밋 접두/접미는 서식 대상이 아니다.
- 확정 후 버퍼와 한자 상태는 모두 비운다.

### 2.7 취소·포커스 이탈

| 경로 | 동작 |
|---|---|
| Escape / 팝업 미지원 키 재처리 | `hanja_recommit` 을 커밋(대상①: "국" 또는 단어 모드 preedit 전체, 대상②·③: 없음) → 팝업 닫기 → 미지원 키는 이어서 재처리 |
| `CancelHanja` RPC(팝업 밖 클릭·Wayland deactivate 등) | 같은 텍스트를 데몬이 `CommitText` 로 발행 |
| `FocusOut`/`Reset` RPC | 엔진 재생성 전에 같은 텍스트를 캡처해 커밋 |
| TSF 포커스 이탈(`OnSetFocus`) | 조합 텍스트는 앱이 `OnCompositionTerminated` 로 문서에 남기므로 재커밋하지 않는다(중복 삽입 방지). 오버레이 조합 앱(`composition_unsupported`)에서는 옛 컨텍스트에 삽입을 시도하고 실패하면 로그. 한자 상태 정리는 `engine.reset()` 분기와 "전이 포커스 reset 스킵(조합 보존)" 분기 **양쪽 공통**으로 실행한다(`text_service.rs:1904-1930` 스킵 분기도 `popup_ipc.hide()` 는 실행하므로 엔진만 hanja_mode 로 남으면 다음 키가 팝업 dispatch 로 샌다). 스킵 분기·정상 앱은 `cancel_hanja` + 조합 텍스트 keep 확정(materialize), 재커밋·삽입 시도 없음 |

원칙: **이미 확정돼 앱에 있는 접두와 선택 영역은 절대 건드리지 않는다.**

### 2.8 비밀번호 게이트(fail-closed)

- 비밀번호/PIN 필드(`content_purpose.should_block_hangul()`)에서는 **진입 시** 버퍼와 기존 surrounding(`surrounding_text/cursor/anchor`)을 비우고, 이후 채우지도 저장하지도 않는다. 종전 `set_surrounding_text`(`surrounding.rs:71-76`)는 **새 값**만 거부·클리어하고 차단 분기(`:38-47`)는 잔류를 지우지 않는다 — 목적 통지보다 먼저 도착한 선택 스냅샷(Qt mid-focus 에코모드 전환 `:319-336`)이 남아 목적 게이트가 없는 `typefix_convert`(`:183`)에 노출된다. 차단 분기 첫 줄 정리에 surrounding 3필드 클리어를 포함한다.
- pull 경로(`GetHanjaCandidates`)는 `press_key` 의 영문 강제를 거치지 않으므로 `start_hanja_conversion` 첫머리에서 차단한다(종전엔 없던 게이트).
- 프런트 이중 게이트(Qt): `m_contentPurpose` 가 Password/PIN 이면 `update()` 재질의 결과 대신 `("",0,0)` 만 전송한다 — 엔진이 버리더라도 비번 평문을 DBus 로 실어 보내지 않는다(TSF `key_handler.rs:417 atf_active` 식 프런트 게이트 관례).
- 프런트·데몬 로그(v1.4): target·PRIMARY(v1.6 퇴역)·surrounding 의 **길이(코드포인트 수)·개수만** 남기고 텍스트 평문은 남기지 않는다 — GTK3·GTK4 공용 DBus 클라이언트(`GetHanjaCandidates`·`GetSpecialCharCandidates`·`HanjaCandidatesReordered`)·Qt 공용 클라이언트·GTK3 선택 복원 로그(`how=…, reason=…, sel=N자`)가 이에 따른다. 데몬 `SetSurroundingText` 로그는 엔진 워커가 처리한 뒤 길이(`len=N자`)만 찍는다. 같은 원칙을 GTK3/GTK4 모듈(`unim_mask` 가 `(N자)` 반환, 민감 필드는 `***`)·Qt5/Qt6 입력 컨텍스트·GTK/Qt 공용 클라이언트의 ProcessKeyEvent 결과·커밋·FocusOut/Reset·AutoTypeFix·CommitText 로그와 데몬 `service.rs` 의 ProcessKeyEvent·ChordIdleFlush·CancelSpecialChar·CommitEmoji·`top_row` 로그에도 적용했다. **잔여(알려진 항목)**: 엔진 코어(`src/input_engine/press_key.rs`·`candidates.rs`·`popup_dispatch.rs`, `src/hangul/*`)의 키별 추적 로그는 입력 자모·음절을 평문으로 찍는다(`UNIM_DEVELOP=1` 한정, 종전부터). 조합 추적 로그 전반의 재설계가 필요해 이번 범위에서 제외한다.
- **대상③ 로그(v1.6, 평문 금지)**: 한자키마다 한 줄 — 결정 이유 코드(`ok`·`composing`·`selection`·`stale`·`selpossible`·`nocap`·`nosur`·`norun`·`cursor0`·`nomatch`·`sensitive`·`exhausted` — 직전 재타로 후보를 소진한 뒤의 `ProcessKey` 폴백 한자키, 이모지로 넘어간다)와 **수치만**: 구간 길이(`run=N자`), 후보 수, 고른 후보의 길이·a·b(`len=4 a=2 b=2`), 거부 플래그를 세운 마지막 사건의 **종류**(`flag=shift_nav|ctrl_a|pointer_select|primary_changed|focus_in|sel_unknown`). surrounding·target·후보 문자열은 남기지 않는다. 확정 로그는 `a·b·커밋 길이·delete 결과`만. `SurroundingHint` 수신 로그는 종류와 keyval 만(키 이름은 탐색·편집 키에 한해 기록, 문자 키는 `char` 로 뭉갠다 — 비번 필드 밖이라도 타이핑 재구성이 되지 않게). GNOME 확장은 PRIMARY 를 **읽지 않으므로** PRIMARY 내용·길이 로그 자체가 없어진다(`owner-changed` 사실과 mime 개수만).
- 로그에는 target **길이만** 남긴다: 종전 `candidates.rs:37-42` `"한자 후보 발견: '{}' -> {} 개"` 는 target 을 평문으로 찍는데, target 이 최대 18자 타이핑 텍스트로 넓어지고 `UNIM_DEVELOP=1` 이면 `~/.unim-log/…` 파일에 영속한다(`logging.rs:82-88,:115-118`). XIM·터미널은 목적 통지가 없어 비번 조각이 남을 수 있다 → `'{}'` 를 `{}자` 로 바꾼다. 신규 로그("선택 단어 한자 불일치", "후보 없음")는 텍스트 없음 유지.
- 팝업이 열린 채 비밀번호 목적이 도착하면 팝업을 닫고 **재커밋하지 않는다**(비번 필드에 원문 재삽입 금지). XIM 은 목적 통지 자체가 없어 구조적 잔여(종전 preedit 노출과 동급).
  - 순서: 한자 상태 정리(버퍼 clear + 팝업 취소로 `korean_context`·`preedit_cache` 비움)는 `set_content_purpose` 차단 분기의 **첫 줄**이어야 한다. 종전 코드는 같은 분기에서 `flush_preedit()` 을 먼저 부르는데(`surrounding.rs:38-47`), 한자 팝업은 preedit 을 유지한 채 열리므로 정리가 그 뒤에 오면 "국" 이 `commit_buffer` 로 흘러 다음 키 처리에서 비번 필드에 실린다.
  - 데몬 배선: `SetContentType` 요청은 응답 채널이 없고 워커에는 시그널 발행 수단이 없으므로, 요청에 `oneshot` 응답(팝업을 닫았는지 bool)을 추가하고 `service.rs` 가 응답을 받아 `HidePopup` 을 발행한다(빈 텍스트 `redirect_commit_and_hide("")` = HidePopup 만). 이것이 없으면 후보 창이 비번 필드 위에 다음 팝업 RPC 때까지 남는다.

### 2.9 실패 피드백 (선택) [Q6]

선택은 했는데 사전에 없으면 팝업이 안 뜬다. 접근성 옵션 `toggle_announce_beep` 이 켜져 있을 때만 짧은 비프를 내고, 데몬 로그 `ENGINE "선택 단어 한자 불일치"` 를 남긴다. 기본 무음이라 회귀 0.

### 2.10 프런트엔드별 지원 매트릭스

(v1.6 개정) ✔ 지원 · △ 조건부 · ✘ 미지원. "능력" 은 §4.6 `SetClientCapabilities` 비트(R=`REPLACE_SURROUNDING`, S=`SELECTION_RELIABLE`, P=`POINTER_HINTS`).

| 프런트엔드 | 대상① (조합 중) | 대상② (선택) | 대상③ (커서가 걸친 단어) | 능력 | 비고 |
|---|---|---|---|---|---|
| GTK3 모듈 | ✔ | ✔ GtkEditable·GtkTextView 위젯 직접 조회(Widget, v1.4) | ✔ 같은 위젯 한정 / 그 밖의 위젯은 ✘(`SELECTION_UNKNOWN` → 이모지 — X11 세션의 LibreOffice 문서 창·Firefox·Thunderbird·WebKitGTK 가 여기에 든다, §2.4b.8) | R·S | `set_surrounding` 에 anchor 가 없어 **한자키 직전** `unim_push_selection_for_hanja` 가 위젯을 조회해 복원한다: `GtkEditable`(`gtk_editable_get_selection_bounds`)·`GtkTextView`(같은 줄 선택만, 줄 오프셋). 위젯 선택 문자열이 surrounding 부분 문자열과 다르면 포기. 복원값은 `SetSurroundingText` **동기 호출**(`unim_dbus_set_surrounding_text_sync`)로 보낸 뒤 Wayland(`ProcessKeyEvent`)·X11(`GetHanjaCandidates`) 두 분기가 이어진다(§2.4.5). **v1.4~v1.5 의 같은 프로세스 PRIMARY 폴백(`unim_primary_decide`)은 퇴역**(§2.10.1) — 직접 조회할 수 없는 위젯은 `SurroundingHint(SELECTION_UNKNOWN)` 을 보내 대상③ 을 막는다. 비밀번호/PIN 은 읽지도 보내지도 않는다. `unim->selection_index` 는 건드리지 않는다(선택 삭제 경로 오작동 방지). 대상③ 확정 = `gtk_im_context_delete_surrounding(-a, a+b)` → TRUE 일 때만 커밋 |
| GTK4 모듈 | ✔ | ✔ App-pull(매 키 `retrieve-surrounding`) | ✔ | R·S | 선택 삭제 래퍼 게이트(§4.5). 대상③ 확정은 GTK3 과 같고 `is_focused` 가 아니면 드롭(문서 무변경 — 안전 방향, §4.1 와 같은 가드). **잔여 위험 → 위젯 종류 거름 채택(구현)**: `S` 는 "위젯이 `set_surrounding_with_selection` 으로 anchor 를 준다" 는 가정이다. 폐기 예정 API `gtk_im_context_set_surrounding`(anchor=cursor)만 부르는 GTK4 위젯(서드파티·LibreOffice gtk4 VCL·GtkDrawingArea 등)은 선택이 있어도 선택 없음으로 보인다. 모듈은 두 경우를 구별할 수 없으므로 한자키 시점에 클라이언트 위젯이 `GtkText`·`GtkTextView` 계열이 아니면 `SurroundingHint(SELECTION_UNKNOWN)` 을 보내 그 한자키의 대상③ 을 막는다(이모지). 하니스 시나리오 `hanja-word-cursor-unknown-widget-gtk4`(GtkDrawingArea 코어 필드)가 이 경로를 고정한다. LibreOffice 는 `c==0` 방어로 이미 막힌다 |
| Qt5/6 | ✔ | ✔ App-pull(`ImAnchorPosition`) | ✔ | R·S | surrounding 을 `update(Qt::InputMethodQueries)` 시 갱신(커스텀 `hanja_keys` 포함, **빈 텍스트도 전송**, 비번 필드는 `("",0,0)` — §2.8) + 래퍼 게이트(§4.5). 한자키 시점 재질의만으로는 부족: `input_context.cpp:381` 은 F9·Hangul_Hanja 하드코딩 pull 경로라 다른 `hanja_keys` 는 `processKey`(:408) → 엔진 분기로 가는데, surrounding 은 focus-in 1회(:554-564, 비어 있으면 미전송)뿐이라 QLineEdit Tab 포커스의 전체 선택 스냅샷이 stale 로 남아 `has_selection()` 오판 → 커서에 엉뚱한 단어 삽입 또는 이모지·한자 모두 침묵. **v1.6: 한자키 시점 재질의 결과는 변경 여부와 무관하게 송신**(§2.4b.6). 대상③ 확정 = `QInputMethodEvent::setCommitString(text, -a, a+b)`(한글 음절은 BMP 라 UTF-16 1단위 = 1자). **`ImAnchorPosition` 질의가 무효(QVariant invalid — 위젯이 답하지 않음)면 그 한자키에 `SELECTION_UNKNOWN`** 을 보낸다(v1.6 검토 보강 — 답하지 않는 위젯의 선택은 IM 이 모른다. 현행처럼 anchor=cursor 로 채우면 `S` 가 거짓이 된다). LibreOffice qt5/qt6/kf VCL 은 `ImAnchorPosition` 을 정확히 준다(QtWidget.cxx L683-720) |
| GNOME 확장 — 앱이 anchor 를 주는 경우 (GTK4·Qt6·Chromium text-input-v3) | ✔ | △ App-push, 신선할 때만(§2.4b.6) | ✔ 거부 플래그·낡음 게이트 아래(삭제 글자 수 실측 §6.5 (10) 뒤 활성) | R·P | text-input-v3 → mutter → `UnimInputMethod`. 앱 선택 보고가 **불규칙하고 늦다**(2026-10-04 실측: Shift+Right 는 anchor≠cursor 로 보고, Shift+Home 은 미보고, 마우스 선택은 다음 클릭 때 직전 상태가 한 번 늦게, 더블클릭 선택은 끝내 미보고). 낡음 게이트가 이 지연을 막는다. mutter 의 `request_surrounding` 은 캐시를 되풀이할 뿐이라 IM 이 당겨 올 수 없다(L149-163). 대상③ 확정 = `Clutter.InputMethod.delete_surrounding(-a, a+b)` + `commit_text`(실패를 알 수 없음). 교체 전 preedit 선클리어(`UnimInputMethod.clearPreedit()`)·`SPEC.md` §2.6 은 대상① 교체에 계속 쓰인다. `onAutoTypeFix` 의 `_hasFocus` 게이트는 새 시그널 처리에도 같게 둔다(St 팝업 클릭 시 유지 여부는 §6.5) |
| GNOME 확장 — anchor 를 못 주는 앱 (GTK3·LibreOffice gtk3/gtk4 VCL·Firefox) | ✔ (LibreOffice 는 preedit 포함 관측 학습 §2.2.3) | ✘ — 선택이 있어도 anchor=cursor | ✔ 거부 플래그·낡음 게이트·`c==0` 방어 아래(§6.5 (10) 뒤 활성) | R·P | 이 앱들에서는 **문장 중간 선택 변환이 없다** — 커서를 단어에 두고 한자키(대상③). 선택한 채 한자키는 거부 플래그로 이모지. 정확한 선택이 꼭 필요해지면 AT-SPI 질의 PoC(§7) |
| 순수 Wayland (input-method-v2) | ✔ | ✔/✘ 앱에 따라(위 두 행과 같은 구분), App-push 신선할 때만 | **✘ 보류** | R | 바이트 판별(§4.2) + `SurroundingText` → `SetSurroundingText`. IM 이 포인터·PRIMARY 사건을 볼 수단이 없어 GTK3·LibreOffice·Firefox 앱의 마우스 선택을 거를 수 없다 → `P` 를 선언하지 못해 대상③ 이 꺼진다(§2.4b.1-2). ext/wlr data-control 의 primary 변경 신호를 IM 이 받을 수 있는지 실측(§6.5) 뒤 재검토. 확정 채널 구현 시 바이트 = a×3·b×3(구간은 전부 완성 음절) |
| XIM | ✔ | ✘(idle=이모지) — 선택 전달 수단 없음 | ✘ — surrounding 없음 | — | 교체 채널은 자가 주입 BackSpace N+1(`handler.rs` `PopupEvent::AutoTypeFix`)을 **한 번에 1개씩 직렬 주입**한다 — 일괄 주입하면 libX11 의 단일 FABRICATED 플래그 때문에 되돌린 BS 가 재필터돼 확정 뒤에 도착한다(2026-09 docker L3 실측 `대한大韓` → 직렬화로 해소). 교체 도착 시 preedit 표시 선클리어(마우스 확정 잔상 방지 — `handle_popup_event` 에는 `user_ic` 가 없으므로 `CommitText` 분기처럼 `last_focused_ic_info` 로 IC 를 재구성해 `server.preedit_draw(&mut ic, "")`, NOTHING/POSITION IC 는 PeWindow 숨김 경로. 구현 비용이 크면 선클리어를 포기하고 "마우스 확정 시 잔상 잔여(키보드 확정은 정상)" 로 비고 정정). 스팟 점프 Reset([Q9](a)) |
| Windows TSF | ✔ | ✔ App-pull(`GetSelection`) | ✔ | 엔진 setter R·S | cross-compile 검증만(§4.4). 엔진 생성 직후 `set_hanja_word_replace_capable(true)`·(v1.6) `set_surrounding_word_caps(R·S)`. 대상③ 은 `read_hanja_surrounding` 이 커서 **뒤**도 읽어야 한다(§4.4) |
| IMM32 | 대상① 미지원(단음절 유지) | ✘ | ✘ | — | 코드 무변경. 교체 채널·팝업 액션 드레인이 없으므로 코어 플래그 기본 false 로 종전 바이트 동일(§2.2.2 4). `unim-capi` 소비자도 동일 |
| X11 Chromium/Electron (GTK IM)·터미널(VTE 등) | ✔ (surrounding 없음 → 검증 없이 버퍼 기반) | ✘ | ✘ — surrounding 없음(`SetSurroundingText(){}` 빈 함수, input_method_context_impl_gtk.cc L322-326) | — | idle = 이모지(종전) |

- **선택 삭제 부작용 제거(v3.5, 유지)**: 이모지 팝업이 띄우던 ZWSP preedit(51d083b)가 GTK4 에서 preedit-start 로 선택을 지웠고 cursor-jump flush·COMMIT 모드 reset 으로 ZWSP 가 문서에 커밋되기도 했다 — 확장이 팝업 표시 시 preedit 을 건드리지 않는다. 시험이 정적으로 고정한다.
- **LibreOffice 실측(2026-10-06, 유지)**: Writer/Calc/Impress(VCL 편집기)는 surrounding 에 **preedit 을 포함**해 보고하고(들쭉날쭉 — 예: 확정 0자+preedit 'ㅅ' → cursor=1 len=1, '서' 확정+'사' → 2/2) Text Editor·Writer 대화상자 필드는 미포함 — 엔진이 관측으로 학습한다(§2.2.3).

#### 2.10.1 PRIMARY 경유 보강 퇴역 기록 (v1.2~v1.5 → v1.6, [Q11])

| 항목 | 도입 | v1.6 처리 |
|---|---|---|
| GNOME 확장 idle 한자키 PRIMARY **내용** 읽기(`St.Clipboard` PRIMARY, 비동기 150ms, 키 큐 재정렬) | v1.2 (2026-10-04) | **퇴역** — 읽지 않는다. 대신 `Meta.Selection` `owner-changed` 를 거부 플래그 신호로만(§2.4b.7) |
| 인접 판정(커서 바로 앞/뒤 정확 일치)·모호 거부 | v1.2 | 퇴역 |
| 유일 일치 보강(surrounding 안 정확히 한 번) | v1.3 | 퇴역 |
| 키보드 선택 길이 추적·대조(`kbdmismatch`) | v1.5 (2026-10-06) | 퇴역 — 키 사건은 엔진이 거부 플래그로 분류(§2.4b.7) |
| 가짜 anchor `SetSurroundingText` 송신 후 복원 | v1.2 | 퇴역 — 확장은 Mutter 값을 그대로 전달만 한다 |
| 읽기 실패 이유 코드(`noowner`·`nomime`·`xfer`·`empty`·`timeout`) | v1.5 | 퇴역(읽기가 없다) |
| GTK3 모듈 같은 프로세스 PRIMARY 폴백(`unim_primary_decide`, `gtk-common/src/unim_primary_selection.c`·`.h`) | v1.4 (2026-10-05) | 퇴역 — 위젯 직접 조회만 유지, 그 밖은 `SELECTION_UNKNOWN` |
| 시험 `scripts/test-gnome-primary-selection.mjs`, `tests/unit-c/test_primary_selection.c`(§6.1b) | v1.2·v1.4 | 퇴역 — 거부 플래그·포인터 분류 시험으로 대체(§6) |

퇴역 근거:
- **PRIMARY 는 선택 API 가 아니다.** LibreOffice Writer 는 선택을 시작할 때 한 번만 소유권을 잡고(sw select.cxx:407-423) 내용은 **처음 읽힌 시점 것으로 굳으며**(swdtflvr.cxx:496-532) GDK3 Wayland 캐시가 겹친다(gdkselection-wayland.c:1031-1041). Calc 셀·Impress 는 키보드 선택을 PRIMARY 에 반영하지 않는다(impedit.cxx:1764-1767). 실측된 "PRIMARY 가 실제 선택보다 짧아 확정 때 글자가 사라지는" 문제(Writer: End → Shift+Left×2 → 2초 뒤 한자키에 PRIMARY 1자·실제 선택 2자)가 이 구조로 설명된다 — 상태 머신을 더 보강해도 드래그·이전 내용 잔류·editeng 은 남는다.
- 업스트림 ibus-mozc 도 PRIMARY 감시를 제거했다(google/mozc#779, 2023-07).
- 대상③ 이 선택 없이 같은 일을 더 적은 키로 하므로, 앱이 IM 에 알리지 않는 선택을 **추정**할 이유가 사라졌다. 남는 위험(알리지 않은 선택이 실제로 있음)은 추정 대신 **거부**로 막는다.
- 퇴역 대상 코드는 전부 미커밋이다. 지우기 전에 브랜치나 패치로 백업한다(구현 단계 — 기현님 지시 시).

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
| `row_hanja_keys_subtitle` (v1.6 개정안) | 쉼표로 구분 (예: Hanja, F9) — 조합 중이면 어절/음절, 선택했으면 그 단어, 커서가 한글 단어에 걸쳐 있으면 그 단어를 한자로 변환, 그 외에는 이모지 팝업 | Comma separated (e.g. Hanja, F9) — converts the word/syllable being typed, the selected word, or the Hangul word at the cursor; otherwise opens the emoji popup |
| `row_hanja_keys_tooltip` (v1.6 개정안) | 한자/특수문자 팝업을 띄울 키. 예: Hanja, F9. 한글 조합 중: 방금 친 어절(예: 대한민국)이 한자 사전에 있으면 단어 후보, 없으면 마지막 음절 후보. 앱에서 한글 단어를 선택한 채 누르면 그 단어의 한자 후보. 선택 없이 커서가 한글 단어 끝·안·앞에 있으면 그 단어의 한자 후보(가장 긴 단어부터, 한자키를 다시 누르면 다음 후보). 커서 앞뒤가 한글이 아니면 이모지 팝업. | Keys that open the Hanja/special-char popup, e.g. Hanja, F9. While composing: word candidates for the word you just typed (e.g. 대한민국) if it is in the dictionary, otherwise last-syllable candidates. With a Hangul word selected: candidates for that word. With no selection and the cursor at, inside or before a Hangul word: candidates for that word (longest first; press again for the next). Otherwise: emoji popup. |
| CLI `help_ck_hanja_output_format` | 한자 출력 형식 (hanja, hangul-hanja, hanja-hangul). 漢字 / 한자(漢字) / 漢字(한자) | Hanja output format (hanja, hangul-hanja, hanja-hangul): 漢字 / 한자(漢字) / 漢字(한자) |
| CLI `hanja_output_format_label` / `_note` | 한자 출력 형식 / (단음절·단어 변환 공통) | Hanja output format / (applies to syllable and word conversion) |
| CLI `error_invalid_hanja_output_format` | 잘못된 한자 출력 형식: %{value}. 가능한 값: %{allowed} | Invalid hanja output format: %{value}. Allowed: %{allowed} |
| CLI `hanja_output_format_changed` | 한자 출력 형식을 '%{fmt}'(으)로 변경했습니다. | Hanja output format set to '%{fmt}'. |
| Slint `@tr("한자 출력 형식")` (title·accessible-label) | 한자 출력 형식 | Hanja Output Format |
| Slint `@tr(description)` | 漢字 · 한자(漢字) · 漢字(한자) — 한자 팝업에서 확정할 때 넣는 형식 | 漢字 · 한자(漢字) · 漢字(한자) — text inserted when you confirm a hanja candidate |
| Slint 옵션 라벨 | 코어 `display_name()`(한국어 리터럴) 그대로 — `commit_unit` 선례와 동일한 **알려진 갭**(영어 로케일에서 「한자만/한글(한자)/한자(한글)」 한국어 노출). .slint 쪽 `@tr` 3개 매핑은 선례 정정과 함께 별도 | — |
| Slint `@tr("한글을 한자로 변환할 때 사용할 키입니다.")` (한자 변환 키 row description, v1.6 개정안, `settings.slint:810` + `.po:244` msgstr) | 조합 중이면 어절/음절, 선택했으면 그 단어, 커서가 한글 단어에 걸쳐 있으면 그 단어를 한자로 바꾸고, 그 외에는 이모지 팝업을 여는 키입니다. | The key that converts the word/syllable being typed, the selected word, or the Hangul word at the cursor, and otherwise opens the emoji popup. |

GTK en 로케일 문자열은 `Hanja Output Format`(대문자 F) — CHANGELOG 영문 항목의 UI 이름도 이것과 일치시킨다.

**v1.6 문구 반영 범위**: 위 "v1.6 개정안" 3행은 구현 단계에서 GTK ko/en 로케일·Slint `.po`·사용자 매뉴얼 ko/en(§3.2 9 — 한자키 3분기 표를 4분기 "조합 중 / 선택 / 커서가 걸친 단어 / 이모지" 로, **커서가 걸친 단어 변환 튜토리얼 소절 신설**: 커서 두는 위치별 예시·재타 순환·이모지로 가는 법·선택 변환이 안 되는 앱 목록)·FAQ·루트 README·`make help-html`(§3.2 10)과 함께 바꾼다. 매뉴얼 문구도 조작 부담은 키 입력 수로 서술한다.

**Q1 승인 전 문구(대상② 서술 제외)**: `row_hanja_keys_*`(GTK)·Slint 한자 변환 키 description·CHANGELOG Added 1행 후반부·매뉴얼 선택 변환 소절·루트 README 선택 행은 대상②(선택 변환)를 확정 서술하므로 Q1 게이트에 묶인다. 승인 전에 U9/U14 가 착수하면 다음 2벌 문구 중 **대상① 판**을 쓴다 — GTK subtitle: "쉼표로 구분 (예: Hanja, F9) — 조합 중이면 방금 친 어절/음절 한자 변환, 그 외에는 이모지 팝업" / "Comma separated (e.g. Hanja, F9) — converts the word/syllable being typed, otherwise opens the emoji popup"; tooltip 은 위 개정 문구에서 "앱에서 한글 단어를 선택한 채 …" 문장을 뺀 것; Slint description: "조합 중이면 어절/음절 한자 변환, 그 외에는 이모지 팝업을 여는 키입니다." / "The key that converts the word/syllable being typed and otherwise opens the emoji popup." Q1 거절 시 GTK ko/en 로케일·Slint .po·CHANGELOG·매뉴얼이 존재하지 않는 기능을 안내하지 않도록 하는 장치다. GNOME 사용자는 prefs.js 리다이렉트로 Slint 앱만 보므로 Slint 행을 빼면 두 GUI 안내가 갈린다.

---

## 4. 교체 프로토콜

### 4.1 `AutoTypefixApply` 재사용 (Linux 데몬)

- 신규 시그널·신규 `PopupAction`·RPC 시그니처 변경은 **없다**(대상①·② 한정 — 대상③ 은 v1.6 에서 시그널 1·메서드 2 를 더한다, §4.6). `SelectHanja(u)->s` 는 그대로 서식 적용 문자열을 반환한다.
- 대상③ 은 `AutoTypefixApply` 를 쓰지 않는다 — 그 시그니처에는 커서 **뒤** 삭제 자리가 없고(`delete_chars` 는 커서 앞), 프런트의 XTest BackSpace 폴백이 대상③ 에는 맞지 않는다(§2.4b.5). 페이로드는 `HanjaReplacement` 에 `delete_after_chars`(기본 0)를 더해 구분한다: 0 이고 대상①이면 종전 `AutoTypefixApply`(바이트 동일), 대상③ 이면 항상 `ReplaceSurroundingText`.
- 키보드 확정(`ProcessKeyEvent`): 엔진이 확정 접두가 있을 때만 out-of-band 페이로드 `HanjaReplacement{delete_chars, preedit_chars, text}` 를 남기고, 데몬이 이를 기존 `EngineResponse.auto_typefix` 필드에 실어 응답 후 `AutoTypefixApply(delete_chars, text, "")` 를 발행한다. 응답의 preedit 은 "" (프런트가 조합 음절을 먼저 지운다).
- 마우스 확정(`SelectHanja` RPC): 데몬 내부 응답을 `SelectHanjaOutcome{None, Commit(text), Replace{delete_chars,text}}` 로 구분해 `Replace` 면 `CommitText` 대신 `AutoTypefixApply` 를 popup-owner path 로 발행하고 `HidePopup` 을 잇는다.
- 상호배제: 팝업 확정 프레임에서는 AutoTypeFix 검사가 게이트로 건너뛰므로 두 교체가 같은 프레임에 겹치지 않는다. AutoTypeFix 모드 전환 시의 Global 카테고리 전파 블록은 한자 교체 프레임에서 발화하지 않도록 게이트한다.
- 소비 후 버퍼 폐기: ATF 블록(`engine_worker.rs:1338-1345`)은 `popup_action.is_none()` 게이트로 확정 프레임을 통째로 건너뛰어 `KeystrokeBuffer` 의 `push`/`update_on_commit`/`clear` 도 실행되지 않는다. RecentWord 확정 후 문서는 "大韓民國"(4자)인데 버퍼는 11키·`committed_chars=3`·`has_preedit=true` 로 남고 역방향 `delete_chars = committed_chars + has_preedit`(`src/auto_typefix/reverse.rs:13`) 가 이 stale 값을 쓴다 → 한자 교체 프레임(`ProcessKeyEvent` 의 `hanja_replaced`, `SelectHanja` RPC 경로 모두)에서 `keystroke_buffers.remove(&context_id)`(모드 전환 선례 `:1290`), TSF 는 `state.buf.clear()`(`auto_typefix.rs:221` 선례).
- 시그널 순서: 동기 RPC 응답(preedit "") → 메인루프에서 시그널 수신(AutoTypeFix 역방향과 동일).
- GTK4 `on_auto_typefix` 는 `!unim->is_focused` 면 드롭한다(`immodule.c:475`) — 종전 `on_commit_text`(:451-461) 에는 없던 조건. 마우스 확정(SelectHanja)을 교체 채널로 옮기면 팝업 클릭이 포커스를 뺏는 환경에서 교체가 조용히 드롭되고 HidePopup 은 이미 발행돼 "국" 이 유실될 수 있다. **프런트 우회는 넣지 않는다(현행 가드 유지)**: `preedit_text=="" && delete_chars>0` 는 역방향 AutoTypeFix 프레임과 같은 시그니처라(`engine_worker.rs:1713-1716` — preedit "" + delete>0 + ASCII commit) 우회가 역방향 ATF 의 포커스 가드까지 풀고, 포커스가 다른 창으로 넘어간 뒤 `delete_surrounding` 이 실패(Electron 등)하면 `:488-519` XTest BackSpace 폴백이 **현재 포커스 창**에 실제 키로 들어간다(현행 '조용히 드롭' 은 안전). popup-service 클릭이 GTK4 포커스를 뺏는 것으로 §6.5 실측에서 확인되면 popup-service 쪽(X11 override-redirect / layer-shell `keyboard_interactivity=none`)에서 고친다. 꼭 프런트 우회가 필요해지면 판별자를 한자 전용(commit_text 에 ASCII 도 한글도 아닌 문자 포함 — §4.2 와 동일)으로 두고 XTest 폴백 경로는 우회에서 제외한다. GNOME `onAutoTypeFix` 의 `_hasFocus` 게이트(`extension.js:284`)도 같은 성격 — St 팝업 클릭 시 유지 여부 실측(§6.5), 유지 안 되면 같은 원칙(렌더러 쪽 수정 우선).
- `SetContentType` 응답 채널(§2.8): `EngineRequest::SetContentType` 에 `response: oneshot::Sender<bool>` 추가, `service.rs set_content_type` 이 await 후 true 면 `redirect_commit_and_hide("")`. 프런트 쪽 DBus 시그니처는 불변(`SetContentType(u)` 호출 방식).
- AutoTypeFix 순방향 교정 뒤 버퍼 시드: 데몬 순방향 적용은 `engine.reset()` 뒤 마지막 음절만 replay 하므로(`engine_worker.rs:1555-1562`) 교정된 접두 "대한민" 이 버퍼에 없다. **replay 뒤 `engine.clear_commit()`(`:1600`) 직후**에 `engine.recent_clear(); for c in fix.commit_text.chars() { engine.recent_push_char(c) }` 로 시드한다 — 데몬 스스로 "replay 에서 commit 이 발생할 수 있다" 고 전제하고 `clear_commit` 을 부르므로, reset 직후·replay 전에 시드하면 replay 중 한글 commit 델타가 래퍼 (4) 로 시드 위에 추가 push 돼 surrounding 없는 XIM/Wayland 에서 검증 없이 과다 삭제될 수 있다. replay 뒤 시드는 replay 부작용과 무관하게 결정적이다. 영타→한글 교정 직후 한자키는 상용 경로라 v1 포함. Windows TSF 순방향(`unim-tsf/src/auto_typefix.rs` 비-word 분기)도 같은 위치(replay·`clear_commit` 뒤)에서 같은 시드를 한다 — 빠져 있어 교정 직후 한자키가 단음절로 퇴화하던 것을 2026-09-30 실기에서 확인해 보강.

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
- 대상③(v1.6): 엔진 생성 직후 `set_surrounding_word_caps(REPLACE_SURROUNDING | SELECTION_RELIABLE)`(두 생성 지점 모두 — §2.2.2 4 와 같은 이유). idle 한자키에서 `read_hanja_surrounding` 이 커서 **앞뿐 아니라 뒤도** 읽어야 한다(현행은 커서 앞 최대 4096 유닛, composition.rs:1763-1781 — 뒤로 최소 18자). 선택이 있으면 종전대로 선택(대상②). 확정은 문서 범위 `[c−a, c+b)` 를 한 편집 세션에서 `SetText` 로 치환한다. 범위 획득 실패, `ShiftStart`/`ShiftEnd` 의 실제 이동량이 a·b 와 다름, 범위 텍스트가 target 과 다름 가운데 하나라도 있으면 커밋하지 않는다(§2.4b.5). CUAS·오버레이 조합 앱은 idle 이라 조합 문제가 없다. 런타임은 VM 검증 항목(§6.4).
- `OnSetFocus` 캡처 래퍼(종전 버그): 한자 팝업 중 포커스 이탈 시 bare `engine.reset()` 이 상태를 버렸다. 정상 앱은 조합을 문서 텍스트로 확정(keep) 후 상태 정리, 오버레이 앱은 복구 시도 후 로그(§2.7).
- 렌더러(`unim-popup-win`): 헤더 `DT_END_ELLIPSIS`, compact 한자 열 실측 폭, expanded 셀은 **폭 고정 + 셀 텍스트 ellipsis**(§2.5.4). wire 프로토콜 무변경. 동결 설계서 `docs/dev/windows/popup-renderer-design.md`(:416 compact 열 폭·:422 격자) 문구 갱신.
- 검증: `make check-windows` + `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu`(WIN_CRATES 밖). 런타임은 VM 대기.

### 4.5 GTK4·Qt 선택 삭제 래퍼 게이트 [Q4]

GTK4(`immodule.c:1061`)와 Qt(`input_context.cpp:446`)는 엔진 결과가 `consumed` 이기만 하면 앱 선택 영역을 먼저 지운다. 이대로면 한자키(GTK4) 또는 팝업 중 Esc(Qt)가 **선택을 지워** 취소해도 원문이 사라지고, 불일치 선택 + 한자키(소비, 팝업 없음)도 텍스트를 조용히 지운다.

- 개정: `consumed && (commit 비어있지 않음 || preedit 비어있지 않음)` 일 때만 선택을 지운다.
- 효과: 확정 키(commit 있음)에서는 종전대로 삭제→커밋(치환), 팝업 열림·내비·Esc·한/영 전환 같은 "아무것도 넣지 않는 소비 키" 에서는 선택이 남는다. 바뀌는 기존 동작은 "선택한 채 한/영 전환키를 누르면 선택이 지워지던 것" 뿐이며 의도된 동작이 아니다. L3 회귀 케이스로 고정한다.
- (v1.6) 대상③ 확정(`ReplaceSurroundingText`)은 이 래퍼를 거치지 않는다 — 선택이 없음을 전제로 한 명시 삭제다.

### 4.6 대상③ 프로토콜 (v1.6 — 신설)

InputContext(`org.atit.unim.InputContext`)에 메서드 2·시그널 1 을 더한다. 기존 메서드·시그널 시그니처는 바꾸지 않는다. 모두 InputContext 직렬 처리(§2.4.5, `unim-dbus/SPEC.md` §4.4)에 기대어 이후 호출보다 먼저 엔진에 반영된다.

| 종류 | 이름 | 시그니처 | 의미 |
|---|---|---|---|
| 메서드 | `SetClientCapabilities` | `caps: u` → — | 컨텍스트 능력 선언. FocusIn 전후 아무 때나, 바뀔 때만 보내도 된다. 보내지 않으면 0(대상③ 꺼짐, 종전과 바이트 동일). 비트: `0x1 REPLACE_SURROUNDING`(프런트가 `ReplaceSurroundingText` 를 처리한다) · `0x2 SELECTION_RELIABLE`(보고하는 anchor 가 한자키 시점의 실제 선택 — App-pull·Widget, §2.4.6) · `0x4 POINTER_HINTS`(포인터·PRIMARY 사건을 `SurroundingHint` 로 보고한다). 대상③ 허용 = 0x1 이 있고 0x2 또는 0x4 가 있음 |
| 메서드 | `SurroundingHint` | `kind: u, keyval: u, state: u` → — | 엔진이 직접 보지 못한 사건 보고(응답 없이 보내도 된다). `kind`: `1 KEY`(프런트가 엔진을 거치지 않고 앱으로 넘긴 키 — keyval·state 로 엔진이 §2.4b.7 표대로 분류) · `2 POINTER_PLAIN`(Shift 없는 단일 클릭) · `3 POINTER_SELECT`(Shift+클릭·드래그·더블/세 번 클릭) · `4 PRIMARY_CHANGED`(PRIMARY 소유자 변경 — 내용 없음) · `5 SELECTION_UNKNOWN`(이번 한자키 한정, 포커스 위젯의 선택을 알 수 없음). `kind≠1` 이면 keyval·state 는 0 |
| 시그널 | `ReplaceSurroundingText` | `delete_before: u, delete_after: u, commit_text: s, expect_text: s` (`uuss`) | 대상③ 확정. 프런트는 커서 기준 앞 `delete_before`·뒤 `delete_after` 글자를 지우고 `commit_text` 를 커밋한다. `expect_text`(지울 한글 원문, 비어 있지 않음)를 pull 프런트(GTK3/4·Qt)는 삭제 직전 문서 재조회와 완전 일치 대조하고 어긋나면 커밋하지 않는다(GNOME·TSF 는 대조 생략 — TSF 는 편집 세션 안에서 자체 대조). 삭제 실패를 알 수 있으면 커밋하지 않는다(§2.4b.5). popup-owner path 로 발행하고 `HidePopup` 을 잇는다 |

프런트별 실행:

| 프런트 | 능력 | `ReplaceSurroundingText` 실행 | `SurroundingHint` |
|---|---|---|---|
| GTK3 모듈 | R·S | `expect_text` 대조 뒤 `gtk_im_context_delete_surrounding(ctx, -a, a+b)` → TRUE 면 `commit` | 한자키 직전 위젯 조회 불가 시 `SELECTION_UNKNOWN` |
| GTK4 모듈 | R·S | 같음(대조 포함), `is_focused` 아니면 드롭 | 한자키 시점 위젯이 `GtkText`·`GtkTextView` 계열이 아니면 `SELECTION_UNKNOWN` (D4) |
| Qt5/6 | R·S | `QInputMethodEvent::setCommitString(text, -a, a+b)` 한 이벤트 | 한자키 시점 `ImAnchorPosition` 무효면 `SELECTION_UNKNOWN` |
| GNOME 확장 | R·P | `UnimInputMethod.delete_surrounding(-a, a+b)` → `commit_text`(교체 전 preedit 없음 — idle) | 우회한 탐색·편집·단축키는 `KEY`(조합 flush 뒤), stage `captured-event` 의 버튼 눌림·이동으로 `POINTER_PLAIN`/`POINTER_SELECT`, `global.display.get_selection()` `owner-changed`(`SELECTION_PRIMARY`)로 `PRIMARY_CHANGED` |
| 순수 Wayland | R(보류 — 선언하지 않음) | `delete_surrounding_text(a×3, b×3)` + `commit_string` + `commit(serial)` | 수단 없음(§2.10) |
| XIM·IMM32 | — | — | — |
| TSF(인프로세스) | 엔진 setter R·S | 범위 `[c−a, c+b)` 를 읽어 target 과 같을 때만 치환(§4.4·§2.4b.5) | — |

- 엔진 페이로드: `HanjaReplacement { delete_chars: a, delete_after_chars: b, preedit_chars: 0, text }`. 데몬은 `delete_after_chars` 유무가 아니라 **대상 종류**(`HanjaSource::SurroundingWord`)로 채널을 고른다(§4.1).
- 상호배제: 대상③ 확정 프레임도 AutoTypeFix 검사를 건너뛰고 `KeystrokeBuffer` 를 폐기한다(§4.1 과 같음).
- 기존 `DeleteSurroundingText(i,u)` 시그널(SmartBackspace 실험용, 프런트 미배선)은 쓰지 않는다 — 삭제와 커밋이 두 시그널로 갈라지면 사이에 다른 이벤트가 끼어든다.

---

## 5. POPUP_SPEC 개정안 (2026-09-26 승인 → POPUP_SPEC v3.4 반영) [Q1]

아래 문구를 `POPUP_SPEC.md` 에 반영했다. 반영하면서 [Q7](a)·[Q8](a) 결정에 맞춰 5.1(c)·5.4·5.5 의 불일치 선택 동작을 "키만 소비" 에서 이모지 폴백으로 고치고 규칙 11(접미 축소, 5.4b)을 추가했다 — 아래는 그 반영판이다. 조항 번호는 현행 `POPUP_SPEC.md`(v3.3) 기준.

### 5.1 §3.7 동작 규칙 — 2번 교체

> 2. **대상**: 다음 순서로 결정한다.
>    - (a) 조합 중이고 preedit 의 마지막 글자가 완성 음절이면: 음절 확정 모드에서는 「엔진이 기억하는 최근 확정 한글 음절(최대 17자) + preedit」, 단어 확정 모드에서는 「preedit 전체」의 접미 가운데 한자 사전에 있는 **가장 긴** 문자열(2자 이상, 최대 18자). 예: "대한민"+"국" → "대한민국". 없으면 preedit 의 마지막 음절(종전 규칙, 예: "국").
>    - (b) 조합 중이고 마지막 글자가 미완성 자모이면: 종전 규칙(마지막 글자, 초성이면 규칙 7 특수문자 전환).
>    - (c) 조합 중이 아니고 앱 선택 영역(`SetSurroundingText` 의 `cursor_pos != anchor_pos`)이 있으면: 선택 텍스트(앞뒤 공백 제외)가 전부 완성 음절이고 18자 이하이며 사전에 정확히 있을 때 그 문자열. 조건을 만족하지 않으면 선택이 없는 것과 같이 종전 idle 동작(이모지 팝업, §9.2)으로 폴백하고 선택 텍스트는 건드리지 않는다.
>    - 최근 확정 음절 버퍼는 비한글 확정(공백·구두점·영문·숫자·특수문자·이모지·한자 포함)·Enter·커서 이동·Backspace 통과(끝 1자 제거)·한/영 전환·포커스 이탈·Reset·비밀번호 필드·팝업 확정/취소 시 비워진다.
>    - (a)에서 접미가 이미 확정된 글자를 포함하면(음절 확정 모드) 그 글자 수를 "확정 접두 길이" 로 기억한다. 단어 확정 모드에서 접미 앞의 나머지 preedit 은 확정 시 그대로 함께 커밋한다.

### 5.2 §3.7 동작 규칙 — 4번 교체

> 4. **선택 시**: `SelectHanja(globalIndex)` → 엔진이 **출력 형식 설정(`hanja_output_format`: 漢字 / 한자(漢字) / 漢字(한자))을 적용한 문자열** 반환(단음절 변환에도 동일 적용) → 프론트엔드가 커밋. 대상에 확정 접두가 포함되면(규칙 2(a) 음절 확정 모드) 데몬은 `CommitText` 대신 `AutoTypefixApply(delete_chars=접두 글자 수, commit_text=서식 문자열, preedit_text="")` 를 popup-owner path 로 발행하고 프론트엔드는 AutoTypeFix 와 동일한 삭제→커밋을 수행한다. 규칙 2(c) 는 `delete_chars=0` 커밋이며 위젯이 선택 영역을 치환한다(판정에서 제외한 앞뒤 공백은 커밋 문자열에 되붙인다).

### 5.3 §3.7 동작 규칙 — 5번 교체

> 5. **취소 시**: `CancelHanja()` → 엔진이 팝업 진입 때 preedit 에서 내려간 텍스트(음절 확정 모드: 마지막 음절, 단어 확정 모드: 누적 preedit 전체, 선택 영역: 없음)를 반환 → 프론트엔드가 그대로 커밋(없으면 커밋 없음) → 팝업 닫기. 이미 확정돼 앱에 있는 접두와 선택 영역은 건드리지 않는다.

### 5.4 §3.7 동작 규칙 — 10번 신설

> 10. **선택 영역 변환 지원 범위**: 규칙 2(c) 는 선택 영역을 `SetSurroundingText` 로 전달하는 프론트엔드(GTK4·Qt5/6·GNOME Shell 확장·Wayland·Windows TSF)에서만 동작한다. GTK3·XIM 은 선택 정보를 전달하지 못하므로 종전 idle 동작(이모지 팝업)을 유지한다. *(v3.6 에서 대체됨 — GTK3 는 한자키 직전 위젯 조회·로컬 PRIMARY 로 선택을 복원해 지원 범위에 포함, XIM 만 미지원.)* 선택이 있으나 사전에 정확히 없으면 역시 종전 idle 동작(이모지 팝업)이다.

### 5.4b §3.7 동작 규칙 — 11번 신설

> 11. **팝업 중 한자키 재타 (접미 축소)**: 규칙 2(a) 대상으로 연 팝업에서 한자키를 다시 누르면 target 을 더 짧은 접미 가운데 사전에 있는 가장 긴 것(1자면 마지막 음절)으로 옮겨 팝업을 갱신한다(예: "대한민국" → "민국" → "국"). 규칙 2(c) 선택 영역 대상·target 1자·Ctrl/Alt/Super 조합이면 축소하지 않고 종전 동작(푸시 경로: 팝업 닫고 키 재처리 / 풀 경로: 같은 팝업 재발행).

### 5.5 §9.2 idle Hanja 키 dispatch 정책 — 인용문 교체

> **idle Hanja 키 dispatch 정책 (v3.4)**: Hanja 키는 `input_category` 와 무관하게
> `press_key()` 의 언어 분기 직전에 처리. 조합 중이면 한자 변환(§3.7 규칙 2(a)(b)).
> preedit/조합 idle 이면 (i) 앱 선택 영역이 사전에 정확히 있는 한글이면 §3.7 규칙 2(c) 선택 단어 변환,
> (ii) 그 외(선택 없음·불일치 선택)는 emoji popup 트리거(종전 v3.2 동작 — 불일치 선택은 건드리지 않는다).
> 종전엔 `process_korean_key` 안에 있어 영문 모드 첫 Hanja 키가 not_consumed 로 떨어져 무시되던 회귀가 있었다.

### 5.6 §2.4 DBus 메서드 정리 — 행 주석 보강

> `GetHanjaCandidates` 의 `target` 은 다음절 어절일 수 있다. `SelectHanja` 의 반환 `s` 는 출력 형식이 적용된 문자열이며, 확정 접두가 있으면 데몬이 `CommitText` 대신 `AutoTypefixApply(u,s,s)` 를 발행한다. `CancelHanja` 는 규칙 5 의 텍스트를 커밋한다. 시그니처 변경 없음.

### 5.7 §11 변경 이력 — 행 추가

> | 2026-09-26 | **v3.4** | **한자 단어 변환 — §3.7 규칙 2 대상 확장(최근 확정 음절+조합 최장 접미·단어 모드 preedit 접미·앱 선택 영역), 규칙 4/5 확정·취소 페이로드 개정, 규칙 10 선택 변환 지원 범위, 규칙 11 팝업 중 한자키 재타 접미 축소, §9.2 idle 정책에 선택 영역 예외(불일치 선택은 이모지 폴백). 확정 시 `AutoTypefixApply` 를 접두 교체 채널로 재사용, `SelectHanja` 반환 문자열에 출력 형식 설정(`hanja_output_format`) 적용. 헤더 ellipsize·Windows compact 한자 열 동적 폭. 별도 규격: `HANJA_WORD_SPEC.md`** |

### 5.8 `unim-dbus/SPEC.md` 보강 (승인 불필요 — 신규 기능 문서화)

- `:254` `GetHanjaCandidates`: "target 은 다음절 어절일 수 있다".
- `:255` `SelectHanja`: "반환은 출력 형식 적용 문자열. 확정 접두가 있으면 `CommitText` 대신 `AutoTypefixApply(u,s,s)`(preedit_text="") 가 popup-owner path 로 발행된다".
- `:256` `CancelHanja`: "팝업 진입 때 내려간 preedit 부분만 커밋(선택 영역 대상은 커밋 없음)".
- `:285` `AutoTypefixApply`: "한자 단어 교체에도 사용 — 그때 `delete_chars`=확정 접두 글자 수, `preedit_text=""`".
- `:264` `SetSurroundingText`: "한자 선택 변환(대상②) 감지·접두 정합성 검증에도 사용. Wayland 는 `done` 마다, Qt 는 `update()` 마다(변경 시, 빈 텍스트 포함), TSF 는 idle 한자키 시점에 전송".

### 5.9 POPUP_SPEC 반영 절차

**(2026-09-26 완료)** Q1 승인 → §5.1~5.7 문구를 `POPUP_SPEC.md` 에 반영(v3.4) → 이 문서 상태를 "승인·반영" 으로 갱신. `CONTRIBUTING.md` 팝업 변경 6지점 체크리스트에 `POPUP_SPEC.md` 가 들어 있으므로 별도 작업 단위(PLAN U14b)로 잡는다. §9.2 정책 이탈(idle+선택 → 이모지 미발동)과 규칙 10 은 **Q1 승인 전에는 코드 착수하지 않는다**(엔진 U8 의 `selection_target` 분기·데몬 U11). 문서·UI 문구 중 대상② 서술(GTK `row_hanja_keys_*`·Slint 한자 변환 키 description·CHANGELOG Added 1행 후반부·매뉴얼 선택 변환 소절·루트 README 선택 행)도 같은 게이트 — 승인 전에는 §3.3 의 "Q1 승인 전 문구(대상① 판)" 를 쓴다.

### 5.10 v1.6 개정 — POPUP_SPEC v3.7 (2026-10-12, 문서 개정안)

대상③ 은 이 절에 문구를 따로 두지 않고 `POPUP_SPEC.md` 에 직접 반영했다(v3.7 — §3.7 규칙 2(d)·4·5·10·11·12, §9.2 인용문, §11 이력 "기현님 승인 2026-10-12"). 미커밋 개정안이며 기현님 검토 뒤 확정한다. `CONTRIBUTING.md` 팝업 변경 체크리스트의 다른 지점(렌더러·DBus·시험)은 구현 단계에서 맞춘다.

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
| 단어 팝업 목록 고정(v1.1, §2.5.6) | 단어 대상에서 Period·toggle 무시·`expand_visible=false`, 축소 후 1자가 되면 확장 복귀, 단음절 회귀(`word_popup_has_no_expand_and_shrink_to_one_char_restores_it`, `popup_state.rs` `hanja_word_target_ignores_expand`·`hanja_single_syllable_keeps_expand`) |
| 선택 순서(v1.4, §2.4.5) | `selection_latest_snapshot_wins`(`set("서사",1,2)`→`set("서사",0,2)` 뒤 F9 → target "서사"), `stale_partial_selection_targets_single_char`(`set("서사",1,2)` 뒤 F9 → "사" — 낡은 부분 선택 결함 경로 문서화), `selection_escape_leaves_no_residue`(`set("서사",0,2)`→F9→Esc 뒤 commit 비어 있고 한자 모드 아님, 이어 `set("서사",1,2)`→F9 → "사" — 이전 대상이 남지 않음) |
| preedit 포함 학습(v1.5, §2.2.3) | Writer 시퀀스(ㅅ→1/1, 서→1/1, 섯→1/1, 확정+사→2/2 → 한자키 → "서사" 2자·확정 접두 1; 학습 기회 없으면 "사" 1자 강등), 미포함 앱 대조(학습 안 됨), 들쭉날쭉(학습 후 preedit 빠진 보고도 통과), 끝 글자 같은 보고 오탐 없음, 포커스 해제·`reset()` 관측 단절, 드리프트 보강(`recent_clear`, 미학습 컨텍스트는 불변), `before == pre`(preedit 만 보고) 학습·TSF 모두 1자 강등 |
| 대상③ 후보(v1.6, §2.4b.2·§2.4b.3) | §2.4b.3 예 표의 각 행을 그대로 단언(후보 순서·a/b): `국어교육│`·`국어│교육`·`국│어교육`·`대한│민국`·`abc│국어`; 구간 경계(공백·자모·한자·이모지·줄바꿈에서 멈춤); 19자 이상 구간에서 18자 상한; 서로게이트(BMP 밖) 문자가 구간 밖에 있을 때 오프셋; `c == 0` → 대상③ 없음(`cursor0`); 커서 양옆 비한글 → 이모지(`norun`); 사전 일치 0 → 이모지(`nomatch`) |
| 대상③ 재타·소진(v1.6, §2.4b.4) | 재타마다 다음 후보·a/b·즐겨찾기 키 갱신, 마지막 다음 재타 → 한자 팝업 닫힘·커밋 없음·이모지 팝업, pull 소진 뒤 `ProcessKey` 한자키 → 이모지(`surrounding_word_exhausted`), 1음절 후보에서 expand 복귀 |
| 대상③ 확정·취소(v1.6, §2.4b.5) | 키보드·`SelectHanja` 두 경로 모두 페이로드 `{delete_chars:a, delete_after_chars:b, preedit_chars:0, text}`·`AutoTypefixApply` 아님; 서식 3종; 확정 뒤 `surrounding_stale`·`recent` 비움·거부 플래그 해제; Esc·`CancelHanja` → 커밋 없음 |
| 우선순위(v1.6, §2.1) | 조합 중이면 surrounding 에 단어가 있어도 대상①; 신선한 일치 선택 → 대상②; 신선한 **불일치** 선택 → 이모지(대상③ 아님); 낡은 선택 → 이모지 |
| 신선도(v1.6, §2.4b.6) | 커밋 뒤·통과 키 뒤·`SurroundingHint(KEY/POINTER_*)` 뒤 → 이모지(`stale`), 새 `SetSurroundingText` 뒤 → 대상③; 수정자 단독 키는 낡음 아님; Reset·FocusOut 재생성에서 이월, FocusIn 에서 초기화 |
| 거부 플래그(v1.6, §2.4b.7) | 설정 표 각 사건 → 이모지(`selpossible`), 해제 표 각 사건 → 대상③; App-pull(`SELECTION_RELIABLE`) 컨텍스트에서 Shift+→ 뒤 클릭(힌트 없음)에도 대상③; `SELECTION_UNKNOWN` 은 그 한자키만; 순서 뒤바뀜(평이동 뒤 늦은 `PRIMARY_CHANGED`) → 이모지 |
| 능력(v1.6, §4.6) | `SetClientCapabilities` 미호출 → 대상③ 없음·v1.5 와 바이트 동일; `R` 만 → 없음; `R·S`·`R·P` → 있음 |
| 비밀번호(v1.6) | 비번 진입 뒤 한자키 → 대상③ 없음, 로그에 surrounding·target 평문 없음(길이·이유 코드만) |
| 회귀 | `cargo test --workspace` 전량(AutoTypeFix `tests.rs`·`tests_atf_hotkey`·`tests_scenarios`·`tests_popup_change_page`) 경고 0 |

### 6.1b C 단위 (`tests/unit-c/test_primary_selection.c`, v1.4) — **v1.6 퇴역**
**(v1.6)** GTK3 PRIMARY 폴백이 퇴역하므로(§2.10.1) 이 시험과 `make test-gtk-logic` 의 PRIMARY 단계·CI 단계도 함께 걷어 낸다. 아래는 기록. 대신 GNOME 확장의 포인터 분류(단일·Shift·드래그 4px·500ms 더블클릭)를 gi 의존 없는 순수 모듈로 떼어 node 단위 시험을 두고(`make test-gnome-logic` 자리 재사용), 확장 정적 시험에 "PRIMARY 내용 읽기(`St.Clipboard`·`get_text`) 호출 없음" 을 더한다.

GTK3 선택 복원의 PRIMARY 판정(`unim_primary_decide`)을 GNOME 확장 판정 시험(`scripts/test-gnome-primary-selection.mjs`)의 사례 그대로 옮기고, 서로게이트(BMP 밖) 오프셋·19자 거부·앞뒤 모두 일치("가가|가가") 거부·유일 일치 2회 거부를 더했다. `make test-gtk-logic`(glib-2.0 없으면 건너뜀, CI `linux-ci.yml` 에 단계 포함). JS 판정을 바꾸면 이 시험과 C 포팅을 함께 바꾼다.

### 6.2 L2 DBus (`tests/unim-test-dbus`)
evdev 키열 `18 24 34 37 31 30 38 31 19 49 19` → Hanja(123). (1) `GetHanjaCandidates` target=="대한민국" → `AutoTypefixApply` 스트림 구독 후 `SelectHanja(0)` → `(3, "大韓民國", "")`. (2) `SetSurroundingText("대한민국 만세",0,4)` → Hanja → target=="대한민국" → `SelectHanja(0)` → `CommitText "大韓民國"`. (3) `SetSurroundingText("abc def",0,3)` → Hanja → 후보 없음 → `ProcessKey` 폴백에서 이모지 팝업(Q8(a)) — pull 단계 조기 발행 없음. (4, 선택) `SetConfigYaml` 로 `HangulHanja` → "대한민국(大韓民國)". (5) 팝업 열림 → `SetContentType(Password)` → `HidePopup` 시그널 수신·`CommitText` 미발행. (6) AutoTypeFix 순방향 교정(영타 "eogksals" → "대한민") 직후 "국"+Hanja → target=="대한민국"(버퍼 시드).

(v1.4, 순서 불변식 §2.4.5) 데몬 크레이트 시험 `unim-dbus/tests/surrounding_order.rs`: p2p 연결(소켓쌍)로 `SetSurroundingText` 를 `NoReplyExpected` 로 8회 연달아 보낸 직후 `GetHanjaCandidates` 의 target 이 마지막 선택과 일치함을 20회 단언하고, `InputContextHandler` 의 `spawn_tasks_for_methods()` 가 false 임을 단언한다(zbus 5 로 올리면 이 단언이 깨져 속성 방향 반전을 드러낸다).

(7, v1.6 대상③) `SetClientCapabilities(0x3)` → `SetSurroundingText("국어교육 만세",4,4)` → Hanja → target=="국어교육" → `SelectHanja(0)` → `ReplaceSurroundingText(4, 0, "國語敎育", "국어교육")`·`CommitText`/`AutoTypefixApply` 미발행·`HidePopup`. (8) 같은 입력 `cursor=2` → Hanja 재타 순서 국어교육(2/2) → 국어(2/0) → 교육(0/2). (9) `SetClientCapabilities` 미호출 → 같은 입력에서 이모지 팝업(바이트 동일). (10) `SetClientCapabilities(0x5)` → `SurroundingHint(4,0,0)`(PRIMARY_CHANGED) → Hanja → 이모지; 이어 `SurroundingHint(2,0,0)`(POINTER_PLAIN) → `SetSurroundingText` 재송신 → Hanja → 대상③. (11) 직렬 순서: `SurroundingHint` 를 `NoReplyExpected` 로 보낸 직후 `GetHanjaCandidates` 가 그 힌트를 반영함(§6.2 순서 시험 확장).

(2b, v1.1) 대상② "대한민국" 팝업 → `TogglePopupExpand` → `PopupRender` `cols=1`·`expand_visible=false`·`expand_text="⊞"`, 단음절 "한" 은 `expand_visible=true`·토글 뒤 `⊟`.

### 6.3 L3 Xvfb 하네스 (`tests/harness/scenarios/hanja_word.json`)
(1) `hanja-word-syllable`: 11키 → `{committed:"대한민", preedit:"국"}` → F9 → 1 → `{committed:"大韓民國", preedit:"", rendered:"大韓民國"}`. (2) `hanja-word-selection`(GTK4 앱 필드): 11키 → space → Left → shift+Home → F9 → 1. (3) `selection-kept-on-consumed-key`(래퍼 게이트 회귀): `abc` → 선택 → 한/영 전환키 → committed "abc" 유지 → F9 → "abc" 유지. (4, 선택) `hanja-word-custom-key`(Qt 앱, `hanja_keys` 에 F9 아닌 키): 11키 → 커스텀 한자키 → 팝업 헤더 "대한민국"(stale 선택 스냅샷 없이 `update()` 갱신 경로 확인). `commit_unit`/서식 자동 적용 확장은 선택 — 채택 시 `harness.py` 에 `set_config_field(path, value)`(`GetConfigYaml` → 키 패치 → `SetConfigYaml`, 종료 시 복원) 헬퍼가 **필수**(레거시 `SetConfig` 는 두 키를 모르므로 layout 패턴 복제는 조용히 실패).

(v1.6) `hanja-word-cursor`(GTK4·Qt 시험 앱 코어 필드, GTK3 는 네이티브 GtkEntry 필드 — GTK3 코어 필드는 `GtkDrawingArea` 라 직접 조회가 안 돼 `SELECTION_UNKNOWN` 으로 대상③ 이 막힌다): "국어교육 만세" 입력 → Home → Right×2(`국어│교육`) → F9 → 헤더 "국어교육" → F9 → "국어" → 1 → `{committed:"國語교육 만세"}`. `hanja-word-cursor-end`: "대한민국" 입력 → space → BackSpace → F9 → 1 → "大韓民國". `hanja-word-cursor-selection-guard`(GTK3 시험 앱 코어 필드 — 선택을 IM 에 알리지 않는 앱의 재현): "대한민국 만세" → Shift+Home → F9 → 이모지 팝업·Esc 뒤 문서 무변경. `hanja-word-cursor-unknown-widget`(GTK3 코어 필드, 선택 없음): F9 → 이모지(`SELECTION_UNKNOWN`). `hanja-word-cursor-exhaust`: 1음절 후보까지 재타 → 다음 F9 에 이모지 팝업. 시험 앱의 선택 기능(`unim_test_field` anchor·PRIMARY 내보내기)은 그대로 쓴다 — PRIMARY 는 이제 앱이 내보내는 변경 신호의 재현용이다.

(v1.1) `word-popup-no-expand` — 단어 팝업이 열린 채 `.` 를 눌러도 팝업이 유지되고(`.` 미입력) 1 확정이 그대로 된다.

### 6.4 Windows
`make check-windows` 경고 0 + `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu`. VM 검증 항목(이 환경 불가): (v1.6) 대상③ 커서 앞뒤 읽기·범위 `[c−a, c+b)` 치환(메모장·워드패드·Word·한글), 확정 2지점 교체, CUAS(synth) 다글자 삭제, 오버레이 폴백, `insert_text` 선택 치환, `OnSetFocus` 래퍼, 렌더러 폭.

### 6.5 실측 항목(리포 밖)
**2026-10-04 실측 반영**: Mutter→데몬 `SetSurroundingText` 는 선택(cursor≠anchor)을 한 번도 전달하지 않았다(0/321). 오프셋 단위는 문제없음(커밋 뒤 cursor 57→58 문자 단위, len 228→231 바이트). **v1.2 PRIMARY 경유 보강 실기 확인(재로그인 필요)**: (1) gnome-text-editor(GTK4)·GTK3 앱에서 한글 단어를 드래그 선택 → 한자키 → 한자 팝업(대상②) → 확정 시 선택이 치환되는지, (2) 선택 후 마우스로 다른 곳 클릭(선택 해제) → 한자키 → 이모지 폴백인지(PRIMARY 가 커서에 안 붙음), (3) 비밀번호 필드에서 한자키가 PRIMARY 를 읽지 않는지(`UNIM_DEVELOP=1` 로그에 `PRIMARY` 줄 없음), (4) 한자키 연타·직후 일반 키에서 키 순서·반복이 유지되는지(눌림→뜸 순서, 키 반복이 풀리지 않음), (5) 데몬 로그에서 `SetSurroundingText` 가 선택 때만 anchor≠cursor 로 한 번 가고 곧 원복되는지, (6) PRIMARY 소유 앱이 응답하지 않을 때(150ms) 한자키가 지연만 있고 이모지로 폴백하는지. 남은 실기 확인: `WAYLAND_DEBUG=client gnome-text-editor 2>&1 | grep set_surrounding` 로 GTK4 가 드래그 선택 때 송신하지 않는지, 이모지 마우스 확정 시 ZWSP 없이도 commit 이 휘발하지 않는지(휘발하면 마우스 확정 직전에만 ZWSP 1회).

**v1.6 대상③ 실측 항목(기존 세션 로그로 가능한 것 먼저, 앱 실행은 승인 뒤)**: (1) §2.10 의 "Shift+Right 때 cursor=4 anchor=5" 기록이 어느 클라이언트에서 나왔는지(소스상 GTK3·LibreOffice 는 anchor≠cursor 를 낼 수 없다 — GTK3·LO 라면 재조사), (2) GNOME 경로에서 GTK4·GTK3·Writer·Calc 가 **평이동·단일 클릭 직후** surrounding 을 곧바로 보고하는지(안 하면 그 앱에서 대상③ 이 낡음으로 막힌다, §2.4b.6), (3) Writer 에서 Shift+←·드래그 직후 surrounding 이 "선택 문자열, cursor 0" 으로 바로 오는지(§2.4b.8), (4) mutter PRIMARY `owner-changed` 와 키·포인터 사건의 도착 순서·지연(GTK3 gedit, Writer 첫 Shift, Calc 셀 키보드 선택 시 미발생 확인), (5) Writer 선택 없음에서 `delete_surrounding(-a, a+b)` + commit 이 정확한지, (6) GTK4 드래그 선택 직후 anchor 최신성(UNIM gtk4 모듈 경로와 imwayland 경로 각각), (7) gofu 의 Electron 앱(Obsidian·VS Code)이 text-input-v3 로 오는지 GTK IM 으로 오는지(GTK IM 이면 대상③ 불가), (8) 순수 Wayland 에서 IM 이 primary 선택 변경 신호를 받을 수 있는지(ext/wlr data-control — 되면 `P` 선언 후 대상③ 활성), (9) Qt `setCommitString(text, -a, a+b)` 의 커서 뒤 삭제가 QLineEdit·QTextEdit·QtQuick TextInput/TextEdit·LibreOffice qt6 에서 정확한지, (10) **GNOME 경로 삭제 글자 수 정확성(대상③ 활성 전제)**: GTK4·GTK3·Writer·Firefox·Chromium 에서 `delete_surrounding(-a, a+b)` 가 한글 정확히 a+b 자를 지우는지 — mutter 문자→바이트, 앱 툴킷 바이트→문자 변환 가운데 하나라도 바이트를 문자로 다루면 3배 과다 삭제(GNOME 의 AutoTypeFix 는 `vkbd` 가상 BackSpace 연타라 이 경로를 쓰지 않으므로 그 실측 기록으로 **갈음할 수 없다** — `delete_surrounding` 을 직접 실측한다), (11) GTK4 모듈에서 폐기 예정 `gtk_im_context_set_surrounding` 만 부르는 위젯이 gofu 에 있는지(§2.10 GTK4 잔여 위험), (12) X11 세션 LibreOffice(GTK3 모듈)에서 선택 상태 `delete_surrounding` 이 실제로 FALSE 인지(§2.4b.8 완화 판단용).

Mutter `vfunc_set_surrounding` 호출 빈도·오프셋 단위(바이트/문자 — 바이트면 확장 쪽에서 문자 단위 변환, 엔진은 초과 오프셋을 거부한다 §2.4.1), GtkTextView·St.Entry·Chromium(text-input-v3)·Electron 의 commit 시 선택 치환 여부, Qt UTF-16 오프셋의 서로게이트 처리, 앱별 surrounding 보고 지연, popup-service 클릭 시 GTK4 `is_focused` 유지 여부(X11 override-redirect / layer-shell keyboard_interactivity — §4.1), GNOME St 팝업 클릭 시 `UnimInputMethod._hasFocus` 유지 여부(`extension.js:284` 게이트 — §4.1), XIM 스팟 점프 Reset 의 실제 비용(사전 재파싱 시간·클릭 뒤 첫 키 지연 — 문제되면 경량 RPC, Q9(a)).

#### §6.5 (10) 실측 절차 — GNOME `delete_surrounding` 삭제 단위 (2026-10-15 준비, 기현님 승인)

목적: GNOME 확장 경로에서 `delete_surrounding(-a, a+b)` 가 한글 **정확히 a+b 자**를 지우는지(문자 단위) 앱별로 확인한다. 손으로 앱마다 몇 번 시험하면 끝나도록 데몬이 교체 직후 문서를 비교해 판정을 로그 한 줄로 남긴다(관측 전용 — 동작은 바꾸지 않는다).

**장치**
- 켜는 스위치: GSettings `org.gnome.shell.extensions.unim` 의 `surrounding-word-replace`(boolean, **기본 false**, prefs UI 미노출, 실측 전용). 켜지면 확장이 `SetClientCapabilities` 에 `0x5`(`REPLACE_SURROUNDING | POINTER_HINTS`)를, 꺼지면 `0x4` 를 보낸다. 키를 바꾸면 **재로그인 없이** 현재 컨텍스트에 능력이 다시 선언된다. GUI·CLI(`config.yaml`) 설정 동기 대상이 아닌 GNOME 전용 GSettings 키 예외(`unim-gnome-extension/SPEC.md` §5.1).
- 판정 로그(데몬, `engine_worker.rs` + `replace_verify.rs`): `ReplaceSurroundingText` 를 낼 때(키보드·마우스 확정 공통, 모든 프런트) 그 컨텍스트의 직전 surrounding(메모리에만)과 a·b·commit 길이를 보관하고, **같은 컨텍스트의 다음 `SetSurroundingText`**(발행 3초 이내, 그 사이 FocusIn·FocusOut·컨텍스트 파괴 없을 때)와 코드포인트 단위로 비교한다. 로그에는 평문 없이 길이·판정만 남는다(§2.8).
  `[REPLACE_VERIFY] ctx=N frontend=GNOME expect(before=a, after=b, ins=k) actual(before=x, after=y, ins=z) verdict=…`

| verdict | 뜻 |
|---|---|
| `ok` | 문서가 기대한 그대로 바뀜(삭제 a+b 자, 삽입 k 자) |
| `over` | 기대보다 **많이** 지움 — 글자 유실. 한글 3바이트라 a×3 만큼 지워진 경우가 전형(`actual.before` ≈ 3a) |
| `under` | 기대보다 적게 지움 — 옛 글자가 남고 한자가 끼어듦 |
| `mismatch` | 삭제량은 맞거나 어긋났는데 삽입이 기대와 다름(커밋 누락·내용 불일치) |
| `unknown` | 판정 불가 — surrounding 창이 이동했거나 커서와 무관한 변경. 같은 시험을 한 번 더 한다 |
| `timeout` | 3초 안에 바뀐 `SetSurroundingText` 가 오지 않음 — 앱이 보고하지 않았거나, 교체 자체가 버려졌다(GNOME 확장 `_hasFocus` 게이트, GTK·Qt 모듈의 `expect_text` 불일치 거부) — 이 앱은 이 방법으로 판정 불가, 눈으로 결과를 기록한다. 로그는 다음 요청 처리 시 찍힌다 |

**절차** (시험은 **버려도 되는 새 문서**에서 — `over` 는 글자를 지운다)
1. deb 설치(`make install`·deb) 후 **재로그인**. 로그를 보려면 로그인 **전에** `~/.config/environment.d/unim-develop.conf` 에 `UNIM_DEVELOP=1` 한 줄을 넣어 둔다(데몬 로그는 이 변수가 있어야 나온다. 끝나면 파일을 지우고 재로그인 — 엔진 코어 추적 로그는 타이핑 평문을 남긴다, §2.8 잔여). 확장 사본 점검: `gnome-extensions info unim-gnome@from104.github.io` 의 경로가 시스템 설치본인지(사용자 사본이 가리면 새 코드가 안 돈다 — `unim-gnome-extension/SPEC.md` §7).
2. 스위치 켜기(스키마가 확장 디렉터리 안에만 있어 `--schemadir` 가 필요하다):
   `gsettings --schemadir /usr/share/gnome-shell/extensions/unim-gnome@from104.github.io/schemas set org.gnome.shell.extensions.unim surrounding-word-replace true`
   (`~/.local/share/gnome-shell/extensions/…` 사본을 쓰는 개발 환경이면 그 `schemas` 경로.) 확인: `journalctl --user -t org.atit.unim.InputMethod -o cat | grep 'SetClientCapabilities' | tail -3` 에 `caps=0x5` 가 보여야 한다.
3. 앱마다 시험 문장 `오늘은 학교에서 국어교육 한국어 공부를 열심히 한다` 를 입력한다(문장 맨 앞 `c==0` 은 §2.4b.2 LibreOffice 방어로 대상③ 이 없다. 대상 단어 앞뒤에 **9자 이상**을 두는 이유: 앱이 바이트 수를 글자 수로 받아 3배로 지우려 해도 문서 끝에서 잘려 결과가 우연히 맞으면 `ok` 로 위장된다). 아래 세 위치에서 각각 **한자키 → 팝업의 1번 후보 확정** 후 **다음 시험 전 Ctrl+Z 또는 문장 재입력**으로 원문을 되돌린다. 기대값은 첫 후보 기준이다.

| 커서 위치 | 예시 | 기대 `expect(before, after)` |
|---|---|---|
| 단어 끝 | `…한국어|` | `(3, 0)` |
| 단어 중간 | `…국어|교육 …` | `(2, 2)` (첫 후보 `국어교육`) |
| 단어 처음 | `…국어교육 |한국어` | `(0, 3)` |

4. 판정 보기(시험 한 번마다 한 줄이 쌓인다):
   `journalctl --user -t org.atit.unim.InputMethod -o cat | grep REPLACE_VERIFY | tail -20`
   (`UNIM_DEVELOP=1` 파일 로그는 `~/.unim-log/*unim-daemon*.log`.)
5. 대상 앱: **gedit(GTK3) · gnome-text-editor(GTK4) · LibreOffice Writer · Qt6 앱**(예: Kate/KWrite 또는 Qt6 `QTextEdit`), 가능하면 Firefox·Chromium 입력란도. 앱당 위 세 위치를 각 1회(총 3줄 이상).
   - **경로 확인(필수)**: 판정 줄의 `frontend=` 가 **`GNOME`** 인 줄만 (10)의 근거로 쓴다. `GTK3`·`GTK4`·`Qt5`·`Qt6` 등 다른 값이면 그 앱은 GNOME 확장이 아니라 unim 자체 모듈 경로로 온 것이다 — 그 줄은 (10)이 아니라 (9) 등 해당 프런트 항목의 기록이며, GSettings 스위치와 무관하게 대상③ 이 이미 켜져 있는 경로다. 그 줄은 (10)의 근거로 쓰지 않는다.
   - **GNOME 경로로 시험 띄우기**: 기현님 세션 환경은 `QT_IM_MODULE=unim`(실측 확인; `GTK_IM_MODULE=wayland`)이라 Qt6 앱은 기본으로 unim Qt 플러그인 경로(`frontend=Qt6`, `setCommitString`)로 간다. Qt 앱을 GNOME 경로로 시험하려면 `QT_IM_MODULE=wayland <앱>` 처럼 띄운다. GTK 앱은 `echo $GTK_IM_MODULE` 이 `unim` 이 아닌지 확인한다(im-config `25_unim.rc` 는 `GTK_IM_MODULE=unim` 도 설정하므로 환경에 따라 gedit 도 GNOME 경로가 아닐 수 있다 — 필요하면 `GTK_IM_MODULE=wayland <앱>`). 시험마다 `frontend=GNOME` 인지 로그에서 확인한다.
6. **끝나면 반드시 끈다**: `gsettings --schemadir <위와 같은 경로> set org.gnome.shell.extensions.unim surrounding-word-replace false` (그리고 1단계의 `UNIM_DEVELOP` 파일 제거). 이어서 아래 기록 표를 이 문서에 채운다.

**판정 기준**
- **모든 앱·세 위치에서 `verdict=ok`** → 기본값 활성(확장의 기본 `true` 또는 상수화)을 검토한다. 기본값 변경은 기현님 승인 후.
- **`over` 가 하나라도 있으면 그 앱 경로는 유지 금지** — 그 앱에서는 GNOME 경로 대상③ 을 켜지 않는다(앱 거름 규칙 또는 전체 보류). 전형은 `actual.before ≈ 3a`(바이트 삭제).
- `under` 는 글자 유실은 아니지만 결과가 틀리므로 같은 취급(원인 규명 전 기본 활성 금지).
- `mismatch`·`unknown`·`timeout` 은 판정 보류 — 재시험하고, 반복되면 해당 앱은 "측정 불가"로 기록하고 눈으로 확인한 결과를 적는다.

**실측 기록**(시험 후 채운다)

| 날짜 | 앱(툴킷) | 단어 끝 | 중간 | 처음 | 비고 |
|---|---|---|---|---|---|
| — | gedit (GTK3) | | | | |
| — | gnome-text-editor (GTK4) | | | | |
| — | LibreOffice Writer | | | | |
| — | Qt6 앱 | | | | |

---

## 7. v2 메모

- **target 축소/확장 키** [Q7]: 팝업 중 한자키 재타 → 접미 1자 축소(대한민국 → 한민국 → 민국 → 국), Shift+한자키 → 확장. v1 은 단어 일치 시 단어 후보만 보인다.
- **조사 분리**: "대한민국은" 에서 "대한민국" 만 — 조사 목록 + 최장일치. v1 은 "조사를 붙이기 전에 한자키" 를 매뉴얼에 안내.
- **후보 랭킹**: 사전 등장순이 빈도순이라는 보장이 없다. 즐겨찾기로 보완, 빈도 데이터는 v2.
- ~~AutoTypeFix 순방향 교정 뒤 버퍼 시드~~ → v1 로 당김(§4.1, U11 1줄).
- ~~XIM 커서 점프 감지~~ → v1 [Q9](a) 로 당김(Reset 송신 — 엔진 재생성·chord 강제 flush 동반, idle 구간당 1회 디바운스). 버퍼만 비우는 경량 RPC `ClearRecentSyllables`(엔진 `recent_clear` 만, `unim-dbus/SPEC.md` 1행)는 Reset 비용이 실측에서 문제될 때 v1.1 — "Reset 이 값싸다" 는 전제는 성립하지 않으므로 '불필요' 로 단정하지 않는다.
- ~~**XIM 대상②**: X11 PRIMARY 선택 우회.~~ → v1.6 에서 폐기(PRIMARY 내용 추정 퇴역, §2.10.1).
- **(v1.6) 출처를 싣는 단일 계약**: `Snapshot{text, cursor, anchor, sel_src: App|Widget|Push|None, preedit: Yes|No|Unknown}` 하나와 엔진의 단일 `Replace{delete_before, delete_after, text}` 명령으로 대상①②③ 확정을 통일하고 프런트별 선택 삭제 블록 세 벌(GTK4·Qt·GTK3)을 걷는다. 능력 비트(§4.6)는 그 전 단계다. Writer 는 선택이 있을 때 `DeleteSurroundingText` 를 거부하므로(edtwin.cxx:7044) 선택 치환은 여전히 앱에 맡긴다.
- **(v1.6) AT-SPI 선택 질의 PoC**: GNOME 의 GTK3·LibreOffice 에서 "문장 중간 선택 변환" 이 꼭 필요해지면 한자키 시점에만 `Text.GetSelection` 을 비동기 1회 질의. **이벤트 리스너 금지**(at-spi-bus-launcher 가 `toolkit-accessibility=true` 를 gsettings 에 영구 저장 — Firefox·Qt·Chromium a11y 가 켜진다), 18자 이하만 `GetText`, PASSWORD_TEXT 역할이면 중단, 로그는 길이만. 앱 실행이 필요하므로 기현님 승인 뒤.
- **(v1.6) 순수 Wayland 대상③**: IM 이 primary 선택 변경 신호를 받을 수단이 확인되면 `POINTER_HINTS` 대체로 켠다(§2.10).
- **기능 on/off 설정**: 두지 않는다 — "다음절 일치 없음 = 종전" 이 곧 안전망이고, 스위치는 설정 동기 지점 9곳과 테스트 매트릭스를 두 배로 만든다. 필요해지면 `KoreanConfig.hanja_word_conversion: bool` 을 §3.2 표대로 추가.

---

## 8. 변경 이력

| 날짜 | 버전 | 변경 내용 |
|---|---|---|
| 2026-09-17 | v1-draft | 초안 — 세 설계안(엔진·UX·플랫폼) 심사 종합, POPUP_SPEC 개정안 §5 수록, 승인 대기 |
| 2026-09-17 | v1-draft2 | 검증 반영 — 선택 오프셋 양끝 클램프(패닉 방지), `before` 빈 문자열 검증 실패, 비번 게이트 순서·데몬 HidePopup 응답 채널, TSF OnSetFocus 양 분기, 바이트 동일 불변식을 preedit 1자로 한정, ATF 순방향 시드·XIM 스팟 Reset v1 승격, Windows 격자 폭 고정 유지, 동기 지점 표 확장(Slint .po·help-html·매뉴얼 범위), §5.9 반영 절차, Q7 대안·Q8·Q9 신설 |
| 2026-09-17 | v1-draft3 | 2차 검증 반영 — 호스트 능력 플래그 `hanja_word_replace_capable`(기본 false, IMM32/capi 회귀 차단), chord 한자키 확정 음절 흡수, 선택 오프셋 초과를 클램프 대신 **거부**(단위 불일치 fail-closed), `prefix+pre` 검증 절을 TSF 전용 플래그로 한정, 비번 진입 시 surrounding 잔류 제거·Qt 프런트 이중 게이트·로그 길이만, GTK4 `is_focused` 우회 철회(역방향 ATF 시그니처 동일·XTest 폴백 위험)·GNOME `_hasFocus` 실측 항목, XIM 스팟 점프 판정 기준("IM 이 유발하지 않은 갱신"+디바운스)·Reset 비용 정정, ATF 순방향 시드를 replay 뒤로, 한자 교체 후 KeystrokeBuffer 폐기, TSF `apply_reverse_event` 시그니처 2개 추가, Qt surrounding 을 `update()` 갱신으로, 동기 지점에 `src/SPEC.md`·`unim-cli/SPEC.md`·루트 README·매뉴얼 ko/en 1:1 추가, Q1 승인 전 2벌 문구·Slint 한자 키 row |
| 2026-09-26 | v1 | 기현님 최종 승인(Q1~Q9 결정대로) — POPUP_SPEC v3.4 반영(§5 를 Q7(a)·Q8(a) 판으로 갱신, 규칙 11 추가), §2.5.5 신설(끊긴 참조 복구), §2.4.1·§6.2(3) 을 Q8(a) 이모지 폴백으로 정정 |
| 2026-09-26 | v1 | §2.10 XIM 대상① — 교체 채널 BS 직렬 주입으로 결함 해소, ✔ 복원 |
| 2026-09-30 | v1 | §4.1 ATF 순방향 버퍼 시드 — Windows TSF 누락 보강(실기 확인) |
| 2026-10-04 | v1.1 | GNOME 경로 선택 미전달 실측(0/321)·대상② PRIMARY 경유(보강안, 승인 대기)·단어 팝업 목록 고정(§2.5.6) |
| 2026-10-04 | v1.2 | GNOME 확장 PRIMARY 경유 대상② 보강 **구현**(기현님 승인) — idle 한자키 한정·읽기 전용 비동기·커서 정확 인접·18자·비번 fail-closed·실제 surrounding+anchor 이동·처리 후 복원, 순수 모듈 `primary_selection.js` + node 단위 시험(§2.10 GNOME 행·§6.5) |
| 2026-10-04 | v1.3 | GNOME 선택 보고 실측 정정(불규칙·지연 — 0/321 은 마우스 한정)·PRIMARY 유일 일치 보강(§2.10)·한자키 게이트 이유 코드 로그·사용자 사본이 설치본을 가리는 문제(`make dev-extension` 0.4.1 사본이 0.4.4 를 가려 ZWSP 제거·PRIMARY 코드가 로드되지 않았음) 진단·경고(확장 버전·경로 로그, make install·deb postinst 경고) |
| 2026-10-05 | v1.4 | GTK3 대상② 지원(기현님 승인) — 한자키 직전 위젯 조회·로컬 PRIMARY 로 선택 복원 후 `SetSurroundingText` 동기 송신(§1.3·§2.10), 순서 불변식 §2.4.5(데몬 InputContext 직렬 처리 + 선택은 한자키 전에 엔진 반영 — GTK4 낡은 선택으로 1자 대상이 돼 '서' 가 유실되던 결함 수정), 로그 길이만(§2.8), 테스트 §6.1·§6.1b·§6.2, §5.4 규칙 10 은 v3.6 에서 대체됨 표시. XIM 만 미지원으로 남음 |
| 2026-10-06 | v1.5 | LibreOffice VCL 편집기 surrounding preedit 포함 실측 — §2.2.3 `prefix+pre` 절 게이트를 "TSF 에서만" → "TSF 선언 또는 관측 학습"(엔진 `learned_includes_preedit`, FocusIn·재생성 해제·드리프트 보강), §2.10 GNOME 행 실측(LibreOffice preedit 포함·PRIMARY 지연), §6.1 시험 표 추가(기현님 승인) |
| 2026-10-12 | v1.6 | **문서 개정안(기현님 검토 대기·미커밋)** — 기현님 확정 설계: 대상③ "커서가 걸친 단어"(§2.4b — 구간 추출·후보 순서 표·재타 순환·소진 시 이모지·확정 = 커서 기준 앞 a·뒤 b 삭제 + 커밋, `c==0` LibreOffice 방어), 대상 우선순위(조합 중 → ① / 신뢰 가능한 선택 → ② / 선택 없음 → ③ / 이모지, §2.1·Q10), 선택 출처 신뢰 등급(App-pull·Widget 만 신뢰, App-push 는 신선할 때만, Guess 퇴역 — §2.4.6·Q12), 신선도 게이트(§2.4b.6), 거부 플래그 설정·해제 표(§2.4b.7), 프로토콜 `SetClientCapabilities`·`SurroundingHint`·`ReplaceSurroundingText`(§4.6), 프런트별 지원표 ①②③ 재작성(§2.10), **PRIMARY 내용 읽기·인접/유일 판정·키보드 길이 추적·GTK3 PRIMARY 폴백 퇴역**(§2.10.1·Q11), 대상③ 로그 평문 금지(§2.8), UI 문구 개정안(§3.3), TSF 커서 뒤 읽기(§4.4), 시험 표(§6 — §6.1b 퇴역), v2 메모(Snapshot 계약·AT-SPI PoC). POPUP_SPEC v3.7·`unim-gnome-extension/SPEC.md` §2.8 동시 개정 |
| 2026-10-12 | v1.6 검토 | 문서 검토 보강(미커밋) — §2.4b.3 예 표를 실제 사전 기준으로 정정(어교·한민 추가), §2.4b.5 프로토콜별 실패 의미 표·TSF 확정 전 내용 대조·DBus `expect_text` 안(결정 대기), §2.4b.8 경로별 정리(X11 GTK3 모듈 LibreOffice = 대상③ ✘), §2.10 GTK4 폐기 API 잔여 위험·Qt `ImAnchorPosition` 무효 시 `SELECTION_UNKNOWN`, GNOME 경로 대상③ 은 삭제 글자 수 실측(§6.5 (10)) 뒤 활성, §6.5 (10)~(12) |
| 2026-10-12 | v1.6 구현 반영 | 구현 직후 검증 반영 — §4.6 시그니처를 구현 계약으로 정정(`ReplaceSurroundingText(uuss)` = delete_before·delete_after·commit_text·expect_text, `SetClientCapabilities(u)` 비트 0x1/0x2/0x4, `SurroundingHint(uuu)` kind 1..5), §2.4b.5 `expect_text` 내용 대조 "결정 대기" → 채택·구현(타입 불일치 페이로드는 프런트가 무시), **D4** §2.10 GTK4 잔여 위험 → 위젯 종류 거름(GtkText·GtkTextView 외 `SELECTION_UNKNOWN`) 채택 + 하니스 `hanja-word-cursor-unknown-widget-gtk4`, **D1** §2.8 이유 코드 `exhausted` 추가, **D2** GNOME AutoTypeFix 는 가상 BackSpace 라 §6.5 (10) 실측을 대체할 수 없음(별도 실측 필요) 정정. R1(`국|어교육` 행): 기현님 결정(2026-10-12) — 규칙 순서 유지, 예시 표를 사전 그대로(어교 포함)로 정정 |
| 2026-10-15 | v1.6 실측 준비 | 기현님 승인 — 실측 준비. GNOME 확장의 하드코딩 상수 `SURROUNDING_WORD_REPLACE_ENABLED` 를 GSettings 키 `surrounding-word-replace`(기본 false, prefs 미노출, 켜면 caps 0x5 · 끄면 0x4, 실시간 재선언)로 교체, 데몬에 `[REPLACE_VERIFY]` 교체 검증 로그(관측 전용 — 발행 직전 surrounding 과 다음 `SetSurroundingText` 를 코드포인트 비교, verdict ok/over/under/mismatch/unknown/timeout), §6.5 (10) 실측 절차·판정 기준·기록 표 추가 |
