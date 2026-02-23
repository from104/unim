//! UNIM GUI 통합 프로세스
//!
//! 시스템 트레이 아이콘, 한자/특수문자 팝업, 설정 다이얼로그를 통합 제공합니다.
//! GTK4/libadwaita 기반이며, DBus 시그널을 구독하여 상태를 실시간 반영합니다.

mod hanja_popup;
mod settings_dialog;
mod special_popup;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use ksni::blocking::TrayMethods;
use ksni::menu::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use unim::unim_log;

use unim::status::InputCategory;
use unim_dbus::client::InputMethodProxy;

// ============================================================================
// 인디케이터 상태 및 트레이
// ============================================================================

/// 인디케이터 상태
#[derive(Debug, Clone, Copy, PartialEq)]
struct IndicatorState {
    category: InputCategory,
}

impl Default for IndicatorState {
    fn default() -> Self {
        Self {
            category: InputCategory::English,
        }
    }
}

/// 팝업 액션 (트레이/DBus → GTK 이벤트 루프)
#[derive(Debug, Clone)]
enum GuiAction {
    ShowModePopup,
    UpdateCategory(InputCategory),
    /// 한자 팝업 표시
    ShowHanjaPopup {
        target: String,
        candidates: Vec<(String, String)>,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    },
    /// 특수문자 팝업 표시
    ShowSpecialPopup {
        target: String,
        characters: Vec<String>,
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    },
    /// 팝업 숨김
    HidePopup,
    /// 설정 다이얼로그 열기
    OpenSettings,
}

/// ksni 트레이 구현
#[derive(Debug)]
struct UnimTray {
    state: Arc<RwLock<IndicatorState>>,
    popup_tx: Sender<GuiAction>,
}

impl ksni::Tray for UnimTray {
    fn id(&self) -> String {
        "unim-gui".into()
    }

    fn icon_theme_path(&self) -> String {
        "/usr/share/icons/hicolor/scalable/apps".into()
    }

