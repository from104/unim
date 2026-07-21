//! `docs/user/**` 의 사용자 매뉴얼 4종을 언어별 **자족 오프라인 HTML** 한 장으로 병합한다.
//!
//! ```text
//! docs/user/user-guide/README-ko.md        ┐
//! docs/user/faq/README-ko.md               ├─▶ help/unim-help-ko.html
//! docs/user/troubleshooting/README-ko.md   │
//! docs/user/keyboard-shortcuts/README-ko.md┘
//! (README.md 4종 → help/unim-help-en.html)
//! ```
//!
//! 산출물은 저장소에 커밋한다(사전 생성). 패키징은 파일 복사만 하므로 deb/rpm/MSI 에
//! 이 생성기 의존성이 들어가지 않는다.
//!
//! 핵심 작업은 **링크 재작성**이다. 원본 4종은 서로를 상대경로로 링크하는데
//! (`../faq/README-ko.md#q3-...`), 병합 후에는 그게 문서 내 앵커여야 한다.
//! 문서 밖(`docs/dev/**`, 루트 `README.md` 등)이나 앵커를 못 찾은 링크는
//! **빌드를 깨뜨리지 않고** GitHub 절대 URL 로 폴백하고 경고로 남긴다.
//! 오프라인 문서가 통째로 실패하는 것보다, 링크 하나가 온라인으로 새는 편이 낫다.

mod slug;
mod template;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use slug::Slugger;
use template::Page;

const REPO_BLOB: &str = "https://github.com/from104/unim/blob/main";
const ONLINE_DOCS: &str = "https://github.com/from104/unim/tree/main/docs/user";
const DOC_ROOT: &str = "docs/user";

/// 병합 대상 4종. 순서가 곧 목차·본문 순서다.
struct DocDef {
    /// 앵커 네임스페이스 겸 `<section>` id.
    id: &'static str,
    dir: &'static str,
    fallback_title_ko: &'static str,
    fallback_title_en: &'static str,
}

