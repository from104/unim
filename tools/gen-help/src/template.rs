//! 자족(self-contained) HTML 셸 — CSS 인라인, 외부 리소스 참조 0.
//!
//! `C:\Program Files\UNIM\help\` 나 `/usr/share/unim/help/` 아래에서 `file://` 로 열어도
//! 완전히 동작해야 한다. 따라서 CDN·웹폰트·원격 이미지·fetch 를 일절 쓰지 않는다.
//! 폰트는 OS 내장 스택만 지정한다.

/// 접근성 우선 스타일. 큰 본문 글자·넓은 행간·높은 대비, 다크/라이트 자동 추종.
const CSS: &str = r#"
:root {
  --bg: #ffffff;
  --fg: #16181d;
  --fg-muted: #4a5060;
  --accent: #0b5ed7;
  --accent-soft: #e8f0fe;
  --border: #d3d7e0;
  --code-bg: #f2f4f8;
  --note-bg: #fff8e1;
  --note-border: #d9a400;
  --table-head: #eef1f6;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #14161a;
    --fg: #e9ecf1;
    --fg-muted: #a8b0c0;
    --accent: #7db3ff;
    --accent-soft: #1d2a3f;
    --border: #363b45;
    --code-bg: #1d2026;
    --note-bg: #2b2413;
    --note-border: #c9a227;
    --table-head: #1d2026;
  }
}

* { box-sizing: border-box; }

html { -webkit-text-size-adjust: 100%; }

body {
  margin: 0;
  padding: 0 1.25rem 6rem;
  background: var(--bg);
  color: var(--fg);
  font-family: "Pretendard", "Noto Sans KR", "Malgun Gothic", "Apple SD Gothic Neo",
               system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  font-size: 18px;
  line-height: 1.85;
  word-break: keep-all;
  overflow-wrap: anywhere;
}

.wrap { max-width: 62rem; margin: 0 auto; }

/* ── 상단 바 ─────────────────────────────────────────────── */
.topbar {
  position: sticky;
  top: 0;
  z-index: 10;
  background: var(--bg);
  border-bottom: 2px solid var(--border);
  padding: 0.9rem 0;
  margin-bottom: 1.5rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem 1.25rem;
  align-items: baseline;
  justify-content: space-between;
}
.topbar .brand { font-size: 1.35rem; font-weight: 700; }
.topbar .brand .ver { font-size: 0.85rem; font-weight: 400; color: var(--fg-muted); }
.langswitch {
  display: inline-block;
  padding: 0.5rem 1.1rem;
  border: 2px solid var(--accent);
  border-radius: 0.5rem;
  font-weight: 600;
  text-decoration: none;
  color: var(--accent);
}
.langswitch:hover, .langswitch:focus { background: var(--accent-soft); }

/* ── 생성물 안내 ─────────────────────────────────────────── */
.gennotice {
  background: var(--note-bg);
  border-left: 6px solid var(--note-border);
  border-radius: 0.35rem;
  padding: 0.85rem 1.1rem;
  margin: 0 0 2rem;
  color: var(--fg);
}

/* ── 목차 ───────────────────────────────────────────────── */
.toc {
  border: 2px solid var(--border);
  border-radius: 0.6rem;
  padding: 1.25rem 1.5rem;
  margin-bottom: 3rem;
}
.toc > h2 { margin-top: 0; border: 0; padding: 0; font-size: 1.5rem; }
.toc ol { list-style: none; padding-left: 0; margin: 0; }
.toc > ol > li { margin-bottom: 1.25rem; }
.toc > ol > li > a { font-size: 1.15rem; font-weight: 700; }
.toc ol ol { padding-left: 1.25rem; margin-top: 0.35rem; }
/* 발 마우스 조작 — 클릭 표적을 넉넉히 잡는다. */
.toc a { display: inline-block; padding: 0.3rem 0.2rem; }

/* ── 본문 ───────────────────────────────────────────────── */
section.doc { margin-bottom: 5rem; scroll-margin-top: 5rem; }

