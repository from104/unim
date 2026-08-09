# UNIM 타자 연습 — 디자인 명세서

> **claude.ai Design 입력용 brief**
> 대상 앱: `unim-typing-practice` (v0.3.0)
> 시각 SoT: [DESIGN.md](../DESIGN.md), [design/app.jsx](../design/app.jsx), [design/template.html](../design/template.html)
> 실 코드: [../src/](../src/) (GTK4 + libadwaita, Rust)
>
> 본 문서는 DESIGN.md(텍스트 SoT)와 design/app.jsx(라이브 React mockup SoT)의 모든 디자인 결정을 단일 문서로 모은 것이다. 새 시안/변형을 만들 때는 본 문서의 모든 토큰·치수·라벨 규칙을 우선 충족해야 한다.

---

## 0. 한 줄 요약

UNIM IME 가 활성화된 상태에서, 임의의 한글 자판으로 타자 연습을 하면서 **WPM · CPM · 정확도 · 오타율 · 줄별 WPM(sparkline) · 키 오타 히트맵**을 받아보는 GTK4 + libadwaita 데스크톱 앱. 단일 윈도우, 사이드바 없음, UNIM 데몬 의존. 입력은 native `gtk::Entry`(IME 위임), 색·통계·시각화에 집중.

## 1. 사용자 컨텍스트

- **주 사용자**: 서기현 — 뇌병변 사지마비. 마우스는 오른발, 타이핑은 입에 젓가락.
- **함의**: 키 시퀀스·마우스 정밀도 부담 → 한 화면 노출, 메뉴 1뎁스, 자동 진행, 슬라이더/세그먼티드 선호.
- **2차 사용자**: 한글 자판 사용자 일반(두벌식·세벌식·사용자 정의). UNIM 의 `korean.layout` 으로 자판 자동 인식.

## 2. 디자인 원칙 (DESIGN.md §1 — 잠금)

1. **모노스페이스가 작업의 중심**. 예시글·입력란·통계 숫자·키캡 영문 라벨은 모두 **JetBrains Mono**. 그 외 UI 텍스트만 **Pretendard**.
2. **색은 의미를 갖는다**. `accent` = 진행/일치, `wrong` = 오타. **오타는 항상 *색 + 물결 밑줄* 두 신호** (색약 보조).
3. **카드 위계 1단**. 헤더 / 본문 카드 / 키보드 카드 3개 층위만 허용. **중첩 카드 금지**.
4. **숫자는 `font-feature-settings: "tnum"`** — 통계가 흔들리지 않게.
5. **라이트/다크는 토큰 한 벌로 자동 추종** — 색을 직접 적지 말고 토큰(또는 Adwaita 변수)을 쓴다.

## 3. 환경 / 기술 잠금

| 항목 | 값 | 주석 |
|---|---|---|
| Toolkit | GTK 4 + libadwaita 0.7 (`v1_4`) | Material · iOS 디자인 금지 |
| OS | Linux (X11, Wayland) | macOS · Windows 비대상 |
| 테마 | Adwaita 라이트/다크 자동 추종 | 토큰은 Adwaita 변수로 가능한 한 매핑 |
| 폰트 (mono) | JetBrains Mono / D2Coding / Adwaita Mono / ui-monospace | 시안은 JetBrains Mono, 코드 폴백 D2Coding |
| 폰트 (sans) | Pretendard Variable / Pretendard / Noto Sans CJK KR / Cantarell | |
| 다국어 | ko / en (rust-i18n) | 동일 레이아웃 유지 |
| 데몬 | UNIM 활성 필수 | 비활성 시 `Adw.AlertDialog` (3-step) → 종료 |

## 4. 디자인 토큰 (app.jsx `tokensLight` / `tokensDark` — SoT)

| 토큰 | 라이트 | 다크 | Adwaita 매핑 |
|---|---|---|---|
| `bg` | `#fafaf9` | `#1d1d1d` | `@window_bg_color` |
| `bgRaised` | `#ffffff` | `#262626` | `@view_bg_color` |
| `fg` | `#1c1c1c` | `#ededed` | `@window_fg_color` |
| `fgDim` | `#6b6b6b` | `#9a9a96` | `alpha(@window_fg_color, 0.6)` |
| `fgFaint` | `#a8a8a4` | `#6b6b67` | `alpha(@window_fg_color, 0.4)` |
| `border` | `rgba(20,20,20,0.10)` | `rgba(255,255,255,0.08)` | `@borders` |
| `borderStrong` | `rgba(20,20,20,0.18)` | `rgba(255,255,255,0.14)` | `alpha(@borders, 1.5)` |
| `cardBg` | `#ffffff` | `#2a2a2a` | `@card_bg_color` |
| `cardBgSoft` | `#f3f3f0` | `#222222` | `mix(@view_bg_color, @window_bg_color, 0.5)` |
| `accent` / `correct` | `#1c66c9` | `#62a0ea` | `@accent_color` |
| `accentSoft` / `correctSoft` | `rgba(28,102,201,0.10)` | `rgba(98,160,234,0.14~0.16)` | `alpha(@accent_color, 0.12)` |
| `wrong` | `#d24e15` | `#ff9560` | `@error_color` (커스텀 권장) |
| `wrongSoft` | `rgba(210,78,21,0.12)` | `rgba(255,149,96,0.14)` | `alpha(@error_color, 0.14)` |
| `dim` | `#b6b6b2` | `#5e5e5b` | — (미입력 글자 전용) |

