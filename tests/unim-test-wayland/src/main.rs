//! UNIM Wayland 테스트 앱 — text-input-v3 경로를 화면으로 검증한다.
//!
//! 다른 5개 앱(GTK3·GTK4·Qt5·Qt6·XIM)과 **같은 화면**을 만든다. 스펙·필드
//! 엔진·로그 형식은 `tests/common` 의 C 를 그대로 링크하므로(→ `common-rs`)
//! Rust 로 옮겨 적은 곳이 없다 — 미러가 없으니 어긋날 수가 없다.
//!
//! 코어 필드는 툴킷 위젯이 아니라 **캔버스에 직접 그린다**. 위젯을 쓰면
//! preedit 이 위젯 내부에 숨어 관측되지 않기 때문이다(TEST_APPS.md §2).
//!
//! ## 2026-08-09 재작성 — 이전 판이 먹통이던 이유
//!
//! 1. `zwp_text_input_manager_v3` 를 `Dispatch<WlRegistry, GlobalList>` 로
//!    받으려 했는데, `registry_queue_init` 이 만든 레지스트리의 user-data 는
//!    `GlobalListContents` 라 그 impl 이 **한 번도 불리지 않았다**. 매니저가
//!    영영 None 이라 IME 가 붙지 않았다. → 지금은 시작할 때 `GlobalList::bind`
//!    로 직접 잡는다.
//! 2. 키 처리가 Escape 하나뿐이라 영문·편집키가 전부 무시됐다.
//! 3. `wl_shm` 의 `Argb8888` 은 리틀엔디언이라 메모리 배열이 B,G,R,A 인데
//!    RGBA 로 그리고 있었다(색이 뒤집힘). → `Canvas` 가 바이트를 직접 쓴다.
//!
//! ## XTEST 가 안 통한다
//!
//! Wayland 에는 X 의 XTEST 같은 전역 키 주입이 없다. 그래서 이 앱은
//! `tests/harness` 의 자동시험 대상이 아니다(`harness.py` 의 `xtest: False`).
//! 대신 로그를 과하게 남겨 **사람이 눈으로 보는 검증**을 돕는다.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_void};

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use cosmic_text::{
    Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping,
    SwashCache,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
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
use unim_test_common as tc;
use unim_test_common::daemon::Daemon;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ChangeCause, ContentHint, ContentPurpose, ZwpTextInputV3},
};
use xkbcommon::xkb::Keysym;

const APP_NAME: &str = "wayland";

/// 화면 로그 패널에 보이는 줄 수 — XIM 앱과 같게 둔다.
const LOG_VIEW_LINES: usize = 14;

/* ─── 폰트 ────────────────────────────────────────────────────────────── */

/// 스펙의 크기는 **포인트**다(Xft·Pango 가 그렇게 받는다). cosmic-text 는
/// 픽셀을 받으므로 96 dpi 기준으로 환산한다.
fn pt_to_px(pt: i32) -> f32 {
    pt as f32 * 96.0 / 72.0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fnt {
    Ui,
    Field,
    Log,
}

impl Fnt {
    fn px(self) -> f32 {
        let m = tc::metrics();
        pt_to_px(match self {
            Fnt::Ui => m.font_size_ui,
            Fnt::Field => m.font_size_field,
            Fnt::Log => m.font_size_log,
        })
    }
    fn family(self) -> Family<'static> {
        match self {
            Fnt::Log => Family::Name(tc::font_mono()),
            _ => Family::Name(tc::font_ui()),
        }
    }
}

/* ─── 캔버스 ──────────────────────────────────────────────────────────── */

/// `wl_shm` 버퍼에 직접 쓴다.
///
/// `wl_shm::Format::Argb8888` 은 32비트 값 0xAARRGGBB 를 **리틀엔디언**으로
/// 담으므로 메모리 배열은 B,G,R,A 다. 라이브러리를 끼우면 이 순서를 착각하기
/// 쉬워서(이전 판이 정확히 그랬다) 여기서 바이트를 명시적으로 쓴다.
///
/// 좌표는 **논리 픽셀**로 받고 안에서 `scale` 을 곱한다. 스펙 수치가 전부
/// 96dpi 기준 논리 픽셀이라(TEST_APPS.md §3) 앱 코드에 배율이 흩어지지
/// 않는다. `w`·`h` 만 실제 버퍼 크기(장치 픽셀)다.
struct Canvas<'a> {
    buf: &'a mut [u8],
    w: i32,
    h: i32,
    scale: i32,
}

