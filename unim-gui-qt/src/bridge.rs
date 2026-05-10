//! cxx-qt 브릿지: Rust DBus ↔ Qt QML
//!
//! DBus 시그널을 수신하여 Qt 시그널로 변환합니다.
//! popup 3종(한자/특수문자/이모지)은 PopupModel을 통해 QML에 데이터를 제공합니다.

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use rust_i18n::t;
use std::sync::{Arc, Mutex, RwLock};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    // 백그라운드 스레드에서 Qt 시그널 발행 허용
    impl cxx_qt::Threading for UnimBridge {}

    extern "RustQt" {
        /// UNIM DBus 브릿지 QObject
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_korean)]
        #[qproperty(bool, connected)]
        // popup 상태 프로퍼티
        #[qproperty(bool, popup_visible)]
        #[qproperty(u32, popup_kind)]
        #[qproperty(i32, popup_x)]
        #[qproperty(i32, popup_y)]
        #[namespace = "unim"]
        type UnimBridge = super::UnimBridgeRust;

        // ─── 시그널 ───

        /// 입력 모드 변경
        #[qsignal]
        fn mode_changed(self: Pin<&mut Self>, is_korean: bool);

        /// PopupModel 데이터 갱신 — QML이 다시 그리도록 트리거
        #[qsignal]
        fn popup_render_changed(self: Pin<&mut Self>);

        /// popup 표시
        #[qsignal]
        fn popup_show(self: Pin<&mut Self>);

        /// popup 숨김
        #[qsignal]
        fn popup_hide(self: Pin<&mut Self>);

        // ─── i18n 헬퍼 ───

        /// 모드 상태 라벨
        #[qinvokable]
        fn mode_status_label(self: &UnimBridge, is_korean: bool) -> QString;

        /// 앱 윈도우 타이틀
        #[qinvokable]
        fn window_title(self: &UnimBridge) -> QString;

        // ─── PopupModel 조회 invokable ───

        /// 셀 텍스트 (row, col)
        #[qinvokable]
        fn popup_cell_text(self: &UnimBridge, row: i32, col: i32) -> QString;

        /// 셀 뜻/설명 (row, col)
        #[qinvokable]
        fn popup_cell_meaning(self: &UnimBridge, row: i32, col: i32) -> QString;

        /// 셀 플래그 비트마스크 (row, col) — 0x01=has_data 0x02=selected 0x04=col_hl 0x08=row_hl 0x10=bookmarked
        #[qinvokable]
        fn popup_cell_flags(self: &UnimBridge, row: i32, col: i32) -> u32;

        /// 행 수
        #[qinvokable]
        fn popup_rows(self: &UnimBridge) -> i32;

        /// 열 수
        #[qinvokable]
        fn popup_cols(self: &UnimBridge) -> i32;

        /// 헤더 텍스트
        #[qinvokable]
        fn popup_header_text(self: &UnimBridge) -> QString;

        /// 푸터 텍스트
        #[qinvokable]
        fn popup_footer_text(self: &UnimBridge) -> QString;

        /// 푸터 표시 여부
        #[qinvokable]
        fn popup_show_footer(self: &UnimBridge) -> bool;

        /// 전체 페이지 수
        #[qinvokable]
        fn popup_total_pages(self: &UnimBridge) -> i32;

        /// 현재 페이지 (0-based)
        #[qinvokable]
        fn popup_current_page(self: &UnimBridge) -> i32;

        /// 선택 행
        #[qinvokable]
        fn popup_sel_row(self: &UnimBridge) -> i32;

        /// 선택 열
        #[qinvokable]
        fn popup_sel_col(self: &UnimBridge) -> i32;

        /// expand 버튼 표시 여부
        #[qinvokable]
        fn popup_expand_visible(self: &UnimBridge) -> bool;

        /// expand 버튼 텍스트 (⊞/⊟)
        #[qinvokable]
        fn popup_expand_text(self: &UnimBridge) -> QString;

        /// 탭 레이블 (idx)
        #[qinvokable]
        fn popup_tab_label(self: &UnimBridge, idx: i32) -> QString;

        /// 탭 수
        #[qinvokable]
        fn popup_tab_count(self: &UnimBridge) -> i32;

        /// 활성 탭 인덱스
        #[qinvokable]
        fn popup_active_tab(self: &UnimBridge) -> i32;

        /// 열 헤더 텍스트 (idx)
        #[qinvokable]
        fn popup_col_header(self: &UnimBridge, idx: i32) -> QString;

        /// 열 헤더 강조 여부 (idx)
        #[qinvokable]
        fn popup_col_header_hl(self: &UnimBridge, idx: i32) -> bool;

        /// 행 헤더 텍스트 (idx)
        #[qinvokable]
        fn popup_row_header(self: &UnimBridge, idx: i32) -> QString;

        /// 행 헤더 강조 여부 (idx)
        #[qinvokable]
        fn popup_row_header_hl(self: &UnimBridge, idx: i32) -> bool;

        // ─── DBus 액션 invokable (QML에서 직접 호출) ───

        /// 한자 선택 (page_local_index)
        #[qinvokable]
        fn popup_select_hanja(self: &UnimBridge, page_local_index: u32);

        /// 한자 팝업 취소
        #[qinvokable]
        fn popup_cancel_hanja(self: &UnimBridge);

        /// 페이지 이동 (direction: 음수=이전, 양수=다음)
        #[qinvokable]
        fn popup_change_page(self: &UnimBridge, direction: i32);

        /// expand 토글
        #[qinvokable]
        fn popup_toggle_expand(self: &UnimBridge);

        /// 한자 북마크 토글 (global_index)
        #[qinvokable]
        fn popup_toggle_bookmark(self: &UnimBridge, global_index: u32);

        /// 특수문자 선택 (col, row — 0-based)
        #[qinvokable]
        fn popup_select_special(self: &UnimBridge, col: u32, row: u32);

        /// 특수문자 팝업 취소
        #[qinvokable]
        fn popup_cancel_special(self: &UnimBridge);

        /// 이모지 커밋
        #[qinvokable]
        fn popup_commit_emoji(self: &UnimBridge, emoji_str: QString);

        /// 이모지 카테고리 변경 (idx)
        #[qinvokable]
        fn popup_set_emoji_category(self: &UnimBridge, idx: u32);
    }

    impl cxx_qt::Initialize for UnimBridge {}
}