### 4.1 그림자

| 변수 | 값 (라이트) | 값 (다크) |
|---|---|---|
| `shadow` (카드) | `0 1px 2px rgba(0,0,0,0.04), 0 1px 1px rgba(0,0,0,0.02)` | `0 1px 2px rgba(0,0,0,0.30), 0 1px 1px rgba(0,0,0,0.15)` |
| `shadowKey` (키캡) | `inset 0 1px 0 rgba(255,255,255,0.9), 0 1px 0 rgba(0,0,0,0.04), 0 2px 3px rgba(0,0,0,0.04)` | `inset 0 1px 0 rgba(255,255,255,0.05), 0 1px 0 rgba(0,0,0,0.30), 0 2px 3px rgba(0,0,0,0.20)` |
| 윈도우 외곽 | `0 24px 48px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.06)` | `0 24px 48px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.06)` |

### 4.2 라운드 · 보더

| 토큰 | 값 |
|---|---|
| 윈도우 `border-radius` | **14 px** |
| 카드 `border-radius` | **12 px** |
| 키캡 `border-radius` | **7 px** |
| Big 키 라벨 카드 (시안) | 14 px |
| 카드 보더 | `1px solid @borders` |
| 입체 위젯(키캡·진행률 셀) | `border-bottom: 2px solid borderStrong` |

## 5. 타이포그래피 (DESIGN.md §3)

| 역할 | 폰트 | 크기 | 굵기 | 비고 |
|---|---|---|---|---|
| 타이틀 (헤더) | sans | 13 | 600 | |
| 헤더 자판 코드 (subtitle) | mono | 10 | 400 | `fgDim` |
| 본문 (예시글) | mono | **16** | 400 / 600 | letter-spacing 0.01em, line-height 1.45 |
| 입력란 | mono | 16 | 400 | caret = fg, `blink 1s steps(2)` |
| 통계 큰 숫자 (StatCell) | mono | 24 | 700 | `tnum` |
| 통계 단위 (`WPM`, `%`) | sans | 10–12 | 600 | letter-spacing 0.04em |
| 통계 라벨 (StatCell) | sans | 11 | 400 | `fgDim` |
| 카드 라벨 (`줄 위치`, `진행률`) | sans | 11 | 500 | **uppercase**, letter-spacing 0.04em |
| BigStat (결과) | mono | **44** | 700 | letter-spacing −0.02em, tnum |
| BigStat delta 칩 | mono | 10 | 700 | `accentSoft` 배경, padding `1px 5px` |
| 키캡 메인 라벨 (영문 Base) | mono | 15 | 700 | |
| 키캡 메인 라벨 (한글 Base) | sans | 15 | 700 | **accent 색** |
| 키캡 보조 라벨 (Shift) | mono / sans | 11 | 500 | `fgFaint` |
| 특수키 라벨 | sans | 12 | 600 | `fgDim`. 한/영·한자만 `accent` |
| KbdHint (단축키 칩) | mono | 10 | — | `fgDim`, 보더 칩 |
| 다이얼로그 step 번호 | mono | 11 | 700 | `accentSoft + accent` 라운드 22px |

## 6. 윈도우 셸 (`WindowChrome` — app.jsx 실측)

- 폭 **900 px** 고정, 라운드 14, 외곽 shadow.
- 구조: `HeaderBar(46h) → ViewSwitcher(padding 12,4) → ViewStack`.
- Practice 페이지 영역 = 680 px (윈도우 755), Result 페이지 영역 = 720 px (윈도우 820).

```
┌─[ HeaderBar 46h ]─────────────────────────────────────────────────┐
│ [📂 짧은 글 ▾]      UNIM 타자 연습          [↻] [⋯]   ● ● ●      │
│                     두벌식 표준 · ko_2bulstd                       │
├───────────────────────────────────────────────────────────────────┤
│   ┌─[ ViewSwitcher segmented ]─┐                                  │
│   │ ⌨ 연습  │  📊 결과         │                                  │
│   └────────────────────────────┘                                  │
│                                                                   │
│   [ PracticePage | ResultPage ]                                   │
└───────────────────────────────────────────────────────────────────┘
```

### 6.1 HeaderBar

- 좌측: `CorpusDropdown` — 30h, `cardBg`, `border`, 라운드 7, 내용 `📄 짧은 글 ▾`. 펼침 시 `cardBg` 팝오버 (180w, padding 4, `borderStrong`), MenuRow 30h. 선택 항목 = `accentSoft` 배경 + `accent` 텍스트 600w, 우측에 `3줄 / 8줄 / 20줄` 메타(mono 11, fgDim).
- 중앙: 절대 위치 (`translate(-50%, -50%)`) 2단 타이틀.
- 우측: `HBarButton 30×30 r7` × 2 (↻ 재시작 / ⋯ 메뉴) + Window controls (12 dot ×3, mac 식).
- 메뉴 팝오버: 210w. 항목 = `결과 복사 [⌘C]`, `오타 히트맵 보기`, `연습으로 돌아가기`, `─`, `키 입력 기록 내보내기`, `설정`. 우측 단축키는 `KbdHint`(mono 10 + border 칩).

