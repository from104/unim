//! cxx-qt 브릿지: Rust DBus ↔ Qt QML
//!
//! DBus 시그널을 수신하여 Qt 시그널로 변환합니다.
//! popup 3종(한자/특수문자/이모지)은 PopupModel을 통해 QML에 데이터를 제공합니다.
//! Phase 3: settings 다이얼로그용 Config 프로퍼티 및 invokable 추가.

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
        // ─── Settings 프로퍼티 ───
        // 일반 > 자판
        #[qproperty(QString, korean_layout)]
        #[qproperty(QString, english_layout)]
        #[qproperty(QString, toggle_keys)]
        #[qproperty(QString, hanja_keys)]
        // 일반 > 입력 모드
        #[qproperty(i32, initial_mode)]       // 0=Korean, 1=English
        #[qproperty(i32, mode_sharing)]       // 0=Global, 1=PerApp
        // 일반 > 자동 영문 전환
        #[qproperty(bool, auto_english_enabled)]
        #[qproperty(QString, auto_english_triggers)]
        // 모아치기
        #[qproperty(bool, moachigi_bidir)]
        #[qproperty(i32, moachigi_chord_ms)]  // 0=OFF
        // AutoTypeFix 마스터
        #[qproperty(bool, autotypefix_enabled)]
        #[qproperty(bool, autotypefix_rollback)]
        #[qproperty(i32, autotypefix_obs_secs)]
        #[qproperty(i32, autotypefix_expiry_hours)]
        // AutoTypeFix 순방향
        #[qproperty(bool, fwd_enabled)]
        #[qproperty(i32, fwd_kor_threshold)]
        #[qproperty(i32, fwd_window_ms)]
        #[qproperty(bool, fwd_skip_engword)]
        // AutoTypeFix 역방향
        #[qproperty(bool, rev_enabled)]
        #[qproperty(i32, rev_eng_threshold)]
        #[qproperty(i32, rev_window_ms)]
        #[qproperty(bool, rev_skip_complete)]
        #[qproperty(bool, rev_userdict_enabled)]
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

        /// settings 창 열기 요청
        #[qsignal]
        fn open_settings(self: Pin<&mut Self>);

        /// 저장 완료 토스트 알림 — QML에서 Popup/SnackBar로 표시
        #[qsignal]
        fn toast_message(self: Pin<&mut Self>, text: QString);

        /// settings config 재로드 완료 — QML 프로퍼티 갱신 요청
        #[qsignal]
        fn config_reloaded(self: Pin<&mut Self>);

        /// Blacklist CRUD 완료 — QML ListView 갱신 트리거
        #[qsignal]
        fn blacklist_changed(self: Pin<&mut Self>);

        /// UserDict CRUD 완료 — QML ListView 갱신 트리거
        #[qsignal]
        fn userdict_changed(self: Pin<&mut Self>);

        // ─── i18n 헬퍼 ───

        /// 모드 상태 라벨
        #[qinvokable]
        fn mode_status_label(self: &UnimBridge, is_korean: bool) -> QString;

        /// 앱 윈도우 타이틀
        #[qinvokable]
        fn window_title(self: &UnimBridge) -> QString;

        // ─── Settings i18n 헬퍼 ───

        /// t!() 래퍼 — QML에서 qsTr 대신 Rust i18n 키로 번역
        #[qinvokable]
        fn tr_key(self: &UnimBridge, key: QString) -> QString;

        // ─── Settings 조회 invokable ───

        /// 사용 가능한 한국어 자판 목록 (JSON array string)
        #[qinvokable]
        fn available_korean_layouts(self: &UnimBridge) -> QString;

        /// 사용 가능한 영어 자판 목록 (JSON array string)
        #[qinvokable]
        fn available_english_layouts(self: &UnimBridge) -> QString;

        /// 현재 한국어 자판의 표시 이름
        #[qinvokable]
        fn korean_layout_display(self: &UnimBridge) -> QString;

        /// GNOME 세션 여부
        #[qinvokable]
        fn is_gnome_session(self: &UnimBridge) -> bool;

        /// Wayland 세션 여부
        #[qinvokable]
        fn is_wayland_session(self: &UnimBridge) -> bool;

        // ─── Settings setter invokable ───
        // 주의: cxx-qt가 qproperty마다 set_<name>을 자동 생성하므로,
        //       invokable setter는 "apply_" 접두사를 사용해 충돌 방지.

        #[qinvokable]
        fn apply_korean_layout(self: Pin<&mut Self>, name: QString);

        #[qinvokable]
        fn apply_english_layout(self: Pin<&mut Self>, name: QString);

        #[qinvokable]
        fn apply_toggle_keys(self: Pin<&mut Self>, keys: QString);

        #[qinvokable]
        fn apply_hanja_keys(self: Pin<&mut Self>, keys: QString);

        #[qinvokable]
        fn apply_initial_mode(self: Pin<&mut Self>, mode: i32);

        #[qinvokable]
        fn apply_mode_sharing(self: Pin<&mut Self>, mode: i32);

        #[qinvokable]
        fn apply_auto_english_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_auto_english_triggers(self: Pin<&mut Self>, keys: QString);

        #[qinvokable]
        fn apply_moachigi_bidir(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_moachigi_chord_ms(self: Pin<&mut Self>, ms: i32);

        #[qinvokable]
        fn apply_autotypefix_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_autotypefix_rollback(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_autotypefix_obs_secs(self: Pin<&mut Self>, secs: i32);

        #[qinvokable]
        fn apply_autotypefix_expiry_hours(self: Pin<&mut Self>, hours: i32);

        #[qinvokable]
        fn apply_fwd_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_fwd_kor_threshold(self: Pin<&mut Self>, v: i32);

        #[qinvokable]
        fn apply_fwd_window_ms(self: Pin<&mut Self>, ms: i32);

        #[qinvokable]
        fn apply_fwd_skip_engword(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_rev_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_rev_eng_threshold(self: Pin<&mut Self>, v: i32);

        #[qinvokable]
        fn apply_rev_window_ms(self: Pin<&mut Self>, ms: i32);

        #[qinvokable]
        fn apply_rev_skip_complete(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn apply_rev_userdict_enabled(self: Pin<&mut Self>, enabled: bool);

        /// Config를 파일/DBus 경유 재로드 후 프로퍼티 갱신
        #[qinvokable]
        fn reload_config(self: Pin<&mut Self>);

        // ─── Blacklist CRUD invokable ───

        /// 전체 Blacklist 항목 수
        #[qinvokable]
        fn blacklist_count(self: &UnimBridge) -> i32;

        /// 특정 status 항목 수 (0=Tentative, 1=Confirmed, 2=Inactive)
        #[qinvokable]
        fn blacklist_count_by_status(self: &UnimBridge, status: i32) -> i32;

        /// idx번째 항목의 ASCII 문자열
        #[qinvokable]
        fn blacklist_entry_ascii(self: &UnimBridge, idx: i32) -> QString;

        /// idx번째 항목의 방향 (0=Forward, 1=Reverse)
        #[qinvokable]
        fn blacklist_entry_direction(self: &UnimBridge, idx: i32) -> i32;

        /// idx번째 항목의 상태 (0=Tentative, 1=Confirmed, 2=Inactive)
        #[qinvokable]
        fn blacklist_entry_status(self: &UnimBridge, idx: i32) -> i32;

        /// idx번째 항목의 hit count
        #[qinvokable]
        fn blacklist_entry_hits(self: &UnimBridge, idx: i32) -> i32;

        /// idx번째 항목의 관찰 시각 (Unix seconds)
        #[qinvokable]
        fn blacklist_entry_observed_at(self: &UnimBridge, idx: i32) -> i64;

        /// 해당 status 기준으로 필터링한 전체 idx 목록 (JSON array)
        #[qinvokable]
        fn blacklist_indices_by_status(self: &UnimBridge, status: i32) -> QString;

        /// Tentative → Confirmed 승격
        #[qinvokable]
        fn blacklist_promote(self: Pin<&mut Self>, idx: i32);

        /// Confirmed → Inactive 비활성화
        #[qinvokable]
        fn blacklist_deactivate(self: Pin<&mut Self>, idx: i32);

        /// Inactive → Confirmed 재활성화
        #[qinvokable]
        fn blacklist_reactivate(self: Pin<&mut Self>, idx: i32);

        /// 항목 삭제
        #[qinvokable]
        fn blacklist_remove(self: Pin<&mut Self>, idx: i32);

        /// 디스크에서 재로드
        #[qinvokable]
        fn blacklist_reload(self: Pin<&mut Self>);

        // ─── UserDict CRUD invokable ───

        /// 전체 UserDict 항목 수
        #[qinvokable]
        fn userdict_count(self: &UnimBridge) -> i32;

        /// idx번째 단어
        #[qinvokable]
        fn userdict_word(self: &UnimBridge, idx: i32) -> QString;

        /// idx번째 메모
        #[qinvokable]
        fn userdict_note(self: &UnimBridge, idx: i32) -> QString;

        /// 단어 추가 — 성공: 빈 문자열, 실패: 에러 메시지
        #[qinvokable]
        fn userdict_add(self: Pin<&mut Self>, word: QString, note: QString) -> QString;

        /// idx번째 항목 갱신 — 성공: 빈 문자열, 실패: 에러 메시지
        #[qinvokable]
        fn userdict_update(self: Pin<&mut Self>, idx: i32, word: QString, note: QString) -> QString;

        /// idx번째 항목 삭제
        #[qinvokable]
        fn userdict_remove(self: Pin<&mut Self>, idx: i32);

        /// 디스크에서 재로드
        #[qinvokable]
        fn userdict_reload(self: Pin<&mut Self>);

        /// 사전 검증 — 성공: 빈 문자열, 실패: 에러 메시지 (저장 안 함)
        #[qinvokable]
        fn userdict_validate(self: &UnimBridge, word: QString) -> QString;

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

use unim::config::{Config, InputCategory, ModeSharingMode};
use unim::typefix_blacklist::Blacklist;
use unim::typefix_userdict::UserDictionary;
use unim_gui_common::dbus_client;
use unim_gui_common::popup_dbus;
use unim_gui_common::popup_position::compute_popup_xy;
use unim_gui_common::popup_state::PopupModel;
use unim_gui_common::settings_dbus::save_config_via_dbus;
use unim_gui_common::settings_helpers::{
    apply_korean_profile_choice, collect_korean_profile_choices, is_gnome_session as detect_gnome,
    is_wayland_session as detect_wayland, load_and_resolve,
};
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
    // ─── Settings 상태 ───
    config: Config,
    // 일반 > 자판
    korean_layout: QString,
    english_layout: QString,
    toggle_keys: QString,
    hanja_keys: QString,
    // 일반 > 입력 모드
    initial_mode: i32,
    mode_sharing: i32,
    // 자동 영문 전환
    auto_english_enabled: bool,
    auto_english_triggers: QString,
    // 모아치기
    moachigi_bidir: bool,
    moachigi_chord_ms: i32,
    // AutoTypeFix 마스터
    autotypefix_enabled: bool,
    autotypefix_rollback: bool,
    autotypefix_obs_secs: i32,
    autotypefix_expiry_hours: i32,
    // AutoTypeFix 순방향
    fwd_enabled: bool,
    fwd_kor_threshold: i32,
    fwd_window_ms: i32,
    fwd_skip_engword: bool,
    // AutoTypeFix 역방향
    rev_enabled: bool,
    rev_eng_threshold: i32,
    rev_window_ms: i32,
    rev_skip_complete: bool,
    rev_userdict_enabled: bool,
    // Blacklist / UserDict (CRUD 상태, Mutex 없이 메인 스레드만 접근)
    blacklist: Blacklist,
    userdict: UserDictionary,
}

impl Default for UnimBridgeRust {
    fn default() -> Self {
        let config = Config::load_from_default_path();
        Self::from_config(config)
    }
}

impl UnimBridgeRust {
    fn from_config(config: Config) -> Self {
        let atf = &config.engine.auto_typefix;
        let kor = &config.engine.korean;
        Self {
            is_korean: false,
            connected: false,
            popup_visible: false,
            popup_kind: 0,
            popup_x: 0,
            popup_y: 0,
            popup_model: Arc::new(Mutex::new(PopupModel::new())),
            korean_layout: QString::from(
                unim::config::korean_layout_display_name(&kor.effective_layout_name()),
            ),
            english_layout: QString::from(config.engine.english.layout.as_str()),
            toggle_keys: QString::from(config.engine.toggle_keys.join(", ").as_str()),
            hanja_keys: QString::from(config.engine.hanja_keys.join(", ").as_str()),
            initial_mode: match config.engine.default_category {
                InputCategory::Korean => 0,
                InputCategory::English => 1,
            },
            mode_sharing: match config.engine.mode_sharing {
                ModeSharingMode::Global => 0,
                ModeSharingMode::PerApp => 1,
            },
            auto_english_enabled: config.engine.auto_english.enabled,
            auto_english_triggers: QString::from(
                config.engine.auto_english.trigger_keys.join(", ").as_str(),
            ),
            moachigi_bidir: kor.bidirectional_combine.unwrap_or(false),
            moachigi_chord_ms: kor.chord_window_ms.unwrap_or(0) as i32,
            autotypefix_enabled: atf.enabled,
            autotypefix_rollback: atf.rollback_detection,
            autotypefix_obs_secs: atf.observation_timeout_secs as i32,
            autotypefix_expiry_hours: atf.tentative_expiry_hours as i32,
            fwd_enabled: atf.forward,
            fwd_kor_threshold: atf.kor_syllable_threshold as i32,
            fwd_window_ms: atf.forward_time_window_ms as i32,
            fwd_skip_engword: atf.skip_on_english_word,
            rev_enabled: atf.reverse,
            rev_eng_threshold: atf.eng_word_min_length as i32,
            rev_window_ms: atf.reverse_time_window_ms as i32,
            rev_skip_complete: atf.skip_on_complete_syllable,
            rev_userdict_enabled: atf.user_dict_enabled,
            blacklist: Blacklist::load_from_default_path(),
            userdict: UserDictionary::load_from_default_path(),
            config,
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

    pub fn tr_key(&self, key: QString) -> QString {
        // rust_i18n t!()은 리터럴만 받으므로, 런타임 키를 지원하기 위해
        // GTK locales의 키를 직접 조회하는 방식 대신 알려진 키만 처리.
        // QML은 이를 통해 Rust i18n 번역을 표시한다.
        // 알 수 없는 키는 그대로 반환 (개발 중 새 키 발견 용이).
        let k = key.to_string();
        let translated = match k.as_str() {
            "settings_window_title" => t!("settings_window_title"),
            "page_general_title" => t!("page_general_title"),
            "page_typefix_title" => t!("page_typefix_title"),
            "page_blacklist_title" => t!("page_blacklist_title"),
            "page_userdict_title" => t!("page_userdict_title"),
            "group_layouts_keymap" => t!("group_layouts_keymap"),
            "row_korean_layout" => t!("row_korean_layout"),
            "row_korean_layout_subtitle" => t!("row_korean_layout_subtitle"),
            "row_english_layout" => t!("row_english_layout"),
            "row_toggle_keys" => t!("row_toggle_keys"),
            "row_toggle_keys_subtitle" => t!("row_toggle_keys_subtitle"),
            "row_hanja_keys" => t!("row_hanja_keys"),
            "row_hanja_keys_subtitle" => t!("row_hanja_keys_subtitle"),
            "group_input_mode" => t!("group_input_mode"),
            "row_initial_mode" => t!("row_initial_mode"),
            "row_mode_sharing" => t!("row_mode_sharing"),
            "mode_share_global" => t!("mode_share_global"),
            "mode_share_perapp" => t!("mode_share_perapp"),
            "group_auto_english" => t!("group_auto_english"),
            "row_auto_english_enable" => t!("row_auto_english_enable"),
            "row_auto_english_triggers" => t!("row_auto_english_triggers"),
            "row_auto_english_triggers_subtitle" => t!("row_auto_english_triggers_subtitle"),
            "group_moachigi_title" => t!("group_moachigi_title"),
            "row_moachigi_bidirectional_label" => t!("row_moachigi_bidirectional_label"),
            "row_moachigi_bidirectional_subtitle" => t!("row_moachigi_bidirectional_subtitle"),
            "row_moachigi_chord_label" => t!("row_moachigi_chord_label"),
            "row_moachigi_chord_subtitle" => t!("row_moachigi_chord_subtitle"),
            "group_typefix_master" => t!("group_typefix_master"),
            "row_typefix_master" => t!("row_typefix_master"),
            "row_typefix_master_subtitle" => t!("row_typefix_master_subtitle"),
            "group_typefix_retrigger" => t!("group_typefix_retrigger"),
            "row_typefix_retrigger_subtitle" => t!("row_typefix_retrigger_subtitle"),
            "row_typefix_observe_window" => t!("row_typefix_observe_window"),
            "row_typefix_observe_window_subtitle" => t!("row_typefix_observe_window_subtitle"),
            "row_typefix_tentative_expiry" => t!("row_typefix_tentative_expiry"),
            "row_typefix_tentative_expiry_subtitle" => t!("row_typefix_tentative_expiry_subtitle"),
            "group_typefix_forward" => t!("group_typefix_forward"),
            "row_typefix_enable" => t!("row_typefix_enable"),
            "row_typefix_kor_threshold" => t!("row_typefix_kor_threshold"),
            "row_typefix_kor_threshold_subtitle" => t!("row_typefix_kor_threshold_subtitle"),
            "row_typefix_forward_window" => t!("row_typefix_forward_window"),
            "row_typefix_forward_window_subtitle" => t!("row_typefix_forward_window_subtitle"),
            "row_typefix_skip_engword" => t!("row_typefix_skip_engword"),
            "row_typefix_skip_engword_subtitle" => t!("row_typefix_skip_engword_subtitle"),
            "group_typefix_reverse" => t!("group_typefix_reverse"),
            "row_typefix_eng_threshold" => t!("row_typefix_eng_threshold"),
            "row_typefix_eng_threshold_subtitle" => t!("row_typefix_eng_threshold_subtitle"),
            "row_typefix_reverse_window" => t!("row_typefix_forward_window"),
            "row_typefix_reverse_window_subtitle" => t!("row_typefix_reverse_window_subtitle"),
            "row_typefix_skip_complete" => t!("row_typefix_skip_complete"),
            "row_typefix_skip_complete_subtitle" => t!("row_typefix_skip_complete_subtitle"),
            "row_typefix_userdict" => t!("row_typefix_userdict"),
            "row_typefix_userdict_subtitle" => t!("row_typefix_userdict_subtitle"),
            "settings_toast_saved" => t!("settings_toast_saved"),
            "settings_toast_save_failed" => t!("settings_toast_save_failed", err = ""),
            "common_ok" => t!("common_ok"),
            "common_cancel" => t!("common_cancel"),
            "common_save" => t!("common_save"),
            "common_add" => t!("common_add"),
            "group_display" => t!("group_display"),
            "row_panel_indicator" => t!("row_panel_indicator"),
            "row_panel_indicator_subtitle" => t!("row_panel_indicator_subtitle"),
            "row_show_notification" => t!("row_show_notification"),
            "row_show_notification_subtitle" => t!("row_show_notification_subtitle"),
            "group_ime" => t!("group_ime"),
            "group_ime_subtitle" => t!("group_ime_subtitle"),
            "row_ime_enable" => t!("row_ime_enable"),
            "row_ime_enable_subtitle" => t!("row_ime_enable_subtitle"),
            "page_gnome_title" => t!("page_gnome_title"),
            // Blacklist 페이지
            "blacklist_group_tentative" => t!("blacklist_group_tentative"),
            "blacklist_group_confirmed" => t!("blacklist_group_confirmed"),
            "blacklist_group_inactive" => t!("blacklist_group_inactive"),
            "blacklist_empty" => t!("blacklist_empty"),
            "blacklist_tentative_desc" => t!("blacklist_tentative_desc", count = ""),
            "blacklist_confirmed_desc" => t!("blacklist_confirmed_desc", count = ""),
            "blacklist_inactive_desc" => t!("blacklist_inactive_desc", count = ""),
            "blacklist_btn_confirm" => t!("blacklist_btn_confirm"),
            "blacklist_btn_disable" => t!("blacklist_btn_disable"),
            "blacklist_btn_delete" => t!("blacklist_btn_delete"),
            "blacklist_btn_reactivate" => t!("blacklist_btn_reactivate"),
            "blacklist_kind_forward" => t!("blacklist_kind_forward"),
            "blacklist_kind_reverse" => t!("blacklist_kind_reverse"),
            "blacklist_reload_button" => t!("blacklist_reload_button"),
            // UserDict 페이지
            "userdict_group_title" => t!("userdict_group_title"),
            "userdict_empty" => t!("userdict_empty"),
            "userdict_add_button" => t!("userdict_add_button"),
            "userdict_edit_dialog_title_add" => t!("userdict_edit_dialog_title_add"),
            "userdict_edit_dialog_title_edit" => t!("userdict_edit_dialog_title_edit"),
            "userdict_edit_dialog_word_label" => t!("userdict_edit_dialog_word_label"),
            "userdict_edit_dialog_note_label" => t!("userdict_edit_dialog_note_label"),
            "userdict_validate_empty_word" => t!("userdict_validate_empty_word"),
            "userdict_validate_not_ascii" => t!("userdict_validate_not_ascii"),
            "userdict_validate_duplicate" => t!("userdict_validate_duplicate"),
            "userdict_reload_button" => t!("userdict_reload_button"),
            _ => std::borrow::Cow::Owned(k.clone()),
        };
        QString::from(translated.as_ref())
    }
}

// ─── Settings 조회 invokable ───

impl qobject::UnimBridge {
    pub fn available_korean_layouts(&self) -> QString {
        let choices = collect_korean_profile_choices();
        // JSON array: [{"name":"ko_2bulstd","display":"두벌식 표준"}, ...]
        let json = choices
            .iter()
            .map(|(n, d)| format!("{{\"name\":\"{}\",\"display\":\"{}\"}}", n, d))
            .collect::<Vec<_>>()
            .join(",");
        QString::from(format!("[{}]", json).as_str())
    }

    pub fn available_english_layouts(&self) -> QString {
        let builtins = unim::config::ENGLISH_LAYOUT_BUILTINS;
        let json = builtins
            .iter()
            .map(|l| {
                format!(
                    "{{\"name\":\"{}\",\"display\":\"{}\"}}",
                    l,
                    unim::config::english_layout_display_name(l)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        QString::from(format!("[{}]", json).as_str())
    }

    pub fn korean_layout_display(&self) -> QString {
        let name = self.rust().config.engine.korean.effective_layout_name();
        let display = unim::config::korean_layout_display_name(&name);
        QString::from(display)
    }

    pub fn is_gnome_session(&self) -> bool {
        detect_gnome()
    }

    pub fn is_wayland_session(&self) -> bool {
        detect_wayland()
    }
}

// ─── Settings setter invokable 구현 (apply_ 접두사로 qproperty auto-setter 충돌 방지) ───

/// 저장 + DBus 전파 + QML 토스트 헬퍼
fn save_and_emit(
    pin: Pin<&mut qobject::UnimBridge>,
    label: &str,
) {
    let config = pin.rust().config.clone();
    if let Err(e) = config.save_to_default_path() {
        unim::unim_log!("SETTINGS-QT", "[Settings] 저장 실패 ({}): {}", label, e);
        return;
    }
    save_config_via_dbus(&config, label);
    let msg = QString::from(t!("settings_toast_saved").as_ref());
    pin.toast_message(msg);
}

impl qobject::UnimBridge {
    pub fn apply_korean_layout(mut self: Pin<&mut Self>, name: QString) {
        let name_str = name.to_string();
        let Some(profile) = load_and_resolve(&name_str) else {
            return;
        };
        let display: &str = unim::config::korean_layout_display_name(&name_str);
        let display_qs = QString::from(display);
        {
            let mut rust = self.as_mut().rust_mut();
            apply_korean_profile_choice(&mut rust.config, &name_str, &profile);
            rust.korean_layout = display_qs.clone();
        }
        self.as_mut().set_korean_layout(display_qs);
        save_and_emit(self, "korean_layout");
    }

    pub fn apply_english_layout(mut self: Pin<&mut Self>, name: QString) {
        let name_str = name.to_string();
        let qs = name.clone();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.english.layout = name_str;
            rust.english_layout = qs.clone();
        }
        self.as_mut().set_english_layout(qs);
        save_and_emit(self, "english_layout");
    }

    pub fn apply_toggle_keys(mut self: Pin<&mut Self>, keys: QString) {
        let s = keys.to_string();
        let parsed: Vec<String> = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        let qs = keys.clone();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.toggle_keys = parsed;
            rust.toggle_keys = qs.clone();
        }
        self.as_mut().set_toggle_keys(qs);
        save_and_emit(self, "toggle_keys");
    }

    pub fn apply_hanja_keys(mut self: Pin<&mut Self>, keys: QString) {
        let s = keys.to_string();
        let parsed: Vec<String> = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        let qs = keys.clone();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.hanja_keys = parsed;
            rust.hanja_keys = qs.clone();
        }
        self.as_mut().set_hanja_keys(qs);
        save_and_emit(self, "hanja_keys");
    }

    pub fn apply_initial_mode(mut self: Pin<&mut Self>, mode: i32) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.default_category = if mode == 0 {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            rust.initial_mode = mode;
        }
        self.as_mut().set_initial_mode(mode);
        save_and_emit(self, "initial_mode");
    }

    pub fn apply_mode_sharing(mut self: Pin<&mut Self>, mode: i32) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.mode_sharing = if mode == 1 {
                ModeSharingMode::PerApp
            } else {
                ModeSharingMode::Global
            };
            rust.mode_sharing = mode;
        }
        self.as_mut().set_mode_sharing(mode);
        save_and_emit(self, "mode_sharing");
    }

    pub fn apply_auto_english_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_english.enabled = enabled;
            rust.auto_english_enabled = enabled;
        }
        self.as_mut().set_auto_english_enabled(enabled);
        save_and_emit(self, "auto_english_enabled");
    }

    pub fn apply_auto_english_triggers(mut self: Pin<&mut Self>, keys: QString) {
        let s = keys.to_string();
        let parsed: Vec<String> = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        let qs = keys.clone();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_english.trigger_keys = parsed;
            rust.auto_english_triggers = qs.clone();
        }
        self.as_mut().set_auto_english_triggers(qs);
        save_and_emit(self, "auto_english_triggers");
    }

    pub fn apply_moachigi_bidir(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.korean.bidirectional_combine = Some(enabled);
            rust.moachigi_bidir = enabled;
        }
        self.as_mut().set_moachigi_bidir(enabled);
        save_and_emit(self, "moachigi_bidir");
    }

    pub fn apply_moachigi_chord_ms(mut self: Pin<&mut Self>, ms: i32) {
        let v: Option<u16> = if ms <= 0 { None } else { Some(ms as u16) };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.korean.chord_window_ms = v;
            rust.moachigi_chord_ms = ms;
        }
        self.as_mut().set_moachigi_chord_ms(ms);
        save_and_emit(self, "moachigi_chord_ms");
    }

    pub fn apply_autotypefix_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.enabled = enabled;
            rust.autotypefix_enabled = enabled;
        }
        self.as_mut().set_autotypefix_enabled(enabled);
        save_and_emit(self, "autotypefix_enabled");
    }

    pub fn apply_autotypefix_rollback(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.rollback_detection = enabled;
            rust.autotypefix_rollback = enabled;
        }
        self.as_mut().set_autotypefix_rollback(enabled);
        save_and_emit(self, "autotypefix_rollback");
    }

    pub fn apply_autotypefix_obs_secs(mut self: Pin<&mut Self>, secs: i32) {
        let v = secs.clamp(
            unim::config::AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN as i32,
            unim::config::AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX as i32,
        ) as u8;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.observation_timeout_secs = v;
            rust.autotypefix_obs_secs = v as i32;
        }
        self.as_mut().set_autotypefix_obs_secs(v as i32);
        save_and_emit(self, "autotypefix_obs_secs");
    }

    pub fn apply_autotypefix_expiry_hours(mut self: Pin<&mut Self>, hours: i32) {
        let v = hours.clamp(
            unim::config::AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN as i32,
            unim::config::AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX as i32,
        ) as u16;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.tentative_expiry_hours = v;
            rust.autotypefix_expiry_hours = v as i32;
        }
        self.as_mut().set_autotypefix_expiry_hours(v as i32);
        save_and_emit(self, "autotypefix_expiry_hours");
    }

    pub fn apply_fwd_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.forward = enabled;
            rust.fwd_enabled = enabled;
        }
        self.as_mut().set_fwd_enabled(enabled);
        save_and_emit(self, "fwd_enabled");
    }

    pub fn apply_fwd_kor_threshold(mut self: Pin<&mut Self>, v: i32) {
        let val = v.clamp(
            unim::config::AUTO_TYPEFIX_KOR_THRESHOLD_MIN as i32,
            unim::config::AUTO_TYPEFIX_KOR_THRESHOLD_MAX as i32,
        ) as u8;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.kor_syllable_threshold = val;
            rust.fwd_kor_threshold = val as i32;
        }
        self.as_mut().set_fwd_kor_threshold(val as i32);
        save_and_emit(self, "fwd_kor_threshold");
    }

    pub fn apply_fwd_window_ms(mut self: Pin<&mut Self>, ms: i32) {
        let val = (ms as u32).clamp(
            unim::config::AUTO_TYPEFIX_TIME_WINDOW_MIN,
            unim::config::AUTO_TYPEFIX_TIME_WINDOW_MAX,
        );
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.forward_time_window_ms = val;
            rust.fwd_window_ms = val as i32;
        }
        self.as_mut().set_fwd_window_ms(val as i32);
        save_and_emit(self, "fwd_window_ms");
    }

    pub fn apply_fwd_skip_engword(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.skip_on_english_word = enabled;
            rust.fwd_skip_engword = enabled;
        }
        self.as_mut().set_fwd_skip_engword(enabled);
        save_and_emit(self, "fwd_skip_engword");
    }

    pub fn apply_rev_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.reverse = enabled;
            rust.rev_enabled = enabled;
        }
        self.as_mut().set_rev_enabled(enabled);
        save_and_emit(self, "rev_enabled");
    }

    pub fn apply_rev_eng_threshold(mut self: Pin<&mut Self>, v: i32) {
        let val = v.clamp(
            unim::config::AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN as i32,
            unim::config::AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX as i32,
        ) as u8;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.eng_word_min_length = val;
            rust.rev_eng_threshold = val as i32;
        }
        self.as_mut().set_rev_eng_threshold(val as i32);
        save_and_emit(self, "rev_eng_threshold");
    }

    pub fn apply_rev_window_ms(mut self: Pin<&mut Self>, ms: i32) {
        let val = (ms as u32).clamp(
            unim::config::AUTO_TYPEFIX_TIME_WINDOW_MIN,
            unim::config::AUTO_TYPEFIX_TIME_WINDOW_MAX,
        );
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.reverse_time_window_ms = val;
            rust.rev_window_ms = val as i32;
        }
        self.as_mut().set_rev_window_ms(val as i32);
        save_and_emit(self, "rev_window_ms");
    }

    pub fn apply_rev_skip_complete(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.skip_on_complete_syllable = enabled;
            rust.rev_skip_complete = enabled;
        }
        self.as_mut().set_rev_skip_complete(enabled);
        save_and_emit(self, "rev_skip_complete");
    }

    pub fn apply_rev_userdict_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config.engine.auto_typefix.user_dict_enabled = enabled;
            rust.rev_userdict_enabled = enabled;
        }
        self.as_mut().set_rev_userdict_enabled(enabled);
        save_and_emit(self, "rev_userdict_enabled");
    }

    pub fn reload_config(mut self: Pin<&mut Self>) {
        let new_config = Config::load_from_default_path();
        let atf = &new_config.engine.auto_typefix;
        let kor = &new_config.engine.korean;
        // 프로퍼티 갱신
        let kor_display = QString::from(
            unim::config::korean_layout_display_name(&kor.effective_layout_name())
        );
        let eng_layout = QString::from(new_config.engine.english.layout.as_str());
        let toggle_keys = QString::from(new_config.engine.toggle_keys.join(", ").as_str());
        let hanja_keys = QString::from(new_config.engine.hanja_keys.join(", ").as_str());
        let initial_mode = match new_config.engine.default_category {
            InputCategory::Korean => 0,
            InputCategory::English => 1,
        };
        let mode_sharing = match new_config.engine.mode_sharing {
            ModeSharingMode::Global => 0,
            ModeSharingMode::PerApp => 1,
        };
        let ae_enabled = new_config.engine.auto_english.enabled;
        let ae_triggers = QString::from(new_config.engine.auto_english.trigger_keys.join(", ").as_str());
        let moachigi_bidir = kor.bidirectional_combine.unwrap_or(false);
        let moachigi_chord_ms = kor.chord_window_ms.unwrap_or(0) as i32;
        let atf_enabled = atf.enabled;
        let atf_rollback = atf.rollback_detection;
        let atf_obs = atf.observation_timeout_secs as i32;
        let atf_expiry = atf.tentative_expiry_hours as i32;
        let fwd_en = atf.forward;
        let fwd_kor = atf.kor_syllable_threshold as i32;
        let fwd_ms = atf.forward_time_window_ms as i32;
        let fwd_skip = atf.skip_on_english_word;
        let rev_en = atf.reverse;
        let rev_eng = atf.eng_word_min_length as i32;
        let rev_ms = atf.reverse_time_window_ms as i32;
        let rev_syl = atf.skip_on_complete_syllable;
        let rev_ud = atf.user_dict_enabled;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config = new_config;
        }
        self.as_mut().set_korean_layout(kor_display);
        self.as_mut().set_english_layout(eng_layout);
        self.as_mut().set_toggle_keys(toggle_keys);
        self.as_mut().set_hanja_keys(hanja_keys);
        self.as_mut().set_initial_mode(initial_mode);
        self.as_mut().set_mode_sharing(mode_sharing);
        self.as_mut().set_auto_english_enabled(ae_enabled);
        self.as_mut().set_auto_english_triggers(ae_triggers);
        self.as_mut().set_moachigi_bidir(moachigi_bidir);
        self.as_mut().set_moachigi_chord_ms(moachigi_chord_ms);
        self.as_mut().set_autotypefix_enabled(atf_enabled);
        self.as_mut().set_autotypefix_rollback(atf_rollback);
        self.as_mut().set_autotypefix_obs_secs(atf_obs);
        self.as_mut().set_autotypefix_expiry_hours(atf_expiry);
        self.as_mut().set_fwd_enabled(fwd_en);
        self.as_mut().set_fwd_kor_threshold(fwd_kor);
        self.as_mut().set_fwd_window_ms(fwd_ms);
        self.as_mut().set_fwd_skip_engword(fwd_skip);
        self.as_mut().set_rev_enabled(rev_en);
        self.as_mut().set_rev_eng_threshold(rev_eng);
        self.as_mut().set_rev_window_ms(rev_ms);
        self.as_mut().set_rev_skip_complete(rev_syl);
        self.as_mut().set_rev_userdict_enabled(rev_ud);
        self.config_reloaded();
    }
}

