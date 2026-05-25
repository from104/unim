# UNIM 타자 연습 — 디자인 명세

> GTK4 + libadwaita 기반 한글 타자 연습 앱의 현대적 재설계.
> 본 문서는 `UNIM 타자 연습.html` 시안을 실 코드(Rust + GTK4)로 옮기기 위한 단일 소스 오브 트루스.

---

## 1. 디자인 원칙

1. **모노스페이스가 사용자 작업의 중심**이다. 예시글·입력란·통계 숫자·키캡 라벨은 모두 `JetBrains Mono`. 그 외 UI 텍스트만 `Pretendard`.
2. **색은 의미를 갖는다.** `accent` = 진행/일치, `wrong` = 오타. 색약 보조를 위해 오타는 항상 *색 + 물결 밑줄* 두 가지 신호.
3. **카드 위계 1단.** 헤더 / 본문 카드 / 키보드 카드 3개 층위만 허용. 중첩 카드 금지.
4. **숫자에는 폰트 옵션 `tnum`**. 통계가 흔들리지 않게.
5. **라이트/다크는 토큰 한 벌로 자동 추종.** 색을 직접 적지 말고 토큰(또는 Adwaita 변수)을 쓴다.

---

## 2. 디자인 토큰

`app.jsx` 의 `tokensLight` / `tokensDark` 가 SoT. GTK 측에서는 가능하면 Adwaita 변수로 매핑한다.

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
| `accent` | `#1c66c9` | `#62a0ea` | `@accent_color` |
| `accentSoft` | `rgba(28,102,201,0.10)` | `rgba(98,160,234,0.16)` | `alpha(@accent_color, 0.12)` |
| `correct` | `#1c66c9` | `#62a0ea` | `@accent_color` |
| `wrong` | `#d24e15` | `#ff9560` | `@error_color` (커스텀 권장) |
| `wrongSoft` | `rgba(210,78,21,0.12)` | `rgba(255,149,96,0.14)` | `alpha(@error_color, 0.14)` |

### 그림자

| 변수 | 값 |
|---|---|
| `shadow` (카드) | `0 1px 2px rgba(0,0,0,0.04), 0 1px 1px rgba(0,0,0,0.02)` |
| `shadowKey` (키캡) | `inset 0 1px 0 rgba(255,255,255,0.9), 0 1px 0 rgba(0,0,0,0.04), 0 2px 3px rgba(0,0,0,0.04)` |

### 라운드 / 보더

- 카드 `border-radius: 12px`
- 윈도우 `border-radius: 14px`
- 키캡 `border-radius: 7px`
- 카드 보더 `1px solid @borders`
- 입체감을 주는 위젯(키캡·진행률 셀)은 `border-bottom: 2px solid borderStrong` 사용

---

## 3. 타이포그래피

```
ui.sans   = "Pretendard Variable", "Pretendard",
            "Noto Sans CJK KR", "Cantarell", sans-serif
ui.mono   = "JetBrains Mono", "D2Coding",
            "Adwaita Mono", ui-monospace, monospace
```

| 역할 | 폰트 | 크기 | 굵기 | 비고 |
|---|---|---|---|---|
| 타이틀 (헤더) | sans | 13 | 600 | |
| 헤더 자판 코드 | mono | 10 | 400 | subtitle |
| 본문 (예시글) | mono | 16 | 400 / 600 | letter-spacing 0.01em |
| 입력란 | mono | 16 | 400 | 캐럿은 fg, blink 1s |
| 통계 큰 숫자 | mono | 24~44 | 700 | `font-feature-settings: "tnum"` |
| 통계 단위 (`WPM`, `%`) | sans | 10~12 | 600 | `letter-spacing: 0.04em` |
| 카드 라벨 (`줄 위치`, `진행률`) | sans | 11 | 500 | uppercase, `letter-spacing: 0.04em` |
| 키캡 메인 라벨 | mono / sans | 15 | 700 | 영문 mono, 한글 sans |
| 키캡 보조 라벨 (Shift) | mono / sans | 11 | 500 | 한글은 `accent` 색 |
| 특수키 라벨 | sans | 12 | 600 | |

---

## 4. 컴포넌트