### 6.2 ViewSwitcher (segmented)

- `cardBgSoft` 배경 컨테이너 (라운드 9, padding 3, `border`).
- 탭 = padding `6px 18px`, 라운드 6, font 13/600, 아이콘 + 라벨.
- 활성 탭 = `cardBg` 배경 + `shadow` + `fg`, 비활성 = `fgDim`.

## 7. Practice 페이지 (app.jsx `PracticePage`)

```
padding: 16px 20px 20px 20px
grid-template-columns: 1fr 280px
gap: 18px

┌──────────────────────────────┐  ┌──────────────────┐
│ TargetCard (.card, vexpand)  │  │ LinePosCard      │
│                              │  ├──────────────────┤
│  ✓  oldline (fgFaint)        │  │ StatsCard 2×2    │
│  ▶  current  (accentSoft)    │  ├──────────────────┤
│     pending  (dim)           │  │ ProgressCard     │
│     pending  (dim)           │  │ (20 segments)    │
│     pending  (dim)           │  ├──────────────────┤
│                              │  │ IMEWarning       │
└──────────────────────────────┘  └──────────────────┘
┌──────────────────────────────────────────────────────┐
│ InputField (accent border + accentSoft ring + 한글)  │
└──────────────────────────────────────────────────────┘
[ Keyboard — 5 rows ANSI 106, centered, cardBgSoft 카드 ]
```

### 7.1 TargetCard / TargetLine

- 카드: `cardBg` + `border` + 라운드 12 + `shadow`, padding `14px 0`, `overflow: hidden`.
- 한 줄(TargetLine) padding `8px 18px 8px 14px`, 좌측 16px 마커 컬럼 + mono 16/1.45 본문.
- 줄 상태별:
  - **done**: 본문 = `fgFaint`. 마커 = `<CheckIcon />` (fgFaint).
  - **current**: 배경 `accentSoft` + `border-left: 3px solid accent`. 마커 = **회전 삼각형 spinner**.
  - **pending**: 본문 = `dim`. 마커 빈칸.
- transition: `background 200ms ease`.

#### 현재 줄 글자 색칠

- `i < correct` → `correct` (600w).
- `correct ≤ i < typed` → `wrong` (600w) + **`text-decoration: underline wavy` (두께 1.5px, offset 4px)**.
- `i ≥ typed` → `dim`.
- caret 위치(`i == typed`): 글자 좌측에 2×(text-height) `fg` 바, `blink 1s steps(2)`.

### 7.2 Spinner — *rotateX 회전 삼각형*

```html
<svg width="14" height="14" viewBox="0 0 14 14">
  <polygon points="3,2 12,7 3,12" fill="accent" />
</svg>
```

```css
@keyframes tri-flip { 0% { transform: rotateX(0deg); } 100% { transform: rotateX(360deg); } }
animation: tri-flip 1.1s cubic-bezier(0.65, 0.05, 0.35, 1) infinite;
perspective: 60px; transform-style: preserve-3d;
```

### 7.3 InputField

- `cardBg`, **`border: 1.5px solid accent`**, 라운드 12, padding `14px 18px`.
- **링**: `box-shadow: 0 0 0 4px accentSoft, shadow`.
- mono 16, fg 색, 캐럿은 2×18 `fg` 바 `blink`.
- 우측 IME 칩: mono 10, `fgDim`, `border` 보더, 라운드 4, 텍스트 `한글` / `EN`.

### 7.4 LinePosCard

- 카드 padding `12px 14px`, baseline 정렬, space-between.
- 좌: `줄 위치` 11/500 uppercase letter-spacing 0.04em fgDim.
- 우: `<22/700/fg> / <14/fgFaint> <14/fgDim>` (mono, baseline 정렬).

### 7.5 StatsCard (2×2)

- 카드 padding 14, `display: grid; grid-template-columns: 1fr 1fr; gap: 16px 14px`.
- 각 셀(StatCell): vertical
  - 상단: `<24/700/tnum/mono [색]> <10/600/fgDim/letter-spacing 0.04em "WPM"|"CPM"|"%"|"%">`
  - 하단: `<11/fgDim "분당 단어"|"분당 글자"|"정확도"|"오타율">`
- 정확도 값은 `correct`, 오타율 값은 `wrong` 색.

### 7.6 ProgressCard

- 카드 padding `12px 14px`.
- 상단 row: 좌 `진행률` (11/500 uppercase fgDim) / 우 `<n %>` (11/600 mono fg).
- 본체: **20개 균등 segment**, `gap: 2px`, `height: 6`, 라운드 2.
  - filled (`i < floor(p*20)`): `accent`, border `transparent`.
  - partial (`i == floor(p*20)`): `accentSoft` + `border`.
  - empty: `cardBgSoft` + `border`.

### 7.7 IMEWarning

- padding `10px 12px`, 라운드 10, 배경 `wrongSoft` + 보더 `wrongSoft`.
- 좌: `<WarnIcon />` (wrong, marginTop 1).
- 우 2단: `<11.5/600 "ASCII 키만 수집">` + `<11.5/fgDim "IME 가 켜져 있어도 키맵에는 영문 키만 표시됩니다.">`.

### 7.8 Keyboard (5행 ANSI 106 — DESIGN.md §4.4)

