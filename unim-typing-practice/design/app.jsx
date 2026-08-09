/* UNIM 타자 연습 — modern redesign components */

// ============================================================
// Design tokens
// ============================================================
const tokensLight = {
  bg: "#fafaf9",
  bgRaised: "#ffffff",
  fg: "#1c1c1c",
  fgDim: "#6b6b6b",
  fgFaint: "#a8a8a4",
  border: "rgba(20, 20, 20, 0.10)",
  borderStrong: "rgba(20, 20, 20, 0.18)",
  cardBg: "#ffffff",
  cardBgSoft: "#f3f3f0",
  correct: "#1c66c9",
  correctSoft: "rgba(28, 102, 201, 0.10)",
  wrong: "#d24e15",
  wrongSoft: "rgba(210, 78, 21, 0.12)",
  dim: "#b6b6b2",
  accent: "#1c66c9",
  accentSoft: "rgba(28, 102, 201, 0.10)",
  shadow: "0 1px 2px rgba(0,0,0,0.04), 0 1px 1px rgba(0,0,0,0.02)",
  shadowKey: "inset 0 1px 0 rgba(255,255,255,0.9), 0 1px 0 rgba(0,0,0,0.04), 0 2px 3px rgba(0,0,0,0.04)"
};
const tokensDark = {
  bg: "#1d1d1d",
  bgRaised: "#262626",
  fg: "#ededed",
  fgDim: "#9a9a96",
  fgFaint: "#6b6b67",
  border: "rgba(255, 255, 255, 0.08)",
  borderStrong: "rgba(255, 255, 255, 0.14)",
  cardBg: "#2a2a2a",
  cardBgSoft: "#222222",
  correct: "#62a0ea",
  correctSoft: "rgba(98, 160, 234, 0.14)",
  wrong: "#ff9560",
  wrongSoft: "rgba(255, 149, 96, 0.14)",
  dim: "#5e5e5b",
  accent: "#62a0ea",
  accentSoft: "rgba(98, 160, 234, 0.16)",
  shadow: "0 1px 2px rgba(0,0,0,0.30), 0 1px 1px rgba(0,0,0,0.15)",
  shadowKey: "inset 0 1px 0 rgba(255,255,255,0.05), 0 1px 0 rgba(0,0,0,0.30), 0 2px 3px rgba(0,0,0,0.20)"
};

// ============================================================
// Window chrome (libadwaita-style)
// ============================================================
function WindowChrome({ tokens, dark, children, title = "UNIM 타자 연습", subtitle = "두벌식 표준 · ko_2bulstd", view = "practice", showCorpusDropdown = false, showMenu = false, height = 820 }) {
  return (
    <div style={{
      width: 900, height,
      background: tokens.bg, color: tokens.fg,
      borderRadius: 14, overflow: "hidden",
      boxShadow: dark ?
      "0 24px 48px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.06)" :
      "0 24px 48px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.06)",
      fontFamily: '"Pretendard", "Inter", -apple-system, "Segoe UI", sans-serif',
      fontSize: 14, position: "relative",
      display: "flex", flexDirection: "column"
    }}>
      <HeaderBar tokens={tokens} dark={dark} title={title} subtitle={subtitle} showCorpusDropdown={showCorpusDropdown} showMenu={showMenu} />
      <ViewSwitcher tokens={tokens} view={view} />
      <div style={{ flex: 1, minHeight: 0, position: "relative", overflow: "hidden" }}>
        {children}
      </div>
    </div>);

}

function HeaderBar({ tokens, dark, title, subtitle, showCorpusDropdown, showMenu }) {
  return (
    <div style={{
      height: 46, flexShrink: 0,
      borderBottom: `1px solid ${tokens.border}`,
      background: dark ? "rgba(255,255,255,0.02)" : "rgba(0,0,0,0.015)",
      display: "flex", alignItems: "center",
      padding: "0 8px", gap: 6, position: "relative"
    }}>
      <CorpusDropdown tokens={tokens} dark={dark} expanded={showCorpusDropdown} />
      <div style={{
        position: "absolute", left: "50%", top: "50%",
        transform: "translate(-50%, -50%)",
        textAlign: "center", lineHeight: 1.2
      }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: tokens.fg }}>{title}</div>
        <div style={{ fontSize: 10, color: tokens.fgDim, marginTop: 1, fontFamily: '"JetBrains Mono", ui-monospace, monospace' }}>{subtitle}</div>
      </div>
      <div style={{ flex: 1 }}></div>
      <HBarButton tokens={tokens}>
        <RestartIcon />
      </HBarButton>
      <div style={{ position: "relative" }}>
        <HBarButton tokens={tokens} active={showMenu}>
          <MenuIcon />
        </HBarButton>
        {showMenu && <MenuPopover tokens={tokens} dark={dark} />}
      </div>
      <WindowControls tokens={tokens} dark={dark} />
    </div>);

}

function HBarButton({ children, tokens, active }) {
  return (
    <button style={{
      width: 30, height: 30, borderRadius: 7,
      border: "none",
      background: active ? tokens.borderStrong : "transparent",
      color: tokens.fg, cursor: "pointer",
      display: "grid", placeItems: "center"
    }}>{children}</button>);

}

function CorpusDropdown({ tokens, dark, expanded }) {
  return (
    <div style={{ position: "relative" }}>
      <button style={{
        height: 30, padding: "0 10px 0 12px",
        borderRadius: 7,
        border: `1px solid ${tokens.border}`,
        background: tokens.cardBg,
        color: tokens.fg, fontSize: 13, fontWeight: 500,
        display: "flex", alignItems: "center", gap: 8, cursor: "pointer",
        fontFamily: "inherit"
      }}>
        <DocIcon />
        짧은 글
        <ChevronIcon />
      </button>
      {expanded &&
      <div style={{
        position: "absolute", top: 36, left: 0,
        width: 180, padding: 4,
        background: tokens.cardBg,
        border: `1px solid ${tokens.borderStrong}`,
        borderRadius: 10,
        boxShadow: dark ? "0 12px 28px rgba(0,0,0,0.6)" : "0 12px 28px rgba(0,0,0,0.12)",
        zIndex: 10
      }}>
          <MenuRow tokens={tokens} selected>짧은 글<span style={{ color: tokens.fgDim, fontSize: 11, fontFamily: '"JetBrains Mono", monospace' }}>3줄</span></MenuRow>
          <MenuRow tokens={tokens}>중간 글<span style={{ color: tokens.fgDim, fontSize: 11, fontFamily: '"JetBrains Mono", monospace' }}>8줄</span></MenuRow>
          <MenuRow tokens={tokens}>긴 글<span style={{ color: tokens.fgDim, fontSize: 11, fontFamily: '"JetBrains Mono", monospace' }}>20줄</span></MenuRow>
        </div>
      }
    </div>);

}

