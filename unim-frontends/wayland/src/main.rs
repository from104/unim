//! UNIM Wayland 입력 방식 프론트엔드
//!
//! Wayland 환경에서 한글 입력을 제공합니다.
//! input-method-v2 프로토콜을 사용합니다.

mod im_handler;

use log::{error, info};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_seat::WlSeat,
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::ZwpInputMethodV2,
};

use im_handler::InputMethodHandler;

/// Wayland 애플리케이션 상태
struct AppData {
    config: unim::config::Config,
    seat: Option<WlSeat>,
    im_manager: Option<ZwpInputMethodManagerV2>,
    input_method: Option<ZwpInputMethodV2>,
    handler: Option<InputMethodHandler>,
}

impl AppData {
    fn new(config: unim::config::Config) -> Self {
        Self {
            config,
            seat: None,
            im_manager: None,
            input_method: None,
            handler: None,
        }
    }
}

fn main() {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("UNIM Wayland 입력 방식 시작...");

    // 설정 로드
    let config = unim::config::Config::load_from_default_path();
    info!("설정 로드 완료");

    // Wayland 연결
    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(err) => {
            error!("Wayland 연결 실패: {}", err);
            std::process::exit(1);
        }
    };

    info!("Wayland 연결 성공");

    // 이벤트 큐 및 레지스트리 초기화
    let (globals, mut event_queue) = match registry_queue_init::<AppData>(&conn) {
        Ok(result) => result,
        Err(err) => {
            error!("레지스트리 초기화 실패: {}", err);
            std::process::exit(1);
        }
    };

    let qh = event_queue.handle();
    let mut app_data = AppData::new(config);

    // wl_seat 바인딩
    let seat: WlSeat = match globals.bind(&qh, 1..=9, ()) {
        Ok(seat) => seat,
        Err(err) => {
            error!("wl_seat 바인딩 실패: {}", err);
            std::process::exit(1);
        }
    };
    app_data.seat = Some(seat.clone());
    info!("wl_seat 바인딩 성공");

    // input_method_manager_v2 바인딩
    let im_manager: ZwpInputMethodManagerV2 = match globals.bind(&qh, 1..=1, ()) {
        Ok(manager) => manager,
        Err(err) => {
            error!("zwp_input_method_manager_v2 바인딩 실패: {}", err);
            error!("컴포지터가 input-method-v2 프로토콜을 지원하지 않습니다.");
            std::process::exit(1);
        }
    };
    app_data.im_manager = Some(im_manager.clone());
    info!("zwp_input_method_manager_v2 바인딩 성공");

    // input_method 생성
    let input_method = im_manager.get_input_method(&seat, &qh, ());
    app_data.input_method = Some(input_method);
    info!("zwp_input_method_v2 생성 완료");

    // 핸들러 초기화
    app_data.handler = Some(InputMethodHandler::new(&app_data.config));

    info!("이벤트 루프 시작...");

    // 이벤트 루프
    loop {
        match event_queue.blocking_dispatch(&mut app_data) {
            Ok(_) => {}
            Err(err) => {
                error!("이벤트 디스패치 오류: {}", err);
                break;
            }
        }
    }

    info!("UNIM Wayland 입력 방식 종료");
}

// Dispatch implementations

impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, GlobalListContents> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_registry::WlRegistry,
        _event: wayland_client::protocol::wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // 레지스트리 이벤트 처리 (registry_queue_init이 처리함)
    }
}

impl Dispatch<WlSeat, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: wayland_client::protocol::wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Seat 이벤트 처리 (필요시 확장)
    }
}

impl Dispatch<ZwpInputMethodManagerV2, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpInputMethodManagerV2,
        _event: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Manager는 이벤트 없음
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &ZwpInputMethodV2,
        event: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::Event;

        if let Some(handler) = state.handler.as_mut() {
            match event {
                Event::Activate => {
                    log::debug!("입력 방식 활성화");
                    handler.activate();
                }
                Event::Deactivate => {
                    log::debug!("입력 방식 비활성화");
                    handler.deactivate(proxy);
                }
                Event::SurroundingText { text, cursor, anchor } => {
                    log::trace!("주변 텍스트: cursor={}, anchor={}", cursor, anchor);
                    handler.set_surrounding_text(&text, cursor, anchor);
                }
                Event::TextChangeCause { cause } => {
                    log::trace!("텍스트 변경 원인: {:?}", cause);
                }
                Event::ContentType { hint, purpose } => {
                    log::trace!("컨텐츠 타입: hint={:?}, purpose={:?}", hint, purpose);
                }
                Event::Done => {
                    log::trace!("Done 이벤트");
                    handler.done(proxy, &mut state.config);
                }
                Event::Unavailable => {
                    log::warn!("입력 방식 사용 불가");
                }
                _ => {}
            }
        }
    }
}