- 카드: padding 12, `cardBgSoft` + `border` + 라운드 14 + `shadow`.
- 키: `KEY_U = 46`, `KEY_H = 50`, `KEY_GAP = 5`, `ROW_GAP = 5`.
- 행 폭(15u) = 1.5+1.25+1.5+1.25+4+1.25+1.5+1.25+1.5 (Row 4), 1~3행과 정확히 일치. **Row 4 우측 공백 없음**.

| Row | 키 |
|---|---|
| 0 | `` ` 1 2 3 4 5 6 7 8 9 0 - = Backspace(2u) `` |
| 1 | `Tab(1.5) Q W E R T Y U I O P [ ] \(1.5)` |
| 2 | `Caps(1.75) A S D F G H J K L ; ' Enter(2.25)` |
| 3 | `LShift(2.25) Z X C V B N M , . / RShift(2.75)` |
| 4 | `Ctrl(1.5) Meta(1.25) Alt(1.5) 한자(1.25) Space(4) 한/영(1.25) Alt(1.5) Menu(1.25) Ctrl(1.5)` |

#### 7.8.1 일반 키캡 (4-corner Grid 2×2)

```
┌─────────────────────────────┐
│ A (mono 11 fgFaint)      ㅃ │  ← 좌상=영문 Shift(보조)   우상=한글 Shift(우측 정렬)
│                             │
│ a (mono 15/700 fg)       ㅂ │  ← 좌하=영문 Base(메인)    우하=한글 Base(accent 600 우측)
└─────────────────────────────┘
```

- padding `3px 5px 4px 5px`, `display: grid; grid-template-columns: 1fr 1fr; grid-template-rows: 1fr 1fr; line-height: 1`.
- 보더: `1px @borders` + `border-bottom: 2px borderStrong` (입체감).
- 그림자: `shadowKey`.
- transition: `all 120ms ease-out`.

#### 7.8.2 상태

- **hover**: 보더 색만 `alpha(accent, 0.5)`.
- **pressed**: bg `accent`, border `accent`, **border-bottom 동일** (입체감 제거), 모든 라벨 색 흰색 계열 (Shift 보조 = `rgba(255,255,255,0.7)`), shadow `inset 0 1px 2px rgba(0,0,0,0.15)`.

#### 7.8.3 특수 키캡

- 가운데 정렬 단일 라벨 (sans 12/600).
- 색: 일반 `fgDim`, **한/영·한자만 `accent`**.
- 폭: `unit * KEY_U + (unit - 1) * KEY_GAP`.

## 8. Result 페이지 (app.jsx `ResultPage`)