const DOCS: [DocDef; 4] = [
    DocDef {
        id: "user-guide",
        dir: "user-guide",
        fallback_title_ko: "사용자 매뉴얼",
        fallback_title_en: "User Guide",
    },
    DocDef {
        id: "keyboard-shortcuts",
        dir: "keyboard-shortcuts",
        fallback_title_ko: "단축키",
        fallback_title_en: "Keyboard Shortcuts",
    },
    DocDef {
        id: "faq",
        dir: "faq",
        fallback_title_ko: "자주 묻는 질문",
        fallback_title_en: "FAQ",
    },
    DocDef {
        id: "troubleshooting",
        dir: "troubleshooting",
        fallback_title_ko: "문제 해결",
        fallback_title_en: "Troubleshooting",
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Lang {
    Ko,
    En,
}

impl Lang {
    const ALL: [Lang; 2] = [Lang::Ko, Lang::En];

    fn file_name(self) -> &'static str {
        match self {
            Lang::Ko => "README-ko.md",
            Lang::En => "README.md",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Lang::Ko => "ko",
            Lang::En => "en",
        }
    }

    fn other(self) -> Lang {
        match self {
            Lang::Ko => Lang::En,
            Lang::En => Lang::Ko,
        }
    }

    fn out_file(self) -> String {
        format!("unim-help-{}.html", self.code())
    }
}

/// 문서 하나를 훑어 얻은 색인 — 링크 검증에 필요한 앵커 집합과 제목.
struct DocIndex {
    ids: Vec<String>,
    title: String,
}

/// 링크 재작성 집계.
#[derive(Default)]
struct Stats {
    internal: usize,
    cross_lang: usize,
    github_fallback: usize,
    external: usize,
    warnings: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut root = PathBuf::from(".");
    let mut out_dir = PathBuf::from("help");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--root" if i + 1 < args.len() => {
                root = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--out" if i + 1 < args.len() => {
                out_dir = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "-h" | "--help" => {
                println!("usage: unim-gen-help [--root <repo-root>] [--out <dir>]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unim-gen-help: 알 수 없는 인자 {other}");
                return ExitCode::FAILURE;
            }
        }
    }

    match run(&root, &out_dir) {
        Ok(stats) => {
            for w in &stats.warnings {
                eprintln!("warning: {w}");
            }
            println!(
                "링크 재작성: 내부 앵커 {} · 언어 교차 {} · GitHub 폴백 {} · 외부 URL 유지 {} · 경고 {}",
                stats.internal,
                stats.cross_lang,
                stats.github_fallback,
                stats.external,
                stats.warnings.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("unim-gen-help: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path, out_dir: &Path) -> Result<Stats, String> {
    // 1단계 — 8개 문서를 모두 읽고 앵커 색인을 만든다.
    //         교차 링크를 검증하려면 렌더링 전에 전체 id 집합이 필요하다.
    let mut sources: HashMap<(usize, Lang), String> = HashMap::new();
    let mut indexes: HashMap<(usize, Lang), DocIndex> = HashMap::new();

    for (di, doc) in DOCS.iter().enumerate() {
        for lang in Lang::ALL {
            let path = root.join(DOC_ROOT).join(doc.dir).join(lang.file_name());
            let md = std::fs::read_to_string(&path)
                .map_err(|e| format!("{}: 읽기 실패 — {e}", path.display()))?;
            indexes.insert((di, lang), index_doc(&md, doc, lang));
            sources.insert((di, lang), md);
        }
    }

    // 상대경로 → (문서, 언어) 역인덱스. 링크 대상이 병합 범위 안인지 판별한다.
    let mut known: HashMap<String, (usize, Lang)> = HashMap::new();
    for (di, doc) in DOCS.iter().enumerate() {
        for lang in Lang::ALL {
            known.insert(
                format!("{DOC_ROOT}/{}/{}", doc.dir, lang.file_name()),
                (di, lang),
            );
        }
    }

    // 2단계 — 언어별로 렌더링.
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("{}: 디렉토리 생성 실패 — {e}", out_dir.display()))?;

    let mut stats = Stats::default();
    for lang in Lang::ALL {
        let mut body = String::new();
        let mut toc = String::from("<ol>\n");

        for (di, doc) in DOCS.iter().enumerate() {
            let md = &sources[&(di, lang)];
            let index = &indexes[&(di, lang)];

            let (fragment, entries) =
                render_doc(md, di, lang, &known, &indexes, &mut stats);

            let _ = write!(
                body,
                "<section class=\"doc\" id=\"{}\">\n{}\n<a class=\"backtotop\" href=\"#toc\">{}</a>\n</section>\n",
                doc.id,
                fragment,
                match lang {
                    Lang::Ko => "▲ 목차로",
                    Lang::En => "▲ Back to contents",
                }
            );

            let _ = write!(
                toc,
                "<li><a href=\"#{}\">{}</a>",
                doc.id,
                escape_html(&index.title)
            );
            if !entries.is_empty() {
                toc.push_str("\n<ol>\n");
                for (id, text) in entries {
                    let _ = writeln!(toc, "<li><a href=\"#{id}\">{}</a></li>", escape_html(&text));
                }
                toc.push_str("</ol>\n");
            }
            toc.push_str("</li>\n");
        }
        toc.push_str("</ol>\n");

        let (title, brand, notice, switch_label, toc_title) = match lang {
            Lang::Ko => (
                "UNIM 도움말",
                "UNIM 도움말",
                format!(
                    "이 문서는 <code>docs/user/</code> 의 마크다운에서 <b>자동 생성</b>된 오프라인 사본이다. \
                     내용이 오래됐거나 링크가 깨졌다면 최신판을 <a href=\"{ONLINE_DOCS}\">GitHub 문서</a>에서 확인한다."
                ),
                "English",
                "목차",
            ),
            Lang::En => (
                "UNIM Help",
                "UNIM Help",
                format!(
                    "This page is an offline copy <b>generated automatically</b> from the Markdown in <code>docs/user/</code>. \
                     If anything looks out of date or a link is broken, see the latest version in the <a href=\"{ONLINE_DOCS}\">GitHub docs</a>."
                ),
                "한국어",
                "Contents",
            ),
        };

        let page = Page {
            lang_code: lang.code(),
            title,
            brand,
            version: env!("CARGO_PKG_VERSION"),
            gen_notice: &notice,
            lang_switch_label: switch_label,
            lang_switch_href: &lang.other().out_file(),
            toc_title,
            toc: &toc,
            body: &body,
        };

        let out_path = out_dir.join(lang.out_file());
        std::fs::write(&out_path, page.render())
            .map_err(|e| format!("{}: 쓰기 실패 — {e}", out_path.display()))?;
        println!("생성: {}", out_path.display());
    }

    Ok(stats)
}

// ─── 1단계: 앵커 색인 ────────────────────────────────────────────────────────

fn index_doc(md: &str, doc: &DocDef, lang: Lang) -> DocIndex {
    let mut slugger = Slugger::new();
    let mut ids = Vec::new();
    let mut title = None;

    for (level, text) in headings(md) {
        let id = slugger.slug(&text);
        if title.is_none() && level == HeadingLevel::H1 {
            title = Some(text);
        }
        ids.push(id);
    }

    DocIndex {
        ids,
        title: title.unwrap_or_else(|| {
            match lang {
                Lang::Ko => doc.fallback_title_ko,
                Lang::En => doc.fallback_title_en,
            }
            .to_string()
        }),
    }
}

/// 마크다운에서 (레벨, 평문 텍스트) 헤딩 목록을 순서대로 뽑는다.
/// 코드펜스 안의 `# 주석`은 파서가 알아서 걸러 준다 — 정규식으로 훑으면 안 되는 이유다.
fn headings(md: &str) -> Vec<(HeadingLevel, String)> {
    let mut out = Vec::new();
    let mut current: Option<(HeadingLevel, String)> = None;

    for event in Parser::new_ext(md, md_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => current = Some((level, String::new())),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = current.take() {
                    out.push(h);
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, buf)) = current.as_mut() {
                    buf.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn md_options() -> Options {
    // SMART_PUNCTUATION 은 쓰지 않는다 — 따옴표를 바꿔 놓으면 원문 대조가 어려워진다.
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
}

// ─── 2단계: 렌더링 ───────────────────────────────────────────────────────────

/// 문서 하나를 HTML 조각으로 변환하고, 목차용 (id, 텍스트) 항목을 함께 돌려준다.
fn render_doc(
    md: &str,
    doc_idx: usize,
    lang: Lang,
    known: &HashMap<String, (usize, Lang)>,
    indexes: &HashMap<(usize, Lang), DocIndex>,
    stats: &mut Stats,
) -> (String, Vec<(String, String)>) {
    let doc = &DOCS[doc_idx];
    let base_dir = format!("{DOC_ROOT}/{}", doc.dir);

    let mut slugger = Slugger::new();
    let mut toc_entries = Vec::new();
    let mut out_events: Vec<Event> = Vec::new();

    // 헤딩 안의 인라인 이벤트를 모았다가 id 를 계산한 뒤에 내보낸다.
    let mut heading: Option<(HeadingLevel, Vec<Event>, String)> = None;
    let mut seen_title = false;
    // 링크마다 "raw <a> 로 찍었는가"를 쌓아 두고 닫는 쪽에서 맞춰 닫는다.
    let mut raw_link: Vec<bool> = Vec::new();

    for event in Parser::new_ext(md, md_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((level, Vec::new(), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                let Some((level, inner, text)) = heading.take() else {
                    continue;
                };
                let id = format!("{}--{}", doc.id, slugger.slug(&text));
                // 병합하면 문서 제목들이 한 페이지에 모이므로 레벨을 한 단계씩 내린다.
                let rendered_level = (level_num(level) + 1).min(6);

                // 문서의 첫 h1 은 페이지 목차의 1단 항목, h1/h2 는 2단 항목이 된다.
                let is_title = level == HeadingLevel::H1 && !seen_title;
                if is_title {
                    seen_title = true;
                } else if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    toc_entries.push((id.clone(), text.clone()));
                }

                out_events.push(Event::Html(CowStr::from(format!(
                    "<h{rendered_level} id=\"{id}\">"
                ))));
                out_events.extend(inner);
                out_events.push(Event::Html(CowStr::from(format!("</h{rendered_level}>\n"))));
            }
            // 표는 좁은 화면에서 본문을 가로로 밀지 않도록 스크롤 래퍼로 감싼다.
            Event::Start(Tag::Table(aligns)) => {
                out_events.push(Event::Html(CowStr::from("<div class=\"tablewrap\">")));
                out_events.push(Event::Start(Tag::Table(aligns)));
            }
            Event::End(TagEnd::Table) => {
                out_events.push(Event::End(TagEnd::Table));
                out_events.push(Event::Html(CowStr::from("</div>")));
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let ev = match rewrite_link(
                    &dest_url, doc_idx, lang, &base_dir, known, indexes, stats,
                ) {
                    // 문서 내 앵커는 `<a>` 를 직접 찍는다. pulldown-cmark 의 href 이스케이프를
                    // 거치면 `#troubleshooting--빌드-실패` 가 퍼센트 인코딩되어 raw 한글 id 와
                    // 글자 그대로는 어긋난다(브라우저는 디코딩해 맞춰 주지만, 굳이 기대지 않는다).
                    Rewritten::Anchor(href) => {
                        raw_link.push(true);
                        Event::InlineHtml(CowStr::from(format!(
                            "<a href=\"{}\">",
                            escape_html(&href)
                        )))
                    }
                    // 진짜 URL 은 퍼센트 인코딩이 옳다 — 기본 렌더러에 맡긴다.
                    Rewritten::Url(dest) => {
                        raw_link.push(false);
                        Event::Start(Tag::Link {
                            link_type,
                            dest_url: CowStr::from(dest),
                            title,
                            id,
                        })
                    }
                };
                push(&mut heading, &mut out_events, ev);
            }
            Event::End(TagEnd::Link) => {
                let ev = if raw_link.pop().unwrap_or(false) {
                    Event::InlineHtml(CowStr::from("</a>"))
                } else {
                    Event::End(TagEnd::Link)
                };
                push(&mut heading, &mut out_events, ev);
            }
            other => {
                // 헤딩 텍스트는 슬러그 계산에 쓰이므로 따로 모은다.
                if let Some((_, _, text)) = heading.as_mut() {
                    if let Event::Text(t) | Event::Code(t) = &other {
                        text.push_str(t);
                    }
                }
                push(&mut heading, &mut out_events, other);
            }
        }
    }

    let mut html_out = String::new();
    html::push_html(&mut html_out, out_events.into_iter());
    (html_out, toc_entries)
}

/// 헤딩을 수집 중이면 헤딩 버퍼로, 아니면 본문으로 이벤트를 흘린다.
fn push<'a>(
    heading: &mut Option<(HeadingLevel, Vec<Event<'a>>, String)>,
    out: &mut Vec<Event<'a>>,
    ev: Event<'a>,
) {
    match heading.as_mut() {
        Some((_, inner, _)) => inner.push(ev),
        None => out.push(ev),
    }
}

fn level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ─── 링크 재작성 ─────────────────────────────────────────────────────────────

/// 재작성 결과. `Anchor` 는 병합 HTML 안(또는 다른 언어판)을 가리키는 앵커,
/// `Url` 은 바깥 세상을 가리키는 진짜 URL 이다. 렌더링 방식이 달라 구분한다.
enum Rewritten {
    Anchor(String),
    Url(String),
}

fn rewrite_link(
    dest: &str,
    doc_idx: usize,
    lang: Lang,
    base_dir: &str,
    known: &HashMap<String, (usize, Lang)>,
    indexes: &HashMap<(usize, Lang), DocIndex>,
    stats: &mut Stats,
) -> Rewritten {
    // 외부 URL·mailto 는 손대지 않는다.
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with("mailto:")
        || dest.starts_with("data:")
    {
        stats.external += 1;
        return Rewritten::Url(dest.to_string());
    }

    let (path_part, fragment) = match dest.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (dest, None),
    };

    // 대상 문서 판별: 프래그먼트만 있으면 자기 자신, 아니면 경로를 정규화해 조회한다.
    let target = if path_part.is_empty() {
        Some((doc_idx, lang))
    } else {
        let resolved = normalize_path(base_dir, path_part);
        known.get(&resolved).copied()
    };

    let Some((t_doc, t_lang)) = target else {
        // 병합 범위 밖(docs/dev/**, 루트 README, 릴리스 노트 등) → GitHub 절대 URL.
        stats.github_fallback += 1;
        return Rewritten::Url(github_url(&normalize_path(base_dir, path_part), fragment));
    };

    let target_id = &DOCS[t_doc].id;

    // 앵커 없는 링크는 대상 문서의 <section> 을 가리킨다.
    let Some(frag) = fragment.filter(|f| !f.is_empty()) else {
        return if t_lang == lang {
            stats.internal += 1;
            Rewritten::Anchor(format!("#{target_id}"))
        } else {
            stats.cross_lang += 1;
            Rewritten::Anchor(format!("{}#{target_id}", t_lang.out_file()))
        };
    };

    // 프래그먼트는 GitHub 슬러그다. 우리 id 규칙은 `{doc}--{slug}`.
    if indexes[&(t_doc, t_lang)].ids.iter().any(|id| id == frag) {
        return if t_lang == lang {
            stats.internal += 1;
            Rewritten::Anchor(format!("#{target_id}--{frag}"))
        } else {
            stats.cross_lang += 1;
            Rewritten::Anchor(format!("{}#{target_id}--{frag}", t_lang.out_file()))
        };
    }

    // 원본 문서에 없는 앵커 — 빌드를 깨뜨리지 않고 경고 + GitHub 폴백.
    stats.warnings.push(format!(
        "[{}] {} → {}#{frag} : 대상 문서에 해당 앵커가 없다 (원본 링크가 깨져 있음) → GitHub 폴백",
        lang.code(),
        DOCS[doc_idx].id,
        DOCS[t_doc].id
    ));
    stats.github_fallback += 1;
    Rewritten::Url(github_url(
        &format!("{DOC_ROOT}/{}/{}", DOCS[t_doc].dir, t_lang.file_name()),
        Some(frag),
    ))
}

fn github_url(repo_rel_path: &str, fragment: Option<&str>) -> String {
    match fragment.filter(|f| !f.is_empty()) {
        Some(f) => format!("{REPO_BLOB}/{repo_rel_path}#{f}"),
        None => format!("{REPO_BLOB}/{repo_rel_path}"),
    }
}

/// `base` 기준 상대경로를 저장소 루트 기준 경로로 정규화한다.
fn normalize_path(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_parent_traversal() {
        assert_eq!(
            normalize_path("docs/user/user-guide", "../faq/README-ko.md"),
            "docs/user/faq/README-ko.md"
        );
        assert_eq!(
            normalize_path("docs/user/faq", "../../../CONTRIBUTING.md"),
            "CONTRIBUTING.md"
        );
        assert_eq!(
            normalize_path("docs/user/keyboard-shortcuts", "README.md"),
            "docs/user/keyboard-shortcuts/README.md"
        );
    }

    #[test]
    fn headings_ignore_code_fences() {
        let md = "# Real\n\n```sh\n# not a heading\n```\n\n## Also real\n";
        let hs = headings(md);
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0].1, "Real");
        assert_eq!(hs[1].1, "Also real");
    }

    #[test]
    fn github_url_keeps_fragment() {
        assert_eq!(
            github_url("docs/dev/specs/POPUP_SPEC.md", Some("a-b")),
            "https://github.com/from104/unim/blob/main/docs/dev/specs/POPUP_SPEC.md#a-b"
        );
        assert_eq!(
            github_url("README.md", None),
            "https://github.com/from104/unim/blob/main/README.md"
        );
    }
}