use unim_gui_common::dbus_client;
use unim_gui_common::popup_dbus;
use unim_gui_common::popup_position::compute_popup_xy;
use unim_gui_common::popup_state::PopupModel;
use unim_gui_common::tray::TrayController;
use unim_gui_common::types::{GuiAction, IndicatorState};

/// Rust 측 QObject 구조체
pub struct UnimBridgeRust {
    is_korean: bool,
    connected: bool,
    // popup 상태 프로퍼티
    popup_visible: bool,
    popup_kind: u32,
    popup_x: i32,
    popup_y: i32,
    // toolkit-free 팝업 모델 (백그라운드 스레드와 공유)
    popup_model: Arc<Mutex<PopupModel>>,
}

#[allow(clippy::derivable_impls)]
impl Default for UnimBridgeRust {
    fn default() -> Self {
        Self {
            is_korean: false,
            connected: false,
            popup_visible: false,
            popup_kind: 0,
            popup_x: 0,
            popup_y: 0,
            popup_model: Arc::new(Mutex::new(PopupModel::new())),
        }
    }
}

// ─── i18n / 기본 invokable ───

impl qobject::UnimBridge {
    pub fn mode_status_label(&self, is_korean: bool) -> QString {
        let s = if is_korean {
            t!("modepopup_status_korean")
        } else {
            t!("modepopup_status_english")
        };
        QString::from(&s.to_string())
    }