function MenuRow({ tokens, selected, children }) {
  return (
    <div style={{
      height: 30, padding: "0 10px",
      display: "flex", alignItems: "center", justifyContent: "space-between",
      gap: 8, borderRadius: 6,
      background: selected ? tokens.accentSoft : "transparent",
      color: selected ? tokens.accent : tokens.fg,
      fontSize: 13, fontWeight: selected ? 600 : 400,
      cursor: "pointer"
    }}>{children}</div>);

}

function MenuPopover({ tokens, dark }) {
  return (
    <div style={{
      position: "absolute", top: 36, right: 0,
      width: 210, padding: 4,
      background: tokens.cardBg,
      border: `1px solid ${tokens.borderStrong}`,
      borderRadius: 10,
      boxShadow: dark ? "0 12px 28px rgba(0,0,0,0.6)" : "0 12px 28px rgba(0,0,0,0.12)",
      zIndex: 20
    }}>
      <MenuRow tokens={tokens}>결과 복사<KbdHint tokens={tokens}>⌘C</KbdHint></MenuRow>
      <MenuRow tokens={tokens}>오타 히트맵 보기</MenuRow>
      <MenuRow tokens={tokens}>연습으로 돌아가기</MenuRow>
      <div style={{ height: 1, background: tokens.border, margin: "4px 6px" }}></div>
      <MenuRow tokens={tokens}>키 입력 기록 내보내기</MenuRow>
      <MenuRow tokens={tokens}>설정</MenuRow>
    </div>);

}

function KbdHint({ tokens, children }) {
  return <span style={{
    fontSize: 10, fontFamily: '"JetBrains Mono", monospace',
    color: tokens.fgDim,
    padding: "1px 5px", border: `1px solid ${tokens.border}`,
    borderRadius: 4
  }}>{children}</span>;
}

function WindowControls({ tokens, dark }) {
  return (
    <div style={{ display: "flex", gap: 6, marginLeft: 8 }}>
      <WinDot color="#febc2e" />
      <WinDot color="#28c840" />
      <WinDot color="#ff5f57" />
    </div>);

}
function WinDot({ color }) {
  return <div style={{ width: 12, height: 12, borderRadius: 6, background: color }}></div>;
}

function ViewSwitcher({ tokens, view }) {
  return (
    <div style={{
      flexShrink: 0,
      display: "flex", justifyContent: "center",
      padding: "12px 0 4px 0"
    }}>
      <div style={{
        display: "inline-flex",
        background: tokens.cardBgSoft,
        border: `1px solid ${tokens.border}`,
        borderRadius: 9,
        padding: 3
      }}>
        <SwitchTab tokens={tokens} active={view === "practice"} icon={<KeyboardIcon />}>연습</SwitchTab>
        <SwitchTab tokens={tokens} active={view === "result"} icon={<ChartIcon />}>결과</SwitchTab>
      </div>
    </div>);

}
function SwitchTab({ tokens, active, children, icon }) {
  return (
    <div style={{
      padding: "6px 18px", borderRadius: 6,
      display: "flex", alignItems: "center", gap: 8,
      fontSize: 13, fontWeight: 600,
      background: active ? tokens.cardBg : "transparent",
      color: active ? tokens.fg : tokens.fgDim,
      boxShadow: active ? tokens.shadow : "none",
      cursor: "pointer"
    }}>{icon}{children}</div>);

}

// ============================================================
// Icons (line, 14px)
// ============================================================
function Icon({ size = 14, children, stroke = "currentColor", strokeWidth = 1.6 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none"
    stroke={stroke} strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round">
      {children}
    </svg>);

}
const ChevronIcon = () => <Icon><path d="M4 6.5l4 3 4-3" /></Icon>;
const RestartIcon = () => <Icon><path d="M3 8a5 5 0 1 0 1.5-3.5" /><path d="M3 3v3h3" /></Icon>;
const MenuIcon = () => <Icon><circle cx="3" cy="8" r="0.8" fill="currentColor" stroke="none" /><circle cx="8" cy="8" r="0.8" fill="currentColor" stroke="none" /><circle cx="13" cy="8" r="0.8" fill="currentColor" stroke="none" /></Icon>;
const DocIcon = () => <Icon><path d="M3.5 2.5h6l2 2v9h-8z" /><path d="M5.5 7h5M5.5 9.5h5M5.5 12h3" /></Icon>;
const KeyboardIcon = () => <Icon><rect x="1.5" y="3.5" width="13" height="9" rx="1.5" /><path d="M4 6.5h.01M6.5 6.5h.01M9 6.5h.01M11.5 6.5h.01M4 9h.01M6.5 9h.01M9 9h.01M11.5 9h.01M4.5 11.5h7" /></Icon>;
const ChartIcon = () => <Icon><path d="M2 13.5h12" /><rect x="3.5" y="9" width="2" height="4" /><rect x="7" y="6" width="2" height="7" /><rect x="10.5" y="3.5" width="2" height="9.5" /></Icon>;
const WarnIcon = () => <Icon strokeWidth={1.4}><path d="M8 2.5L14 13H2z" /><path d="M8 6.5v3" /><circle cx="8" cy="11.2" r="0.5" fill="currentColor" stroke="none" /></Icon>;
const CheckIcon = () => <Icon strokeWidth={1.8}><path d="M3 8.5L6.5 12 13 4.5" /></Icon>;
const SparkleIcon = () => <Icon><path d="M8 2v3M8 11v3M2 8h3M11 8h3M4.5 4.5l2 2M9.5 9.5l2 2M11.5 4.5l-2 2M6.5 9.5l-2 2" /></Icon>;

// ============================================================
// Practice page
// ============================================================
function PracticePage({ tokens, dark }) {
  const lines = [
  { text: "오늘도 즐거운 하루를 보내자.", state: "done" },
  {
    text: "한글은 과학적인 문자입니다.",
    state: "current",
    typed: "한글은 과혁",
    correct: 4 // "한글은 "
  },
  { text: "하늘이 푸르고 바람이 시원하다.", state: "pending" },
  { text: "꾸준한 연습은 정확도를 만든다.", state: "pending" },
  { text: "조금 천천히, 그러나 정확하게.", state: "pending" }];

  return (
    <div style={{
      padding: "16px 20px 20px 20px",
      display: "grid", gridTemplateColumns: "1fr 280px", gap: 18,
      gridTemplateRows: "auto 1fr",
      boxSizing: "border-box", height: "680px"
    }}>
      {/* Left column */}
      <div style={{
        display: "flex", flexDirection: "column", gap: 12,
        minWidth: 0
      }}>
        <TargetCard tokens={tokens} dark={dark} lines={lines} />
        <InputField tokens={tokens} dark={dark} value="한글은 과혁" />
      </div>

      {/* Right column */}
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <LinePosCard tokens={tokens} current={2} total={5} />
        <StatsCard tokens={tokens} dark={dark} />
        <ProgressCard tokens={tokens} progress={0.34} />
        <IMEWarning tokens={tokens} />
      </div>

      {/* Keyboard - spans both columns */}
      <div style={{ gridColumn: "1 / -1", display: "flex", justifyContent: "center", height: "300px" }}>
        <Keyboard tokens={tokens} dark={dark} pressed="g" />
      </div>
    </div>);

}