### 4.1 윈도우 셸 (`WindowChrome`)
```
┌─[ HeaderBar 46h ]────────────────────────────────────────┐
│ [Corpus ▾]      UNIM 타자 연습          [↻] [⋯] ● ● ●    │
│                 두벌식 표준 · ko_2bulstd                  │
├──────────────────────────────────────────────────────────┤
│   ┌─[ ViewSwitcher segmented ]─┐                          │
│   │ ⌨ 연습  |  📊 결과         │                          │
│   └────────────────────────────┘                          │
│                                                           │
│   [ Page content ]                                        │
└──────────────────────────────────────────────────────────┘
```
- 윈도우 폭 고정 `900px`. 연습 화면 높이 `755px`, 결과 화면 `820px`.
- HeaderBar 좌측: 지문(corpus) 선택 SplitButton.
- HeaderBar 우측: 재시작 / 메뉴 / Window Controls.
- ViewSwitcher: `ToggleButtonGroup` 또는 `AdwViewSwitcher`.

### 4.2 Practice 페이지

```
grid-template-columns: 1fr 280px
grid-gap: 18px
padding: 16px 20px 20px 20px

┌──────────────────────┐ ┌──────────────┐
│ TargetCard (.card)   │ │ LinePosCard  │
│ ┌  done line          │ ├──────────────┤
│ ┃  ▶ current  (←caret)│ │ StatsCard    │
│   pending line       │ │ (2×2 grid)   │
│                      │ ├──────────────┤
│                      │ │ ProgressCard │
│                      │ │ (20 segments)│
│                      │ ├──────────────┤
│                      │ │ IMEWarning   │
└──────────────────────┘ └──────────────┘
┌──────────────────────┐
│ InputField (accent)  │
└──────────────────────┘
[ Keyboard — 5 rows ANSI 106, centered ]
```

**TargetCard 줄 상태**
- `done`: 글자 색 `fgFaint`, 좌측 16px 컬럼에 체크 아이콘.
- `current`: 배경 `accentSoft`, 좌측 3px `accent` 보더, 컬럼에 스피너.
- `pending`: 글자 색 `dim`, 컬럼 빈칸.

**스피너** — *오른쪽으로 뾰족한 삼각형, 가로축(rotateX) 회전*
```css
@keyframes tri-flip { 0% { transform: rotateX(0); } 100% { transform: rotateX(360deg); } }
```
`<svg>` 안에 `<polygon points="3,2 12,7 3,12" fill="accent" />`, `animation: tri-flip 1.1s ease-in-out infinite`.

**InputField**
- 보더 `1.5px solid accent` + 링 `box-shadow: 0 0 0 4px accentSoft`.
- 우측에 IME 상태 칩 (`한글` / `EN`).

**ProgressCard**
- 20 segment, segment 폭 균등, gap 2px, 높이 6px.
- filled: `accent`, partial: `accentSoft`, empty: `cardBgSoft` + 보더.

### 4.3 Result 페이지

```
ResultHeader (제목 + [결과 복사] [다시 시작])
BigStatsCard (4-cell, divider, 큰 숫자 + delta chip)
DurationCard (Sparkline) | KeyCountCard (KV rows)
HeatmapSection (메인 키보드와 동일 5행 ANSI)
```

**BigStat**
- 숫자 44px / 700w / `letter-spacing: -0.02em`.
- delta 칩: 5px×1px 패딩, `accentSoft` 배경, mono 10/700. 부호 보존 (+/-) 그대로.

**Sparkline** — `gtk::DrawingArea`, 220×56, Cairo.
- 면적: `accent` 0.25 → 0 그라데이션.
- 라인: `accent`, 1.5px.
- 마지막 점만 dot.

**Heatmap**
- *메인 키보드와 동일한* `KEYBOARD_ROWS` 사용 (단일 소스).
- 일반 키 size 38×38, 특수키 회색 비활성 표시.
- 셀 배경 = `mix(cardBgSoft, wrong, intensity)`.
- `intensity > 0.5` 인 hot 키: 보더 `wrong`, 외부 `0 0 0 2px wrongSoft` 글로우, 우상단에 카운트 배지.
- 4-corner 라벨 시스템은 그대로 유지.

### 4.4 Keyboard

| 속성 | 값 |
|---|---|
| 키 단위(`KEY_U`) | 46 |
| 키 높이(`KEY_H`) | 50 |
| 키 사이 gap | 5 |
| 행 사이 gap | 5 |
| 카드 padding | 12 |
| 카드 배경 | `cardBgSoft` |