// ─── Blacklist CRUD invokable 구현 ───

impl qobject::UnimBridge {
    pub fn blacklist_count(&self) -> i32 {
        self.rust().blacklist.entries.len() as i32
    }

    pub fn blacklist_count_by_status(&self, status: i32) -> i32 {
        let target = status_from_i32(status);
        self.rust()
            .blacklist
            .entries
            .iter()
            .filter(|e| e.status == target)
            .count() as i32
    }

    pub fn blacklist_entry_ascii(&self, idx: i32) -> QString {
        if idx < 0 {
            return QString::from("");
        }
        self.rust()
            .blacklist
            .entries
            .get(idx as usize)
            .map(|e| QString::from(e.ascii.as_str()))
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn blacklist_entry_direction(&self, idx: i32) -> i32 {
        use unim::typefix_blacklist::Direction;
        if idx < 0 {
            return 0;
        }
        self.rust()
            .blacklist
            .entries
            .get(idx as usize)
            .map(|e| match e.direction {
                Direction::Forward => 0,
                Direction::Reverse => 1,
            })
            .unwrap_or(0)
    }

    pub fn blacklist_entry_status(&self, idx: i32) -> i32 {
        use unim::typefix_blacklist::EntryStatus;
        if idx < 0 {
            return 0;
        }
        self.rust()
            .blacklist
            .entries
            .get(idx as usize)
            .map(|e| match e.status {
                EntryStatus::Tentative => 0,
                EntryStatus::Confirmed => 1,
                EntryStatus::Inactive => 2,
            })
            .unwrap_or(0)
    }

    pub fn blacklist_entry_hits(&self, idx: i32) -> i32 {
        if idx < 0 {
            return 0;
        }
        self.rust()
            .blacklist
            .entries
            .get(idx as usize)
            .map(|e| e.hit_count as i32)
            .unwrap_or(0)
    }

    pub fn blacklist_entry_observed_at(&self, idx: i32) -> i64 {
        if idx < 0 {
            return 0;
        }
        self.rust()
            .blacklist
            .entries
            .get(idx as usize)
            .map(|e| e.observed_at as i64)
            .unwrap_or(0)
    }

    pub fn blacklist_indices_by_status(&self, status: i32) -> QString {
        let target = status_from_i32(status);
        let indices: Vec<String> = self
            .rust()
            .blacklist
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.status == target)
            .map(|(i, _)| i.to_string())
            .collect();
        QString::from(format!("[{}]", indices.join(",")).as_str())
    }