h1, h2, h3, h4, h5, h6 { line-height: 1.35; scroll-margin-top: 5rem; }
h1 { font-size: 2.1rem; margin: 0 0 0.5rem; }
h2 {
  font-size: 1.85rem;
  margin-top: 3.5rem;
  padding-bottom: 0.4rem;
  border-bottom: 3px solid var(--border);
}
h3 { font-size: 1.45rem; margin-top: 2.75rem; }
h4 { font-size: 1.2rem; margin-top: 2rem; }
h5, h6 { font-size: 1.05rem; margin-top: 1.5rem; }

a { color: var(--accent); }
a:hover, a:focus { text-decoration-thickness: 2px; }

:focus-visible { outline: 3px solid var(--accent); outline-offset: 2px; }

code, kbd, pre, samp {
  font-family: "JetBrains Mono", "D2Coding", ui-monospace, SFMono-Regular,
               Menlo, Consolas, monospace;
}
code { background: var(--code-bg); padding: 0.1em 0.35em; border-radius: 0.3em; font-size: 0.9em; }
pre {
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-radius: 0.5rem;
  padding: 1rem 1.15rem;
  overflow-x: auto;          /* 긴 코드는 자기 상자 안에서만 가로 스크롤 */
  line-height: 1.6;
  font-size: 0.95rem;
}
pre code { background: none; padding: 0; font-size: inherit; }

blockquote {
  margin: 1.5rem 0;
  padding: 0.4rem 1.25rem;
  border-left: 6px solid var(--accent);
  background: var(--accent-soft);
  border-radius: 0 0.35rem 0.35rem 0;
  color: var(--fg);
}
blockquote > :first-child { margin-top: 0; }
blockquote > :last-child { margin-bottom: 0; }

/* 표는 본문을 밀어내지 않고 자기 래퍼 안에서 스크롤한다. */
.tablewrap { overflow-x: auto; margin: 1.5rem 0; }
table { border-collapse: collapse; width: 100%; font-size: 0.95rem; }
th, td { border: 1px solid var(--border); padding: 0.6rem 0.85rem; text-align: left; vertical-align: top; }
th { background: var(--table-head); font-weight: 700; }

hr { border: 0; border-top: 2px solid var(--border); margin: 3rem 0; }

ul, ol { padding-left: 1.6rem; }
li { margin: 0.35rem 0; }

.backtotop {
  display: inline-block;
  margin-top: 2.5rem;
  padding: 0.45rem 1rem;
  border: 2px solid var(--border);
  border-radius: 0.5rem;
  text-decoration: none;
  font-size: 0.95rem;
}
.backtotop:hover, .backtotop:focus { background: var(--accent-soft); }

@media (max-width: 40rem) {
  body { font-size: 17px; padding: 0 0.9rem 4rem; }
  h1 { font-size: 1.7rem; }
  h2 { font-size: 1.45rem; }
  h3 { font-size: 1.25rem; }
}

@media print {
  .topbar, .backtotop { display: none; }
  body { font-size: 11pt; }
}
"#;

pub struct Page<'a> {
    pub lang_code: &'a str,
    pub title: &'a str,
    pub brand: &'a str,
    pub version: &'a str,
    pub gen_notice: &'a str,
    pub lang_switch_label: &'a str,
    pub lang_switch_href: &'a str,
    pub toc_title: &'a str,
    pub toc: &'a str,
    pub body: &'a str,
}

impl Page<'_> {
    pub fn render(&self) -> String {
        format!(
            r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="unim-gen-help">
<meta name="color-scheme" content="light dark">
<title>{title}</title>
<style>{css}</style>
</head>
<body>
<div class="wrap">
<header class="topbar">
  <span class="brand">{brand} <span class="ver">v{version}</span></span>
  <a class="langswitch" href="{lang_href}">{lang_label}</a>
</header>

<p class="gennotice">{notice}</p>

<nav class="toc" aria-label="{toc_title}">
<h2 id="toc">{toc_title}</h2>
{toc}
</nav>

{body}
</div>
</body>
</html>
"#,
            lang = self.lang_code,
            title = self.title,
            css = CSS,
            brand = self.brand,
            version = self.version,
            lang_href = self.lang_switch_href,
            lang_label = self.lang_switch_label,
            notice = self.gen_notice,
            toc_title = self.toc_title,
            toc = self.toc,
            body = self.body,
        )
    }
}
