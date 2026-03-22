//! 한자 팝업 윈도우
//!
//! DBus ShowHanjaPopup 시그널을 받아 한자 후보 목록을 표시합니다.
//! POPUP_SPEC.md Section 3 규격을 준수합니다.

use gtk4::prelude::*;
use unim::unim_log;

use crate::popup_positioning::{self, DisplayServer};

const PAGE_SIZE: usize = 9;

/// 한자 팝업 상태
pub struct HanjaPopup {
    pub window: gtk4::Window,
    list_box: gtk4::ListBox,
    page_label: gtk4::Label,
    target_label: gtk4::Label,
    display_server: DisplayServer,
    /// 전체 후보 목록
    candidates: Vec<(String, String)>,
    /// 현재 페이지 (0-based)
    current_page: usize,
    /// 현재 선택 인덱스 (페이지 내, 0-based)
    selected: usize,
    /// 저장된 context_path (DBus 콜백용)
    context_path: String,
}

impl HanjaPopup {
    /// 새 한자 팝업 생성
    pub fn new(app: &libadwaita::Application) -> Self {
        let display_server = popup_positioning::detect_display_server();

        let window = gtk4::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(280)
            .build();
        window.add_css_class("unim-hanja-popup");

        // 메인 컨테이너
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        vbox.add_css_class("unim-hanja-vbox");

        // 타겟 글자 라벨
        let target_label = gtk4::Label::new(None);
        target_label.add_css_class("hanja-target");
        target_label.set_halign(gtk4::Align::Start);
        vbox.append(&target_label);

        // 후보 리스트
        let list_box = gtk4::ListBox::new();
        list_box.add_css_class("hanja-list");
        list_box.set_selection_mode(gtk4::SelectionMode::Single);

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .min_content_height(28 * PAGE_SIZE as i32)
            .build();
        scroll.set_child(Some(&list_box));
        vbox.append(&scroll);

        // 페이지 라벨
        let page_label = gtk4::Label::new(None);
        page_label.add_css_class("page-label");
        page_label.set_halign(gtk4::Align::End);
        vbox.append(&page_label);

        window.set_child(Some(&vbox));

        // 마우스 클릭으로 후보 선택
        list_box.connect_row_activated(|_list_box, row| {
            let index = row.index() as u32;
            select_hanja_via_dbus(index);
        });

        // AT-SPI 접근성
        window.update_property(&[gtk4::accessible::Property::Label("한자 후보")]);

        Self {
            window,
            list_box,
            page_label,
            target_label,
            display_server,
            candidates: Vec::new(),
            current_page: 0,
            selected: 0,
            context_path: String::new(),
        }
    }

    /// 한자 팝업 표시
    pub fn show(
        &mut self,
        context_path: String,
        target: &str,
        candidates: Vec<(String, String)>,
        x: i32,
        y: i32,
        _w: i32,
        h: i32,
    ) {
        if self.display_server == DisplayServer::GnomeWayland {
            return;
        }

        self.context_path = context_path;
        self.candidates = candidates;
        self.current_page = 0;
        self.selected = 0;

        self.target_label.set_text(&format!("'{}'의 한자", target));
        self.update_page();

        popup_positioning::position_popup(&self.window, x, y, h, self.display_server);
        self.window.set_visible(true);
        unim_log!("INDICATOR", "[Popup] 한자 팝업 표시: target='{}'", target);
    }

    /// 현재 페이지의 후보 목록 업데이트
    fn update_page(&self) {
        // 기존 행 제거
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let total_pages = (self.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        let start = self.current_page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.candidates.len());

        for (i, (hanja, meaning)) in self.candidates[start..end].iter().enumerate() {
            let row = gtk4::ListBoxRow::new();
            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            // 번호 라벨
            let num_label = gtk4::Label::new(Some(&format!("{}.", i + 1)));
            num_label.add_css_class("hanja-num");
            num_label.set_width_chars(2);
            num_label.set_xalign(1.0);
            hbox.append(&num_label);

            // 한자
            let hanja_label = gtk4::Label::new(Some(hanja));
            hanja_label.add_css_class("hanja-char");
            hbox.append(&hanja_label);

            // 의미
            let meaning_label = gtk4::Label::new(Some(meaning));
            meaning_label.add_css_class("hanja-meaning");
            meaning_label.set_hexpand(true);
            meaning_label.set_halign(gtk4::Align::Start);
            meaning_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            hbox.append(&meaning_label);

            row.set_child(Some(&hbox));
            self.list_box.append(&row);
        }

        // 페이지 표시
        if total_pages > 1 {
            self.page_label
                .set_text(&format!("{}/{}", self.current_page + 1, total_pages));
            self.page_label.set_visible(true);
        } else {
            self.page_label.set_visible(false);
        }
    }