    pub fn blacklist_promote(mut self: Pin<&mut Self>, idx: i32) {
        if idx < 0 {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blacklist.promote_to_confirmed(idx as usize);
            let _ = rust.blacklist.save_to_default_path();
        }
        self.as_mut().blacklist_changed();
    }

    pub fn blacklist_deactivate(mut self: Pin<&mut Self>, idx: i32) {
        if idx < 0 {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blacklist.deactivate(idx as usize);
            let _ = rust.blacklist.save_to_default_path();
        }
        self.as_mut().blacklist_changed();
    }

    pub fn blacklist_reactivate(mut self: Pin<&mut Self>, idx: i32) {
        if idx < 0 {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blacklist.reactivate_as_confirmed(idx as usize);
            let _ = rust.blacklist.save_to_default_path();
        }
        self.as_mut().blacklist_changed();
    }

    pub fn blacklist_remove(mut self: Pin<&mut Self>, idx: i32) {
        if idx < 0 {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blacklist.remove(idx as usize);
            let _ = rust.blacklist.save_to_default_path();
        }
        self.as_mut().blacklist_changed();
    }

    pub fn blacklist_reload(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blacklist = Blacklist::load_from_default_path();
        }
        self.as_mut().blacklist_changed();
    }
}

// ─── UserDict CRUD invokable 구현 ───