function TargetCard({ tokens, dark, lines }) {
  return (
    <div style={{
      flex: 1, minHeight: 0,
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: "14px 0",
      boxShadow: tokens.shadow,
      overflow: "hidden", position: "relative",
      display: "flex", flexDirection: "column"
    }}>
      {lines.map((line, i) =>
      <TargetLine key={i} tokens={tokens} line={line} />
      )}
    </div>);

}

function TargetLine({ tokens, line }) {
  const isCurrent = line.state === "current";
  const isDone = line.state === "done";
  return (
    <div style={{
      padding: "8px 18px 8px 14px",
      display: "flex", alignItems: "center", gap: 10,
      background: isCurrent ? tokens.accentSoft : "transparent",
      borderLeft: `3px solid ${isCurrent ? tokens.accent : "transparent"}`,
      transition: "background 200ms ease"
    }}>
      <div style={{ width: 16, flexShrink: 0, display: "grid", placeItems: "center" }}>
        {isCurrent && <Spinner tokens={tokens} />}
        {isDone && <div style={{ color: tokens.fgFaint }}><CheckIcon /></div>}
      </div>
      <div style={{
        fontFamily: '"JetBrains Mono", "D2Coding", ui-monospace, monospace',
        fontSize: 16, lineHeight: 1.45, letterSpacing: "0.01em"
      }}>
        {renderLineChars(tokens, line)}
      </div>
    </div>);

}

function renderLineChars(tokens, line) {
  if (line.state === "done") {
    return <span style={{ color: tokens.fgFaint }}>{line.text}</span>;
  }
  if (line.state === "pending") {
    return <span style={{ color: tokens.dim }}>{line.text}</span>;
  }
  // current line
  const chars = Array.from(line.text);
  const typed = Array.from(line.typed || "");
  return chars.map((ch, i) => {
    let color = tokens.dim;
    let weight = 400;
    let textDecoration = "none";
    let textUnderlineOffset = "auto";
    if (i < typed.length) {
      if (i < line.correct) {
        color = tokens.correct;
        weight = 600;
      } else {
        color = tokens.wrong;
        weight = 600;
        textDecoration = "underline wavy";
        textUnderlineOffset = "4px";
      }
    }
    // caret indicator
    const isCaret = i === typed.length;
    return (
      <span key={i} style={{ position: "relative", color, fontWeight: weight, textDecoration, textUnderlineOffset, textDecorationThickness: "1.5px" }}>
        {isCaret && <span style={{
          position: "absolute", left: -1, top: 2, bottom: 2,
          width: 2, background: tokens.fg, borderRadius: 1,
          animation: "blink 1s steps(2) infinite"
        }}></span>}
        {ch}
      </span>);

  });
}

function Spinner({ tokens }) {
  return (
    <div style={{
      width: 14, height: 14,
      perspective: 60,
      display: "grid", placeItems: "center"
    }}>
      <svg width="14" height="14" viewBox="0 0 14 14" style={{
        animation: "tri-flip 1.1s cubic-bezier(0.65, 0.05, 0.35, 1) infinite",
        transformOrigin: "50% 50%",
        transformStyle: "preserve-3d"
      }}>
        <polygon points="3,2 12,7 3,12" fill={tokens.accent} />
      </svg>
    </div>);

}

