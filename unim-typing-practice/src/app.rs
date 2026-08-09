//! 메인 윈도우 + 통합 CSS + 데몬 비활성 다이얼로그 (3-step).
//!
//! DESIGN.md §6 WindowChrome 1:1.
//! 시작 시 UNIM 데몬 가용성을 확인하여, 비활성화면 3-step 안내 dialog 만 띄우고 종료.

use gtk4::prelude::*;
use gtk4::{self as gtk};
use libadwaita::prelude::*;
use libadwaita::{self as adw};

use unim_keymap_common::keyboard_view::KEYBOARD_CSS;

use crate::daemon_check;
use crate::practice_page;

pub fn build_window(app: &adw::Application) {
    if !daemon_check::is_daemon_running() {
        show_daemon_inactive_dialog(app);
        return;
    }

    let toast_overlay = adw::ToastOverlay::new();
    let header = adw::HeaderBar::new();
    let page = practice_page::build(toast_overlay.clone(), &header);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&page);
    page.set_hexpand(true);
    page.set_vexpand(true);
    body.set_vexpand(true);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&header);
    outer.append(&body);

    toast_overlay.set_child(Some(&outer));

    // 창 크기 고정 — 사용자 실측치 968×756 GDK 논리 px (scale=2 HiDPI).
    // resizable(false) 로 모든 환경에서 동일 시각 비율 유지.
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(&*rust_i18n::t!("app_title"))
        .default_width(968)
        .default_height(756)
        .resizable(false)
        .content(&toast_overlay)
        .build();

    apply_css();
    window.present();
}

/// 데몬 비활성 dialog — DESIGN.md §9 stepped variant.
/// MessageDialog (heading + body) + custom extra_child(3-step) + 닫고 종료 / 도움말 버튼.
fn show_daemon_inactive_dialog(app: &adw::Application) {
    let dialog = adw::MessageDialog::builder()
        .heading(&*rust_i18n::t!("dialog_unim_inactive_title"))
        .body(&*rust_i18n::t!("dialog_unim_inactive_body"))
        .modal(true)
        .build();

    // 상단 아이콘 (extra_child 의 첫 위젯).
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(8);

    // 3-step 가이드.
    for (n, title_key, body_key) in [
        ("1", "dialog_step_1_title", "dialog_step_1_body"),
        ("2", "dialog_step_2_title", "dialog_step_2_body"),
        ("3", "dialog_step_3_title", "dialog_step_3_body"),
    ] {
        let step = make_dialog_step(
            n,
            &rust_i18n::t!(title_key),
            &rust_i18n::t!(body_key),
        );
        body.append(&step);
    }
    dialog.set_extra_child(Some(&body));

    dialog.add_response("help", &rust_i18n::t!("dialog_btn_help"));
    dialog.add_response("close", &rust_i18n::t!("dialog_btn_close"));
    dialog.set_response_appearance("close", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");

    let app_clone = app.clone();
    dialog.connect_response(None, move |dlg, _resp| {
        dlg.close();
        app_clone.quit();
    });
    dialog.present();
}

/// 한 step 위젯 — 22×22 라운드 번호 + title/body.
fn make_dialog_step(n: &str, title: &str, body: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_halign(gtk::Align::Fill);

    let num = gtk::Label::builder()
        .label(n)
        .css_classes(["typing-dialog-step-num"])
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .width_request(22)
        .height_request(22)
        .build();
    row.append(&num);

    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_hexpand(true);
    let t = gtk::Label::builder()
        .label(title)
        .css_classes(["typing-dialog-step-title"])
        .xalign(0.0)
        .build();
    let b = gtk::Label::builder()
        .label(body)
        .css_classes(["typing-dialog-step-body", "dim-label"])
        .xalign(0.0)
        .wrap(true)
        .build();
    texts.append(&t);
    texts.append(&b);
    row.append(&texts);

    row
}

fn apply_css() {
    let provider = gtk::CssProvider::new();
    let combined = format!(
        r#"
/* keymap-cell — 키맵 그리드(히트맵) 위젯 보존. */
button.keymap-cell {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    padding: 4px;
}}
button.keymap-cell-selected {{
    background: alpha(@accent_bg_color, 0.35);
    border: 2px solid @accent_color;
}}

/* ─── Cards ──────────────────────────────────────────────────────── */
.typing-card,
.typing-target-card,
.typing-keyboard-card {{
    background: @card_bg_color;
    border: 1px solid alpha(@borders, 0.6);
    border-radius: 12px;
    box-shadow: 0 1px 2px alpha(black, 0.04);
}}
/* TargetCard padding 14 0 — 줄 단위 row 자체에 좌우 padding 적용. */
.typing-target-card {{ padding: 14px 0; }}
/* Keyboard card — cardBgSoft (시안 다크 #222 / 라이트 #f3f3f0).
 * 아래쪽 안쪽 패딩 0 — 키맵 마지막 행이 카드 하단에 닿음 (사용자 요구). */
.typing-keyboard-card {{
    background: alpha(@window_fg_color, 0.05);
    border: 1px solid alpha(@borders, 0.5);
    border-radius: 14px;
    padding: 12px 12px 0 12px;
}}

/* ─── TargetLine ─────────────────────────────────────────────────── */
.typing-target-line {{
    padding: 8px 18px 8px 14px;
    transition: background 200ms ease;
    border-left: 3px solid transparent;
}}
.typing-target-line.typing-line-current {{
    background: alpha(@accent_color, 0.10);
    border-left-color: @accent_color;
}}
.typing-target-line-label {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 16pt;
}}
.typing-line-check {{ color: alpha(@window_fg_color, 0.4); }}

/* ─── InputField (1.5px accent + 4px ring) ────────────────────────── */
.typing-input-card {{
    background: @card_bg_color;
    border: 1.5px solid @accent_color;
    border-radius: 12px;
    padding: 10px 14px;
    box-shadow: 0 0 0 4px alpha(@accent_color, 0.10);
}}
entry.typing-input {{
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 16pt;
}}
entry.typing-input text {{
    font-family: inherit;
    font-size: inherit;
    background: transparent;
}}
.typing-ime-chip {{
    font-family: "JetBrains Mono", monospace;
    font-size: 9pt;
    color: alpha(@window_fg_color, 0.6);
    border: 1px solid alpha(@borders, 0.6);
    border-radius: 4px;
    padding: 2px 6px;
}}

/* ─── Card label (uppercase) ─────────────────────────────────────── */
.typing-card-label {{
    font-size: 9pt;
    font-weight: 500;
    letter-spacing: 0.04em;
}}

/* ─── Numeric ─────────────────────────────────────────────────────── */
.typing-numeric-prominent {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 18pt;
    font-weight: 700;
    font-feature-settings: "tnum";
}}
.typing-numeric-small {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 9pt;
    font-weight: 600;
}}