impl Canvas<'_> {
    fn fill_all(&mut self, rgb: u32) {
        let (r, g, b) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        for px in self.buf.chunks_exact_mut(4) {
            px[0] = b;
            px[1] = g;
            px[2] = r;
            px[3] = 0xff;
        }
    }

    /// 불투명 사각형 (논리 좌표).
    fn rect(&mut self, rgb: u32, x: i32, y: i32, w: i32, h: i32) {
        let s = self.scale;
        self.blend_dev(rgb, 0xff, x * s, y * s, w * s, h * s);
    }

    /// 테두리 — Xlib 의 `XDrawRectangle` + 선 두께와 같은 자리에 그린다.
    fn frame(&mut self, rgb: u32, x: i32, y: i32, w: i32, h: i32, lw: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.rect(rgb, x, y, w, lw);
        self.rect(rgb, x, y + h - lw, w, lw);
        self.rect(rgb, x, y, lw, h);
        self.rect(rgb, x + w - lw, y, lw, h);
    }

    /// 알파 합성(src-over), **장치 좌표**. 글자 안티에일리어싱이 이 길로 온다.
    fn blend_dev(&mut self, rgb: u32, alpha: u8, x: i32, y: i32, w: i32, h: i32) {
        if alpha == 0 || w <= 0 || h <= 0 {
            return;
        }
        let (sr, sg, sb) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
        let a = alpha as u32;
        let inv = 255 - a;
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        for yy in y.max(0)..y1 {
            for xx in x.max(0)..x1 {
                let i = ((yy * self.w + xx) * 4) as usize;
                if alpha == 0xff {
                    self.buf[i] = sb as u8;
                    self.buf[i + 1] = sg as u8;
                    self.buf[i + 2] = sr as u8;
                } else {
                    self.buf[i] = ((sb * a + self.buf[i] as u32 * inv) / 255) as u8;
                    self.buf[i + 1] = ((sg * a + self.buf[i + 1] as u32 * inv) / 255) as u8;
                    self.buf[i + 2] = ((sr * a + self.buf[i + 2] as u32 * inv) / 255) as u8;
                }
                self.buf[i + 3] = 0xff;
            }
        }
    }
}

/* ─── 글자 ────────────────────────────────────────────────────────────── */

struct Text {
    fs: FontSystem,
    cache: SwashCache,
    /// 정수 배율(HiDPI). 폰트는 이 배율로 **또렷하게** 그리고, 나머지 계산은
    /// 전부 논리 픽셀로 되돌려 스펙 수치와 맞춘다.
    scale: i32,
}

impl Text {
    fn new() -> Self {
        Text {
            fs: FontSystem::new(),
            cache: SwashCache::new(),
            scale: 1,
        }
    }

    fn shape(&mut self, f: Fnt, s: &str) -> TextBuffer {
        let px = f.px() * self.scale as f32;
        let mut b = TextBuffer::new(&mut self.fs, Metrics::new(px, (px * 1.35).round()));
        // 폭 제한을 두지 않는다 — 필드 안에서 줄바꿈이 일어나면 캐럿 계산과
        // 화면이 어긋난다. 넘치는 글자는 필드 밖으로 나가 잘린다.
        b.set_size(&mut self.fs, Some(100_000.0), None);
        b.set_text(&mut self.fs, s, Attrs::new().family(f.family()), Shaping::Advanced);
        b.shape_until_scroll(&mut self.fs, false);
        b
    }

    /// (ascent, line height) — 논리 픽셀. 다른 앱이 Xft/Pango 에서 얻는 값과
    /// 같은 쓰임이다.
    fn vmetrics(&mut self, f: Fnt) -> (i32, i32) {
        let s = self.scale as f32;
        let b = self.shape(f, "한Ag");
        match b.layout_runs().next() {
            Some(r) => ((r.line_y / s).round() as i32, (r.line_height / s).round() as i32),
            None => (f.px() as i32, (f.px() * 1.35) as i32),
        }
    }

    /// 논리 픽셀 폭.
    fn width(&mut self, f: Fnt, s: &str) -> i32 {
        if s.is_empty() {
            return 0;
        }
        let sc = self.scale as f32;
        let b = self.shape(f, s);
        (b.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max) / sc).ceil() as i32
    }

    /// `x`·`baseline` 은 **논리 픽셀**, `baseline` 은 첫 줄의 베이스라인 —
    /// Xft 의 `XftDrawStringUtf8` 과 같은 기준이라 다른 앱과 글자가 맞는다.
    fn draw(&mut self, c: &mut Canvas, f: Fnt, rgb: u32, x: i32, baseline: i32, s: &str) {
        if s.is_empty() {
            return;
        }
        let sc = self.scale;
        let buf = self.shape(f, s);
        let ly = buf.layout_runs().next().map(|r| r.line_y).unwrap_or(0.0);
        let ox = x * sc;
        let oy = (baseline as f32 * sc as f32 - ly).round() as i32;
        let Text { fs, cache, .. } = self;
        buf.draw(fs, cache, TextColor::rgb(0xff, 0xff, 0xff), |gx, gy, gw, gh, col| {
            c.blend_dev(rgb, col.a(), ox + gx, oy + gy, gw as i32, gh as i32);
        });
    }
}

/* ─── 로그 패널 ───────────────────────────────────────────────────────── */

thread_local! {
    static LOG_PANEL: RefCell<VecDeque<String>> =
        const { RefCell::new(VecDeque::new()) };
}

fn log_sink(line: &str) {
    LOG_PANEL.with(|l| {
        let mut l = l.borrow_mut();
        if l.len() >= LOG_VIEW_LINES {
            l.pop_front();
        }
        l.push_back(line.to_string());
    });
}

fn log_lines() -> Vec<String> {
    LOG_PANEL.with(|l| l.borrow().iter().cloned().collect())
}

/* ─── 캐럿 측정 콜백 ──────────────────────────────────────────────────── */

/// `unim_field_caret_from_x` 가 부르는 폭 측정 함수.
///
/// `user` 는 `&mut Text`. GUI 콜백은 단일 스레드에서 직렬로만 불리고 이
/// 함수 안에서 다시 필드 엔진을 부르지 않으므로 재진입이 없다.
extern "C" fn measure_cb(utf8: *const c_char, nbytes: usize, user: *mut c_void) -> c_int {
    if utf8.is_null() || nbytes == 0 || user.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(utf8 as *const u8, nbytes) };
    let s = String::from_utf8_lossy(bytes);
    let t = unsafe { &mut *(user as *mut Text) };
    t.width(Fnt::Field, &s)
}