function InputField({ tokens, value }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1.5px solid ${tokens.accent}`,
      borderRadius: 12,
      padding: "14px 18px",
      boxShadow: `0 0 0 4px ${tokens.accentSoft}, ${tokens.shadow}`,
      display: "flex", alignItems: "center", gap: 10
    }}>
      <div style={{
        fontFamily: '"JetBrains Mono", "D2Coding", ui-monospace, monospace',
        fontSize: 16, color: tokens.fg, flex: 1
      }}>
        {value}<span style={{
          display: "inline-block", width: 2, height: 18, background: tokens.fg,
          marginLeft: 1, verticalAlign: "text-bottom",
          animation: "blink 1s steps(2) infinite"
        }}></span>
      </div>
      <div style={{
        fontFamily: '"JetBrains Mono", monospace', fontSize: 10,
        color: tokens.fgDim, padding: "2px 6px",
        border: `1px solid ${tokens.border}`, borderRadius: 4
      }}>한글</div>
    </div>);

}

function LinePosCard({ tokens, current, total }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: "12px 14px",
      boxShadow: tokens.shadow,
      display: "flex", alignItems: "baseline", justifyContent: "space-between"
    }}>
      <div style={{ fontSize: 11, color: tokens.fgDim, fontWeight: 500, letterSpacing: "0.04em", textTransform: "uppercase" }}>
        줄 위치
      </div>
      <div style={{ display: "flex", alignItems: "baseline", gap: 2, fontFamily: '"JetBrains Mono", monospace' }}>
        <span style={{ fontSize: 22, fontWeight: 700, color: tokens.fg }}>{current}</span>
        <span style={{ fontSize: 14, color: tokens.fgFaint, margin: "0 2px" }}>/</span>
        <span style={{ fontSize: 14, color: tokens.fgDim }}>{total}</span>
      </div>
    </div>);

}

function StatsCard({ tokens, dark }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: 14,
      boxShadow: tokens.shadow,
      display: "grid", gridTemplateColumns: "1fr 1fr", gap: "16px 14px"
    }}>
      <StatCell tokens={tokens} value="38" unit="WPM" label="분당 단어" trend="up" />
      <StatCell tokens={tokens} value="192" unit="CPM" label="분당 글자" trend="up" />
      <StatCell tokens={tokens} value="92" unit="%" label="정확도" color={tokens.correct} />
      <StatCell tokens={tokens} value="8" unit="%" label="오타율" color={tokens.wrong} />
    </div>);

}
function StatCell({ tokens, value, unit, label, color, trend }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 4 }}>
        <span style={{
          fontSize: 24, fontWeight: 700, lineHeight: 1,
          color: color || tokens.fg,
          fontFamily: '"JetBrains Mono", monospace',
          fontVariantNumeric: "tabular-nums"
        }}>{value}</span>
        <span style={{ fontSize: 10, fontWeight: 600, color: tokens.fgDim, letterSpacing: "0.04em" }}>{unit}</span>
      </div>
      <div style={{ fontSize: 11, color: tokens.fgDim }}>{label}</div>
    </div>);

}

function ProgressCard({ tokens, progress }) {
  const segments = 20;
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: "12px 14px",
      boxShadow: tokens.shadow
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 8 }}>
        <span style={{ fontSize: 11, color: tokens.fgDim, fontWeight: 500, letterSpacing: "0.04em", textTransform: "uppercase" }}>진행률</span>
        <span style={{ fontSize: 11, color: tokens.fg, fontWeight: 600, fontFamily: '"JetBrains Mono", monospace' }}>{Math.round(progress * 100)}%</span>
      </div>
      <div style={{ display: "flex", gap: 2 }}>
        {Array.from({ length: segments }).map((_, i) => {
          const filled = i < Math.floor(progress * segments);
          const partial = i === Math.floor(progress * segments);
          return (
            <div key={i} style={{
              flex: 1, height: 6, borderRadius: 2,
              background: filled ? tokens.accent : partial ? tokens.accentSoft : tokens.cardBgSoft,
              border: `1px solid ${filled ? "transparent" : tokens.border}`
            }}></div>);

        })}
      </div>
    </div>);

}

function IMEWarning({ tokens }) {
  return (
    <div style={{
      padding: "10px 12px",
      background: tokens.wrongSoft,
      border: `1px solid ${tokens.wrongSoft}`,
      borderRadius: 10,
      display: "flex", alignItems: "flex-start", gap: 8
    }}>
      <div style={{ color: tokens.wrong, marginTop: 1 }}><WarnIcon /></div>
      <div style={{ fontSize: 11.5, lineHeight: 1.4, color: tokens.fg }}>
        <div style={{ fontWeight: 600, marginBottom: 2 }}>ASCII 키만 수집</div>
        <div style={{ color: tokens.fgDim }}>IME 가 켜져 있어도 키맵에는 영문 키만 표시됩니다.</div>
      </div>
    </div>);

}

// ============================================================
// Result page
// ============================================================
function ResultPage({ tokens, dark, empty = false }) {
  if (empty) return <EmptyResult tokens={tokens} />;
  return (
    <div style={{
      padding: 20,
      display: "flex", flexDirection: "column", gap: 16,
      height: "100%", boxSizing: "border-box", overflow: "auto"
    }}>
      <ResultHeader tokens={tokens} />
      <BigStatsCard tokens={tokens} />
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 14 }}>
        <DurationCard tokens={tokens} />
        <KeyCountCard tokens={tokens} />
      </div>
      <HeatmapSection tokens={tokens} dark={dark} />
    </div>);

}

function ResultHeader({ tokens }) {
  return (
    <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
      <div>
        <div style={{ fontSize: 11, color: tokens.fgDim, fontWeight: 500, letterSpacing: "0.04em", textTransform: "uppercase" }}>이번 세션</div>
        <div style={{ fontSize: 22, fontWeight: 700, marginTop: 2 }}>두벌식 표준 · 짧은 글</div>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <PillButton tokens={tokens}>결과 복사</PillButton>
        <PillButton tokens={tokens} primary>다시 시작</PillButton>
      </div>
    </div>);

}

function PillButton({ tokens, primary, children }) {
  return (
    <button style={{
      height: 32, padding: "0 14px", borderRadius: 8,
      border: primary ? "none" : `1px solid ${tokens.borderStrong}`,
      background: primary ? tokens.accent : tokens.cardBg,
      color: primary ? "#fff" : tokens.fg,
      fontSize: 12.5, fontWeight: 600,
      cursor: "pointer", fontFamily: "inherit"
    }}>{children}</button>);

}

function BigStatsCard({ tokens }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 14,
      padding: "24px 20px",
      boxShadow: tokens.shadow,
      display: "grid", gridTemplateColumns: "1fr 1fr 1fr 1fr",
      position: "relative", overflow: "hidden"
    }}>
      <BigStat tokens={tokens} value="38" unit="WPM" label="분당 단어" big delta="+4" />
      <Divider tokens={tokens} />
      <BigStat tokens={tokens} value="192" unit="CPM" label="분당 글자" delta="+22" />
      <Divider tokens={tokens} />
      <BigStat tokens={tokens} value="92" unit="%" label="정확도" color={tokens.correct} delta="+1.2" />
      <Divider tokens={tokens} />
      <BigStat tokens={tokens} value="8" unit="%" label="오타율" color={tokens.wrong} delta="-1.2" deltaNeg />
    </div>);

}
function Divider({ tokens }) {
  return <div style={{
    position: "absolute", width: 1, top: 24, bottom: 24, background: tokens.border,
    left: "auto"
  }}></div>;
}
function BigStat({ tokens, value, unit, label, color, delta, deltaNeg }) {
  const goodDelta = !deltaNeg; // for accuracy +1.2 good, for error -1.2 good
  return (
    <div style={{ padding: "0 14px", display: "flex", flexDirection: "column", alignItems: "flex-start", gap: 4 }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 6 }}>
        <span style={{
          fontSize: 44, fontWeight: 700, lineHeight: 1,
          color: color || tokens.fg,
          fontFamily: '"JetBrains Mono", monospace',
          fontVariantNumeric: "tabular-nums",
          letterSpacing: "-0.02em"
        }}>{value}</span>
        <span style={{ fontSize: 12, fontWeight: 600, color: tokens.fgDim, letterSpacing: "0.04em" }}>{unit}</span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: 12, color: tokens.fgDim }}>{label}</span>
        <span style={{
          fontSize: 10, fontWeight: 600,
          color: tokens.correct,
          fontFamily: '"JetBrains Mono", monospace',
          padding: "1px 5px", borderRadius: 4,
          background: tokens.correctSoft
        }}>{delta}</span>
      </div>
    </div>);

}

function DurationCard({ tokens }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: 14,
      boxShadow: tokens.shadow
    }}>
      <div style={{ fontSize: 11, color: tokens.fgDim, fontWeight: 500, letterSpacing: "0.04em", textTransform: "uppercase", marginBottom: 10 }}>줄별 WPM</div>
      <Sparkline tokens={tokens} />
    </div>);

}

function Sparkline({ tokens }) {
  const data = [22, 28, 31, 35, 30, 38, 42, 39, 44, 41, 46, 43];
  const max = 50;
  const w = 100,h = 50;
  const points = data.map((v, i) => `${i / (data.length - 1) * w},${h - v / max * h}`).join(" ");
  return (
    <svg viewBox={`-2 -4 ${w + 4} ${h + 8}`} width="100%" height={56} style={{ display: "block" }} preserveAspectRatio="none">
      <defs>
        <linearGradient id="sparkfill" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor={tokens.accent} stopOpacity="0.25" />
          <stop offset="1" stopColor={tokens.accent} stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon points={`0,${h} ${points} ${w},${h}`} fill="url(#sparkfill)" />
      <polyline points={points} fill="none" stroke={tokens.accent} strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
      {data.map((v, i) =>
      <circle key={i} cx={i / (data.length - 1) * w} cy={h - v / max * h} r="1.5"
      fill={i === data.length - 1 ? tokens.accent : "transparent"} />
      )}
    </svg>);

}

function KeyCountCard({ tokens }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: 14,
      boxShadow: tokens.shadow,
      display: "flex", flexDirection: "column", gap: 8
    }}>
      <div style={{ fontSize: 11, color: tokens.fgDim, fontWeight: 500, letterSpacing: "0.04em", textTransform: "uppercase" }}>입력 통계</div>
      <KeyCountRow tokens={tokens} label="입력한 글자" value="248" />
      <KeyCountRow tokens={tokens} label="오타" value="20" color={tokens.wrong} />
      <KeyCountRow tokens={tokens} label="백스페이스" value="14" />
      <KeyCountRow tokens={tokens} label="소요 시간" value="01:18" mono />
    </div>);

}
function KeyCountRow({ tokens, label, value, color, mono }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", fontSize: 12 }}>
      <span style={{ color: tokens.fgDim }}>{label}</span>
      <span style={{
        fontWeight: 600,
        color: color || tokens.fg,
        fontFamily: '"JetBrains Mono", monospace',
        fontVariantNumeric: "tabular-nums"
      }}>{value}</span>
    </div>);

}

function HeatmapSection({ tokens, dark }) {
  return (
    <div style={{
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 12,
      padding: 18,
      boxShadow: tokens.shadow
    }}>
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 700 }}>키 오타 히트맵</div>
          <div style={{ fontSize: 11, color: tokens.fgDim, marginTop: 2 }}>이 자판에서 가장 자주 틀린 키</div>
        </div>
        <HeatmapLegend tokens={tokens} />
      </div>
      <HeatmapBoard tokens={tokens} dark={dark} />
    </div>);

}

function HeatmapLegend({ tokens }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
      <span style={{ fontSize: 10, color: tokens.fgDim }}>적음</span>
      {[0.1, 0.3, 0.5, 0.7, 0.9].map((v, i) =>
      <div key={i} style={{
        width: 16, height: 10, borderRadius: 2,
        background: heatColor(tokens, v)
      }}></div>
      )}
      <span style={{ fontSize: 10, color: tokens.fgDim }}>많음</span>
    </div>);

}

function heatColor(tokens, intensity) {
  if (intensity < 0.05) return tokens.cardBgSoft;
  // interpolate between cardBgSoft and wrong color
  const a = Math.min(1, intensity);
  return `color-mix(in oklch, ${tokens.wrong} ${a * 100}%, ${tokens.cardBgSoft})`;
}

function HeatmapBoard({ tokens, dark }) {
  // 각 키 별 오타 빈도 (0..1)
  const heats = {
    "1": 0.0, "2": 0.0, "3": 0.05, "4": 0.0, "5": 0.0, "6": 0.0, "7": 0.0, "8": 0.0, "9": 0.0, "0": 0.0,
    "q": 0.0, "w": 0.05, "e": 0.1, "r": 0.85, "t": 0.4, "y": 0.15, "u": 0.0, "i": 0.05, "o": 0.0, "p": 0.0,
    "a": 0.0, "s": 0.0, "d": 0.0, "f": 0.05, "g": 0.65, "h": 0.0, "j": 0.05, "k": 0.0, "l": 0.0, ";": 0.0,
    "z": 0.0, "x": 0.0, "c": 0.05, "v": 0.95, "b": 0.5, "n": 0.0, "m": 0.0, ",": 0.0, ".": 0.0, "/": 0.0
  };

  // 히트맵 전용 축소 단위
  const HU = 38;       // 키 폭 단위
  const HH = 38;       // 키 높이
  const HGAP = 4;
  const HROW_GAP = 4;

  return (
    <div style={{
      display: "flex", flexDirection: "column",
      gap: HROW_GAP, alignItems: "center"
    }}>
      {KEYBOARD_ROWS.map((row, ri) =>
        <div key={ri} style={{ display: "flex", gap: HGAP }}>
          {row.map((key, ki) => {
            const [lower, upper, hLower, hUpper, w, type] = key;
            const width = w * HU + (w - 1) * HGAP;
            const isSpecial = type === "special";
            const h = heats[lower] || 0;
            const hot = h > 0.5;
            return (
              <HeatKey
                key={ki} tokens={tokens}
                width={width} height={HH}
                isSpecial={isSpecial}
                lower={lower} upper={upper}
                hLower={hLower} hUpper={hUpper}
                heat={h} hot={hot}
              />);
          })}
        </div>
      )}
    </div>);
}

function HeatKey({ tokens, width, height, isSpecial, lower, upper, hLower, hUpper, heat, hot }) {
  if (isSpecial) {
    return (
      <div style={{
        width, height, borderRadius: 6,
        background: tokens.cardBgSoft,
        border: `1px solid ${tokens.border}`,
        display: "grid", placeItems: "center",
        color: tokens.fgFaint,
        fontSize: 10, fontWeight: 500,
      }}>{lower}</div>);
  }
  return (
    <div style={{
      width, height, borderRadius: 6,
      background: heatColor(tokens, heat),
      border: `1px solid ${hot ? tokens.wrong : tokens.border}`,
      boxShadow: hot ? `0 0 0 2px ${tokens.wrongSoft}` : "none",
      padding: "2px 4px 3px 4px",
      display: "grid",
      gridTemplateColumns: "1fr 1fr",
      gridTemplateRows: "1fr 1fr",
      lineHeight: 1, position: "relative",
    }}>
      <div style={{ fontSize: 8, color: hot ? "rgba(255,255,255,0.65)" : tokens.fgFaint, fontFamily: '"JetBrains Mono", monospace' }}>{upper}</div>
      <div style={{ fontSize: 8, color: hot ? "rgba(255,255,255,0.65)" : tokens.fgFaint, textAlign: "right" }}>{hUpper}</div>
      <div style={{ fontSize: 12, color: hot ? "#fff" : tokens.fg, fontWeight: 700, fontFamily: '"JetBrains Mono", monospace', alignSelf: "end" }}>{lower}</div>
      <div style={{ fontSize: 12, color: hot ? "#fff" : tokens.accent, fontWeight: 700, textAlign: "right", alignSelf: "end" }}>{hLower}</div>
      {hot && (
        <div style={{
          position: "absolute", top: -6, right: -6,
          background: tokens.wrong, color: "#fff",
          fontSize: 9, fontWeight: 700, padding: "1px 5px", borderRadius: 6,
          fontFamily: '"JetBrains Mono", monospace',
        }}>{Math.round(heat * 20)}</div>
      )}
    </div>);
}

function EmptyResult({ tokens }) {
  return (
    <div style={{
      height: "100%", display: "flex", flexDirection: "column",
      alignItems: "center", justifyContent: "center", gap: 16, padding: 40
    }}>
      <div style={{
        width: 72, height: 72, borderRadius: 36,
        background: tokens.cardBgSoft,
        display: "grid", placeItems: "center",
        color: tokens.fgFaint
      }}>
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 19h18M5 16V9M9 16V5M13 16v-9M17 16v-5M21 16v-3" />
        </svg>
      </div>
      <div style={{ textAlign: "center", maxWidth: 320 }}>
        <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 6 }}>아직 완료된 세션이 없어요</div>
        <div style={{ fontSize: 13, color: tokens.fgDim, lineHeight: 1.5 }}>한 회 끝내면 WPM · 정확도 · 오타 히트맵이 여기에 표시됩니다.</div>
      </div>
      <PillButton tokens={tokens} primary>연습으로 가기</PillButton>
    </div>);

}

// ============================================================
// Keyboard widget
// ============================================================
const KEY_U = 46;
const KEY_GAP = 5;
const KEY_H = 50;
const ROW_GAP = 5;

// 5-row ANSI 106-key (두벌식 표준)
const KEYBOARD_ROWS = [
  [
  ["`", "~", "", "", 1],
  ["1", "!", "ㅂ", "ㅃ", 1],
  ["2", "@", "ㅈ", "ㅉ", 1],
  ["3", "#", "ㄷ", "ㄸ", 1],
  ["4", "$", "ㄱ", "ㄲ", 1],
  ["5", "%", "ㅅ", "ㅆ", 1],
  ["6", "^", "ㅛ", "", 1],
  ["7", "&", "ㅕ", "", 1],
  ["8", "*", "ㅑ", "", 1],
  ["9", "(", "ㅐ", "ㅒ", 1],
  ["0", ")", "ㅔ", "ㅖ", 1],
  ["-", "_", "", "", 1],
  ["=", "+", "", "", 1],
  ["Backspace", "", "", "", 2, "special"]],

  [
  ["Tab", "", "", "", 1.5, "special"],
  ["q", "Q", "ㅂ", "ㅃ", 1],
  ["w", "W", "ㅈ", "ㅉ", 1],
  ["e", "E", "ㄷ", "ㄸ", 1],
  ["r", "R", "ㄱ", "ㄲ", 1],
  ["t", "T", "ㅅ", "ㅆ", 1],
  ["y", "Y", "ㅛ", "", 1],
  ["u", "U", "ㅕ", "", 1],
  ["i", "I", "ㅑ", "", 1],
  ["o", "O", "ㅐ", "ㅒ", 1],
  ["p", "P", "ㅔ", "ㅖ", 1],
  ["[", "{", "", "", 1],
  ["]", "}", "", "", 1],
  ["\\", "|", "", "", 1.5]],

  [
  ["Caps", "", "", "", 1.75, "special"],
  ["a", "A", "ㅁ", "", 1],
  ["s", "S", "ㄴ", "", 1],
  ["d", "D", "ㅇ", "", 1],
  ["f", "F", "ㄹ", "", 1],
  ["g", "G", "ㅎ", "", 1],
  ["h", "H", "ㅗ", "", 1],
  ["j", "J", "ㅓ", "", 1],
  ["k", "K", "ㅏ", "", 1],
  ["l", "L", "ㅣ", "", 1],
  [";", ":", "", "", 1],
  ["'", "\"", "", "", 1],
  ["Enter", "", "", "", 2.25, "special"]],

  [
  ["Shift", "", "", "", 2.25, "special"],
  ["z", "Z", "ㅋ", "", 1],
  ["x", "X", "ㅌ", "", 1],
  ["c", "C", "ㅊ", "", 1],
  ["v", "V", "ㅍ", "", 1],
  ["b", "B", "ㅠ", "", 1],
  ["n", "N", "ㅜ", "", 1],
  ["m", "M", "ㅡ", "", 1],
  [",", "<", "", "", 1],
  [".", ">", "", "", 1],
  ["/", "?", "", "", 1],
  ["Shift", "", "", "", 2.75, "special"]],

  [
  ["Ctrl", "", "", "", 1.5, "special"],
  ["Meta", "", "", "", 1.25, "special"],
  ["Alt", "", "", "", 1.5, "special"],
  ["한자", "", "", "", 1.25, "special"],
  ["Space", "", "", "", 4, "special"],
  ["한/영", "", "", "", 1.25, "special"],
  ["Alt", "", "", "", 1.5, "special"],
  ["Menu", "", "", "", 1.25, "special"],
  ["Ctrl", "", "", "", 1.5, "special"]]];