    pub fn window_title(&self) -> QString {
        QString::from(&t!("qt_window_title").to_string())
    }
}

// ─── PopupModel 조회 invokable 구현 ───

impl qobject::UnimBridge {
    pub fn popup_cell_text(&self, row: i32, col: i32) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if row < 0 || col < 0 {
            return QString::from("");
        }
        match model.cell(row as u32, col as u32) {
            Some((text, _, _)) => QString::from(text.as_str()),
            None => QString::from(""),
        }
    }

    pub fn popup_cell_meaning(&self, row: i32, col: i32) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if row < 0 || col < 0 {
            return QString::from("");
        }
        match model.cell(row as u32, col as u32) {
            Some((_, meaning, _)) => QString::from(meaning.as_str()),
            None => QString::from(""),
        }
    }

    pub fn popup_cell_flags(&self, row: i32, col: i32) -> u32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if row < 0 || col < 0 {
            return 0;
        }
        match model.cell(row as u32, col as u32) {
            Some((_, _, flags)) => *flags,
            None => 0,
        }
    }

    pub fn popup_rows(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.rows as i32
    }

    pub fn popup_cols(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.cols as i32
    }

    pub fn popup_header_text(&self) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        QString::from(model.header_text.as_str())
    }

    pub fn popup_footer_text(&self) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        QString::from(model.footer_text.as_str())
    }

    pub fn popup_show_footer(&self) -> bool {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.show_footer
    }

    pub fn popup_total_pages(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.total_pages as i32
    }

    pub fn popup_current_page(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.current_page as i32
    }

    pub fn popup_sel_row(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.sel_row as i32
    }

    pub fn popup_sel_col(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.sel_col as i32
    }

    pub fn popup_expand_visible(&self) -> bool {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.expand_visible
    }

    pub fn popup_expand_text(&self) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        QString::from(model.expand_text.as_str())
    }

    pub fn popup_tab_label(&self, idx: i32) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if idx < 0 {
            return QString::from("");
        }
        model
            .tab_labels
            .get(idx as usize)
            .map(|s| QString::from(s.as_str()))
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn popup_tab_count(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.tab_labels.len() as i32
    }

    pub fn popup_active_tab(&self) -> i32 {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        model.active_tab_index as i32
    }

    pub fn popup_col_header(&self, idx: i32) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if idx < 0 {
            return QString::from("");
        }
        model
            .col_headers
            .get(idx as usize)
            .map(|(s, _)| QString::from(s.as_str()))
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn popup_col_header_hl(&self, idx: i32) -> bool {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if idx < 0 {
            return false;
        }
        model
            .col_headers
            .get(idx as usize)
            .map(|(_, hl)| *hl)
            .unwrap_or(false)
    }

    pub fn popup_row_header(&self, idx: i32) -> QString {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if idx < 0 {
            return QString::from("");
        }
        model
            .row_headers
            .get(idx as usize)
            .map(|(s, _)| QString::from(s.as_str()))
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn popup_row_header_hl(&self, idx: i32) -> bool {
        let model = self.rust().popup_model.lock().unwrap_or_else(|e| e.into_inner());
        if idx < 0 {
            return false;
        }
        model
            .row_headers
            .get(idx as usize)
            .map(|(_, hl)| *hl)
            .unwrap_or(false)
    }
}

// ─── DBus 액션 invokable 구현 ───

impl qobject::UnimBridge {
    pub fn popup_select_hanja(&self, page_local_index: u32) {
        popup_dbus::select_hanja_via_dbus(page_local_index);
    }

    pub fn popup_cancel_hanja(&self) {
        popup_dbus::cancel_hanja_via_dbus();
    }

    pub fn popup_change_page(&self, direction: i32) {
        popup_dbus::popup_change_page_via_dbus(direction);
    }

    pub fn popup_toggle_expand(&self) {
        popup_dbus::toggle_popup_expand_via_dbus();
    }