/* ─── 앱 ──────────────────────────────────────────────────────────────── */

/// text-input-v3 의 보류 상태. `done` 에서 한꺼번에 적용한다 — 이벤트가 올
/// 때마다 바로 적용하면 원자성이 깨져 화면이 중간 상태를 보인다.
#[derive(Default)]
struct Pending {
    preedit: Option<(String, i32, i32)>,
    commit: Option<String>,
    delete: Option<(u32, u32)>,
}

struct App {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    window: Window,

    ti_manager: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,
    ti_serial: u32,
    ti_enabled: bool,
    pending: Pending,

    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    mods: Modifiers,

    width: u32,
    height: u32,
    /// 정수 배율(HiDPI). 논리 좌표는 그대로 두고 버퍼만 키운다.
    scale: i32,
    configured: bool,
    dirty: bool,
    quit: bool,
    ready: bool,
    kb_focus: bool,

    text: Text,
    ui_ascent: i32,
    ui_h: i32,
    log_h: i32,
    field_ascent: i32,

    fields: Vec<Box<tc::Field>>,
    active: usize,
    fields_top: i32,
    log_top: i32,

    daemon: Daemon,
    last_commit: String,

    attached: Option<smithay_client_toolkit::shm::slot::Buffer>,

    /// `--dump-frame PATH` — 다음 프레임을 PPM 으로 한 번 저장한다.
    dump_path: Option<String>,

    /// 이벤트 루프 쪽에서 그릴 때도 `surface.frame` 을 걸어야 해서 들고 있는다.
    qh: QueueHandle<App>,
}

impl App {
    fn cur(&self) -> &tc::Field {
        &self.fields[self.active]
    }
    fn cur_mut(&mut self) -> &mut tc::Field {
        &mut self.fields[self.active]
    }

    /* ── 레이아웃 ── */

    fn relayout(&mut self) {
        let m = tc::metrics();
        let (asc, h) = self.text.vmetrics(Fnt::Ui);
        self.ui_ascent = asc;
        self.ui_h = h;
        self.log_h = self.text.vmetrics(Fnt::Log).1;
        self.field_ascent = self.text.vmetrics(Fnt::Field).0;

        let n_status = tc::status_labels().len() as i32;
        self.fields_top = m.margin + n_status * (self.ui_h + 4) + m.section_gap + 12;
        let bottom = tc::layout(&mut self.fields, self.fields_top, m.win_width, 1.0);
        self.log_top = bottom + m.section_gap;

        for f in &self.fields {
            tc::log_raw(
                "field.geometry",
                &format!(
                    "\"field\":\"{}\",\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
                    f.id(),
                    f.x,
                    f.y,
                    f.w,
                    f.h
                ),
            );
        }
        // Wayland 클라이언트는 자기 창의 화면 좌표를 알 수 없다 — 절대 좌표를
        // 못 남기는 것이 XTEST 자동시험이 불가능한 이유와 같은 뿌리다.
        tc::log_note("절대 좌표 없음 — Wayland 는 창 위치를 클라이언트에 알려주지 않는다");
    }

    /* ── 그리기 ── */