impl qobject::UnimBridge {
    pub fn userdict_count(&self) -> i32 {
        self.rust().userdict.reverse_words.len() as i32
    }

    pub fn userdict_word(&self, idx: i32) -> QString {
        if idx < 0 {
            return QString::from("");
        }
        self.rust()
            .userdict
            .reverse_words
            .get(idx as usize)
            .map(|e| QString::from(e.word.as_str()))
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn userdict_note(&self, idx: i32) -> QString {
        if idx < 0 {
            return QString::from("");
        }
        self.rust()
            .userdict
            .reverse_words
            .get(idx as usize)
            .and_then(|e| e.note.as_deref())
            .map(QString::from)
            .unwrap_or_else(|| QString::from(""))
    }

    pub fn userdict_add(mut self: Pin<&mut Self>, word: QString, note: QString) -> QString {
        let w = word.to_string();
        let n = note.to_string();
        let note_opt = if n.trim().is_empty() { None } else { Some(n) };
        let ok = {
            let mut rust = self.as_mut().rust_mut();
            let added = rust.userdict.add(&w, note_opt);
            if added {
                let _ = rust.userdict.save_to_default_path();
            }
            added
        };
        if ok {
            self.as_mut().userdict_changed();
            QString::from("")
        } else {
            // 실패 원인 판별
            let err = userdict_error_for_add(&self.rust().userdict, &w);
            err
        }
    }

    pub fn userdict_update(
        mut self: Pin<&mut Self>,
        idx: i32,
        word: QString,
        note: QString,
    ) -> QString {
        if idx < 0 {
            return QString::from(t!("userdict_validate_empty_word").as_ref());
        }
        let w = word.to_string();
        let n = note.to_string();
        let note_opt = if n.trim().is_empty() { None } else { Some(n) };
        let ok = {
            let mut rust = self.as_mut().rust_mut();
            let updated = rust.userdict.update_at(idx as usize, &w, note_opt);
            if updated {
                let _ = rust.userdict.save_to_default_path();
            }
            updated
        };
        if ok {
            self.as_mut().userdict_changed();
            QString::from("")
        } else {
            userdict_error_for_add(&self.rust().userdict, &w)
        }
    }

    pub fn userdict_remove(mut self: Pin<&mut Self>, idx: i32) {
        if idx < 0 {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.userdict.remove_at(idx as usize);
            let _ = rust.userdict.save_to_default_path();
        }
        self.as_mut().userdict_changed();
    }

    pub fn userdict_reload(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.userdict = UserDictionary::load_from_default_path();
        }
        self.as_mut().userdict_changed();
    }

