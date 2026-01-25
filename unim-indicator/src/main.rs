//! UNIM 상태 표시 인디케이터
//!
//! 시스템 트레이에 입력 모드를 표시하고,
//! DBus 시그널을 구독하여 아이콘을 업데이트합니다.
//! UNIM 데몬을 자식 프로세스로 관리합니다.
//! 현대적인 GTK4/libadwaita 기반 팝업 윈도우를 제공합니다.

use std::path::PathBuf;
use std::process::{Child, Command};
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
use log::{debug, error, info, warn};

use unim::status::InputCategory;
use unim_dbus::client::InputMethodProxy;

// ============================================================================
// 데몬 관리자
// ============================================================================

/// UNIM 데몬 관리자
struct DaemonManager {
    child: Option<Child>,
    binary_path: PathBuf,
}

impl DaemonManager {
    /// 새 DaemonManager 생성
    fn new() -> Self {
        let binary_path = Self::find_daemon_binary();
        Self {
            child: None,
            binary_path,
        }
    }

    /// 데몬 바이너리 경로 탐색
    fn find_daemon_binary() -> PathBuf {
        // 1. 환경 변수
        if let Ok(path) = std::env::var("UNIM_DAEMON_PATH") {
            let p = PathBuf::from(&path);
            if p.exists() {
                info!("데몬 바이너리 (환경변수): {:?}", p);
                return p;
            }
        }

        // 2. 시스템 경로
        let system_paths = [
            PathBuf::from("/usr/libexec/unim-daemon"),
            PathBuf::from("/usr/bin/unim-daemon"),
        ];
        for p in system_paths {
            if p.exists() {
                info!("데몬 바이너리 (시스템): {:?}", p);
                return p;
            }
        }

        // 3. 현재 실행 파일과 동일 디렉토리
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let sibling_path = exe_dir.join("unim-daemon");
                if sibling_path.exists() {
                    info!("데몬 바이너리 (동일 디렉토리): {:?}", sibling_path);
                    return sibling_path;
                }
            }
        }

        // 4. 빌드 디렉토리 (개발용)
        let dev_paths = [
            PathBuf::from("/home/from104/work/unim/target/release/unim-daemon"),
            PathBuf::from("/home/from104/work/unim/target/debug/unim-daemon"),
        ];
        for p in dev_paths {
            if p.exists() {
                info!("데몬 바이너리 (개발): {:?}", p);
                return p;
            }
        }

        // 기본값 (PATH에서 찾기)
        warn!("데몬 바이너리를 찾을 수 없음. PATH에서 탐색 예정.");
        PathBuf::from("unim-daemon")
    }

    /// 데몬 시작
    fn start(&mut self) -> Result<(), String> {
        if self.is_running() {
            info!("데몬이 이미 실행 중");
            return Ok(());
        }

        info!("데몬 시작: {:?}", self.binary_path);

        match Command::new(&self.binary_path).arg("-n").spawn() {
            Ok(child) => {
                info!("데몬 시작됨 (PID: {})", child.id());
                self.child = Some(child);
                Ok(())
            }
            Err(e) => {
                error!("데몬 시작 실패: {}", e);
                Err(format!("데몬 시작 실패: {}", e))
            }
        }
    }

    /// 데몬 중지
    fn stop(&mut self) {
        if let Some(ref mut child) = self.child {
            info!("데몬 중지 (PID: {})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
    }

    /// 데몬 실행 상태 확인
    fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(status)) => {
                    info!("데몬 종료됨: {:?}", status);
                    false
                }
                Err(e) => {
                    error!("데몬 상태 확인 오류: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// 주기적으로 상태 확인 및 자동 재시작
    fn monitor_and_restart(&mut self) {
        if !self.is_running() && self.child.is_some() {
            warn!("데몬이 예기치 않게 종료됨. 재시작 시도...");
            self.child = None;
            if let Err(e) = self.start() {
                error!("데몬 자동 재시작 실패: {}", e);
            }
        }
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        self.stop();
    }
}

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

    // 데몬 관리자 생성 및 시작
    let daemon_manager = Arc::new(Mutex::new(DaemonManager::new()));
    {
        let mut mgr = daemon_manager.lock().unwrap();
        if let Err(e) = mgr.start() {
            error!("데몬 초기 시작 실패: {}", e);
        }
    }

    // kDaemon이 DBus 서비스를 시작할 때까지 잠시 대기
    thread::sleep(Duration::from_millis(500));

    // 상태 초기화
    let state = Arc::new(RwLock::new(IndicatorState::default()));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<PopupAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // 데몬 모니터링 스레드
    let daemon_manager_ctrl = daemon_manager.clone();
    thread::spawn(move || loop {
        if let Ok(mut mgr) = daemon_manager_ctrl.lock() {
            mgr.monitor_and_restart();
        }
        thread::sleep(Duration::from_secs(2));
    });

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
        .popup-window {
            background: alpha(@window_bg_color, 0.95);
        }
        
        .mode-button {
            min-width: 80px;
            min-height: 80px;
            font-size: 32px;
            font-weight: bold;
            border-radius: 16px;
            transition: all 200ms ease;
        }
        
        .mode-button:hover {
            background: alpha(@accent_bg_color, 0.3);
        }
        
        .mode-active {
            background: @accent_bg_color;
            color: @accent_fg_color;
        }
        
        .mode-active:hover {
            background: shade(@accent_bg_color, 1.1);
        }
        
        .title-label {
            font-size: 18px;
            font-weight: bold;
            margin-bottom: 8px;
        }
        
        .status-label {
            font-size: 14px;
            color: alpha(@window_fg_color, 0.7);
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
        .title("UNIM")
        .default_width(300)
        .default_height(240)
        .resizable(false)
        .deletable(true)
        .build();

    window.add_css_class("popup-window");

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("UNIM 입력기", "입력 모드 전환"))
        .build();

    let current_category = state
        .read()
        .map(|s| s.category)
        .unwrap_or(InputCategory::English);

    let korean_btn = gtk4::Button::builder()
        .tooltip_text("한국어 모드로 전환")
        .build();
    korean_btn.add_css_class("mode-button");

    let english_btn = gtk4::Button::builder()
        .tooltip_text("영어 모드로 전환")
        .build();
    english_btn.add_css_class("mode-button");

    match current_category {
        InputCategory::Korean => {
            korean_btn.set_label("한");
            korean_btn.add_css_class("mode-active");
            english_btn.set_label("A");
        }
        InputCategory::English => {
            korean_btn.set_label("한");
            english_btn.set_label("A");
            english_btn.add_css_class("mode-active");
        }
    }

    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 24);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(20);
    button_box.set_margin_bottom(16);
    button_box.append(&korean_btn);
    button_box.append(&english_btn);

    let status_label = gtk4::Label::new(Some(match current_category {
        InputCategory::Korean => "현재: 한국어 모드",
        InputCategory::English => "현재: 영어 모드",
    }));
    status_label.add_css_class("status-label");
    status_label.set_margin_bottom(16);

    let korean_btn_clone = korean_btn.clone();
    let english_btn_clone = english_btn.clone();
    let status_label_clone = status_label.clone();
    let state_clone = state.clone();
    korean_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone.write() {
            s.category = InputCategory::Korean;
        }
        korean_btn_clone.add_css_class("mode-active");
        english_btn_clone.remove_css_class("mode-active");
        status_label_clone.set_text("현재: 한국어 모드");
        info!("한국어 모드로 전환");
    });

    let korean_btn_clone2 = korean_btn.clone();
    let english_btn_clone2 = english_btn.clone();
    let status_label_clone2 = status_label.clone();
    let state_clone2 = state.clone();
    english_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone2.write() {
            s.category = InputCategory::English;
        }
        english_btn_clone2.add_css_class("mode-active");
        korean_btn_clone2.remove_css_class("mode-active");
        status_label_clone2.set_text("현재: 영어 모드");
        info!("영어 모드로 전환");
    });

    let settings_btn = gtk4::Button::builder()
        .label("설정...")
        .margin_start(40)
        .margin_end(40)
        .margin_bottom(8)
        .build();
    settings_btn.add_css_class("pill");
    settings_btn.connect_clicked(|_| {
        open_settings();
    });

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&header);
    content.append(&button_box);
    content.append(&status_label);
    content.append(&settings_btn);

    window.set_content(Some(&content));

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

    let window_clone2 = window.clone();
    window.connect_is_active_notify(move |w| {
        if !w.is_active() {
            window_clone2.set_visible(false);
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