```
padding: 20
gap: 16
┌─ ResultHeader  ───────────────────────────────────────────┐
│ ─── 이번 세션 ───        [결과 복사] [다시 시작 primary]   │
│ 두벌식 표준 · 짧은 글                                       │
├─ BigStatsCard (4 cell + 3 dividers) ──────────────────────┤
│   38 WPM   192 CPM   92 %   8 %                            │
│   분당단어 │ 분당글자 │ 정확도(c) │ 오타율(w)                │
│   +4       │ +22      │ +1.2     │ −1.2                    │
├─ Duration | KeyCount ─────────────────────────────────────┤
│ 줄별 WPM       │ 입력 통계                                 │
│ ↗ sparkline    │ 입력 248  / 오타 20w / BS 14 / 01:18      │
├─ HeatmapSection ──────────────────────────────────────────┤
│ "키 오타 히트맵"                  적음 □□■■■ 많음          │
│ 이 자판에서 가장 자주 틀린 키                              │
│ ┌─ 5행 ANSI · 38×38 키 ─────────────────────────────────┐ │
│ │ hot 키: wrong 보더 + 2px wrongSoft glow + 카운트 배지  │ │
│ └────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

### 8.1 ResultHeader

- 좌: uppercase 11 라벨 `이번 세션` + 22/700 `<자판> · <지문>`.
- 우: `PillButton` × 2 (라벨 `결과 복사` / `다시 시작` primary=accent bg, 흰글씨).

### 8.2 BigStatsCard

- 카드 padding `24px 20px`, 라운드 14, `shadow`.
- `grid-template-columns: 1fr 1fr 1fr 1fr` + 절대 위치 divider 3개 (top 24 / bottom 24, 1px `border`).
- BigStat 셀:
  - 상단 baseline row: `<44/700/tnum/letter-spacing -0.02em [색]> <12/600 fgDim letter-spacing 0.04em "WPM"|"%">`.
  - 하단 row: `<12 fgDim 라벨> <delta-chip: mono 10/700, padding 1×5, 라운드 4, accentSoft, accent 색>`.
  - 정확도 색 = `correct`, 오타율 색 = `wrong`. delta 부호 보존 (+4 / +22 / +1.2 / −1.2).

### 8.3 DurationCard — Sparkline

- 카드 padding 14. 상단 라벨 `줄별 WPM` (11/500 uppercase fgDim).
- 본체: 220×56 SVG `Sparkline`.
  - 면적: `linearGradient accent 0.25 → 0`.
  - 라인: `stroke accent, 1.5px, vector-effect: non-scaling-stroke`.
  - 마지막 점만 dot (`r=1.5`, `accent`).
- Rust 구현: `gtk::DrawingArea` + Cairo, `PracticeSession.wpm_per_line: Vec<f64>`.

### 8.4 KeyCountCard

- 카드 padding 14, vertical gap 8.
- 상단 라벨 `입력 통계` (11/500 uppercase fgDim).
- KeyCountRow: `<12 fgDim 라벨>` ↔ `<12/600 [색] mono tnum 값>`.
- 행: `입력한 글자 248` / `오타 20` (wrong) / `백스페이스 14` / `소요 시간 01:18`.

### 8.5 HeatmapSection

- 카드 padding 18. 상단 row: 좌 2단(`14/700 "키 오타 히트맵"` + `11/fgDim "이 자판에서 가장 자주 틀린 키"`) / 우 `HeatmapLegend`.
- Legend: `적음 □□■■■ 많음` (5칸 16×10 라운드 2, intensity 0.1 / 0.3 / 0.5 / 0.7 / 0.9).
- Board: 메인 키보드와 **동일한 `KEYBOARD_ROWS`** 사용. 키 크기 38×38, gap 4, row-gap 4, 중앙 정렬.
- 일반 키: bg = `color-mix(in oklch, wrong α%, cardBgSoft)`. 보더 = `border`.
- **hot 키 (`intensity > 0.5`)**: 보더 `wrong`, 외부 `box-shadow: 0 0 0 2px wrongSoft`, 우상단에 **카운트 배지** (wrong 배경 + 흰글씨 mono 9/700, padding `1px 5px`, 라운드 6, `Math.round(heat * 20)`).
- 4-corner 라벨 유지: 영문 보조 8px `fgFaint`(hot=흰), 한글 보조 8px 우측, 영문 Base 12/700 mono, 한글 Base 12/700 accent(hot=흰).
- 특수키: `cardBgSoft`, `border`, 중앙 라벨 (10/500 `fgFaint`).

### 8.6 EmptyResult

- center vertical, gap 16, padding 40.
- 아이콘 컨테이너 72×72 라운드 36, `cardBgSoft`, `fgFaint` 색. 32×32 막대 그래프 SVG.
- 텍스트 max-width 320, center:
  - `<16/700 "아직 완료된 세션이 없어요">`
  - `<13/fgDim/line-height 1.5 "한 회 끝내면 WPM · 정확도 · 오타 히트맵이 여기에 표시됩니다.">`
- `PillButton primary "연습으로 가기"`.

## 9. 데몬 비활성 다이얼로그 (`DaemonDialog`, `variant: "stepped"`)

- **`Adw.AlertDialog` 권장** (MessageDialog 대체).
- 폭 440. `bgRaised`, 라운드 14, `borderStrong` 보더, 큰 외곽 shadow.
- 상단 영역 padding `24/24/16/24`:
  - 아이콘 컨테이너: **48×48 라운드 24, `wrongSoft` 배경, `wrong` 색** 경고 글리프 (22×22 line stroke).
  - 타이틀: `<16/700 "UNIM 이 실행 중이 아닙니다">`.
  - 본문: `<13/fgDim/line-height 1.55 "타자 연습은 UNIM 입력기가 실행 중일 때만 동작합니다. 아래 단계를 따라 주세요.">`.
- 3-step 본문 (gap 10):

  | # | title | body |
  |---|---|---|
  | 1 | 시스템 트레이에서 UNIM 켜기 | 패널의 UNIM 아이콘을 클릭해 활성화합니다. |
  | 2 | 이 앱을 다시 열기 | UNIM 이 켜진 뒤 본 창을 다시 실행합니다. |
  | 3 | 자판이 자동 인식되는지 확인 | 헤더에 자판 이름이 표시되면 준비 완료입니다. |

  - step 번호: **22×22 라운드 11, `accentSoft` + `accent`, mono 11/700**.
  - title: 13/600. body: 12/fgDim/line-height 1.45.
- 푸터 padding `12px 16px`, top border `border`, 배경 미세 틴트 (라이트 `rgba(0,0,0,0.02)`).
- 푸터 버튼: `[도움말]` (secondary) + **`[닫고 종료]` (primary)**.

## 10. 인터랙션 / 상태머신

1. **앱 부팅** — `daemon_check::is_daemon_running()` → false 이면 **DaemonDialog 3-step** → 닫고 종료. true 이면 메인 윈도우.
2. **첫 세션 자동 시작** — `corpus.short` 로딩 → 첫 줄을 TargetCard 의 current 줄로, 나머지 pending. 입력 포커스.
3. **타이핑** (`gtk::Entry`):
   - `connect_changed` (commit text) → `do_evaluate(committed, allow_complete=true)`.
   - `gtk::Entry::delegate() → gtk::Text::connect_preedit_changed` (조합 중) → `preedit_text = pre`; `do_evaluate(committed, allow_complete=false)`.
   - `connect_activate` (Enter) → `finalize_line(text)` 수동 진행.
4. **평가** (`PracticeSession::evaluate`): `combined = committed + preedit` 와 target prefix 비교 → `LineEval { correct_prefix, input_chars, target_chars, line_complete, progress }`.
5. **색칠** (`paint_target`): 전체 `dim` → 현재 줄 char 별 `correct` / `wrong (+ underline wavy)` / `dim` + caret.
6. **첫 입력** → 현재 줄 spinner 시작 (회전 삼각형).
7. **줄 완료** (`line_complete && preedit.empty`):
   - `commit_line(text)` → 누적 stats + key_stats 가산.
   - 다음 줄: `advance_to_line()` → Entry clear (`drop(sess_ref)` 후 `set_text("")`) → markers 갱신 → grab_focus.
   - 마지막 줄: heatmap 채움 + view_stack "result" + Toast `toast_practice_done`.
8. **키맵 시각 피드백**: body 에 `EventControllerKey` (Capture phase), press/release 모두 `KeyboardView::flash_key(keyval)` (150 ms `ease-out` auto-fade). IME 가 가로채도 깜박임 보장.
9. **100 ms tick**: `sess.tick()` + 4 StatCell 갱신.
10. **헤더바**:
    - CorpusDropdown 변경 → `start_session()`.
    - ↻ → `start_session()`.
    - ⋯ → `win.copy-result` / `win.show-result` / `win.show-practice` + `키 입력 기록 내보내기` / `설정` (확장 슬롯).

### 10.1 모션 명세

| 대상 | 속성 | duration | easing |
|---|---|---|---|
| 키캡 hover/pressed | `all` | 120 ms | `ease-out` |
| TargetLine 상태 전환 | `background` | 200 ms | `ease` |
| Spinner | `rotateX(0 → 360deg)` | 1.1 s | `cubic-bezier(0.65, 0.05, 0.35, 1)` infinite |
| Caret blink | `opacity` | 1 s | `steps(2)` infinite |
| 카드 hover | (없음 — 본 시안은 카드 hover 효과 없음) | — | — |

## 11. 접근성 (DESIGN.md §5)

| 항목 | 기준 |
|---|---|
| Correct `#1c66c9` on `cardBg` (라이트) | ≥ WCAG AA (7.4 : 1) |
| Wrong `#d24e15` on `cardBg` (라이트) | ≥ WCAG AA (4.9 : 1) |
| **오타 표시** | 색 + `text-decoration: underline wavy` (1.5px) **두 신호 동시 필수** |
| 최소 터치/클릭 타깃 | 30 × 30 (헤더 버튼) 이상 |
| 키캡 본문 라벨 | 15 px, 보조 라벨 11 px |
| 색약 대안 (deuteranopia) | Correct `#0077bb` (L 47 / C 0.15) · Wrong `#cc3311` (L 47 / C 0.18) |
| IME 상태 가시화 | InputField 우측 칩 (`한글` / `EN`) **항상** 노출 |
| 데몬 비활성 경고 | 아이콘 + 색 + 텍스트 + step 가이드 4중 신호 |

