//! UNIM 상태 표시 인디케이터
//!
//! 시스템 트레이에 입력 모드를 표시하고,
//! 상태 파일 변경을 감시하여 아이콘을 업데이트합니다.

use std::sync::{Arc, RwLock};
use std::thread;

use gtk::glib;
use gtk::prelude::*;
use inotify::{Inotify, WatchMask};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::{debug, error, info};

use unim::status::{get_status, set_status, status_file_path, InputCategory};

/// 인디케이터 상태
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

fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("UNIM 인디케이터 시작...");

    // GTK 초기화
    gtk::init().expect("GTK 초기화 실패");

    // 상태 초기화 (파일에서 읽기)
    let initial_category = get_status().unwrap_or(InputCategory::Latin);
    let state = Arc::new(RwLock::new(IndicatorState {
        category: initial_category,
    }));

    // AppIndicator 생성
    let mut indicator = AppIndicator::new("unim-indicator", "");
    indicator.set_status(AppIndicatorStatus::Active);
    indicator.set_title("UNIM 입력기");
    
    // 아이콘 테마 경로 설정 (시스템 경로 우선)
    indicator.set_icon_theme_path("/usr/share/icons/hicolor/scalable/apps");
    
    update_indicator_icon(&mut indicator, initial_category);

    // 메뉴 생성
    let menu = create_menu(state.clone());
    indicator.set_menu(&mut menu.clone());

    // 상태 파일 감시 스레드
    let state_watcher = state.clone();
    let (tx, rx) = glib::MainContext::channel::<InputCategory>(glib::Priority::DEFAULT);

    thread::spawn(move || {
        watch_status_file(tx, state_watcher);
    });

    // 메인 컨텍스트에서 상태 변경 처리
    let indicator = Arc::new(RwLock::new(indicator));
    let indicator_update = indicator.clone();

    rx.attach(None, move |category| {
        if let Ok(mut ind) = indicator_update.write() {
            update_indicator_icon(&mut ind, category);
        }
        glib::ControlFlow::Continue
    });

    info!("GTK 메인 루프 시작");
    gtk::main();
}

/// 인디케이터 아이콘 업데이트
fn update_indicator_icon(indicator: &mut AppIndicator, category: InputCategory) {
    let icon_name = match category {
        InputCategory::Hangul => "unim-hangul",
        InputCategory::Latin => "unim-latin",
    };
    
    // 아이콘이 없으면 텍스트로 대체
    let label = match category {
        InputCategory::Hangul => "한",
        InputCategory::Latin => "A",
    };
    
    indicator.set_icon(icon_name);
    indicator.set_label(label, "");
    debug!("아이콘 업데이트: {}", label);
}

/// 메뉴 생성
fn create_menu(state: Arc<RwLock<IndicatorState>>) -> gtk::Menu {
    let menu = gtk::Menu::new();

    // 한글 모드
    let hangul_item = gtk::MenuItem::with_label("한글 (Hangul)");
    let state_hangul = state.clone();
    hangul_item.connect_activate(move |_| {
        if let Ok(mut s) = state_hangul.write() {
            s.category = InputCategory::Hangul;
            let _ = set_status(InputCategory::Hangul);
            info!("한글 모드로 전환");
        }
    });
    menu.append(&hangul_item);

    // 영문 모드
    let latin_item = gtk::MenuItem::with_label("영문 (Latin)");
    let state_latin = state.clone();
    latin_item.connect_activate(move |_| {
        if let Ok(mut s) = state_latin.write() {
            s.category = InputCategory::Latin;
            let _ = set_status(InputCategory::Latin);
            info!("영문 모드로 전환");
        }
    });
    menu.append(&latin_item);

    // 구분선
    let separator = gtk::SeparatorMenuItem::new();
    menu.append(&separator);

    // 설정
    let settings_item = gtk::MenuItem::with_label("설정 (Settings)...");
    settings_item.connect_activate(|_| {
        // DE에 따라 적절한 설정 도구 선택
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
    });
    menu.append(&settings_item);

    // 구분선
    let separator2 = gtk::SeparatorMenuItem::new();
    menu.append(&separator2);

    // 종료
    let quit_item = gtk::MenuItem::with_label("종료 (Quit)");
    quit_item.connect_activate(|_| {
        info!("인디케이터 종료");
        gtk::main_quit();
    });
    menu.append(&quit_item);

    menu.show_all();
    menu
}

/// 상태 파일 감시
fn watch_status_file(tx: glib::Sender<InputCategory>, state: Arc<RwLock<IndicatorState>>) {
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
                                    let _ = tx.send(category);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("inotify 이벤트 읽기 오류: {}", e);
                thread::sleep(std::time::Duration::from_secs(1));
            }
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
