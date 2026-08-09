//! UNIM 테스트 앱 공용 코드 — Rust 껍데기
//!
//! `tests/common` 의 C 구현을 **그대로 링크**한다. 스펙·로그 형식·필드 동작을
//! Rust 로 다시 쓰지 않으므로 GTK·Qt·XIM 앱과 어긋날 수 없다.
//!
//! 설계 근거: docs/dev/testing/TEST_APPS.md §6·§7

use std::ffi::{c_char, c_int, c_uint, CStr, CString};

pub const FIELD_TEXT_MAX: usize = 4096;
pub const FIELD_PREEDIT_MAX: usize = 512;

/* ─── 스펙 ────────────────────────────────────────────────────────────── */

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SpecMetrics {
    pub win_width: c_int,
    pub win_height: c_int,
    pub margin: c_int,
    pub section_gap: c_int,
    pub row_gap: c_int,
    pub label_col_w: c_int,
    pub status_label_w: c_int,
    pub field_h: c_int,
    pub field_h_multi: c_int,
    pub field_pad_x: c_int,
    pub log_h: c_int,
    pub log_lines: c_int,
    pub font_size_ui: c_int,
    pub font_size_field: c_int,
    pub font_size_log: c_int,
    pub font_size_title: c_int,
    pub col_bg: c_uint,
    pub col_panel: c_uint,
    pub col_field_bg: c_uint,
    pub col_field_focus: c_uint,
    pub col_border: c_uint,
    pub col_border_focus: c_uint,
    pub col_text: c_uint,
    pub col_label: c_uint,
    pub col_preedit: c_uint,
    pub col_caret: c_uint,
    pub col_ok: c_uint,
    pub col_warn: c_uint,
    pub col_err: c_uint,
}

#[repr(C)]
pub struct SpecField {
    pub id: *const c_char,
    pub label: *const c_char,
    pub hint: c_int,
    pub purpose: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    None = 0,
    Number = 1,
    Password = 2,
    Search = 3,
    Multiline = 4,
}

/* ─── 필드 ────────────────────────────────────────────────────────────── */

/// C 의 `UnimTestField` 와 **바이트 단위로 같은 배치**여야 한다.
/// 순서를 바꾸면 조용히 깨지므로 `unim_test_field.h` 와 나란히 볼 것.
#[repr(C)]
pub struct Field {
    pub id: *const c_char,
    pub label: *const c_char,
    pub hint: c_int,

    pub committed: [c_char; FIELD_TEXT_MAX],
    pub caret: c_int,
    pub preedit: [c_char; FIELD_PREEDIT_MAX],
    pub preedit_caret: c_int,
    pub composing: c_int,

    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,

    pub focused: c_int,
}

pub type TextWidthFn =
    extern "C" fn(utf8: *const c_char, nbytes: usize, user: *mut std::ffi::c_void) -> c_int;