**키캡 (일반)**
- 2×2 grid:
  - 좌상: 영문 Shift (`mono`, fgFaint, 11px)
  - 우상: 한글 Shift (sans, fgFaint, 11px, 우측 정렬)
  - 좌하: 영문 Base (`mono`, fg, 15px / 700)
  - 우하: 한글 Base (sans, **accent**, 15px / 700, 우측 정렬)
- 보더 `1px @borders` + `border-bottom: 2px @bordersStrong` → 약한 입체감.
- 눌림 (`pressed`): 배경 `accent`, 텍스트 흰색, `border-bottom` 사라짐, inset shadow.
- 호버: 보더만 `alpha(accent, 0.5)` 로.

**키캡 (특수)**
- 가운데 정렬, sans 12/600. 색은 `fgDim`.
- 폭은 `unit` 값으로 비례 계산: `width = unit * KEY_U + (unit - 1) * gap`.

### 4.5 데몬 비활성 다이얼로그
- `Adw.AlertDialog` 권장 (메시지 다이얼로그 대신).
- 상단 아이콘: 44px 라운드, `wrongSoft` 배경, `wrong` 색 경고 글리프.
- 본문은 3-step 가이드. step number는 22px 라운드 칩, `accentSoft` + `accent`, mono 11/700.
- 푸터: `[도움말]` (secondary) + `[닫고 종료]` (primary).

---

## 5. 접근성

| 항목 | 기준 |
|---|---|
| Correct (#1c66c9) on cardBg | ≥ WCAG AA (7.4:1) |
| Wrong (#d24e15) on cardBg | ≥ WCAG AA (4.9:1) |
| 오타 표시 | 색 + 물결 밑줄(`text-decoration: underline wavy`) 항상 함께 |
| 최소 터치 / 클릭 타깃 | 30×30 (헤더 버튼) 이상 |
| 키캡 라벨 | 본문 15px 유지 (4-corner 보조라벨은 11px) |
| 색약 (deuteranopia) 대안 팔레트 | Correct `#0077bb` / Wrong `#cc3311` |
| IME 상태 | 입력란 우측 칩으로 항상 가시화 |
| 데몬 비활성 경고 | 아이콘 + 색 + 텍스트 3중 신호 |

---

## 6. 구현 매핑 (Rust + GTK4)

### 6.1 적용 순서
1. `apply_css()` 의 CSS 교체 → 시각 70% 완성
2. `practice_page.rs`: stat chip / IME 경고 / ProgressCard 위계 정리
3. `keyboard_view.rs`: 4-corner 라벨 + 한글 accent 색 + pressed 상태
4. `result_page.rs`: BigStatsCard + Sparkline DrawingArea
5. 히트맵 색 함수 교체 + 카운트 배지
6. `MessageDialog` → `Adw.AlertDialog` + 3-step 본문

### 6.2 CSS 스니펫 (붙여넣기 시작점)

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

### 6.3 데이터 모델 변경
- `PracticeSession`: `wpm_per_line: Vec<f64>` 추가 (sparkline 입력).
- `KeyStatsTable`: `key -> (typed, errors)` 누적, `errors/typed` 비율로 intensity 계산.
- Heatmap: 임계값(`HEAT_HOT = 0.5`)을 const 로 둔다.

### 6.4 위젯 클래스 컨벤션
- 카드: 모든 wrapper `Frame` 에 `.card`.
- 통계 셀: `Box vertical` → `(Box horizontal [.stat-value + .stat-unit], .stat-label)`.
- 키캡: `Button` → `.kbv-key` + 상태시 `.kbv-pressed`.
- 히트맵 셀: 일반 키 그대로 + intensity 클래스(`.heat-1` ~ `.heat-5`) 또는 인라인 background.

---

## 7. 미해결 / 결정 필요

- **자판별 기본 폰트** — D2Coding 미설치 환경에서 폴백 우선순위 확정 필요.
- **결과 비교 기간** — delta 칩의 기준이 "직전 세션"인지 "최근 5회 평균"인지.
- **히트맵 표본 부족** — N < 20 글자일 때는 히트맵 자체를 숨기는 게 맞아 보임.
- **다크모드 wrong 색** — 현재 `#ff9560` 은 다크 배경에서 약간 채도 과함. 사용자 테스트 후 톤 다운 검토.
- **세그먼티드 진행 바** — `LevelBar(discrete)` vs 수동 Box 20개. 후자가 색 제어 자유로움.
