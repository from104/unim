use calloop::EventLoop;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        Capability, SeatHandler, SeatState,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};
use unim::unim_log;
use wayland_client::{
    globals::GlobalList,
    protocol::{wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ZwpTextInputV3},
};

struct AppData {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,

    text_input_manager: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,

    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    width: u32,
    height: u32,
    surface: wl_surface::WlSurface,

    pool: SlotPool,
    font_system: FontSystem,
    swash_cache: SwashCache,
    buffer: Buffer,

    text: String,
    preedit: String,
    #[allow(dead_code)]
    cursor_begin: i32,
    #[allow(dead_code)]
    cursor_end: i32,

    should_quit: bool,
}

impl AppData {
    fn new(conn: &Connection, qh: &QueueHandle<Self>) -> Self {
        let registry_state = RegistryState::new(&conn.globals());
        let seat_state = SeatState::new(&conn.globals(), qh);
        let output_state = OutputState::new(&conn.globals(), qh);
        let compositor_state = CompositorState::new(&conn.globals(), qh);
        let shm = Shm::bind(&conn.globals(), qh).expect("shm not available");

        let surface = compositor_state.create_surface(qh);

        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(24.0, 30.0);
        let buffer = Buffer::new(&mut font_system, metrics);
        let swash_cache = SwashCache::new();

        let pool = SlotPool::new(1024 * 1024, &shm).expect("Failed to create pool");

        Self {
            registry_state,
            seat_state,
            output_state,
            compositor_state,
            shm,
            text_input_manager: None,
            text_input: None,
            seat: None,
            keyboard: None,
            width: 800,
            height: 600,
            surface,
            pool,
            font_system,
            swash_cache,
            buffer,
            text: String::new(),
            preedit: String::new(),
            cursor_begin: 0,
            cursor_end: 0,
            should_quit: false,
        }
    }

    fn init_text_input(&mut self, qh: &QueueHandle<Self>) {
        if self.text_input.is_some() {
            return;
        }

        if let Some(manager) = &self.text_input_manager {
            if let Some(seat) = &self.seat {
                let text_input = manager.get_text_input(seat, qh, ());
                unim_log!("TEST_WAYLAND", "Text Input v3 생성됨");
                text_input.enable();
                text_input.commit();
                self.text_input = Some(text_input);
            }
        }
    }

    fn draw(&mut self) {
        let width = self.width;
        let height = self.height;
        let stride = width * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
            )
            .expect("create buffer");

        let mut pixmap = Pixmap::new_mut(canvas, width, height).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(30, 30, 46, 255));

        let mut display_text = self.text.clone();
        let preedit_start_idx = display_text.len(); // UTF-8 바이트 인덱스는 복잡할 수 있음, 여기서는 단순 문자 단위
        display_text.push_str(&self.preedit);

        // 텍스트 레이아웃 설정
        self.buffer.set_text(
            &mut self.font_system,
            &display_text,
            Attrs::new()
                .family(Family::SansSerif)
                .color(Color::rgb(205, 214, 244)),
            Shaping::Advanced,
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);

        // 텍스트 그리기
        self.buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgb(205, 214, 244),
            |x, y, w, h, color| {
                let a = color.a();
                if a == 0 {
                    return;
                }

                // 패딩 20px
                let rect = Rect::from_xywh(x as f32 + 20.0, y as f32 + 20.0, w as f32, h as f32);
                if let Some(r) = rect {
                    let paint = Paint {
                        shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
                            color.r(),
                            color.g(),
                            color.b(),
                            a,
                        )),
                        ..Default::default()
                    };
                    let mut pb = PathBuilder::new();
                    pb.push_rect(r);
                    if let Some(path) = pb.finish() {
                        pixmap.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            },
        );

        // Preedit 밑줄 (간이 구현)
        if !self.preedit.is_empty() {
            // 실제 구현 시에는 LayoutRun을 순회하며 정확한 좌표를 얻어야 함
            // 여기서는 테스트 목적으로 단순하게 그림
            let paint = Paint {
                shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
                    243, 139, 168, 255,
                )),
                ..Default::default()
            };

            // 임시 좌표: 텍스트 길이 비례
            let text_width_approx = (self.text.len() * 10) as f32;
            let preedit_width_approx = (self.preedit.len() * 15) as f32;

            // 실제로는 layout run을 통해 좌표를 얻어야 함
            let rect = Rect::from_xywh(
                20.0 + text_width_approx,
                50.0,
                preedit_width_approx.max(10.0),
                2.0,
            )
            .unwrap();

            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }

        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage(0, 0, width as i32, height as i32);
        self.surface.commit();
    }
}