/* ─── StatCell ────────────────────────────────────────────────────── */
.typing-stat-value {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 18pt;
    font-weight: 700;
    font-feature-settings: "tnum";
}}
.typing-stat-unit {{
    font-size: 9pt;
    font-weight: 600;
    color: alpha(@window_fg_color, 0.6);
    letter-spacing: 0.04em;
}}
.typing-stat-label {{
    font-size: 9pt;
    color: alpha(@window_fg_color, 0.6);
}}
.typing-stat-correct {{ color: @accent_color; }}
.typing-stat-wrong   {{ color: #d24e15; }}

/* ─── ProgressCard 20-seg ─────────────────────────────────────────── */
.typing-progress-seg {{
    background: alpha(@window_fg_color, 0.15);
    border: 1px solid alpha(@borders, 1.0);
    border-radius: 2px;
    min-height: 6px;
}}
.typing-progress-seg-filled {{
    background: @accent_color;
    border-color: transparent;
}}
.typing-progress-seg-partial {{
    background: alpha(@accent_color, 0.40);
    border-color: alpha(@accent_color, 0.5);
}}

/* ─── IMEWarning card ─────────────────────────────────────────────── */
.typing-ime-warn-card {{
    background: alpha(#d24e15, 0.10);
    border: 1px solid alpha(#d24e15, 0.18);
    border-radius: 10px;
    padding: 10px 12px;
}}
.typing-ime-warn-icon  {{ color: #d24e15; }}
.typing-ime-warn-title {{ font-weight: 600; font-size: 9pt; }}
.typing-ime-warn-body  {{ font-size: 9pt; }}

/* ─── BigStat (Result page) ───────────────────────────────────────── */
.typing-big-value {{
    font-family: "JetBrains Mono", "D2Coding", monospace;
    font-size: 32pt;
    font-weight: 700;
    font-feature-settings: "tnum";
    letter-spacing: -0.02em;
}}
.typing-big-unit {{
    font-size: 10pt;
    font-weight: 600;
    color: alpha(@window_fg_color, 0.6);
    letter-spacing: 0.04em;
}}
.typing-big-label {{
    font-size: 10pt;
    color: alpha(@window_fg_color, 0.6);
}}

/* ─── Pill button ─────────────────────────────────────────────────── */
.typing-pill-btn {{
    border-radius: 8px;
    padding: 4px 14px;
    font-size: 10pt;
    font-weight: 600;
}}

/* ─── 코퍼스 드롭다운 행 — 호버 시 수정/삭제 아이콘 노출 ─────────────── */
.typing-corpus-row .typing-row-action {{
    opacity: 0;
    transition: opacity 120ms ease;
    min-width: 24px;
    min-height: 24px;
    padding: 2px;
}}
.typing-corpus-row:hover .typing-row-action,
row:hover .typing-corpus-row .typing-row-action,
row:selected .typing-corpus-row .typing-row-action {{
    opacity: 1;
}}
.typing-row-action-danger {{ color: #d24e15; }}

/* ─── 데몬 dialog 3-step ──────────────────────────────────────────── */
.typing-dialog-step-num {{
    background: alpha(@accent_color, 0.12);
    color: @accent_color;
    border-radius: 11px;
    font-family: "JetBrains Mono", monospace;
    font-size: 9pt;
    font-weight: 700;
}}
.typing-dialog-step-title {{ font-size: 10pt; font-weight: 600; }}
.typing-dialog-step-body  {{ font-size: 9pt; }}

{}
"#,
        KEYBOARD_CSS
    );
    provider.load_from_data(&combined);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