    fn status_values(&self) -> Vec<String> {
        self.daemon.status(
            APP_NAME,
            self.cur().id(),
            &self.cur().preedit_str(),
            self.cur().preedit_caret,
            &self.last_commit,
        )
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let m = tc::metrics();
        let (w, h) = (self.width, self.height);
        // 버퍼는 장치 픽셀이다. `set_buffer_scale` 로 컴포지터에 배율을 알려
        // 주므로 HiDPI 에서도 확대 흐림 없이 또렷하다.
        let scale = self.scale;
        let (dw, dh) = (w as i32 * scale, h as i32 * scale);
        let stride = dw * 4;

        let status = self.status_values();
        let logs = log_lines();
        let labels = tc::status_labels();

        let (buffer, canvas) =
            match self.pool.create_buffer(dw, dh, stride, wl_shm::Format::Argb8888) {
                Ok(v) => v,
                Err(e) => {
                    tc::log_error(&format!("shm 버퍼 생성 실패: {e}"));
                    return;
                }
            };

        {
            let mut c = Canvas {
                buf: canvas,
                w: dw,
                h: dh,
                scale,
            };
            // `self.pool`(canvas) 과 아래 필드들은 서로 다른 필드라 동시에
            // 빌려도 된다 — 그래서 여기를 메서드로 빼지 않는다.
            let text = &mut self.text;
            let fields = &self.fields;
            let active = self.active;
            let kb_focus = self.kb_focus;

            c.fill_all(m.col_bg);

            /* ① 상태 */
            text.draw(&mut c, Fnt::Ui, m.col_label, m.margin, m.margin - 4, "① 상태");
            let mut y = m.margin + self.ui_ascent;
            for (i, label) in labels.iter().enumerate() {
                text.draw(&mut c, Fnt::Ui, m.col_label, m.margin, y, label);
                let v = status.get(i).map(String::as_str).unwrap_or("");
                text.draw(&mut c, Fnt::Ui, m.col_text, m.margin + m.status_label_w, y, v);
                y += self.ui_h + 4;
            }

            /* ② 코어 필드 */
            text.draw(
                &mut c,
                Fnt::Ui,
                m.col_label,
                m.margin,
                self.fields_top - 8,
                "② 코어 필드 (IM 직결 · 직접 그리기)",
            );
            for (i, f) in fields.iter().enumerate() {
                draw_field(&mut c, text, f, kb_focus && i == active, self.field_ascent);
            }

            /* ③ 네이티브 위젯 섹션은 없다 — Wayland 클라이언트에는 툴킷 기본
             * 위젯이 없다. XIM 앱과 같은 이유로 의도된 차이다. */

            /* ④ 로그 */
            let mut ly = self.log_top;
            text.draw(&mut c, Fnt::Ui, m.col_label, m.margin, ly + self.ui_ascent, "④ 로그");
            ly += self.ui_h + 6;
            c.rect(
                m.col_panel,
                m.margin,
                ly,
                m.win_width - 2 * m.margin,
                LOG_VIEW_LINES as i32 * (self.log_h + 2) + 8,
            );
            let mut lb = ly + 4 + self.text.vmetrics(Fnt::Log).0;
            for line in &logs {
                self.text
                    .draw(&mut c, Fnt::Log, m.col_text, m.margin + 6, lb, line);
                lb += self.log_h + 2;
            }

            // Wayland 에서는 스크린샷 권한이 막혀 있는 일이 잦다(GNOME 은
            // 포털 밖 호출을 거부한다). 그래서 앱이 자기 화면을 직접 뱉는다 —
            // "화면의 진실" 을 눈으로 확인하는 유일한 길이다.
            // 매 프레임 덮어쓴다 — 파일에는 늘 **마지막 화면**이 남는다.
            if let Some(path) = self.dump_path.as_deref() {
                dump_ppm(c.buf, c.w, c.h, path);
            }
        }

        let surface = self.window.wl_surface();
        surface.set_buffer_scale(scale);
        if let Err(e) = buffer.attach_to(surface) {
            tc::log_error(&format!("버퍼 attach 실패: {e}"));
            return;
        }
        surface.damage_buffer(0, 0, dw, dh);
        // 다음 프레임 시각을 받아 두면 컴포지터가 그릴 준비가 됐을 때만 다시
        // 그리게 된다. 이전 판은 이 요청이 없어 `frame()` 이 영영 안 불렸다.
        surface.frame(qh, surface.clone());
        surface.commit();

        // 버퍼는 컴포지터가 읽어 갈 때까지 살아 있어야 한다.
        self.attached = Some(buffer);
        self.dirty = false;

        if !self.ready {
            self.ready = true;
            tc::log_ready();
        }
    }

    /* ── text-input-v3 ── */

    fn ensure_text_input(&mut self, qh: &QueueHandle<Self>) {
        if self.text_input.is_some() {
            return;
        }
        let (Some(mgr), Some(seat)) = (&self.ti_manager, &self.seat) else {
            return;
        };
        let ti = mgr.get_text_input(seat, qh, ());
        tc::log_note("zwp_text_input_v3 생성");
        self.text_input = Some(ti);
    }

    /// 활성 필드의 상태를 IM 에 통째로 보낸다.
    ///
    /// text-input-v3 는 `enable` 뒤 상태가 기본값으로 되돌아가므로 켤 때마다
    /// 전부 다시 보내야 한다.
    fn send_state(&mut self, enable: bool) {
        let Some(ti) = self.text_input.clone() else {
            return;
        };
        let m = tc::metrics();

        if enable && !self.ti_enabled {
            ti.enable();
            self.ti_enabled = true;
            tc::log_note("text-input enable");
        } else if !enable && self.ti_enabled {
            ti.disable();
            self.ti_enabled = false;
            tc::log_note("text-input disable");
            ti.commit();
            self.ti_serial = self.ti_serial.wrapping_add(1);
            tc::log_raw("ti.commit", "\"serial\":0,\"field\":\"(disable)\"");
            return;
        }
        if !self.ti_enabled {
            return;
        }

        let (hint, purpose) = content_type(self.cur().hint());
        ti.set_content_type(hint, purpose);

        // 비밀번호 필드는 주변 문맥을 보내지 않는다 — 실제 툴킷도 그렇고,
        // AutoTypeFix·한자 팝업 억제가 이 경로로 걸리는지 보는 자리다.
        if self.cur().hint() == tc::Hint::Password {
            tc::log_surrounding("retrieve", "(비밀번호 — 보내지 않음)", -1, 0, 0);
        } else {
            let s = self.cur().committed_str();
            let caret = self.cur().caret;
            // 프로토콜 상한 4000 바이트. 넘으면 캐럿 주변만 보낸다.
            let (s, caret) = clamp_surrounding(&s, caret);
            ti.set_surrounding_text(s.clone(), caret, caret);
            tc::log_surrounding("retrieve", &s, caret, 0, s.chars().count() as i32);
        }

        ti.set_text_change_cause(ChangeCause::InputMethod);

        let before = self.cur().before_caret();
        let cx = self.cur().x + m.field_pad_x + self.text.width(Fnt::Field, &before);
        let (fy, fh) = (self.cur().y, self.cur().h);
        ti.set_cursor_rectangle(cx, fy, 2, fh);

        ti.commit();
        self.ti_serial = self.ti_serial.wrapping_add(1);
        tc::log_raw(
            "ti.commit",
            &format!(
                "\"serial\":{},\"field\":\"{}\"",
                self.ti_serial,
                self.cur().id()
            ),
        );
    }