extern "C" {
    /* 스펙 */
    fn unim_spec_metrics() -> *const SpecMetrics;
    fn unim_spec_core_field(i: c_int) -> *const SpecField;
    fn unim_spec_n_core_fields() -> c_int;
    fn unim_spec_status_label(i: c_int) -> *const c_char;
    fn unim_spec_n_status() -> c_int;
    fn unim_spec_font_ui() -> *const c_char;
    fn unim_spec_font_mono() -> *const c_char;
    fn unim_spec_win_title_fmt() -> *const c_char;

    /* 로거 */
    fn unim_log_init(app: *const c_char, argc: c_int, argv: *const *const c_char);
    fn unim_log_ready();
    fn unim_log_shutdown();
    fn unim_log_env(toolkit_version: *const c_char);
    fn unim_log_key(
        phase: *const c_char, keyval: c_uint, keysym: c_uint, hw: c_uint,
        state: c_uint, s: *const c_char, filtered: c_int,
    );
    fn unim_log_im(phase: *const c_char, field: *const c_char,
                   result: *const c_char, elapsed_ms: f64);
    fn unim_log_click(x: c_int, y: c_int, field: *const c_char,
                      before: c_int, after: c_int);
    fn unim_log_reset(field: *const c_char, reason: *const c_char);
    fn unim_log_raw(ev: *const c_char, json_kv: *const c_char);
    fn unim_log_note_str(msg: *const c_char);
    fn unim_log_warn_str(msg: *const c_char);
    fn unim_log_error_str(msg: *const c_char);
    fn unim_log_preedit(phase: *const c_char, field: *const c_char,
                        text: *const c_char, cursor: c_int, attrs: *const c_char);
    fn unim_log_commit(field: *const c_char, text: *const c_char);
    fn unim_log_focus(phase: *const c_char, field: *const c_char, prev: *const c_char);
    fn unim_log_dbus(kind: *const c_char, iface: *const c_char, member: *const c_char,
                     detail: *const c_char, elapsed_ms: f64);
    fn unim_log_surrounding(kind: *const c_char, text: *const c_char, cursor: c_int,
                            offset: c_int, n_chars: c_int);
    fn unim_log_set_sink(sink: Option<LogSinkFn>, user: *mut std::ffi::c_void);

    /* 필드 엔진 */
    fn unim_field_init(f: *mut Field, spec: *const SpecField);
    fn unim_field_layout(fields: *mut Field, n: c_int, top: c_int,
                         width: c_int, scale: f64) -> c_int;
    // `unim_field_hit` 은 쓰지 않는다 — Rust 쪽 필드는 Box 라 메모리가 연속이
    // 아니어서 C 배열로 넘길 수 없다. 대신 아래 `hit()` 이 같은 판정을 한다
    // (단순 사각형 포함 검사라 어긋날 여지가 없다).
    fn unim_field_commit(f: *mut Field, text: *const c_char);
    fn unim_field_preedit_start(f: *mut Field);
    fn unim_field_set_preedit(f: *mut Field, text: *const c_char, cursor: c_int);
    fn unim_field_preedit_end(f: *mut Field);
    fn unim_field_backspace(f: *mut Field);
    fn unim_field_delete(f: *mut Field);
    fn unim_field_insert(f: *mut Field, text: *const c_char);
    fn unim_field_move_caret(f: *mut Field, dir: c_int);
    fn unim_field_caret_home(f: *mut Field);
    fn unim_field_caret_end(f: *mut Field);
    fn unim_field_clear(f: *mut Field);
    fn unim_field_set_focus(f: *mut Field, focused: c_int, prev_id: *const c_char);
    fn unim_field_rendered(f: *const Field, out: *mut c_char, n: usize) -> *const c_char;
    fn unim_field_display(f: *const Field, out: *mut c_char, n: usize) -> *const c_char;
    fn unim_field_before_caret(f: *const Field, out: *mut c_char, n: usize) -> *const c_char;
    fn unim_field_caret_from_x(f: *const Field, x: c_int, m: TextWidthFn,
                               user: *mut std::ffi::c_void) -> c_int;
    fn unim_field_log_render(f: *const Field);
}

/* ─── 안전한 껍데기 ───────────────────────────────────────────────────── */

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("<NUL 포함 문자열>").unwrap())
}

unsafe fn to_str<'a>(p: *const c_char) -> &'a str {
    if p.is_null() { "" } else { CStr::from_ptr(p).to_str().unwrap_or("") }
}

pub fn metrics() -> &'static SpecMetrics {
    unsafe { &*unim_spec_metrics() }
}

pub fn n_core_fields() -> usize {
    unsafe { unim_spec_n_core_fields() as usize }
}

pub fn status_labels() -> Vec<&'static str> {
    unsafe {
        (0..unim_spec_n_status())
            .map(|i| to_str(unim_spec_status_label(i)))
            .collect()
    }
}

pub fn font_ui() -> &'static str { unsafe { to_str(unim_spec_font_ui()) } }
pub fn font_mono() -> &'static str { unsafe { to_str(unim_spec_font_mono()) } }

/// 창 제목. 하네스가 이 문자열로 창을 찾으므로 스펙의 서식을 그대로 쓴다.
pub fn win_title(frontend: &str) -> String {
    unsafe { to_str(unim_spec_win_title_fmt()) }.replace("%s", frontend)
}

/* ── 로거 ── */

pub fn log_init(app: &str) {
    let argv: Vec<CString> = std::env::args().map(|a| cstr(&a)).collect();
    let ptrs: Vec<*const c_char> = argv.iter().map(|c| c.as_ptr()).collect();
    unsafe { unim_log_init(cstr(app).as_ptr(), ptrs.len() as c_int, ptrs.as_ptr()) }
}

pub fn log_env(toolkit_version: &str) {
    unsafe { unim_log_env(cstr(toolkit_version).as_ptr()) }
}

pub fn log_ready() { unsafe { unim_log_ready() } }
pub fn log_shutdown() { unsafe { unim_log_shutdown() } }

pub fn log_note(msg: &str) { unsafe { unim_log_note_str(cstr(msg).as_ptr()) } }
pub fn log_warn(msg: &str) { unsafe { unim_log_warn_str(cstr(msg).as_ptr()) } }
pub fn log_error(msg: &str) { unsafe { unim_log_error_str(cstr(msg).as_ptr()) } }