    fn icon_name(&self) -> String {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);
        match category {
            InputCategory::Korean => "unim-korean".into(),
            InputCategory::English => "unim-english".into(),
        }
    }

    fn title(&self) -> String {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);
        match category {
            InputCategory::Korean => "UNIM - 한국어".into(),
            InputCategory::English => "UNIM - 영어".into(),
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);

        let mode_desc = match category {
            InputCategory::Korean => "한국어 모드",
            InputCategory::English => "영어 모드",
        };

        ksni::ToolTip {
            title: "UNIM 입력기".into(),
            description: mode_desc.into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.popup_tx.send(GuiAction::ShowModePopup);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            {
                let current_category = self
                    .state
                    .read()
                    .map(|s| s.category)
                    .unwrap_or(InputCategory::English);
                let korean_label = if current_category == InputCategory::Korean {
                    "✓ 한국어 모드 (Korean)"
                } else {
                    "   한국어 모드 (Korean)"
                };
                StandardItem {
                    label: korean_label.into(),
                    activate: Box::new(|this: &mut Self| {
                        if let Ok(mut s) = this.state.write() {
                            s.category = InputCategory::Korean;
                            let _ = this
                                .popup_tx
                                .send(GuiAction::UpdateCategory(InputCategory::Korean));
                            unim_log!("INDICATOR", "한국어 모드로 전환");
                        }
                    }),
                    ..Default::default()
                }
            }
            .into(),
            {
                let current_category = self
                    .state
                    .read()
                    .map(|s| s.category)
                    .unwrap_or(InputCategory::English);
                let english_label = if current_category == InputCategory::English {
                    "✓ 영어 모드 (English)"
                } else {
                    "   영어 모드 (English)"
                };
                StandardItem {
                    label: english_label.into(),
                    activate: Box::new(|this: &mut Self| {
                        if let Ok(mut s) = this.state.write() {
                            s.category = InputCategory::English;
                            let _ = this
                                .popup_tx
                                .send(GuiAction::UpdateCategory(InputCategory::English));
                            unim_log!("INDICATOR", "영어 모드로 전환");
                        }
                    }),
                    ..Default::default()
                }
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "설정 (Settings)...".into(),
                activate: Box::new(|_: &mut Self| {
                    open_settings();
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "종료 (Quit)".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| {
                    unim_log!("INDICATOR", "인디케이터 종료");
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

// ============================================================================
// 메인 함수
// ============================================================================

fn main() {
    unim_log!("INDICATOR", "UNIM GUI 시작...");

    // --settings 인자 확인: 설정 다이얼로그만 표시
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--settings") {
        run_settings_only();
        return;
    }

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<GuiAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // 트레이 메뉴 "설정" 에서 open_settings() 호출 시 GTK 이벤트 루프로 전달
    if let Ok(mut tx) = SETTINGS_TX.lock() {
        *tx = Some(popup_tx.clone());
    }

    // DBus 시그널 감시 스레드 (ksni와 완전 분리)
    let dbus_state = state.clone();
    let dbus_popup_tx = popup_tx.clone();
    // 트레이 업데이트 요청 채널 (DBus -> ksni)
    let (tray_update_tx, tray_update_rx) = std::sync::mpsc::channel::<()>();

    thread::spawn(move || {
        // 별도의 tokio 런타임 생성 (독립 스레드)
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "tokio 런타임 생성 실패: {}", e);
                return;
            }
        };

        // handle을 전달하지 않고, 채널만 사용
        rt.block_on(watch_dbus_signals(
            dbus_state,
            tray_update_tx,
            dbus_popup_tx,
        ));
    });

    // ksni 트레이 시작 (별도 스레드)
    let tray_state = state.clone();

    thread::spawn(move || {
        let tray = UnimTray {
            state: tray_state,
            popup_tx: popup_tx.clone(),
        };
        match tray.spawn() {
            Ok(handle) => {
                unim_log!("INDICATOR", "시스템 트레이 시작됨");
                // 트레이 업데이트 요청 대기 및 처리
                loop {
                    // 100ms 타임아웃으로 채널 폴링
                    match tray_update_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) => {
                            // 트레이 아이콘 업데이트
                            handle.update(|_| {});
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            // 타임아웃 - 계속 대기
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            // 채널 닫힘 - 종료
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                unim_log!("INDICATOR", "시스템 트레이 시작 실패: {}", e);
            }
        }
    });

    // GTK4/libadwaita 앱 시작 (메인 스레드)
    run_gtk_app(state, popup_rx);
}

/// DBus 서비스 이름
const UNIM_BUS_NAME: &str = "org.atit.unim.InputMethod";

/// DBus GlobalModeChanged 시그널 구독하여 트레이 업데이트 (비동기)
/// NameOwnerChanged 시그널을 감시하여 데몬 시작/종료 시 자동 재연결
async fn watch_dbus_signals(
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<GuiAction>,
) {
    use futures_util::StreamExt;

    loop {
        // DBus 연결
        let connection = match zbus::Connection::session().await {
            Ok(conn) => conn,
            Err(e) => {
                unim_log!("INDICATOR", "DBus 세션 연결 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        unim_log!("INDICATOR", "[DBus] 세션 버스 연결됨, 서비스 감시 시작...");

        // org.freedesktop.DBus 프록시 (NameOwnerChanged 감시용)
        let dbus_proxy = match zbus::fdo::DBusProxy::new(&connection).await {
            Ok(p) => p,
            Err(e) => {
                unim_log!("INDICATOR", "DBusProxy 생성 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 현재 서비스 소유자 확인
        let has_owner = match dbus_proxy
            .name_has_owner(UNIM_BUS_NAME.try_into().unwrap())
            .await
        {
            Ok(has) => has,
            Err(_) => false,
        };

        if has_owner {
            unim_log!(
                "INDICATOR",
                "[DBus] {} 서비스 발견, 연결 시도...",
                UNIM_BUS_NAME
            );
            // 서비스가 있으면 즉시 연결 시도
            watch_mode_signals(
                &connection,
                state.clone(),
                tray_update_tx.clone(),
                popup_tx.clone(),
            )
            .await;
        } else {
            unim_log!(
                "INDICATOR",
                "[DBus] {} 서비스 없음, DBus Activation 시도...",
                UNIM_BUS_NAME
            );

            // DBus Activation: StartServiceByName 호출로 데몬 자동 시작
            match dbus_proxy
                .start_service_by_name(UNIM_BUS_NAME.try_into().unwrap(), 0)
                .await
            {
                Ok(_) => {
                    unim_log!("INDICATOR", "[DBus] {} 서비스 활성화 성공", UNIM_BUS_NAME);
                    // 활성화 후 잠시 대기 후 연결
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    watch_mode_signals(
                        &connection,
                        state.clone(),
                        tray_update_tx.clone(),
                        popup_tx.clone(),
                    )
                    .await;
                }
                Err(e) => {
                    unim_log!(
                        "INDICATOR",
                        "[DBus] {} 서비스 활성화 실패: {}, 대기 중...",
                        UNIM_BUS_NAME,
                        e
                    );
                }
            }
        }

        // NameOwnerChanged 시그널 구독
        let mut stream = match dbus_proxy.receive_name_owner_changed().await {
            Ok(s) => s,
            Err(e) => {
                unim_log!("INDICATOR", "NameOwnerChanged 구독 실패: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 서비스 소유자 변경 감시
        while let Some(signal) = stream.next().await {
            if let Ok(args) = signal.args() {
                let name = args.name.as_str();
                if name != UNIM_BUS_NAME {
                    continue;
                }

                let old_owner = args.old_owner.as_ref().map(|s| s.as_str()).unwrap_or("");
                let new_owner = args.new_owner.as_ref().map(|s| s.as_str()).unwrap_or("");

                if old_owner.is_empty() && !new_owner.is_empty() {
                    // 서비스 등장
                    unim_log!(
                        "INDICATOR",
                        "[DBus] {} 서비스 등장 (owner: {})",
                        UNIM_BUS_NAME,
                        new_owner
                    );

                    // 모드 시그널 감시 시작 (서비스 종료될 때까지)
                    watch_mode_signals(
                        &connection,
                        state.clone(),
                        tray_update_tx.clone(),
                        popup_tx.clone(),
                    )
                    .await;
                } else if !old_owner.is_empty() && new_owner.is_empty() {
                    // 서비스 소멸
                    unim_log!("INDICATOR", "[DBus] {} 서비스 소멸", UNIM_BUS_NAME);

                    // 연결 안됨 상태로 표시 (기본값 English로 리셋하지 않음)
                    let _ = tray_update_tx.send(());
                }
            }
        }

        // 스트림 종료 시 재연결 시도
        unim_log!(
            "INDICATOR",
            "[DBus] NameOwnerChanged 스트림 종료, 재연결 시도..."
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// GlobalModeChanged 시그널 감시 (서비스가 연결된 상태에서 호출)
async fn watch_mode_signals(
    connection: &zbus::Connection,
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<GuiAction>,
) {
    use futures_util::StreamExt;

    // InputMethod 프록시 생성
    let proxy = match InputMethodProxy::new(connection).await {
        Ok(p) => p,
        Err(e) => {
            unim_log!("INDICATOR", "InputMethod 프록시 생성 실패: {}", e);
            return;
        }
    };

    // 초기 모드 조회
    match proxy.get_global_mode().await {
        Ok(is_korean) => {
            let category = if is_korean {
                InputCategory::Korean
            } else {
                InputCategory::English
            };
            if let Ok(mut s) = state.write() {
                s.category = category;
            }
            let _ = tray_update_tx.send(());
            let _ = popup_tx.send(GuiAction::UpdateCategory(category));
            unim_log!("INDICATOR", "[DBus] 초기 모드 조회: {:?}", category);
        }
        Err(e) => {
            unim_log!("INDICATOR", "초기 모드 조회 실패: {}", e);
        }
    }

    // GlobalModeChanged 시그널 구독
    unim_log!("INDICATOR", "[DBus] GlobalModeChanged 시그널 구독 시작...");

    let mut stream = match proxy.receive_global_mode_changed().await {
        Ok(s) => s,
        Err(e) => {
            unim_log!("INDICATOR", "시그널 구독 실패: {}", e);
            return;
        }
    };

    while let Some(signal) = stream.next().await {
        match signal.args() {
            Ok(args) => {
                let is_korean = args.is_korean;
                let category = if is_korean {
                    InputCategory::Korean
                } else {
                    InputCategory::English
                };

                let should_update = {
                    if let Ok(s) = state.read() {
                        s.category != category
                    } else {
                        true
                    }
                };

                if should_update {
                    if let Ok(mut s) = state.write() {
                        s.category = category;
                    }
                    unim_log!("INDICATOR", "[DBus] 모드 변경 감지: {:?}", category);
                    let _ = tray_update_tx.send(());
                    let _ = popup_tx.send(GuiAction::UpdateCategory(category));
                }
            }
            Err(e) => {
                unim_log!("INDICATOR", "시그널 인자 파싱 오류: {}", e);
            }
        }
    }

    unim_log!(
        "INDICATOR",
        "[DBus] GlobalModeChanged 스트림 종료 (서비스 종료?)"
    );
}

// ============================================================================
// GTK4 UI
// ============================================================================

/// GTK4/libadwaita 앱 실행
fn run_gtk_app(state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<GuiAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.gui")
        .build();

    app.connect_activate(move |app| {
        // 다크 모드 강제
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        load_css();
        let window = build_popup_window(app, state.clone());

        // 한자/특수문자 팝업 생성
        let hanja_popup = hanja_popup::HanjaPopup::new();
        let special_popup = special_popup::SpecialPopup::new();

        // 팝업 콜백 설정 (TODO: DBus SelectHanja/SelectSpecialChar 호출 연동)
        hanja_popup.connect_select(|idx| {
            unim_log!("INDICATOR", "한자 선택: index={}", idx);
            // TODO: DBus SelectHanja(idx) + 팝업 숨김
        });
        hanja_popup.connect_cancel(|| {
            unim_log!("INDICATOR", "한자 취소");
            // TODO: DBus CancelHanja()
        });
        special_popup.connect_select(|idx| {
            unim_log!("INDICATOR", "특수문자 선택: index={}", idx);
            // TODO: DBus SelectSpecialChar(idx) + 팝업 숨김
        });
        special_popup.connect_cancel(|| {
            unim_log!("INDICATOR", "특수문자 취소");
            // TODO: DBus CancelSpecialChar()
        });

        let window_clone = window.clone();
        let popup_rx_clone = popup_rx.clone();
        let app_clone = app.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if let Ok(rx) = popup_rx_clone.lock() {
                while let Ok(action) = rx.try_recv() {
                    match action {
                        GuiAction::ShowModePopup => {
                            window_clone.present();
                        }
                        GuiAction::UpdateCategory(_category) => {
                            // UI 업데이트는 창이 표시될 때 자동 처리
                        }
                        GuiAction::ShowHanjaPopup {
                            target,
                            candidates,
                            cursor_x,
                            cursor_y,
                            cursor_width,
                            cursor_height,
                        } => {
                            special_popup.hide();
                            hanja_popup.show(
                                &target,
                                candidates,
                                cursor_x,
                                cursor_y,
                                cursor_width,
                                cursor_height,
                            );
                        }
                        GuiAction::ShowSpecialPopup {
                            target,
                            characters,
                            top_row,
                            cursor_x,
                            cursor_y,
                            cursor_width,
                            cursor_height,
                        } => {
                            hanja_popup.hide();
                            special_popup.show(
                                &target,
                                characters,
                                &top_row,
                                cursor_x,
                                cursor_y,
                                cursor_width,
                                cursor_height,
                            );
                        }
                        GuiAction::HidePopup => {
                            hanja_popup.hide();
                            special_popup.hide();
                        }
                        GuiAction::OpenSettings => {
                            settings_dialog::show_settings_dialog(&app_clone);
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    });

    app.run_with_args::<String>(&[]);
}

/// CSS 스타일 로드
fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        /* 프리미엄 다크 테마 디자인 */
        window.popup-window {
            background-color: #1e1e2e;
            color: #cdd6f4;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 20px;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
        }

        .main-container {
            padding: 24px;
        }
        
        .mode-tile-container {
            margin-bottom: 20px;
        }

        .mode-button {
            min-width: 100px;
            min-height: 100px;
            font-size: 38px;
            font-weight: 800;
            border-radius: 16px;
            background: rgba(255, 255, 255, 0.05);
            color: rgba(255, 255, 255, 0.6);
            border: 2px solid transparent;
            transition: all 250ms cubic-bezier(0.4, 0, 0.2, 1);
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        }
        
        .mode-button:hover {
            background: rgba(255, 255, 255, 0.1);
            transform: translateY(-2px);
            box-shadow: 0 6px 12px rgba(0, 0, 0, 0.2);
        }
        
        .korean-btn.mode-active {
            background: linear-gradient(135deg, #3584e4 0%, #1c71d8 100%);
            color: white;
            border-color: rgba(255, 255, 255, 0.3);
            box-shadow: 0 8px 20px rgba(53, 132, 228, 0.4);
        }

        .english-btn.mode-active {
            background: linear-gradient(135deg, #5e5c64 0%, #3d3d3d 100%);
            color: white;
            border-color: rgba(255, 255, 255, 0.3);
            box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3);
        }
        
        .title-section {
            margin-bottom: 16px;
        }

        .title-label {
            font-size: 20px;
            font-weight: 800;
            color: white;
            letter-spacing: -0.5px;
        }
        
        .status-badge {
            font-size: 13px;
            font-weight: 500;
            padding: 4px 12px;
            border-radius: 20px;
            background: rgba(255, 255, 255, 0.08);
            color: rgba(255, 255, 255, 0.7);
        }

        .settings-button {
            background: transparent;
            color: rgba(255, 255, 255, 0.5);
            font-weight: 600;
            border-radius: 12px;
            padding: 8px 0;
            transition: all 200ms ease;
        }

        .settings-button:hover {
            background: rgba(255, 255, 255, 0.05);
            color: white;
        }

        /* === 한자 팝업 스타일 === */
        .hanja-popup {
            background-color: #1e1e2e;
            color: #cdd6f4;
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 12px;
        }
        .hanja-container {
            padding: 12px;
            min-width: 280px;
        }
        .hanja-target {
            font-size: 13px;
            font-weight: 600;
            color: #89b4fa;
            margin-bottom: 8px;
            padding: 4px 8px;
        }
        .hanja-row {
            padding: 6px 8px;
            border-radius: 6px;
            transition: background 150ms ease;
        }
        .hanja-row:hover {
            background: rgba(255, 255, 255, 0.05);
        }
        .hanja-selected {
            background: rgba(137, 180, 250, 0.2);
        }
        .hanja-num {
            font-size: 12px;
            color: rgba(255, 255, 255, 0.4);
            min-width: 20px;
        }
        .hanja-char {
            font-size: 18px;
            font-weight: 700;
            color: white;
            min-width: 28px;
        }
        .hanja-meaning {
            font-size: 12px;
            color: rgba(255, 255, 255, 0.6);
        }
        .hanja-page {
            font-size: 11px;
            color: rgba(255, 255, 255, 0.4);
            padding: 6px;
        }

        /* === 특수문자 팝업 스타일 === */
        .special-popup {
            background-color: #1e1e2e;
            color: #cdd6f4;
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 12px;
        }
        .special-container {
            padding: 12px;
        }
        .special-target {
            font-size: 13px;
            font-weight: 600;
            color: #a6e3a1;
            margin-bottom: 6px;
            padding: 4px 8px;
        }
        .special-header {
            margin-bottom: 2px;
        }
        .special-header-spacer {
            min-width: 28px;
        }
        .special-header-key {
            font-size: 11px;
            font-weight: 700;
            color: rgba(255, 255, 255, 0.4);
            min-width: 28px;
            min-height: 20px;
        }
        .special-header-active {
            color: #a6e3a1;
        }
        .special-grid {
            /* grid uses row/col spacing */
        }
        .special-row-num {
            font-size: 11px;
            font-weight: 600;
            color: rgba(255, 255, 255, 0.4);
            min-width: 20px;
        }
        .special-cell {
            font-size: 16px;
            min-width: 28px;
            min-height: 28px;
            border-radius: 4px;
            transition: background 100ms ease;
        }
        .special-cell:hover {
            background: rgba(255, 255, 255, 0.08);
        }
        .special-cell-selected {
            background: rgba(166, 227, 161, 0.25);
            font-weight: 700;
        }
        .special-page {
            font-size: 11px;
            color: rgba(255, 255, 255, 0.4);
            padding: 4px;
        }
        "#,
    );

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// 팝업 윈도우 생성
fn build_popup_window(app: &adw::Application, state: Arc<RwLock<IndicatorState>>) -> adw::Window {
    let window = adw::Window::builder()
        .application(app)
        .default_width(320)
        .resizable(false)
        .deletable(true)
        .build();

    window.add_css_class("popup-window");

    let current_category = state
        .read()
        .map(|s| s.category)
        .unwrap_or(InputCategory::English);

    // 메인 컨테이너
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.add_css_class("main-container");

    // 타이틀 섹션
    let title_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    title_box.add_css_class("title-section");

    let title_label = gtk4::Label::builder()
        .label("UNIM")
        .halign(gtk4::Align::Start)
        .build();
    title_label.add_css_class("title-label");

    let status_badge = gtk4::Label::builder()
        .label(match current_category {
            InputCategory::Korean => "한국어 입력 중",
            InputCategory::English => "영어 입력 중",
        })
        .halign(gtk4::Align::Start)
        .build();
    status_badge.add_css_class("status-badge");

    title_box.append(&title_label);
    title_box.append(&status_badge);

    // 타일 버튼 섹션
    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    button_box.add_css_class("mode-tile-container");
    button_box.set_halign(gtk4::Align::Center);

    let korean_btn = gtk4::Button::builder()
        .label("한")
        .tooltip_text("한국어 모드로 전환")
        .build();
    korean_btn.add_css_class("mode-button");
    korean_btn.add_css_class("korean-btn");

    let english_btn = gtk4::Button::builder()
        .label("A")
        .tooltip_text("영어 모드로 전환")
        .build();
    english_btn.add_css_class("mode-button");
    english_btn.add_css_class("english-btn");

    match current_category {
        InputCategory::Korean => korean_btn.add_css_class("mode-active"),
        InputCategory::English => english_btn.add_css_class("mode-active"),
    }

    button_box.append(&korean_btn);
    button_box.append(&english_btn);

    // 설정 버튼
    let settings_btn = gtk4::Button::builder()
        .label("설정 도구 열기")
        .halign(gtk4::Align::Fill)
        .build();
    settings_btn.add_css_class("settings-button");

    main_box.append(&title_box);
    main_box.append(&button_box);
    main_box.append(&settings_btn);

    window.set_content(Some(&main_box));

    // 이벤트 핸들러
    let status_badge_clone = status_badge.clone();
    let korean_btn_clone = korean_btn.clone();
    let english_btn_clone = english_btn.clone();
    let state_clone = state.clone();
    korean_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone.write() {
            s.category = InputCategory::Korean;
            korean_btn_clone.add_css_class("mode-active");
            english_btn_clone.remove_css_class("mode-active");
            status_badge_clone.set_text("한국어 입력 중");
            unim_log!("INDICATOR", "한국어 모드로 전환");
        }
    });

    let status_badge_clone2 = status_badge.clone();
    let korean_btn_clone2 = korean_btn.clone();
    let english_btn_clone2 = english_btn.clone();
    let state_clone2 = state.clone();
    english_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone2.write() {
            s.category = InputCategory::English;
            english_btn_clone2.add_css_class("mode-active");
            korean_btn_clone2.remove_css_class("mode-active");
            status_badge_clone2.set_text("영어 입력 중");
            unim_log!("INDICATOR", "영어 모드로 전환");
        }
    });

    settings_btn.connect_clicked(|_| {
        open_settings();
    });

    // 창 닫기 관련 제어
    let window_clone = window.clone();
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            window_clone.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    let window_focus_clone = window.clone();
    window.connect_is_active_notify(move |w| {
        if !w.is_active() {
            window_focus_clone.set_visible(false);
        }
    });

    window
}

// ============================================================================
// 설정 도구
// ============================================================================

/// `unim-gui --settings` 모드: 설정 다이얼로그만 표시하고 종료
fn run_settings_only() {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.settings")
        .build();

    app.connect_activate(|app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        settings_dialog::show_settings_dialog(app);
    });

    app.run_with_args::<String>(&[]);
}

/// 내장 설정 다이얼로그를 GTK 이벤트 루프에 GuiAction으로 요청
fn open_settings() {
    if let Ok(tx) = SETTINGS_TX.lock() {
        if let Some(tx) = tx.as_ref() {
            let _ = tx.send(GuiAction::OpenSettings);
        }
    }
}

/// 설정 오픈 요청용 전역 Sender
static SETTINGS_TX: std::sync::Mutex<Option<Sender<GuiAction>>> = std::sync::Mutex::new(None);