    /// `done` 이 왔을 때 보류 상태를 프로토콜이 정한 순서대로 적용한다.
    fn apply_pending(&mut self, serial: u32) {
        if serial != self.ti_serial {
            tc::log_warn(&format!(
                "done serial={serial} 인데 우리 commit 은 {} — IM 이 낡은 상태를 봤다",
                self.ti_serial
            ));
        }
        let p = std::mem::take(&mut self.pending);
        let id = self.cur().id().to_string();
        let old_preedit = self.cur().preedit_str();
        let mut changed = false;

        // 1) 주변 문맥 삭제
        if let Some((before, after)) = p.delete {
            if before > 0 || after > 0 {
                tc::log_surrounding("delete", "", -1, -(before as i32), after as i32);
                // preedit 은 IM 소유다. 삭제 전에 걷어내야 확정 텍스트만 지운다.
                if !old_preedit.is_empty() {
                    self.cur_mut().set_preedit("", 0);
                }
                delete_surrounding(self.cur_mut(), before, after);
                changed = true;
            }
        }
        // 2) 확정
        if let Some(t) = p.commit {
            if !t.is_empty() {
                if !self.cur().preedit_str().is_empty() {
                    self.cur_mut().set_preedit("", 0);
                }
                tc::log_commit(&id, &t);
                self.last_commit = t.clone();
                self.cur_mut().commit(&t);
                changed = true;
            }
        }
        // 3) 새 preedit
        let new_preedit = p.preedit.clone().map(|(t, _, _)| t).unwrap_or_default();
        match p.preedit {
            Some((t, cb, _ce)) if !t.is_empty() => {
                if self.cur().composing == 0 {
                    self.cur_mut().preedit_start();
                }
                let cursor = if cb < 0 { t.len() as i32 } else { cb };
                tc::log_preedit("changed", &id, &t, cursor, Some("underline"));
                self.cur_mut().set_preedit(&t, cursor);
            }
            _ => {
                if self.cur().composing != 0 || !self.cur().preedit_str().is_empty() {
                    tc::log_preedit("end", &id, "", -1, None);
                    self.cur_mut().preedit_end();
                }
            }
        }
        if new_preedit != old_preedit {
            changed = true;
        }

        // ⚠️ **바뀐 게 없으면 commit 을 되쏘지 않는다.**
        //
        // IM 은 클라이언트의 `commit` 마다 `done` 으로 답한다. done 을 받을
        // 때마다 조건 없이 commit 하면 둘이 끝없이 주고받는다 — 2026-08-09
        // 첫 실행에서 6초에 6000번을 돌았다.
        if changed {
            self.send_state(true);
        }
        self.dirty = true;
    }

    /* ── 포커스 ── */

    fn focus_field(&mut self, idx: usize, reason: &str) {
        if idx == self.active {
            return;
        }
        let prev_id = self.cur().id().to_string();

        if self.cur().composing != 0 || !self.cur().preedit_str().is_empty() {
            tc::log_reset(&prev_id, reason);
            // XIM 의 `XmbResetIC` 처럼 조합 중이던 문자열을 **동기로 돌려받는
            // 길이 text-input-v3 에는 없다.** disable 로 IM 조합을 끊고, 앱은
            // 자기 preedit 만 정리한다. 이 뒤에 commit_string 이 늦게 오면
            // 그건 진짜 회귀 신호다(웹뷰 클릭 커밋 버그와 같은 계열).
            self.send_state(false);
            self.cur_mut().preedit_end();
        }

        self.fields[self.active].set_focus(false, None);
        self.active = idx;
        self.fields[self.active].set_focus(true, Some(&prev_id));
        self.send_state(true);
        self.dirty = true;
    }

    /* ── 키 ── */

    fn mod_state(&self) -> u32 {
        // X 의 비트와 같게 맞춘다 — 다른 앱 로그와 나란히 읽기 위해서다.
        (self.mods.shift as u32) | ((self.mods.ctrl as u32) << 2) | ((self.mods.alt as u32) << 3)
    }

    fn on_key(&mut self, ev: KeyEvent) {
        let ks = ev.keysym;
        let state = self.mod_state();
        tc::log_key(
            "press",
            ks.raw(),
            ev.raw_code,
            state,
            ev.utf8.as_deref(),
            0,
        );

        // Tab 은 필드 순환이 우선이다 (Qt 처럼 툴킷이 먼저 삼키지 않도록).
        if ks == Keysym::Tab || ks == Keysym::ISO_Left_Tab {
            let n = self.fields.len();
            let dir = if self.mods.shift { n - 1 } else { 1 };
            self.focus_field((self.active + dir) % n, "tab");
            return;
        }

        let n = self.fields.len();
        match ks {
            Keysym::BackSpace => self.cur_mut().backspace(),
            Keysym::Delete => self.cur_mut().delete(),
            Keysym::Left => self.cur_mut().move_caret(-1),
            Keysym::Right => self.cur_mut().move_caret(1),
            Keysym::Home => self.cur_mut().caret_home(),
            Keysym::End => self.cur_mut().caret_end(),
            Keysym::Escape => self.cur_mut().clear(),
            Keysym::Return | Keysym::KP_Enter => {
                if self.cur().hint() == tc::Hint::Multiline {
                    self.cur_mut().insert("\n");
                } else {
                    self.focus_field((self.active + 1) % n, "enter");
                    return;
                }
            }
            _ => {
                // IM 이 삼키지 않고 넘어온 문자(영문 모드 등). 제어코드는 뺀다.
                if let Some(t) = ev.utf8.as_deref() {
                    if !t.is_empty() && !t.chars().any(|c| c.is_control()) {
                        self.last_commit = t.to_string();
                        tc::log_commit(self.cur().id(), t);
                        self.cur_mut().commit(t);
                    }
                }
            }
        }
        self.send_state(true);
        self.dirty = true;
    }