function Keyboard({ tokens, dark, pressed }) {
  return (
    <div style={{
      padding: 12,
      background: tokens.cardBgSoft,
      border: `1px solid ${tokens.border}`,
      borderRadius: 14,
      boxShadow: tokens.shadow, height: "30px"
    }}>
      <div style={{ display: "flex", flexDirection: "column", gap: ROW_GAP }}>
        {KEYBOARD_ROWS.map((row, ri) =>
        <div key={ri} style={{ display: "flex", gap: KEY_GAP }}>
            {row.map((key, ki) => {
            const [lower, upper, hLower, hUpper, w, type] = key;
            const width = w * KEY_U + (w - 1) * KEY_GAP;
            const isPressed = lower === pressed;
            return (
              <KeyCap
                key={ki} tokens={tokens}
                width={width}
                pressed={isPressed}
                type={type}
                lower={lower} upper={upper}
                hLower={hLower} hUpper={hUpper} />);


          })}
          </div>
        )}
      </div>
    </div>);

}

function KeyCap({ tokens, width, pressed, type, lower, upper, hLower, hUpper }) {
  const isSpecial = type === "special";
  if (isSpecial) {
    return (
      <div style={{
        width, height: KEY_H, borderRadius: 7,
        background: pressed ? tokens.accent : tokens.cardBg,
        border: `1px solid ${pressed ? tokens.accent : tokens.border}`,
        borderBottom: `2px solid ${pressed ? tokens.accent : tokens.borderStrong}`,
        boxShadow: pressed ? "none" : tokens.shadowKey,
        display: "grid", placeItems: "center",
        color: pressed ? "#fff" : tokens.fgDim,
        fontSize: 12, fontWeight: 600,
        transition: "all 120ms ease-out"
      }}>{lower}</div>);

  }
  return (
    <div style={{
      width, height: KEY_H, borderRadius: 7,
      background: pressed ? tokens.accent : tokens.cardBg,
      border: `1px solid ${pressed ? tokens.accent : tokens.border}`,
      borderBottom: `2px solid ${pressed ? tokens.accent : tokens.borderStrong}`,
      boxShadow: pressed ? "none" : tokens.shadowKey,
      padding: "3px 5px 4px 5px",
      display: "grid",
      gridTemplateColumns: "1fr 1fr",
      gridTemplateRows: "1fr 1fr",
      lineHeight: 1,
      transition: "all 120ms ease-out"
    }}>
      <div style={{ fontSize: 11, color: pressed ? "rgba(255,255,255,0.7)" : tokens.fgFaint, fontWeight: 500, fontFamily: '"JetBrains Mono", monospace' }}>{upper}</div>
      <div style={{ fontSize: 11, color: pressed ? "rgba(255,255,255,0.7)" : tokens.fgFaint, fontWeight: 500, textAlign: "right" }}>{hUpper}</div>
      <div style={{ fontSize: 15, color: pressed ? "#fff" : tokens.fg, fontWeight: 700, fontFamily: '"JetBrains Mono", monospace', alignSelf: "end" }}>{lower}</div>
      <div style={{ fontSize: 15, color: pressed ? "#fff" : tokens.accent, fontWeight: 700, textAlign: "right", alignSelf: "end" }}>{hLower}</div>
    </div>);

}