    /// 네비게이션 업데이트 (PopupNavigate 시그널)
    pub fn navigate(&mut self, page: i32, _total_pages: i32, selected: i32, _rows: i32, _cols: i32, _sel_row: i32, _sel_col: i32) {
        let new_page = page.max(0) as usize;
        let new_selected = selected.max(0) as usize;

        if new_page != self.current_page {
            self.current_page = new_page;
            self.update_page();
        }

        self.selected = new_selected;

        // 페이지 내 선택 인덱스
        let page_index = self.selected.saturating_sub(self.current_page * PAGE_SIZE);
        if let Some(row) = self.list_box.row_at_index(page_index as i32) {
            self.list_box.select_row(Some(&row));
        }
    }

    /// 팝업 숨김
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 팝업이 현재 표시 중인지
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}

/// DBus를 통해 한자 선택
fn select_hanja_via_dbus(page_local_index: u32) {
    use unim_gui_common::types::ACTIVE_CONTEXT_PATH;

    let context_path = {
        ACTIVE_CONTEXT_PATH
            .lock()
            .ok()
            .and_then(|p| p.clone())
    };

    if let Some(path) = context_path {
        unim_log!(
            "INDICATOR",
            "[Popup] 한자 선택 DBus 호출: index={}, path={}",
            page_local_index,
            path
        );
        // 비동기 DBus 호출을 별도 스레드에서 수행
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Ok(conn) = zbus::Connection::session().await {
                    let proxy = zbus::Proxy::new(
                        &conn,
                        "org.atit.unim.InputMethod",
                        path.as_str(),
                        "org.atit.unim.InputContext",
                    )
                    .await;
                    if let Ok(proxy) = proxy {
                        let _: Result<String, _> =
                            proxy.call("SelectHanja", &(page_local_index,)).await;
                    }
                }
            });
        });
    }
}

/// 한자 팝업용 CSS
pub fn popup_css() -> &'static str {
    r#"
    .unim-hanja-popup {
        background-color: rgba(30, 30, 46, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
        padding: 12px;
    }

    .unim-hanja-vbox {
        padding: 0;
        margin: 0;
    }

    .hanja-target {
        color: #89b4fa;
        font-size: 13px;
        font-weight: 600;
        margin-bottom: 6px;
        padding: 0 8px;
    }

    .unim-hanja-popup .hanja-list {
        background: transparent;
        border-radius: 6px;
    }

    .unim-hanja-popup .hanja-list row {
        background: transparent;
        border-radius: 6px;
        min-height: 28px;
        padding: 0 8px;
    }

    .unim-hanja-popup .hanja-list row:selected {
        background-color: rgba(137, 180, 250, 0.2);
    }

    .hanja-num {
        color: #6c7086;
        font-size: 13px;
        min-width: 20px;
    }

    .hanja-char {
        color: #cdd6f4;
        font-size: 16px;
        font-weight: 700;
    }

    .hanja-meaning {
        color: #a6adc8;
        font-size: 13px;
    }

    .unim-hanja-popup .hanja-list row:selected .hanja-char,
    .unim-hanja-popup .hanja-list row:selected .hanja-meaning,
    .unim-hanja-popup .hanja-list row:selected .hanja-num {
        color: #cdd6f4;
    }

    .page-label {
        color: #6c7086;
        font-size: 12px;
        padding: 4px 8px 2px 0;
    }
    "#
}