    fn on_click(&mut self, x: i32, y: i32) {
        let Some(hit) = tc::hit(&self.fields, x, y) else {
            tc::log_click(x, y, "(빈 곳)", -1, -1);
            return;
        };
        if hit != self.active {
            self.focus_field(hit, "click");
        } else if self.cur().composing != 0 || !self.cur().preedit_str().is_empty() {
            // 같은 필드 안에서 조합 중 클릭 — 2026-08-06 회귀의 재현 지점.
            tc::log_reset(self.cur().id(), "click-in-field");
            self.send_state(false);
            self.cur_mut().preedit_end();
            self.send_state(true);
        }

        let before = self.cur().caret;
        let tp: *mut Text = &mut self.text;
        // `tp` 는 바로 아래에서만 쓰이고 `measure_cb` 는 필드 엔진을 다시
        // 부르지 않으므로 재진입이 없다.
        let caret = unsafe { tc::caret_from_x(self.cur(), x, measure_cb, tp as *mut c_void) };
        self.cur_mut().caret = caret;
        let id = self.cur().id().to_string();
        tc::log_click(x, y, &id, before, caret);
        self.cur().log_render();
        self.send_state(true);
        self.dirty = true;
    }
}

/* ─── 그리기 도우미 ───────────────────────────────────────────────────── */

/// XIM 앱의 `draw_field` 와 같은 그림을 만든다 — 확정·조합·캐럿 3단.
fn draw_field(c: &mut Canvas, text: &mut Text, f: &tc::Field, focused: bool, ascent: i32) {
    let m = tc::metrics();

    let (ui_asc, _) = text.vmetrics(Fnt::Ui);
    text.draw(c, Fnt::Ui, m.col_label, m.margin, f.y + ui_asc + 8, f.label());

    c.rect(
        if focused { m.col_field_focus } else { m.col_field_bg },
        f.x,
        f.y,
        f.w,
        f.h,
    );
    c.frame(
        if focused { m.col_border_focus } else { m.col_border },
        f.x,
        f.y,
        f.w,
        f.h,
        if focused { 2 } else { 1 },
    );

    let shown = f.display();
    let tx = f.x + m.field_pad_x;
    let ty = f.y + ascent + 8;

    // 비밀번호 필드는 한 글자가 `•`(3바이트)로 바뀌므로 바이트 경계가 달라진다.
    let (head_bytes, pre_bytes) = if f.hint() == tc::Hint::Password {
        let committed = f.committed_str();
        let head = committed.get(..f.caret as usize).unwrap_or("");
        (
            head.chars().count() * 3,
            f.preedit_str().chars().count() * 3,
        )
    } else {
        (f.caret as usize, f.preedit_str().len())
    };

    let head = shown.get(..head_bytes).unwrap_or("");
    let pre = shown.get(head_bytes..head_bytes + pre_bytes).unwrap_or("");
    let tail = shown.get(head_bytes + pre_bytes..).unwrap_or("");

    let mut x = tx;
    text.draw(c, Fnt::Field, m.col_text, x, ty, head);
    x += text.width(Fnt::Field, head);

    if !pre.is_empty() {
        text.draw(c, Fnt::Field, m.col_preedit, x, ty, pre);
        let pw = text.width(Fnt::Field, pre);
        c.rect(m.col_preedit, x, ty + 3, pw, 2); // 조합 밑줄
        x += pw;
    }
    text.draw(c, Fnt::Field, m.col_text, x, ty, tail);

    if focused {
        let caret_bytes = if f.hint() == tc::Hint::Password {
            let p = f.preedit_str();
            head_bytes + p.get(..f.preedit_caret as usize).unwrap_or("").chars().count() * 3
        } else {
            head_bytes + f.preedit_caret as usize
        };
        let upto = shown.get(..caret_bytes).unwrap_or("");
        let cx = tx + text.width(Fnt::Field, upto);
        c.rect(m.col_caret, cx, f.y + 6, 2, f.h - 12);
    }
}

/* ─── 잡동사니 ────────────────────────────────────────────────────────── */

/// 합성한 화면을 PPM(P6) 으로 저장한다. 의존성 없이 어디서나 열린다.
fn dump_ppm(buf: &[u8], w: i32, h: i32, path: &str) {
    let mut out = Vec::with_capacity((w * h * 3) as usize + 32);
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    // 버퍼는 B,G,R,A 순서다 (Argb8888 리틀엔디언).
    for px in buf.chunks_exact(4) {
        out.push(px[2]);
        out.push(px[1]);
        out.push(px[0]);
    }
    match std::fs::write(path, out) {
        Ok(()) => tc::log_note(&format!("프레임 저장: {path} ({w}x{h})")),
        Err(e) => tc::log_error(&format!("프레임 저장 실패 {path}: {e}")),
    }
}

fn content_type(hint: tc::Hint) -> (ContentHint, ContentPurpose) {
    match hint {
        tc::Hint::Number => (ContentHint::None, ContentPurpose::Number),
        tc::Hint::Password => (
            ContentHint::HiddenText | ContentHint::SensitiveData,
            ContentPurpose::Password,
        ),
        tc::Hint::Multiline => (ContentHint::Multiline, ContentPurpose::Normal),
        // v3 에는 "검색" purpose 가 없다 — 일반과 같게 두고 힌트만 남긴다.
        tc::Hint::Search => (ContentHint::Completion, ContentPurpose::Normal),
        tc::Hint::None => (ContentHint::None, ContentPurpose::Normal),
    }
}