// ============================================================
// Key state detail (for spec card)
// ============================================================
function KeyStatesShowcase({ tokens, dark }) {
  return (
    <div style={{
      padding: 32, background: tokens.bg, color: tokens.fg, height: "100%",
      fontFamily: '"Pretendard", "Inter", sans-serif',
      display: "flex", flexDirection: "column", gap: 24
    }}>
      <div>
        <div style={{ fontSize: 11, color: tokens.fgDim, letterSpacing: "0.04em", textTransform: "uppercase", marginBottom: 4 }}>키 상태</div>
        <div style={{ fontSize: 18, fontWeight: 700 }}>일반 키 · 4-corner 라벨</div>
      </div>
      <div style={{ display: "flex", gap: 24, alignItems: "flex-start" }}>
        <KeyStateExample tokens={tokens} label="기본" />
        <KeyStateExample tokens={tokens} label="호버" hover />
        <KeyStateExample tokens={tokens} label="눌림" pressed />
      </div>

      <div style={{ marginTop: 8 }}>
        <div style={{ fontSize: 11, color: tokens.fgDim, letterSpacing: "0.04em", textTransform: "uppercase", marginBottom: 4 }}>특수 키</div>
        <div style={{ fontSize: 18, fontWeight: 700 }}>가변 폭 · 중앙 라벨</div>
      </div>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
        <SpecialKey tokens={tokens} width={KEY_U * 1.5 + KEY_GAP * 0.5}>Tab</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 1.75 + KEY_GAP * 0.75}>Caps</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 2.25 + KEY_GAP * 1.25}>Shift</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 4 + KEY_GAP * 3}>Space</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 1.25 + KEY_GAP * 0.25} accent>한/영</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 1.25 + KEY_GAP * 0.25} accent>한자</SpecialKey>
        <SpecialKey tokens={tokens} width={KEY_U * 2 + KEY_GAP}>Enter</SpecialKey>
      </div>

      <div style={{ marginTop: 8 }}>
        <div style={{ fontSize: 11, color: tokens.fgDim, letterSpacing: "0.04em", textTransform: "uppercase", marginBottom: 4 }}>라벨 시스템</div>
        <div style={{ fontSize: 18, fontWeight: 700 }}>4-corner 가이드</div>
      </div>
      <div style={{ display: "flex", gap: 32, alignItems: "center" }}>
        <BigKeyLabel tokens={tokens} />
        <div style={{ fontSize: 12, color: tokens.fgDim, lineHeight: 1.6 }}>
          <div>↖ 영문 Shift &nbsp;·&nbsp; ↗ 한글 Shift</div>
          <div>↙ 영문 Base &nbsp;·&nbsp; ↘ 한글 Base</div>
          <div style={{ marginTop: 8 }}>
            영문 = <span style={{ fontFamily: '"JetBrains Mono", monospace', color: tokens.fg }}>mono 600</span><br />
            한글 = <span style={{ color: tokens.accent }}>accent 600</span>
          </div>
        </div>
      </div>
    </div>);

}