## 12. 카피 (i18n key 1:1)

| key | ko | en |
|---|---|---|
| `app_title` | UNIM 타자 연습 | UNIM Typing Practice |
| `practice_corpus` | 연습 지문 | Corpus |
| `corpus_short / _medium / _long` | 짧은/중간/긴 글 | Short / Medium / Long |
| `stat_wpm / _cpm` | WPM / CPM | WPM / CPM |
| `stat_accuracy / _error_rate` | 정확도 / 오타율 | Accuracy / Error rate |
| `btn_restart` | 다시 시작 | Restart |
| `toast_practice_done` | 연습 완료 — WPM %{wpm}, 정확도 %{acc}% | Done — WPM %{wpm}, accuracy %{acc}% |
| `toast_copied` | 결과 복사됨 | Result copied |
| `hint_ime_warning` | 한글 IME가 켜져 있어도 이 창에서는 ASCII 키 입력만 수집합니다. | This window only collects ASCII keys, even if your Hangul IME is enabled. |
| `ime_warning_short` | ASCII 만 입력됩니다 | ASCII keys only |
| `line_position_caption` | 줄 위치 | Line position |
| `practice_active_layout` | 현재 자판 | Active layout |
| `progress_label` | 진행률 | Progress |
| `heatmap_title` | 키 오타 히트맵 | Key error heatmap |
| `view_practice / view_result` | 연습 / 결과 | Practice / Result |
| `header_restart_tooltip / menu_tooltip` | 처음부터 다시 / 메뉴 | Restart from the beginning / Menu |
| `menu_copy_result / view_heatmap / back_to_practice` | 결과 복사 / 오타 히트맵 보기 / 연습으로 돌아가기 | Copy result / Show error heatmap / Back to practice |
| `result_summary` | 이번 세션 | This session |
| `result_no_data` | 아직 완료된 세션이 없어요. 한 회 끝내면 WPM · 정확도 · 오타 히트맵이 여기에 표시됩니다. | No completed session yet. Finish one round to see WPM, accuracy, and the heatmap here. |
| `input_placeholder` | 여기에 그대로 따라 쳐 보세요 | Type the line above here |
| `dialog_unim_inactive_title` | UNIM 이 실행 중이 아닙니다 | UNIM is not running |
| `dialog_unim_inactive_body` | 타자 연습은 UNIM 입력기가 실행 중일 때만 동작합니다. 아래 단계를 따라 주세요. | Typing practice requires UNIM. Please follow the steps below. |
| `dialog_step_1 / _2 / _3` (신규) | 트레이에서 UNIM 켜기 / 이 앱을 다시 열기 / 자판이 자동 인식되는지 확인 | Turn UNIM on from the tray / Reopen this app / Verify the layout is detected |