/// `set_surrounding_text` 는 4000 바이트 상한이 있다. 넘으면 캐럿 주변만.
fn clamp_surrounding(s: &str, caret: i32) -> (String, i32) {
    const MAX: usize = 4000;
    if s.len() <= MAX {
        return (s.to_string(), caret);
    }
    let c = caret.clamp(0, s.len() as i32) as usize;
    let mut start = c.saturating_sub(MAX / 2);
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    let mut end = (start + MAX).min(s.len());
    while end > start && !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[start..end].to_string(), (c - start) as i32)
}

/// `delete_surrounding_text` 는 바이트 수로 온다. 필드 엔진은 글자 단위로
/// 지우므로 요청한 바이트를 다 지울 때까지 반복한다.
fn delete_surrounding(f: &mut tc::Field, before: u32, after: u32) {
    let mut n = before as i32;
    while n > 0 {
        let c0 = f.caret;
        f.backspace();
        let d = c0 - f.caret;
        if d <= 0 {
            break;
        }
        n -= d;
    }
    let mut n = after as i32;
    while n > 0 {
        let l0 = f.committed_str().len() as i32;
        f.delete();
        let d = l0 - f.committed_str().len() as i32;
        if d <= 0 {
            break;
        }
        n -= d;
    }
}

/* ─── Wayland 핸들러 ──────────────────────────────────────────────────── */

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_registry!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _c: &Connection,
        _qh: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let f = new_factor.max(1);
        if f == self.scale {
            return;
        }
        tc::log_note(&format!(
            "scale factor {} → {} (스펙 수치는 96dpi 논리 픽셀, 버퍼만 키운다)",
            self.scale, f
        ));
        self.scale = f;
        self.text.scale = f;
        self.relayout();
        self.dirty = true;
    }

    fn transform_changed(
        &mut self,
        _c: &Connection,
        _qh: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _t: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _c: &Connection,
        qh: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.dirty {
            self.draw(qh);
        }
    }

    fn surface_enter(
        &mut self,
        _c: &Connection,
        _qh: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _o: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _c: &Connection,
        _qh: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _o: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        tc::log_note("wl_seat 등장");
        self.seat = Some(seat);
        self.ensure_text_input(qh);
    }

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Keyboard if self.keyboard.is_none() => {
                match self.seat_state.get_keyboard(qh, &seat, None) {
                    Ok(k) => {
                        self.keyboard = Some(k);
                        tc::log_note("wl_keyboard 확보");
                    }
                    Err(e) => tc::log_error(&format!("wl_keyboard 실패: {e}")),
                }
                self.seat = Some(seat);
                self.ensure_text_input(qh);
            }
            Capability::Pointer if self.pointer.is_none() => {
                match self.seat_state.get_pointer(qh, &seat) {
                    Ok(p) => {
                        self.pointer = Some(p);
                        tc::log_note("wl_pointer 확보");
                    }
                    Err(e) => tc::log_error(&format!("wl_pointer 실패: {e}")),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Keyboard => self.keyboard = None,
            Capability::Pointer => self.pointer = None,
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        self.seat = None;
        self.keyboard = None;
        self.pointer = None;
    }
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        if surface != self.window.wl_surface() {
            return;
        }
        self.kb_focus = true;
        tc::log_focus("in", self.cur().id(), None);
        self.ensure_text_input(qh);
        self.send_state(true);
        self.dirty = true;
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        if surface != self.window.wl_surface() {
            return;
        }
        self.kb_focus = false;
        tc::log_focus("out", self.cur().id(), None);
        self.send_state(false);
        self.dirty = true;
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.on_key(event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        tc::log_key("release", event.keysym.raw(), event.raw_code, self.mod_state(), None, 0);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: u32,
    ) {
        self.mods = modifiers;
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            if e.surface != *self.window.wl_surface() {
                continue;
            }
            if let PointerEventKind::Press { button, .. } = e.kind {
                // BTN_LEFT
                if button == 0x110 {
                    self.on_click(e.position.0 as i32, e.position.1 as i32);
                }
            }
        }
    }
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        tc::log_note("창 닫기 요청");
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        if let (Some(w), Some(h)) = configure.new_size {
            self.width = w.get();
            self.height = h.get();
        }
        tc::log_note(&format!(
            "configure {}x{} (스펙 {}x{})",
            self.width,
            self.height,
            tc::metrics().win_width,
            tc::metrics().win_height
        ));
        self.configured = true;
        self.dirty = true;
        self.draw(qh);
    }
}