    pub fn popup_toggle_bookmark(&self, global_index: u32) {
        popup_dbus::toggle_hanja_bookmark_via_dbus(global_index);
    }

    pub fn popup_select_special(&self, col: u32, row: u32) {
        popup_dbus::select_special_via_dbus(col as usize, row as usize);
    }

    pub fn popup_cancel_special(&self) {
        popup_dbus::cancel_special_via_dbus();
    }

    pub fn popup_commit_emoji(&self, emoji_str: QString) {
        let s = emoji_str.to_string();
        popup_dbus::commit_emoji_via_dbus(s);
    }

    pub fn popup_set_emoji_category(&self, idx: u32) {
        popup_dbus::set_emoji_category_via_dbus(idx);
    }
}

// ─── Constructor / Initialize ───

impl cxx_qt::Constructor<()> for qobject::UnimBridge {
    type NewArguments = ();
    type BaseArguments = ();
    type InitializeArguments = ();

    fn route_arguments(
        _args: (),
    ) -> (
        Self::NewArguments,
        Self::BaseArguments,
        Self::InitializeArguments,
    ) {
        ((), (), ())
    }

    fn new((): ()) -> UnimBridgeRust {
        UnimBridgeRust::default()
    }

    /// QObject 초기화 시 DBus 연결 시작
    fn initialize(self: Pin<&mut Self>, _arguments: Self::InitializeArguments) {
        let state = Arc::new(RwLock::new(IndicatorState::default()));
        let (popup_tx, popup_rx) = std::sync::mpsc::channel::<GuiAction>();
        let (tray_update_tx, _tray_update_rx) = std::sync::mpsc::channel::<()>();

        // popup_model Arc 공유
        let popup_model = self.rust().popup_model.clone();

        // DBus 시그널 감시 (백그라운드 스레드)
        let dbus_state = state.clone();
        {
            let (_dummy_dbus_tx, dummy_dbus_rx) =
                tokio::sync::mpsc::channel::<GuiAction>(1);
            let (dummy_popup_tx, _) = std::sync::mpsc::channel::<GuiAction>();
            let (dummy_dbus_action_tx, _) = tokio::sync::mpsc::channel::<GuiAction>(1);
            let dummy_state = Arc::new(RwLock::new(IndicatorState::default()));
            let dummy_controller = Arc::new(TrayController::new(
                dummy_state,
                dummy_popup_tx,
                dummy_dbus_action_tx,
            ));
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(_) => return,
                };
                rt.block_on(dbus_client::watch_dbus_signals(
                    dbus_state,
                    tray_update_tx,
                    popup_tx,
                    dummy_dbus_rx,
                    dummy_controller,
                ));
            });
        }

        // GuiAction 수신 → Qt 시그널 발행
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            // popup 좌표 계산용 팝업 창 크기 추정치
            // TODO: Qt에서 QGuiApplication::primaryScreen()으로 실제 화면 크기 조회
            const SCREEN_W: i32 = 1920;
            const SCREEN_H: i32 = 1080;
            const POPUP_W: i32 = 300;
            const POPUP_H_COMPACT: i32 = 160;
            const POPUP_H_EXPANDED: i32 = 340;

            while let Ok(action) = popup_rx.recv() {
                let qt = qt_thread.clone();
                match action {
                    GuiAction::UpdateCategory(category) => {
                        let is_korean =
                            category == unim::status::InputCategory::Korean;
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_is_korean(is_korean);
                            bridge.as_mut().set_connected(true);
                            bridge.as_mut().mode_changed(is_korean);
                        })
                        .ok();
                    }

                    GuiAction::ShowHanjaPopup { x, y, h, .. } => {
                        let (px, py) = compute_popup_xy(
                            x, y, h,
                            POPUP_W, POPUP_H_COMPACT,
                            0, 0, SCREEN_W, SCREEN_H,
                        );
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_popup_kind(0); // Hanja
                            bridge.as_mut().set_popup_x(px);
                            bridge.as_mut().set_popup_y(py);
                            bridge.as_mut().set_popup_visible(true);
                            bridge.as_mut().popup_show();
                        })
                        .ok();
                    }

                    GuiAction::ShowSpecialPopup { x, y, h, .. } => {
                        let (px, py) = compute_popup_xy(
                            x, y, h,
                            POPUP_W, POPUP_H_COMPACT,
                            0, 0, SCREEN_W, SCREEN_H,
                        );
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_popup_kind(1); // Special
                            bridge.as_mut().set_popup_x(px);
                            bridge.as_mut().set_popup_y(py);
                            bridge.as_mut().set_popup_visible(true);
                            bridge.as_mut().popup_show();
                        })
                        .ok();
                    }

                    GuiAction::ShowEmojiPopup { x, y, h, .. } => {
                        let (px, py) = compute_popup_xy(
                            x, y, h,
                            POPUP_W, POPUP_H_EXPANDED,
                            0, 0, SCREEN_W, SCREEN_H,
                        );
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_popup_kind(2); // Emoji
                            bridge.as_mut().set_popup_x(px);
                            bridge.as_mut().set_popup_y(py);
                            bridge.as_mut().set_popup_visible(true);
                            bridge.as_mut().popup_show();
                        })
                        .ok();
                    }

                    GuiAction::HidePopup => {
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().set_popup_visible(false);
                            bridge.as_mut().popup_hide();
                        })
                        .ok();
                    }

                    GuiAction::PopupRender {
                        kind,
                        target,
                        header_text,
                        footer_text,
                        show_footer,
                        rows,
                        cols,
                        sel_row,
                        sel_col,
                        current_page,
                        total_pages,
                        cells,
                        col_headers,
                        row_headers,
                        expand_visible,
                        expand_text,
                        tab_labels,
                        active_tab_index,
                    } => {
                        // expanded 여부에 따라 popup 높이 재계산 (expand_visible=false가 expanded 상태)
                        let is_expanded = !expand_visible || expand_text.contains('⊟');
                        let popup_h = if is_expanded { POPUP_H_EXPANDED } else { POPUP_H_COMPACT };
                        let _ = popup_h; // 좌표 재계산은 Show 시점에만 수행

                        {
                            let model_arc = popup_model.clone();
                            let mut model = model_arc.lock().unwrap_or_else(|e| e.into_inner());
                            model.apply_render(
                                kind,
                                target,
                                header_text,
                                footer_text,
                                show_footer,
                                rows,
                                cols,
                                sel_row,
                                sel_col,
                                current_page,
                                total_pages,
                                cells,
                                col_headers,
                                row_headers,
                                expand_visible,
                                expand_text,
                                tab_labels,
                                active_tab_index,
                            );
                        }
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().popup_render_changed();
                        })
                        .ok();
                    }

                    GuiAction::PopupNavigate {
                        page,
                        total_pages,
                        sel_row,
                        sel_col,
                        ..
                    } => {
                        {
                            let model_arc = popup_model.clone();
                            let mut model = model_arc.lock().unwrap_or_else(|e| e.into_inner());
                            model.apply_navigate(page, total_pages, sel_row, sel_col);
                        }
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().popup_render_changed();
                        })
                        .ok();
                    }

                    // HanjaBookmarkChanged: PopupRender가 뒤따라오므로 별도 처리 불필요
                    GuiAction::HanjaBookmarkChanged { .. }
                    | GuiAction::HanjaBookmarkStatesFetched { .. }
                    | GuiAction::HanjaCandidatesReordered { .. } => {
                        // PopupRender signal이 북마크 상태를 포함해 전체를 다시 렌더링함
                    }

                    GuiAction::ShowModePopup
                    | GuiAction::OpenSettings
                    | GuiAction::SetGlobalMode(_) => {}
                }
            }
        });
    }
}