## 13. claude.ai Design 산출 요청

DESIGN.md + design/app.jsx 시안이 이미 라이브 mockup 으로 살아 있다. 본 brief 는 그것을 입력으로 받아 **검토 + 추가 변형**을 요청한다.

### A. 시안 검토 (현 design/app.jsx 기준)

1. 라이트/다크 시각 위계 종합 검토 — 특히 우 컬럼 카드 위계(LinePos/Stats/Progress/IMEWarning)의 시각 비중이 적절한가.
2. **IMEWarning** 의 `wrongSoft` 배경 강도 — 라이트에서 다소 강해 보일 수 있음. 강도 단계 1~3 대안.
3. **Sparkline** 그라데이션 (현 0.25 → 0) — 다크에서 시인성 검증.
4. **Heatmap hot 키 카운트 배지** 의 위치(우상단)·색·폰트 weight 확인.
5. BigStat delta 칩이 항상 `accentSoft` 배경인데, 음의 delta (예: 오타율 −1.2 = 좋은 결과)도 같은 색이 맞는지 / 보색이 맞는지.

### B. 추가 시안

1. **작은 화면 폴백** (창 폭 < 900 px) — 2열 → 1열 stack vs 키맵 가로 스크롤. 시안 각 1.
2. **0.9× / 0.8× 키맵 축소** (1u = 41/37 px) 시안 — 4-corner 라벨 가독성 검증.
3. **Adw.Toast 시각** 다듬기 — `toast_practice_done` 의 한 줄 토스트 (WPM 38 / 정확도 92%).
4. **EmptyResult illustration** 대안 — 현재 막대 그래프 SVG → 더 의미 있는 메타포 (키보드, 결과 카드 placeholder 등) 2종.
5. **CorpusDropdown 펼침 상태** — 메타(줄 수)에 추가로 "약 15초", "약 1분", "약 3분" 예상 시간 라벨 추가안.
6. **헤더 자판 코드** 옆에 자판 종류 칩 (두벌식 / 세벌식 / 사용자) 부속안.

### C. 모션 추가 명세

1. **tri-flip spinner** 의 `cubic-bezier(0.65, 0.05, 0.35, 1)` 적정성 — `linear` / spring 비교 시안.
2. **줄 전환** (current → done): 현재 `background 200ms ease`. 추가로 line의 fade + 한 줄 슬라이드(컬렉션 위로 8px) 모션 검토.
3. **키 pressed 깜박임** — 현재 `all 120ms ease-out` vs spring vs `cubic-bezier(0.2, 0.7, 0.3, 1)` 비교.
4. **caret blink** — `steps(2)` vs `ease-in-out` 비교.

### D. 빈 상태 / 에러 카피

1. DaemonDialog step 3 ("자판이 자동 인식되는지 확인") — 자판 인식 실패 시 fallback step (4. UNIM 설정에서 자판 선택) 추가안.
2. **데몬 *재시작 중*** 상태 — 부팅 직후 NameOwner 가 잠시 없을 수 있다. 0.5s 대기 후 재시도 → 그래도 없으면 dialog 라는 흐름의 *대기 spinner* 시안.

## 14. 변경 불가 / 잠금 항목

| 결정 | 출처 | 이유 |
|---|---|---|
| 좌(예시+입력) · 우(통계+진행+위치) 2열 + 키맵 중앙 | 6·7차 가결 | 공간 효율 |
| 예시·입력 같은 mono 16pt | 5차 가결 / DESIGN.md §3 | 줄 비교 정렬 |
| 회전 삼각형 spinner | DESIGN.md §4.2 / app.jsx | 시안 결정 |
| 키캡 4-corner 라벨 (좌상 영문 Shift / 좌하 영문 Base / 우상 한글 Shift / 우하 한글 Base) | 1·7차 가결 / DESIGN.md §4.4 | 실제 키보드 관습 |
| 입력란 무색 (색은 예시글에만) | 4차 가결 | 시각 잡음 최소 |
| 5행 폭 일치 (Row 4 우측 공백 없음) | 3차 가결 | 시각 균형 |
| 본문 외곽 padding `16px 20px 20px 20px` | DESIGN.md §4.2 | |
| 데몬 비활성 → AlertDialog 3-step → 종료 | DESIGN.md §4.5 | UNIM 의존성 |
| preedit 도 평가에 포함, 완료는 `commit && preedit.empty` 에만 | 7차 가결 | IME 조합 중 오발 방지 |
| 오타 = 색 + `underline wavy` 두 신호 | DESIGN.md §1·§5 | 색약 보조 필수 |
| 카드 위계 1단 (중첩 카드 금지) | DESIGN.md §1 | |
| 통계 숫자 `tnum` | DESIGN.md §1 | 흔들림 방지 |
| 토큰만 사용 (직접 색 코드 금지) | DESIGN.md §1 | 라이트/다크 자동 |

## 15. 구현 매핑 (Rust + GTK4 — DESIGN.md §6)

### 15.1 적용 순서

1. `apply_css()` 의 CSS 교체 → 시각 70 % 완성.
2. `practice_page.rs`: StatCell / IMEWarning / ProgressCard 위계 정리.
3. `keyboard_view.rs`: 4-corner 라벨 + 한글 accent 색 + pressed 상태.
4. `result_page.rs` (신규/확장): BigStatsCard + Sparkline DrawingArea + Duration/KeyCount + HeatmapSection.
5. 히트맵 색 함수 교체 + 카운트 배지.
6. `MessageDialog` → `Adw.AlertDialog` + 3-step 본문.