pub fn log_key(phase: &str, keysym: u32, hw: u32, state: u32,
               text: Option<&str>, filtered: i32) {
    let t = text.map(cstr);
    unsafe {
        unim_log_key(cstr(phase).as_ptr(), keysym, keysym, hw, state,
                     t.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()), filtered)
    }
}

pub fn log_im(phase: &str, field: &str, result: &str, elapsed_ms: f64) {
    unsafe {
        unim_log_im(cstr(phase).as_ptr(), cstr(field).as_ptr(),
                    cstr(result).as_ptr(), elapsed_ms)
    }
}

pub fn log_click(x: i32, y: i32, field: &str, before: i32, after: i32) {
    unsafe { unim_log_click(x, y, cstr(field).as_ptr(), before, after) }
}

pub fn log_reset(field: &str, reason: &str) {
    unsafe { unim_log_reset(cstr(field).as_ptr(), cstr(reason).as_ptr()) }
}

pub fn log_raw(ev: &str, json_kv: &str) {
    unsafe { unim_log_raw(cstr(ev).as_ptr(), cstr(json_kv).as_ptr()) }
}

pub fn log_preedit(phase: &str, field: &str, text: &str, cursor: i32, attrs: Option<&str>) {
    let a = attrs.map(cstr);
    unsafe {
        unim_log_preedit(cstr(phase).as_ptr(), cstr(field).as_ptr(), cstr(text).as_ptr(),
                         cursor, a.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()))
    }
}

pub fn log_commit(field: &str, text: &str) {
    unsafe { unim_log_commit(cstr(field).as_ptr(), cstr(text).as_ptr()) }
}

pub fn log_focus(phase: &str, field: &str, prev: Option<&str>) {
    let p = prev.map(cstr);
    unsafe {
        unim_log_focus(cstr(phase).as_ptr(), cstr(field).as_ptr(),
                       p.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()))
    }
}

pub fn log_dbus(kind: &str, iface: &str, member: &str, detail: &str, elapsed_ms: f64) {
    unsafe {
        unim_log_dbus(cstr(kind).as_ptr(), cstr(iface).as_ptr(), cstr(member).as_ptr(),
                      cstr(detail).as_ptr(), elapsed_ms)
    }
}

pub fn log_surrounding(kind: &str, text: &str, cursor: i32, offset: i32, n_chars: i32) {
    unsafe {
        unim_log_surrounding(cstr(kind).as_ptr(), cstr(text).as_ptr(), cursor, offset, n_chars)
    }
}

/* ── 로그 패널 연결 ── */

type LogSinkFn = extern "C" fn(line: *const c_char, user: *mut std::ffi::c_void);

thread_local! {
    static SINK: std::cell::RefCell<Option<fn(&str)>> = const { std::cell::RefCell::new(None) };
}

extern "C" fn sink_trampoline(line: *const c_char, _user: *mut std::ffi::c_void) {
    let s = unsafe { to_str(line) }.to_string();
    // 콜백 안에서 다시 로그를 남길 수 있으므로 borrow 를 먼저 풀고 부른다.
    let cb = SINK.with(|c| *c.borrow());
    if let Some(cb) = cb {
        cb(&s);
    }
}

/// 사건이 생길 때마다 사람이 읽는 한 줄을 받는다 — 앱 로그 패널용.
pub fn set_log_sink(cb: fn(&str)) {
    SINK.with(|c| *c.borrow_mut() = Some(cb));
    unsafe { unim_log_set_sink(Some(sink_trampoline), std::ptr::null_mut()) }
}

/* ── 필드 ── */

impl Field {
    /// 스펙 i 번 필드로 초기화된 값을 만든다.
    pub fn new(i: usize) -> Box<Field> {
        let mut f: Box<Field> = unsafe { Box::new(std::mem::zeroed()) };
        unsafe { unim_field_init(&mut *f, unim_spec_core_field(i as c_int)) };
        f
    }

    pub fn id(&self) -> &str { unsafe { to_str(self.id) } }
    pub fn label(&self) -> &str { unsafe { to_str(self.label) } }
    pub fn hint(&self) -> Hint {
        match self.hint {
            1 => Hint::Number,
            2 => Hint::Password,
            3 => Hint::Search,
            4 => Hint::Multiline,
            _ => Hint::None,
        }
    }