// --- Delegate Implementations ---

delegate_compositor!(AppData);
delegate_output!(AppData);
delegate_shm!(AppData);
delegate_seat!(AppData);
delegate_keyboard!(AppData);
delegate_registry!(AppData);

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw();
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for AppData {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.seat = Some(seat);
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
            self.init_text_input(qh);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            self.keyboard = None;
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        self.seat = None;
        self.keyboard = None;
    }
}

impl KeyboardHandler for AppData {
    fn enter(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[xkbcommon::xkb::Keysym],
    ) {
        if surface == &self.surface {
            unim_log!("TEST_WAYLAND", "Keyboard Enter: IME 활성화");
            if let Some(ti) = &self.text_input {
                ti.enable();
                ti.commit();
            } else {
                self.init_text_input(qh);
            }
        }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        if surface == &self.surface {
            unim_log!("TEST_WAYLAND", "Keyboard Leave: IME 비활성화");
            if let Some(ti) = &self.text_input {
                ti.disable();
                ti.commit();
            }
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if event.keysym == xkbcommon::xkb::keysyms::KEY_Escape {
            self.should_quit = true;
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl Dispatch<ZwpTextInputManagerV3, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } => {
                unim_log!("TEST_WAYLAND", "Text Input Enter");
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                unim_log!("TEST_WAYLAND", "Text Input Leave");
            }
            zwp_text_input_v3::Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let text = text.unwrap_or_default();
                unim_log!(
                    "TEST_WAYLAND",
                    "Preedit: \"{}\" ({}, {})",
                    text,
                    cursor_begin,
                    cursor_end
                );
                state.preedit = text;
                state.cursor_begin = cursor_begin;
                state.cursor_end = cursor_end;
                state.draw();
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                let text = text.unwrap_or_default();
                unim_log!("TEST_WAYLAND", "Commit: \"{}\"", text);
                state.text.push_str(&text);
                state.preedit.clear();
                state.draw();
            }
            zwp_text_input_v3::Event::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                unim_log!(
                    "TEST_WAYLAND",
                    "Delete Surrounding: before={}, after={}",
                    before_length,
                    after_length
                );
                let chars: Vec<char> = state.text.chars().collect();
                if chars.len() >= before_length as usize {
                    let new_len = chars.len() - before_length as usize;
                    state.text = chars.into_iter().take(new_len).collect();
                    state.draw();
                }
            }
            zwp_text_input_v3::Event::Done { serial } => {
                unim_log!("TEST_WAYLAND", "Text Input Done (serial={})", serial);
            }
            _ => {}
        }
    }
}

// Global Listener
impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, GlobalList> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &wayland_client::protocol::wl_registry::WlRegistry,
        event: wayland_client::protocol::wl_registry::Event,
        data: &GlobalList,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == "zwp_text_input_manager_v3" {
                    let manager = data.bind::<ZwpTextInputManagerV3, _, _>(name, version, qh, ());
                    state.text_input_manager = Some(manager);
                    unim_log!("TEST_WAYLAND", "zwp_text_input_manager_v3 바인딩됨");

                    if state.seat.is_some() {
                        state.init_text_input(qh);
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    unim_log!("TEST_WAYLAND", "Starting unim-test-wayland...");

    let event_loop = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_handlers::registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut app_data = AppData::new(&conn, &qh);

    loop_handle
        .insert_source(event_queue, |_, &mut (), state: &mut AppData| {
            let _ = state;
        })
        .unwrap();

    loop {
        event_loop.dispatch(None, &mut app_data).unwrap();
        if app_data.should_quit {
            break;
        }
    }
}
