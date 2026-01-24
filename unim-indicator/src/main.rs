//! UNIM 상태 표시 인디케이터
//!
//! 시스템 트레이에 입력 모드를 표시하고,
//! 상태 파일 변경을 감시하여 아이콘을 업데이트합니다.
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
use inotify::{Inotify, WatchMask};
use ksni::blocking::TrayMethods;
use ksni::menu::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use log::{debug, error, info, warn};

use unim::status::{get_status, set_status, status_file_path, InputCategory};

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

    /// 데몬 재시작
    fn restart(&mut self) -> Result<(), String> {
        self.stop();
        thread::sleep(Duration::from_millis(500));
        self.start()
    }

    /// 데몬 실행 상태 확인
    fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,  // 아직 실행 중
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
            category: InputCategory::Latin,
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
            .unwrap_or(InputCategory::Latin);
        match category {
            InputCategory::Hangul => "unim-hangul".into(),
            InputCategory::Latin => "unim-latin".into(),
        }
    }

    fn title(&self) -> String {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::Latin);
        match category {
            InputCategory::Hangul => "UNIM - 한글".into(),
            InputCategory::Latin => "UNIM - 영문".into(),
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::Latin);
        
        let mode_desc = match category {
            InputCategory::Hangul => "한글 모드",
            InputCategory::Latin => "영문 모드",
        };
        
        ksni::ToolTip {
            title: "UNIM 입력기".into(),
            description: mode_desc.into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // 트레이 아이콘 클릭 시 팝업 표시
        let _ = self.popup_tx.send(PopupAction::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            // 모드 전환 (현재 모드에 체크 표시)
            {
                let current_category = self
                    .state
                    .read()
                    .map(|s| s.category)
                    .unwrap_or(InputCategory::Latin);
                let hangul_label = if current_category == InputCategory::Hangul {
                    "✓ 한글 모드 (Hangul)"
                } else {
                    "   한글 모드 (Hangul)"
                };
                StandardItem {
                    label: hangul_label.into(),
                    activate: Box::new(|this: &mut Self| {
                        if let Ok(mut s) = this.state.write() {
                            s.category = InputCategory::Hangul;
                            let _ = set_status(InputCategory::Hangul);
                            let _ = this
                                .popup_tx
                                .send(PopupAction::UpdateCategory(InputCategory::Hangul));
                            info!("한글 모드로 전환");
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
                    .unwrap_or(InputCategory::Latin);
                let latin_label = if current_category == InputCategory::Latin {
                    "✓ 영문 모드 (Latin)"
                } else {
                    "   영문 모드 (Latin)"
                };
                StandardItem {
                    label: latin_label.into(),
                    activate: Box::new(|this: &mut Self| {
                        if let Ok(mut s) = this.state.write() {
                            s.category = InputCategory::Latin;
                            let _ = set_status(InputCategory::Latin);
                            let _ = this
                                .popup_tx
                                .send(PopupAction::UpdateCategory(InputCategory::Latin));
                            info!("영문 모드로 전환");
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

    // 상태 초기화 (파일에서 읽기)
    let initial_category = get_status().unwrap_or(InputCategory::Latin);
    let state = Arc::new(RwLock::new(IndicatorState {
        category: initial_category,
    }));

    // 채널들
    let (popup_tx, popup_rx) = mpsc::channel::<PopupAction>();
    let popup_rx = Arc::new(Mutex::new(popup_rx));

    // 데몬 모니터링 스레드
    let daemon_manager_ctrl = daemon_manager.clone();
    thread::spawn(move || {
        loop {
            // 상태 모니터링 및 자동 재시작
            if let Ok(mut mgr) = daemon_manager_ctrl.lock() {
                mgr.monitor_and_restart();
            }
            thread::sleep(Duration::from_secs(2));
        }
    });

    // ksni 트레이 시작 (별도 스레드)
    let tray_state = state.clone();
    let watcher_state = state.clone();
    let watcher_popup_tx = popup_tx.clone();

    thread::spawn(move || {
        let tray = UnimTray {
            state: tray_state,
            popup_tx: popup_tx.clone(),
        };
        match tray.spawn() {
            Ok(handle) => {
                info!("시스템 트레이 시작됨");
                // 상태 파일 감시 및 트레이 업데이트
                watch_status_file_for_tray(watcher_state, handle, watcher_popup_tx);
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
// GTK4 UI
// ============================================================================

/// GTK4/libadwaita 앱 실행
fn run_gtk_app(state: Arc<RwLock<IndicatorState>>, popup_rx: Arc<Mutex<Receiver<PopupAction>>>) {
    let app = adw::Application::builder()
        .application_id("io.github.from104.unim.indicator")
        .build();

    app.connect_activate(move |app| {
        // CSS 로드
        load_css();

        // 팝업 윈도우 생성
        let window = build_popup_window(app, state.clone());

        // 팝업 액션 수신
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

    // 헤더바
    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("UNIM 입력기", "입력 모드 전환"))
        .build();

    // 현재 상태 읽기
    let current_category = state
        .read()
        .map(|s| s.category)
        .unwrap_or(InputCategory::Latin);

    // 모드 버튼들
    let hangul_btn = gtk4::Button::builder()
        .tooltip_text("한글 모드로 전환")
        .build();
    hangul_btn.add_css_class("mode-button");

    let latin_btn = gtk4::Button::builder()
        .tooltip_text("영문 모드로 전환")
        .build();
    latin_btn.add_css_class("mode-button");

    // 초기 활성 상태 설정
    match current_category {
        InputCategory::Hangul => {
            hangul_btn.set_label("한");
            hangul_btn.add_css_class("mode-active");
            latin_btn.set_label("A");
        }
        InputCategory::Latin => {
            hangul_btn.set_label("한");
            latin_btn.set_label("A");
            latin_btn.add_css_class("mode-active");
        }
    }

    // 버튼 박스
    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 24);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(20);
    button_box.set_margin_bottom(16);
    button_box.append(&hangul_btn);
    button_box.append(&latin_btn);

    // 상태 레이블
    let status_label = gtk4::Label::new(Some(match current_category {
        InputCategory::Hangul => "현재: 한글 모드",
        InputCategory::Latin => "현재: 영문 모드",
    }));
    status_label.add_css_class("status-label");
    status_label.set_margin_bottom(16);

    // 버튼 클릭 핸들러
    let hangul_btn_clone = hangul_btn.clone();
    let latin_btn_clone = latin_btn.clone();
    let status_label_clone = status_label.clone();
    let state_clone = state.clone();
    hangul_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone.write() {
            s.category = InputCategory::Hangul;
            let _ = set_status(InputCategory::Hangul);
        }
        hangul_btn_clone.add_css_class("mode-active");
        latin_btn_clone.remove_css_class("mode-active");
        status_label_clone.set_text("현재: 한글 모드");
        info!("한글 모드로 전환");
    });

    let hangul_btn_clone2 = hangul_btn.clone();
    let latin_btn_clone2 = latin_btn.clone();
    let status_label_clone2 = status_label.clone();
    let state_clone2 = state.clone();
    latin_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_clone2.write() {
            s.category = InputCategory::Latin;
            let _ = set_status(InputCategory::Latin);
        }
        latin_btn_clone2.add_css_class("mode-active");
        hangul_btn_clone2.remove_css_class("mode-active");
        status_label_clone2.set_text("현재: 영문 모드");
        info!("영문 모드로 전환");
    });

    // 설정 버튼
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

    // 메인 컨테이너
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&header);
    content.append(&button_box);
    content.append(&status_label);
    content.append(&settings_btn);

    window.set_content(Some(&content));

    // ESC로 닫기
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

    // 포커스 잃으면 숨기기
    let window_clone2 = window.clone();
    window.connect_is_active_notify(move |w| {
        if !w.is_active() {
            window_clone2.set_visible(false);
        }
    });

    window
}

// ============================================================================
// 상태 파일 감시
// ============================================================================

/// 상태 파일 감시 및 트레이 업데이트
fn watch_status_file_for_tray(
    state: Arc<RwLock<IndicatorState>>,
    handle: ksni::blocking::Handle<UnimTray>,
    popup_tx: Sender<PopupAction>,
) {
    let status_path = status_file_path();
    let watch_dir = status_path.parent().expect("상태 파일 디렉토리 없음");

    // 디렉토리가 없으면 생성
    if !watch_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(watch_dir) {
            error!("디렉토리 생성 실패: {}", e);
            return;
        }
    }

    // inotify 설정
    let mut inotify = match Inotify::init() {
        Ok(i) => i,
        Err(e) => {
            error!("inotify 초기화 실패: {}", e);
            return;
        }
    };

    if let Err(e) = inotify.watches().add(
        watch_dir,
        WatchMask::MODIFY | WatchMask::CREATE | WatchMask::MOVED_TO,
    ) {
        error!("inotify 감시 추가 실패: {}", e);
        return;
    }

    info!("상태 파일 감시 시작: {:?}", watch_dir);

    let mut buffer = [0; 1024];
    loop {
        match inotify.read_events_blocking(&mut buffer) {
            Ok(events) => {
                for event in events {
                    if let Some(name) = event.name {
                        if name == "status" {
                            // 상태 파일이 변경됨
                            if let Ok(category) = get_status() {
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
                                    debug!("상태 변경 감지: {:?}", category);
                                    // 트레이 업데이트 트리거
                                    handle.update(|_| {});
                                    // 팝업 UI 업데이트
                                    let _ = popup_tx.send(PopupAction::UpdateCategory(category));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("inotify 이벤트 읽기 오류: {}", e);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

// ============================================================================
// 설정 도구
// ============================================================================

/// DE 환경에 따라 적절한 설정 도구를 실행합니다.
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
            error!(
                "대체 설정 도구도 실패 ({}). 터미널 모드 시도...",
                fallback
            );
            run_interactive_config_in_terminal();
        }
    }
}

/// DE 환경에 따라 적절한 설정 도구 명령어를 반환합니다.
fn detect_settings_command() -> &'static str {
    // XDG_CURRENT_DESKTOP 환경 변수로 DE 감지
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();

        // KDE/Plasma, LXQt 등 Qt 기반 DE
        if desktop_lower.contains("kde")
            || desktop_lower.contains("plasma")
            || desktop_lower.contains("lxqt")
        {
            return "unim-qt-settings";
        }
    }

    // 기본값: GTK 설정 도구 (GNOME, XFCE, MATE, Cinnamon, Budgie 등)
    "unim-gtk-settings"
}

/// 터미널 에뮬레이터에서 unim-config interactive를 실행합니다.
fn run_interactive_config_in_terminal() {
    let cmd = "unim-config interactive";

    // 지원하는 터미널 에뮬레이터 목록 (실행 인자 포함)
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