    pub fn commit(&mut self, text: &str) {
        unsafe { unim_field_commit(self, cstr(text).as_ptr()) }
    }
    pub fn preedit_start(&mut self) { unsafe { unim_field_preedit_start(self) } }
    pub fn set_preedit(&mut self, text: &str, cursor: i32) {
        unsafe { unim_field_set_preedit(self, cstr(text).as_ptr(), cursor) }
    }
    pub fn preedit_end(&mut self) { unsafe { unim_field_preedit_end(self) } }
    pub fn backspace(&mut self) { unsafe { unim_field_backspace(self) } }
    pub fn delete(&mut self) { unsafe { unim_field_delete(self) } }
    pub fn insert(&mut self, text: &str) {
        unsafe { unim_field_insert(self, cstr(text).as_ptr()) }
    }
    pub fn move_caret(&mut self, dir: i32) { unsafe { unim_field_move_caret(self, dir) } }
    pub fn caret_home(&mut self) { unsafe { unim_field_caret_home(self) } }
    pub fn caret_end(&mut self) { unsafe { unim_field_caret_end(self) } }
    pub fn clear(&mut self) { unsafe { unim_field_clear(self) } }
    pub fn set_focus(&mut self, focused: bool, prev_id: Option<&str>) {
        let p = prev_id.map(cstr);
        unsafe {
            unim_field_set_focus(self, focused as c_int,
                                 p.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()))
        }
    }
    pub fn log_render(&self) { unsafe { unim_field_log_render(self) } }

    fn read_buf(&self, f: unsafe extern "C" fn(*const Field, *mut c_char, usize) -> *const c_char)
        -> String {
        let mut buf = vec![0i8; FIELD_TEXT_MAX + FIELD_PREEDIT_MAX];
        unsafe {
            f(self, buf.as_mut_ptr(), buf.len());
            to_str(buf.as_ptr()).to_string()
        }
    }

    /// 화면에 실제로 나타나는 문자열 (확정 + preedit 삽입).
    pub fn rendered(&self) -> String { self.read_buf(unim_field_rendered) }
    /// 화면 표시용 — 비밀번호면 마스킹된 것.
    pub fn display(&self) -> String { self.read_buf(unim_field_display) }
    /// 캐럿 앞부분 (캐럿 x 좌표 계산용).
    pub fn before_caret(&self) -> String { self.read_buf(unim_field_before_caret) }

    pub fn committed_str(&self) -> String {
        unsafe { to_str(self.committed.as_ptr()).to_string() }
    }
    pub fn preedit_str(&self) -> String {
        unsafe { to_str(self.preedit.as_ptr()).to_string() }
    }
}

/// 코어 필드 6개를 스펙대로 배치한다. 반환값은 마지막 필드 아래 y.
pub fn layout(fields: &mut [Box<Field>], top: i32, width: i32, scale: f64) -> i32 {
    // Box 배열은 연속이 아니므로 C 배열로 옮겨 부를 수 없다.
    // 대신 필드마다 한 칸짜리 배열로 계산해 같은 결과를 얻는다.
    let mut y = top;
    for f in fields.iter_mut() {
        let next = unsafe { unim_field_layout(&mut **f, 1, y, width, scale) };
        y = next;
    }
    y
}

/// (x, y) 를 품는 필드 index. 없으면 None.
pub fn hit(fields: &[Box<Field>], x: i32, y: i32) -> Option<usize> {
    fields.iter().position(|f| {
        x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h
    })
}

/// 클릭 x 좌표 → 캐럿 바이트 오프셋. 폭 측정은 앱이 콜백으로 준다.
///
/// # Safety
/// `user` 는 `m` 이 해석할 수 있는 유효한 포인터여야 하고, `m` 이 실행되는
/// 동안 살아 있어야 한다. `m` 안에서 이 함수를 다시 부르면 안 된다.
pub unsafe fn caret_from_x(f: &Field, x: i32, m: TextWidthFn,
                           user: *mut std::ffi::c_void) -> i32 {
    unim_field_caret_from_x(f, x, m, user)
}

/* ─── 데몬 연결·상태 패널 (`dbus` 기능) ───────────────────────────────── */

/// `unim_test_dbus.c` 를 그대로 부른다. 상태 패널 문구를 Rust 로 옮겨 적지
/// 않는 것이 요점 — 6개 앱이 같은 함수에서 같은 문자열을 얻는다.
#[cfg(feature = "dbus")]
pub mod daemon {
    use super::{cstr, to_str};
    use std::ffi::{c_char, c_int, c_void};

    pub const STATUS_VALUE_MAX: usize = 256;

    #[repr(C)]
    struct CDaemon {
        _opaque: [u8; 0],
    }

    #[repr(C)]
    struct StatusInput {
        frontend: *const c_char,
        im_path: *const c_char,
        focus_field: *const c_char,
        preedit: *const c_char,
        preedit_caret: c_int,
        last_commit: *const c_char,
    }