### 15.2 CSS 스니펫 (DESIGN.md §6.2, 붙여넣기 시작점)

```css
/* Cards */
.card {
  background: @card_bg_color;
  border: 1px solid alpha(@borders, 0.6);
  border-radius: 12px;
  box-shadow: 0 1px 2px alpha(black, 0.04);
}

/* Typing target */
textview.typing-target text { color: alpha(@view_fg_color, 0.35); }
.tt-correct { color: @accent_color; font-weight: 600; }
.tt-wrong   { color: #d24e15; font-weight: 600;
              text-decoration: underline wavy;
              text-decoration-thickness: 1.5px; }

/* Current line marker */
.typing-target-marker-row.current {
  background: alpha(@accent_color, 0.08);
  border-left: 3px solid @accent_color;
}

/* Input */
entry.typing-input {
  border: 1.5px solid @accent_color;
  border-radius: 12px;
  padding: 10px 14px;
  box-shadow: 0 0 0 4px alpha(@accent_color, 0.10);
  font-family: "JetBrains Mono", "D2Coding", monospace;
  font-size: 16pt;
}

/* Stats */
.stat-value { font-family: "JetBrains Mono"; font-size: 24px;
              font-weight: 700; font-feature-settings: "tnum"; }
.stat-unit  { font-size: 10px; font-weight: 600; opacity: 0.6; }
.stat-label { font-size: 11px; opacity: 0.6; }

/* Keycap */
.kbv-key {
  background: @card_bg_color;
  border: 1px solid alpha(@borders, 0.8);
  border-bottom: 2px solid alpha(@borders, 1.0);
  border-radius: 7px;
  box-shadow: inset 0 1px 0 alpha(white, 0.6);
  transition: all 120ms ease-out;
}
.kbv-key:hover { border-color: alpha(@accent_color, 0.5); }
.kbv-key.kbv-pressed {
  background: @accent_bg_color;
  border-color: @accent_color;
  color: @accent_fg_color;
  box-shadow: inset 0 1px 2px alpha(black, 0.15);
}
.kbv-corner { font-size: 11px; opacity: 0.55; }
.kbv-main   { font-size: 15px; font-weight: 700; }
.kbv-hangul { color: @accent_color; }
```

### 15.3 데이터 모델 변경

- `PracticeSession`: **`wpm_per_line: Vec<f64>`** 추가 (Sparkline 입력 — 줄 commit 시점에 그 줄의 누적 WPM 푸시).
- `KeyStatsTable`: `key -> (typed, errors)` 누적, intensity = `errors / typed`. `HEAT_HOT = 0.5` const.
- Heatmap: 표본이 부족할 때(< 20 글자) Section 자체를 숨기는 것이 자연스러움 (미해결).

### 15.4 위젯 클래스 컨벤션

- 카드: 모든 wrapper `Frame` 에 `.card`.
- 통계 셀: `Box vertical` → `(Box horizontal [.stat-value + .stat-unit], .stat-label)`.
- 키캡: `Button` 또는 `Frame` → `.kbv-key`, pressed 시 `.kbv-pressed`.
- 히트맵 셀: 일반 키 그대로 + intensity 클래스(`.heat-1`~`.heat-5`) 또는 인라인 background.

## 16. 미해결 / 결정 필요 (DESIGN.md §7)

- **자판별 기본 폰트** — D2Coding 미설치 환경에서 폴백 우선순위 확정 필요.
- **결과 비교 기간** — delta 칩의 기준이 "직전 세션"인지 "최근 5회 평균"인지.
- **히트맵 표본 부족** — N < 20 글자일 때는 히트맵 자체를 숨기는 게 맞아 보임.
- **다크 wrong 색** — 현재 `#ff9560` 은 다크 배경에서 약간 채도 과함. 사용자 테스트 후 톤 다운 검토.
- **세그먼티드 진행 바** — `LevelBar(discrete)` vs 수동 Box 20개. 후자가 색 제어 자유로움.

## 17. 코드 / 시안 진입점

- 실 코드 (Rust): [src/main.rs](../src/main.rs) · [src/app.rs](../src/app.rs) · [src/practice_page.rs](../src/practice_page.rs) · [src/practice_engine.rs](../src/practice_engine.rs) · [src/daemon_check.rs](../src/daemon_check.rs) · [src/active_layout.rs](../src/active_layout.rs) · [src/corpus.rs](../src/corpus.rs)
- 키보드 위젯: [unim-keymap-common/src/keyboard_view.rs](../../unim-keymap-common/src/keyboard_view.rs) — 040eb8b 에서 공유 위젯으로 이관
- i18n: [locales/ko.yml](../locales/ko.yml) · [locales/en.yml](../locales/en.yml)
- 지문 데이터: [data/corpus_ko.txt](../data/corpus_ko.txt)
- 시안 SoT (텍스트): [DESIGN.md](../DESIGN.md)
- 시안 SoT (라이브 React): [design/template.html](../design/template.html) · [design/design-canvas.jsx](../design/design-canvas.jsx) · [design/app.jsx](../design/app.jsx)
