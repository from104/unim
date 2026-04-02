use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use tiny_skia::{FillRule, Paint, PathBuilder, PixmapMut, Rect, Transform};
use unim::unim_log;
use wayland_client::{
    globals::{registry_queue_init, GlobalList},
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
    #[allow(dead_code)]
    compositor_state: CompositorState,
    #[allow(dead_code)]
    xdg_shell: XdgShell,
    shm: Shm,

    text_input_manager: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,

    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    width: u32,
    height: u32,
    window: Window,

    pool: SlotPool,
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_buffer: Buffer,

    text: String,
    preedit: String,
    #[allow(dead_code)]
    cursor_begin: i32,
    #[allow(dead_code)]
    cursor_end: i32,

    first_configure: bool,
    should_quit: bool,
}

impl AppData {
    fn new(globals: &GlobalList, qh: &QueueHandle<Self>, shm: Shm) -> Self {
        let registry_state = RegistryState::new(globals);
        let seat_state = SeatState::new(globals, qh);
        let output_state = OutputState::new(globals, qh);
        let compositor_state =
            CompositorState::bind(globals, qh).expect("compositor not available");
        let xdg_shell = XdgShell::bind(globals, qh).expect("xdg_shell not available");

        let surface = compositor_state.create_surface(qh);
        let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, qh);
        window.set_title("UNIM Wayland Test");
        window.set_app_id("unim-test-wayland");
        window.set_min_size(Some((400, 300)));
        window.commit();

        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(24.0, 30.0);
        let text_buffer = Buffer::new(&mut font_system, metrics);
        let swash_cache = SwashCache::new();

        let pool = SlotPool::new(1024 * 1024, &shm).expect("Failed to create pool");

        Self {
            registry_state,
            seat_state,
            output_state,
            compositor_state,
            xdg_shell,
            shm,
            text_input_manager: None,
            text_input: None,
            seat: None,
            keyboard: None,
            width: 800,
            height: 600,
            window,
            pool,
            font_system,
            swash_cache,
            text_buffer,
            text: String::new(),
            preedit: String::new(),
            cursor_begin: 0,
            cursor_end: 0,
            first_configure: true,
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

        let (wl_buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
            )
            .expect("create buffer");

        let mut pixmap = PixmapMut::from_bytes(canvas, width, height).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(30, 30, 46, 255));

        let mut display_text = self.text.clone();
        let _preedit_start_idx = display_text.len();
        display_text.push_str(&self.preedit);

        // 텍스트 레이아웃 설정
        self.text_buffer.set_text(
            &mut self.font_system,
            &display_text,
            Attrs::new()
                .family(Family::SansSerif)
                .color(Color::rgb(205, 214, 244)),
            Shaping::Advanced,
        );
        self.text_buffer
            .shape_until_scroll(&mut self.font_system, false);

        // 텍스트 그리기
        self.text_buffer.draw(
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
            let paint = Paint {
                shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
                    243, 139, 168, 255,
                )),
                ..Default::default()
            };

            let text_width_approx = (self.text.len() * 10) as f32;
            let preedit_width_approx = (self.preedit.len() * 15) as f32;

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

        let surface = self.window.wl_surface();
        surface.attach(Some(wl_buffer.wl_buffer()), 0, 0);
        surface.damage(0, 0, width as i32, height as i32);
        surface.commit();
    }
}

// --- Delegate Implementations ---