function KeyStateExample({ tokens, label, hover, pressed }) {
  let bg = tokens.cardBg,border = tokens.border,borderBottom = tokens.borderStrong;
  let textColor = tokens.fg,hangulColor = tokens.accent;
  let shadow = tokens.shadowKey;
  if (hover) {
    border = `${tokens.accent}66`;
    borderBottom = `${tokens.accent}88`;
  }
  if (pressed) {
    bg = tokens.accent;border = tokens.accent;borderBottom = tokens.accent;
    textColor = "#fff";hangulColor = "rgba(255,255,255,0.85)";
    shadow = `inset 0 1px 2px rgba(0,0,0,0.15)`;
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "center" }}>
      <div style={{
        width: KEY_U * 1.8, height: KEY_H * 1.8, borderRadius: 10,
        background: bg, border: `1px solid ${border}`,
        borderBottom: `2px solid ${borderBottom}`, boxShadow: shadow,
        padding: "8px 10px",
        display: "grid", gridTemplateColumns: "1fr 1fr", gridTemplateRows: "1fr 1fr",
        fontSize: 13, transition: "all 120ms ease-out"
      }}>
        <div style={{ color: pressed ? "rgba(255,255,255,0.6)" : tokens.fgFaint, fontFamily: '"JetBrains Mono", monospace' }}>A</div>
        <div style={{ color: pressed ? "rgba(255,255,255,0.6)" : tokens.fgFaint, textAlign: "right" }}>ㅃ</div>
        <div style={{ color: textColor, fontWeight: 600, fontFamily: '"JetBrains Mono", monospace', alignSelf: "end", fontSize: 16 }}>a</div>
        <div style={{ color: hangulColor, fontWeight: 600, textAlign: "right", alignSelf: "end", fontSize: 16 }}>ㅂ</div>
      </div>
      <div style={{ fontSize: 11, color: tokens.fgDim }}>{label}</div>
    </div>);

}

function SpecialKey({ tokens, width, children, accent }) {
  return (
    <div style={{
      width, height: KEY_H, borderRadius: 7,
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderBottom: `2px solid ${tokens.borderStrong}`,
      boxShadow: tokens.shadowKey,
      display: "grid", placeItems: "center",
      color: accent ? tokens.accent : tokens.fgDim,
      fontSize: 11, fontWeight: accent ? 600 : 500
    }}>{children}</div>);

}

function BigKeyLabel({ tokens }) {
  return (
    <div style={{
      width: 120, height: 120, borderRadius: 14,
      background: tokens.cardBg,
      border: `1px solid ${tokens.border}`,
      borderBottom: `3px solid ${tokens.borderStrong}`,
      boxShadow: tokens.shadowKey,
      padding: "14px 16px",
      display: "grid", gridTemplateColumns: "1fr 1fr", gridTemplateRows: "1fr 1fr",
      position: "relative"
    }}>
      <div style={{ color: tokens.fgFaint, fontFamily: '"JetBrains Mono", monospace', fontSize: 18 }}>A</div>
      <div style={{ color: tokens.fgFaint, textAlign: "right", fontSize: 18 }}>ㅃ</div>
      <div style={{ color: tokens.fg, fontWeight: 600, fontFamily: '"JetBrains Mono", monospace', alignSelf: "end", fontSize: 28 }}>a</div>
      <div style={{ color: tokens.accent, fontWeight: 600, textAlign: "right", alignSelf: "end", fontSize: 28 }}>ㅂ</div>
    </div>);

}