    type ChangedFn = extern "C" fn(user: *mut c_void);

    extern "C" {
        fn unim_daemon_connect(cb: Option<ChangedFn>, user: *mut c_void) -> *mut CDaemon;
        fn unim_daemon_free(d: *mut CDaemon);
        fn unim_daemon_connected(d: *const CDaemon) -> c_int;
        fn unim_daemon_korean(d: *const CDaemon) -> c_int;
        fn unim_daemon_layout(d: *const CDaemon) -> *const c_char;
        fn unim_daemon_error(d: *const CDaemon) -> *const c_char;
        fn unim_daemon_toggle(d: *mut CDaemon);
        fn unim_daemon_refresh(d: *mut CDaemon);
        fn unim_status_render(d: *const CDaemon, input: *const StatusInput, out: *mut c_char);
        fn unim_status_im_path(frontend: *const c_char) -> *const c_char;

        fn g_main_context_iteration(ctx: *mut c_void, may_block: c_int) -> c_int;
        fn g_main_context_pending(ctx: *mut c_void) -> c_int;
    }

    thread_local! {
        /// 데몬이 모드 변경을 알려왔다는 표시. 콜백 안에서 앱 상태를 만지면
        /// 재진입이 생기므로 깃발만 세우고 앱이 다음 차례에 읽어간다.
        static DIRTY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    extern "C" fn on_changed(_user: *mut c_void) {
        DIRTY.with(|d| d.set(true));
    }

    /// 데몬이 상태 변화를 알려왔는가 (읽으면 깃발이 내려간다).
    pub fn take_dirty() -> bool {
        DIRTY.with(|d| d.replace(false))
    }

    pub struct Daemon(*mut CDaemon);

    impl Daemon {
        /// 데몬이 없어도 성공한다 — 연결 실패 상태의 객체가 온다.
        pub fn connect() -> Self {
            Daemon(unsafe { unim_daemon_connect(Some(on_changed), std::ptr::null_mut()) })
        }

        pub fn connected(&self) -> bool { unsafe { unim_daemon_connected(self.0) != 0 } }
        pub fn korean(&self) -> bool { unsafe { unim_daemon_korean(self.0) != 0 } }
        pub fn layout(&self) -> &str { unsafe { to_str(unim_daemon_layout(self.0)) } }
        pub fn error(&self) -> &str { unsafe { to_str(unim_daemon_error(self.0)) } }
        pub fn toggle(&mut self) { unsafe { unim_daemon_toggle(self.0) } }
        pub fn refresh(&mut self) { unsafe { unim_daemon_refresh(self.0) } }

        /// 상태 패널 6줄의 **값** 문자열 (라벨은 `status_labels()`).
        #[allow(clippy::too_many_arguments)]
        pub fn status(&self, frontend: &str, focus_field: &str, preedit: &str,
                      preedit_caret: i32, last_commit: &str) -> Vec<String> {
            let (f, ff, p, lc) = (cstr(frontend), cstr(focus_field),
                                  cstr(preedit), cstr(last_commit));
            let input = StatusInput {
                frontend: f.as_ptr(),
                im_path: std::ptr::null(),
                focus_field: ff.as_ptr(),
                preedit: p.as_ptr(),
                preedit_caret,
                last_commit: lc.as_ptr(),
            };
            let n = super::status_labels().len();
            let mut buf = vec![0i8; n * STATUS_VALUE_MAX];
            unsafe { unim_status_render(self.0, &input, buf.as_mut_ptr()) };
            (0..n)
                .map(|i| unsafe { to_str(buf.as_ptr().add(i * STATUS_VALUE_MAX)).to_string() })
                .collect()
        }
    }

    impl Drop for Daemon {
        fn drop(&mut self) {
            unsafe { unim_daemon_free(self.0) }
        }
    }

    /// 이 프로세스에서 결정된 IM 경로 요약.
    pub fn im_path(frontend: &str) -> &'static str {
        unsafe { to_str(unim_status_im_path(cstr(frontend).as_ptr())) }
    }

    /// GDBus 시그널을 받아 처리한다. Wayland/calloop 는 GMainContext 를 돌리지
    /// 않으므로 앱이 주기적으로 이걸 불러야 데몬 신호가 도착한다 (Qt 앱의
    /// QTimer 펌프와 같은 이유).
    pub fn pump() {
        unsafe {
            let mut guard = 0;
            while g_main_context_pending(std::ptr::null_mut()) != 0 && guard < 64 {
                g_main_context_iteration(std::ptr::null_mut(), 0);
                guard += 1;
            }
        }
    }
}