    pub fn userdict_validate(&self, word: QString) -> QString {
        let w = word.to_string();
        userdict_error_for_add(&self.rust().userdict, &w)
    }
}

// ─── Blacklist/UserDict 헬퍼 함수 ───

fn status_from_i32(status: i32) -> unim::typefix_blacklist::EntryStatus {
    use unim::typefix_blacklist::EntryStatus;
    match status {
        1 => EntryStatus::Confirmed,
        2 => EntryStatus::Inactive,
        _ => EntryStatus::Tentative,
    }
}

fn userdict_error_for_add(ud: &UserDictionary, word: &str) -> QString {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return QString::from(t!("userdict_validate_empty_word").as_ref());
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return QString::from(t!("userdict_validate_not_ascii").as_ref());
    }
    let key = trimmed.to_lowercase();
    if ud
        .reverse_words
        .iter()
        .any(|e| e.word.to_lowercase() == key)
    {
        return QString::from(t!("userdict_validate_duplicate").as_ref());
    }
    QString::from("")
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

        // 트레이 메뉴 "설정" → bridge.open_settings() 시그널 경로 등록.
        // tray.rs::open_settings()가 SETTINGS_TX.send(OpenSettings) 호출 → popup_rx 수신 → qt 시그널.
        if let Ok(mut tx) = unim_gui_common::types::SETTINGS_TX.lock() {
            *tx = Some(popup_tx.clone());
        }

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

                    GuiAction::OpenSettings => {
                        qt.queue(move |mut bridge| {
                            bridge.as_mut().open_settings();
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

                    GuiAction::ShowModePopup | GuiAction::SetGlobalMode(_) => {}
                }
            }
        });
    }
}