delegate_compositor!(AppData);
delegate_output!(AppData);
delegate_shm!(AppData);
delegate_seat!(AppData);
delegate_keyboard!(AppData);
delegate_registry!(AppData);
delegate_xdg_shell!(AppData);
delegate_xdg_window!(AppData);

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState,];
}

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
        if surface == self.window.wl_surface() {
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
        if surface == self.window.wl_surface() {
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
        if event.keysym == xkbcommon::xkb::Keysym::from(xkbcommon::xkb::keysyms::KEY_Escape) {
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

impl WindowHandler for AppData {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        unim_log!("TEST_WAYLAND", "Window close requested");
        self.should_quit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        if let (Some(w), Some(h)) = configure.new_size {
            self.width = w.get();
            self.height = h.get();
        }
        unim_log!(
            "TEST_WAYLAND",
            "Window configure: {}x{}",
            self.width,
            self.height
        );

        if self.first_configure {
            self.first_configure = false;
            self.draw();
        }
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
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_registry::Event::Global {
                name: _,
                interface,
                version,
            } => {
                if interface == "zwp_text_input_manager_v3" {
                    let manager = data
                        .bind::<ZwpTextInputManagerV3, _, _>(&qh, version..=version, ())
                        .expect("Failed to bind text input manager");
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

// ─── 자동 테스트 (--auto) ────────────────────────────────────────

fn run_auto_test(verbose: bool) -> i32 {
    // unim-test-dbus와 동일한 패턴
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use unim_dbus::client::UnimClient;

        const GREEN: &str = "\x1b[0;32m";
        const RED: &str = "\x1b[0;31m";
        const BOLD: &str = "\x1b[1m";
        const DIM: &str = "\x1b[2m";
        const RESET: &str = "\x1b[0m";

        println!("{BOLD}═══ UNIM Wayland 자동 테스트 ═══{RESET}");

        let client = match UnimClient::connect().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{RED}❌ 데몬 연결 실패: {e}{RESET}");
                return 1;
            }
        };

        let layout = match client.input_method().await {
            Ok(im) => im.get_config("korean_layout").await.unwrap_or_default(),
            Err(_) => String::new(),
        };
        println!("레이아웃: {}", if layout.is_empty() { "기본값" } else { &layout });

        let ctx_path = match client.create_context("wayland-test", "").await {
            Ok(p) => p,
            Err(e) => { eprintln!("{RED}❌ 컨텍스트 실패: {e}{RESET}"); return 1; }
        };

        let im = client.input_method().await.unwrap();
        let ic = client.input_context(&ctx_path).await.unwrap();

        let is_3bul = layout.contains("3bul");

        // 테스트 케이스: (name, keys[(keycode, state)], expected_preedit, expected_commit, korean)
        let cases: Vec<(&str, Vec<(u32, u32)>, &str, &str, bool)> = if is_3bul {
            vec![
                ("초성: ㅎ", vec![(50, 0)], "ㅎ", "", true),
                ("초성+중성: 하", vec![(50, 0), (33, 0)], "하", "", true),
                ("완성형: 한", vec![(50, 0), (33, 0), (31, 0)], "한", "", true),
                ("두 글자: 한글", vec![(50,0),(33,0),(31,0),(37,0),(34,0),(17,0)], "글", "한", true),
                ("Backspace", vec![(50, 0), (33, 0), (14, 0)], "ㅎ", "", true),
                ("스페이스 확정", vec![(50, 0), (33, 0), (57, 0)], "", "하 ", true),
                ("영문 패스스루", vec![(34, 0)], "", "", false),
            ]
        } else {
            vec![
                ("초성: ㅎ", vec![(34, 0)], "ㅎ", "", true),
                ("초성+중성: 하", vec![(34, 0), (37, 0)], "하", "", true),
                ("완성형: 한", vec![(34, 0), (37, 0), (31, 0)], "한", "", true),
                ("두 글자: 한글", vec![(34,0),(37,0),(31,0),(19,0),(50,0),(33,0)], "글", "한", true),
                ("영문 패스스루", vec![(34, 0)], "", "", false),
            ]
        };

        let mut passed = 0;
        let mut failed = 0;

        println!("\n{BOLD}── 조합 테스트 ──{RESET}");
        for (name, keys, exp_pre, exp_com, korean) in &cases {
            let _ = im.set_global_mode(*korean).await;
            let _ = ic.focus_in("").await;

            let mut last_preedit = String::new();
            let mut total_commit = String::new();
            let mut ok = true;

            for (kc, st) in keys {
                match ic.process_key_event(0, *kc, *st).await {
                    Ok((_consumed, pre, com, _tf_del, _tf_repl)) => {
                        if verbose {
                            eprintln!("    {DIM}key={kc} → pre=\"{pre}\" com=\"{com}\"{RESET}");
                        }
                        last_preedit = pre;
                        total_commit.push_str(&com);
                    }
                    Err(e) => { eprintln!("    {RED}ProcessKey 실패: {e}{RESET}"); ok = false; break; }
                }
            }

            if ok && !exp_pre.is_empty() && last_preedit != *exp_pre { ok = false; }
            if ok && !exp_com.is_empty() && total_commit != *exp_com { ok = false; }

            let _ = ic.reset().await;

            if ok {
                passed += 1;
                println!("  {GREEN}PASS{RESET} {name}");
            } else {
                failed += 1;
                println!("  {RED}FAIL{RESET} {name}");
                if !exp_pre.is_empty() && last_preedit != *exp_pre {
                    println!("       {DIM}preedit: 기대=\"{exp_pre}\" 실제=\"{last_preedit}\"{RESET}");
                }
                if !exp_com.is_empty() && total_commit != *exp_com {
                    println!("       {DIM}commit: 기대=\"{exp_com}\" 실제=\"{total_commit}\"{RESET}");
                }
            }
        }

        let _ = ic.destroy().await;

        let total = passed + failed;
        println!();
        if failed == 0 {
            println!("{GREEN}{BOLD}✅ ALL PASS{RESET} ({passed}/{total})");
        } else {
            println!("{RED}{BOLD}❌ {failed} FAILED{RESET} / {passed} passed / {total} total");
        }
        if failed > 0 { 1 } else { 0 }
    })
}

fn main() {
    // --auto 모드: DBus 자동 테스트
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--auto") {
        let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
        std::process::exit(run_auto_test(verbose));
    }

    unim_log!("TEST_WAYLAND", "Starting unim-test-wayland...");

    let mut event_loop: EventLoop<AppData> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let shm = Shm::bind(&globals, &qh).expect("shm not available");
    let mut app_data = AppData::new(&globals, &qh, shm);

    WaylandSource::new(conn, event_queue)
        .insert(loop_handle)
        .unwrap();

    unim_log!("TEST_WAYLAND", "Event loop starting...");

    loop {
        event_loop.dispatch(None, &mut app_data).unwrap();
        if app_data.should_quit {
            break;
        }
    }

    unim_log!("TEST_WAYLAND", "Exiting.");
}
