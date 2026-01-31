//! UNIM 상태 표시 인디케이터
//!
//! 시스템 트레이에 입력 모드를 표시하고,
//! DBus 시그널을 구독하여 아이콘을 업데이트합니다.
//! 현대적인 GTK4/libadwaita 기반 팝업 윈도우를 제공합니다.

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
use log::{debug, error, info};

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

/// 팝업 액션
#[derive(Debug, Clone)]
enum PopupAction {
    Show,
    UpdateCategory(InputCategory),
}

/// ksni 트레이 구현
#[derive(Debug)]
struct UnimTray {
    state: Arc<RwLock<IndicatorState>>,
    popup_tx: Sender<PopupAction>,
}

impl ksni::Tray for UnimTray {
    fn id(&self) -> String {
        "unim-indicator".into()
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
        let _ = self.popup_tx.send(PopupAction::Show);
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
                                .send(PopupAction::UpdateCategory(InputCategory::Korean));
                            info!("한국어 모드로 전환");
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
                                .send(PopupAction::UpdateCategory(InputCategory::English));
                            info!("영어 모드로 전환");
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
                    info!("인디케이터 종료");
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
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("UNIM 인디케이터 시작...");

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<PopupAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

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
                error!("tokio 런타임 생성 실패: {}", e);
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
                info!("시스템 트레이 시작됨");
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
                error!("시스템 트레이 시작 실패: {}", e);
            }
        }
    });

    // GTK4/libadwaita 앱 시작 (메인 스레드)
    run_gtk_app(state, popup_rx);
}

// ============================================================================
// DBus 시그널 감시 (inotify 대체)
// ============================================================================

/// DBus GlobalModeChanged 시그널 구독하여 트레이 업데이트 (비동기)
/// handle을 직접 사용하지 않고 채널로 업데이트 요청
async fn watch_dbus_signals(
    state: Arc<RwLock<IndicatorState>>,
    tray_update_tx: std::sync::mpsc::Sender<()>,
    popup_tx: Sender<PopupAction>,
) {
    // DBus 연결
    let connection = match zbus::Connection::session().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("DBus 세션 연결 실패: {}", e);
            return;
        }
    };

    // InputMethod 프록시 생성
    let proxy = match InputMethodProxy::new(&connection).await {
        Ok(p) => p,
        Err(e) => {
            error!("InputMethod 프록시 생성 실패: {}", e);
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
            // 채널로 업데이트 요청 (트레이 스레드에서 처리)
            let _ = tray_update_tx.send(());
            info!("[DBus] 초기 모드 조회: {:?}", category);
        }
        Err(e) => {
            debug!("초기 모드 조회 실패 (아직 데몬 미시작?): {}", e);
        }
    }

    // GlobalModeChanged 시그널 구독
    info!("[DBus] GlobalModeChanged 시그널 구독 시작...");

    let mut stream = match proxy.receive_global_mode_changed().await {
        Ok(s) => s,
        Err(e) => {
            error!("시그널 구독 실패: {}", e);
            return;
        }
    };

    use futures_util::StreamExt;

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
                    debug!("[DBus] 모드 변경 감지: {:?}", category);
                    // 채널로 업데이트 요청 (트레이 스레드에서 처리)
                    let _ = tray_update_tx.send(());
                    let _ = popup_tx.send(PopupAction::UpdateCategory(category));
                }
            }
            Err(e) => {
                error!("시그널 인자 파싱 오류: {}", e);
            }
        }
    }
}

// ============================================================================
// GTK4 UI
// ============================================================================

/// GTK4/libadwaita 앱 실행
fn run_gtk_app(state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<PopupAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.indicator")
        .build();

    app.connect_activate(move |app| {
        // 시스템 팔레트와 무관하게 다크 모드 강제 적용
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        load_css();
        let window = build_popup_window(app, state.clone());
        let window_clone = window.clone();
        let popup_rx_clone = popup_rx.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if let Ok(rx) = popup_rx_clone.lock() {
                while let Ok(action) = rx.try_recv() {
                    match action {
                        PopupAction::Show => {
                            window_clone.present();
                        }
                        PopupAction::UpdateCategory(_category) => {
                            // UI 업데이트는 창이 표시될 때 자동으로 처리됨
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
            info!("한국어 모드로 전환");
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
            info!("영어 모드로 전환");
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

fn open_settings() {
    let settings_cmd = detect_settings_command();
    info!("설정 도구 실행: {}", settings_cmd);

    let success = std::process::Command::new(&settings_cmd).spawn().is_ok();

    if !success {
        error!("설정 실행 실패 ({}). fallback 시도...", settings_cmd);
        let fallback = if settings_cmd == "unim-gtk-settings" {
            "unim-qt-settings"
        } else {
            "unim-gtk-settings"
        };

        if std::process::Command::new(fallback).spawn().is_err() {
            error!("대체 설정 도구도 실패 ({}). 터미널 모드 시도...", fallback);
            run_interactive_config_in_terminal();
        }
    }
}

fn detect_settings_command() -> &'static str {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.contains("kde")
            || desktop_lower.contains("plasma")
            || desktop_lower.contains("lxqt")
        {
            return "unim-qt-settings";
        }
    }
    "unim-gtk-settings"
}

fn run_interactive_config_in_terminal() {
    let cmd = "unim-config interactive";

    let terminals = vec![
        ("gnome-terminal", vec!["--", "sh", "-c", cmd]),
        ("konsole", vec!["-e", "sh", "-c", cmd]),
        ("xfce4-terminal", vec!["-e", cmd]),
        ("mate-terminal", vec!["-e", cmd]),
        ("lxterminal", vec!["-e", cmd]),
        ("alacritty", vec!["-e", "sh", "-c", cmd]),
        ("kitty", vec!["sh", "-c", cmd]),
        ("xterm", vec!["-e", "sh", "-c", cmd]),
    ];

    for (bin, args) in terminals {
        if std::process::Command::new(bin).args(&args).spawn().is_ok() {
            info!("터미널 실행 성공: {}", bin);
            return;
        }
    }

    error!("적절한 터미널 에뮬레이터를 찾을 수 없습니다.");
}