impl Dispatch<ZwpTextInputManagerV3, ()> for App {
    fn event(
        _s: &mut Self,
        _p: &ZwpTextInputManagerV3,
        _e: <ZwpTextInputManagerV3 as Proxy>::Event,
        _d: &(),
        _c: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, ()> for App {
    fn event(
        app: &mut Self,
        _p: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        _d: &(),
        _c: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } => {
                tc::log_note("text-input enter — IM 이 이 창을 잡았다");
                app.ti_enabled = false;
                app.send_state(true);
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                tc::log_note("text-input leave");
                app.ti_enabled = false;
            }
            zwp_text_input_v3::Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let t = text.unwrap_or_default();
                tc::log_raw(
                    "ti.preedit_string",
                    &format!(
                        "\"text\":\"{}\",\"cursor_begin\":{},\"cursor_end\":{}",
                        t.escape_debug(),
                        cursor_begin,
                        cursor_end
                    ),
                );
                app.pending.preedit = Some((t, cursor_begin, cursor_end));
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                let t = text.unwrap_or_default();
                tc::log_raw("ti.commit_string", &format!("\"text\":\"{}\"", t.escape_debug()));
                app.pending.commit = Some(t);
            }
            zwp_text_input_v3::Event::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                tc::log_raw(
                    "ti.delete_surrounding",
                    &format!("\"before\":{before_length},\"after\":{after_length}"),
                );
                app.pending.delete = Some((before_length, after_length));
            }
            zwp_text_input_v3::Event::Done { serial } => {
                tc::log_raw("ti.done", &format!("\"serial\":{serial}"));
                app.apply_pending(serial);
            }
            _ => {}
        }
    }
}

/* ─── main ────────────────────────────────────────────────────────────── */

fn main() {
    tc::log_init(APP_NAME);
    tc::set_log_sink(log_sink);
    tc::log_env("wayland-client 0.31 / SCTK 0.19 / cosmic-text 0.12");

    let args: Vec<String> = std::env::args().collect();
    let dump_path = args
        .iter()
        .position(|a| a == "--dump-frame")
        .and_then(|i| args.get(i + 1).cloned());

    if std::env::args().any(|a| a == "--auto") {
        tc::log_note(
            "--auto 는 없앴다 — 데몬을 직접 부르는 시험은 프런트엔드 경로를 \
             타지 않아 회귀를 놓친다 (TEST_APPS.md §1). 창을 띄워 눈으로 본다.",
        );
    }

    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            tc::log_error(&format!(
                "Wayland 연결 실패: {e} — WAYLAND_DISPLAY={}",
                std::env::var("WAYLAND_DISPLAY").unwrap_or_default()
            ));
            tc::log_shutdown();
            std::process::exit(1);
        }
    };

    let (globals, event_queue) = registry_queue_init(&conn).expect("registry");
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor 없음");
    let xdg_shell = XdgShell::bind(&globals, &qh).expect("xdg_wm_base 없음");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm 없음");

    // ⚠️ 여기서 직접 잡는 것이 요점이다. 이전 판은 `Dispatch<WlRegistry, ..>`
    // 로 받으려 했으나 `registry_queue_init` 의 레지스트리 user-data 가 달라
    // 그 impl 이 한 번도 불리지 않았고, IME 가 영영 붙지 않았다.
    let ti_manager = match globals.bind::<ZwpTextInputManagerV3, _, _>(&qh, 1..=1, ()) {
        Ok(m) => {
            tc::log_note("zwp_text_input_manager_v3 바인딩");
            Some(m)
        }
        Err(e) => {
            tc::log_warn(&format!(
                "zwp_text_input_manager_v3 없음: {e} — 조합 없이 raw 키만 들어온다"
            ));
            None
        }
    };

    let m = tc::metrics();
    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_title(tc::win_title(APP_NAME));
    window.set_app_id("org.atit.unim.TestWayland");
    // 스펙 크기보다 작아지면 로그 패널이 잘린다 — 최소 크기로 못 박는다.
    window.set_min_size(Some((m.win_width as u32, m.win_height as u32)));
    window.commit();

    // HiDPI(배율 2) 를 미리 감당할 크기로 잡는다 — 모자라면 SlotPool 이 늘리지만
    // 첫 프레임부터 재할당이 나는 건 피한다.
    let pool = SlotPool::new((m.win_width * m.win_height * 4 * 4) as usize, &shm).expect("shm pool");

    let mut fields: Vec<Box<tc::Field>> = (0..tc::n_core_fields()).map(tc::Field::new).collect();
    fields[0].set_focus(true, None);

    let mut app = App {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        window,
        ti_manager,
        text_input: None,
        ti_serial: 0,
        ti_enabled: false,
        pending: Pending::default(),
        seat: None,
        keyboard: None,
        pointer: None,
        mods: Modifiers::default(),
        width: m.win_width as u32,
        height: m.win_height as u32,
        scale: 1,
        configured: false,
        dirty: true,
        quit: false,
        ready: false,
        kb_focus: false,
        text: Text::new(),
        ui_ascent: 0,
        ui_h: 0,
        log_h: 0,
        field_ascent: 0,
        fields,
        active: 0,
        fields_top: 0,
        log_top: 0,
        daemon: Daemon::connect(),
        last_commit: String::new(),
        attached: None,
        dump_path,
        qh: qh.clone(),
    };
    app.relayout();

    let mut event_loop: EventLoop<App> = EventLoop::try_new().expect("calloop");
    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .expect("wayland source");

    tc::log_note("이벤트 루프 시작");

    while !app.quit {
        if let Err(e) = event_loop.dispatch(std::time::Duration::from_millis(50), &mut app) {
            tc::log_error(&format!("이벤트 루프 오류: {e}"));
            break;
        }
        // GDBus 는 GMainContext 위에서 돈다. calloop 은 그걸 돌리지 않으므로
        // 여기서 직접 펌프해야 데몬 신호(모드 변경 등)가 도착한다.
        unim_test_common::daemon::pump();
        if unim_test_common::daemon::take_dirty() {
            app.dirty = true;
        }
        if app.dirty {
            let qh = app.qh.clone();
            app.draw(&qh);
        }
    }

    tc::log_note("종료");
    tc::log_shutdown();
}