// ============================================================
// Daemon inactive dialog
// ============================================================
function DaemonDialog({ tokens, dark, variant = "stepped" }) {
  return (
    <div style={{
      width: 440,
      background: tokens.bgRaised,
      borderRadius: 14,
      border: `1px solid ${tokens.borderStrong}`,
      boxShadow: dark ? "0 24px 60px rgba(0,0,0,0.6)" : "0 24px 60px rgba(0,0,0,0.18)",
      overflow: "hidden",
      fontFamily: '"Pretendard", "Inter", sans-serif',
      color: tokens.fg
    }}>
      <div style={{ padding: "24px 24px 16px 24px" }}>
        <div style={{
          width: 48, height: 48, borderRadius: 24,
          background: tokens.wrongSoft,
          color: tokens.wrong,
          display: "grid", placeItems: "center", marginBottom: 14
        }}>
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 9v4M12 17h.01M10.3 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
          </svg>
        </div>
        <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 6 }}>UNIM 이 실행 중이 아닙니다</div>
        <div style={{ fontSize: 13, color: tokens.fgDim, lineHeight: 1.55 }}>
          타자 연습은 UNIM 입력기가 실행 중일 때만 동작합니다. 아래 단계를 따라 주세요.
        </div>
      </div>
      {variant === "stepped" &&
      <div style={{ padding: "0 24px 18px 24px", display: "flex", flexDirection: "column", gap: 10 }}>
          <DialogStep tokens={tokens} num="1" title="시스템 트레이에서 UNIM 켜기" body="패널의 UNIM 아이콘을 클릭해 활성화합니다." />
          <DialogStep tokens={tokens} num="2" title="이 앱을 다시 열기" body="UNIM 이 켜진 뒤 본 창을 다시 실행합니다." />
          <DialogStep tokens={tokens} num="3" title="자판이 자동 인식되는지 확인" body="헤더에 자판 이름이 표시되면 준비 완료입니다." />
        </div>
      }
      <div style={{
        padding: "12px 16px",
        background: dark ? "rgba(255,255,255,0.02)" : "rgba(0,0,0,0.02)",
        borderTop: `1px solid ${tokens.border}`,
        display: "flex", justifyContent: "flex-end", gap: 8
      }}>
        <PillButton tokens={tokens}>도움말</PillButton>
        <PillButton tokens={tokens} primary>닫고 종료</PillButton>
      </div>
    </div>);

}

function DialogStep({ tokens, num, title, body }) {
  return (
    <div style={{ display: "flex", gap: 12, alignItems: "flex-start" }}>
      <div style={{
        width: 22, height: 22, borderRadius: 11,
        background: tokens.accentSoft, color: tokens.accent,
        display: "grid", placeItems: "center",
        fontSize: 11, fontWeight: 700, flexShrink: 0,
        fontFamily: '"JetBrains Mono", monospace'
      }}>{num}</div>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 1 }}>{title}</div>
        <div style={{ fontSize: 12, color: tokens.fgDim, lineHeight: 1.45 }}>{body}</div>
      </div>
    </div>);

}

// ============================================================
// Color/accessibility spec card
// ============================================================
function ColorAccessCard({ tokens, dark }) {
  return (
    <div style={{
      padding: 32, height: "100%", boxSizing: "border-box",
      background: tokens.bg, color: tokens.fg,
      fontFamily: '"Pretendard", "Inter", sans-serif',
      display: "flex", flexDirection: "column", gap: 18, overflow: "auto"
    }}>
      <div>
        <div style={{ fontSize: 11, color: tokens.fgDim, letterSpacing: "0.04em", textTransform: "uppercase", marginBottom: 4 }}>색 · 접근성</div>
        <div style={{ fontSize: 18, fontWeight: 700 }}>대비비 · 색약 대안 · 모양 신호</div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        <ColorSwatch tokens={tokens} name="Correct" hex="#1c66c9" wcag="AA 7.4" use="일치 글자" />
        <ColorSwatch tokens={tokens} name="Wrong" hex="#d24e15" wcag="AA 4.9" use="오타 글자" />
      </div>

      <div>
        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8 }}>모양 신호 옵션 (색약 보조)</div>
        <div style={{
          padding: "14px 18px",
          background: tokens.cardBg,
          border: `1px solid ${tokens.border}`,
          borderRadius: 10,
          fontFamily: '"JetBrains Mono", monospace', fontSize: 16, lineHeight: 1.6
        }}>
          <span style={{ color: tokens.correct, fontWeight: 600 }}>한글은 과</span>
          <span style={{ color: tokens.wrong, fontWeight: 600, textDecoration: "underline wavy", textDecorationThickness: "1.5px", textUnderlineOffset: "4px" }}>혁</span>
          <span style={{ color: tokens.dim }}>학적인 문자입니다.</span>
        </div>
        <div style={{ fontSize: 11, color: tokens.fgDim, marginTop: 6 }}>오타에는 색 + 물결 밑줄. 색약 사용자도 위치 파악 가능.</div>
      </div>

      <div>
        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8 }}>색약 대안 (deuteranopia)</div>
        <div style={{ display: "flex", gap: 12 }}>
          <div style={{ flex: 1, padding: "12px 14px", background: tokens.cardBg, border: `1px solid ${tokens.border}`, borderRadius: 10 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
              <div style={{ width: 14, height: 14, borderRadius: 4, background: "#0077bb" }}></div>
              <span style={{ fontSize: 12, fontWeight: 600 }}>Correct (cool blue)</span>
            </div>
            <div style={{ fontFamily: '"JetBrains Mono", monospace', fontSize: 11, color: tokens.fgDim }}>#0077bb · L 47 · C 0.15</div>
          </div>
          <div style={{ flex: 1, padding: "12px 14px", background: tokens.cardBg, border: `1px solid ${tokens.border}`, borderRadius: 10 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
              <div style={{ width: 14, height: 14, borderRadius: 4, background: "#cc3311" }}></div>
              <span style={{ fontSize: 12, fontWeight: 600 }}>Wrong (red-orange)</span>
            </div>
            <div style={{ fontFamily: '"JetBrains Mono", monospace', fontSize: 11, color: tokens.fgDim }}>#cc3311 · L 47 · C 0.18</div>
          </div>
        </div>
      </div>
    </div>);

}

function ColorSwatch({ tokens, name, hex, wcag, use }) {
  return (
    <div style={{
      padding: 14, background: tokens.cardBg,
      border: `1px solid ${tokens.border}`, borderRadius: 10,
      display: "flex", alignItems: "center", gap: 12
    }}>
      <div style={{
        width: 44, height: 44, borderRadius: 8, background: hex, flexShrink: 0,
        border: `1px solid ${tokens.border}`
      }}></div>
      <div>
        <div style={{ fontSize: 12, fontWeight: 700 }}>{name}</div>
        <div style={{ fontSize: 11, fontFamily: '"JetBrains Mono", monospace', color: tokens.fgDim }}>{hex}</div>
        <div style={{ fontSize: 10, color: tokens.correct, fontWeight: 600, marginTop: 2 }}>✓ WCAG {wcag}</div>
      </div>
    </div>);

}

// expose
Object.assign(window, {
  tokensLight, tokensDark,
  WindowChrome, PracticePage, ResultPage,
  KeyStatesShowcase, DaemonDialog, ColorAccessCard
});