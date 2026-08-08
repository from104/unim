/**
 * UNIM GNOME 테스트 앱
 *
 * GTK4 판과 **화면이 완전히 같다** — 같은 `unim_test_spec.h` 를 쓰고 상태
 * 패널 구성도 건드리지 않는다. 다른 것은 딱 하나, 이 앱은 `GTK_IM_MODULE` 을
 * 비운 채 Wayland 네이티브로 떠서 **GNOME Shell 확장 경로**
 * (Clutter.InputMethod + text-input-v3)를 탄다는 것뿐이다.
 *
 * 확장 상태 진단은 화면에 넣지 않고 **로그로만** 남긴다. 화면을 앱마다 다르게
 * 만들면 앱 간 대조가 깨지기 때문이다(TEST_APPS.md §3).
 *
 * ⚠️ 이 앱에는 XTEST 키 주입이 닿지 않는다(Wayland 네이티브). 하네스가
 * 자동으로 건너뛴다 — 검증은 수동이다(TEST_APPS.md §9).
 *
 * 실행:
 *   unim-test-gnome              GTK_IM_MODULE 을 비우고 띄울 것
 *   unim-test-gnome --auto       DBus 스모크만 돌리고 종료
 */

#include <gtk/gtk.h>
#include <string.h>

#include "unim_test.h"
#include "unim_test_dbus.h"
#include "unim_test_field.h"
#include "unim_test_log.h"
#include "unim_test_spec.h"

#define APP_NAME "gnome"

/* ─── 앱 상태 ─────────────────────────────────────────────────────────── */

static struct {
    GtkWidget      *window;
    GtkWidget      *canvas;
    GtkTextBuffer  *log_buf;
    GtkWidget      *log_view;
    GtkWidget      *status_val[UNIM_STATUS_N];
    GtkIMContext   *im;
    UnimTestDaemon *daemon;
    GMainLoop      *loop;

    UnimTestField   fields[UNIM_SPEC_N_CORE_FIELDS];
    int             active;
    int             canvas_focused;

    double          scale;
    char            last_commit[512];
    PangoLayout    *measure;
} A;

static UnimTestField *cur(void) { return &A.fields[A.active]; }

static void set_rgb(cairo_t *cr, unsigned rgb) {
    cairo_set_source_rgb(cr, ((rgb >> 16) & 0xff) / 255.0,
                             ((rgb >> 8)  & 0xff) / 255.0,
                             ( rgb        & 0xff) / 255.0);
}

static int S(int v) { return (int)(v * A.scale + 0.5); }

/* ─── 로그 패널 ───────────────────────────────────────────────────────── */

static void log_sink(const char *line, void *user) {
    (void)user;
    if (!A.log_buf) return;

    GtkTextIter end;
    gtk_text_buffer_get_end_iter(A.log_buf, &end);
    gtk_text_buffer_insert(A.log_buf, &end, line, -1);
    gtk_text_buffer_insert(A.log_buf, &end, "\n", -1);

    int n = gtk_text_buffer_get_line_count(A.log_buf);
    if (n > UNIM_SPEC_LOG_LINES) {
        GtkTextIter s, e;
        gtk_text_buffer_get_start_iter(A.log_buf, &s);
        gtk_text_buffer_get_iter_at_line(A.log_buf, &e, n - UNIM_SPEC_LOG_LINES);
        gtk_text_buffer_delete(A.log_buf, &s, &e);
    }
    if (A.log_view) {
        GtkTextMark *mark = gtk_text_buffer_get_insert(A.log_buf);
        gtk_text_buffer_get_end_iter(A.log_buf, &end);
        gtk_text_buffer_move_mark(A.log_buf, mark, &end);
        gtk_text_view_scroll_mark_onscreen(GTK_TEXT_VIEW(A.log_view), mark);
    }
}

/* ─── 상태 패널 ───────────────────────────────────────────────────────── */

static void refresh_status(void) {
    if (!A.status_val[0]) return;
    UnimStatusInput in = {
        .frontend      = APP_NAME,
        .im_path       = NULL,
        .focus_field   = A.canvas_focused ? cur()->id : "(네이티브 위젯)",
        .preedit       = cur()->preedit,
        .preedit_caret = cur()->preedit_caret,
        .last_commit   = A.last_commit,
    };
    char vals[UNIM_STATUS_N][UNIM_STATUS_VALUE_MAX];
    unim_status_render(A.daemon, &in, vals);
    for (int i = 0; i < UNIM_STATUS_N; i++)
        gtk_label_set_text(GTK_LABEL(A.status_val[i]), vals[i]);
}

static void on_daemon_changed(void *user) { (void)user; refresh_status(); }

/* ─── GNOME 확장 경로 진단 ────────────────────────────────────────────── */

/**
 * 확장이 실제로 켜져 있는지 GNOME Shell 에 물어 로그에 남긴다.
 * 화면에는 넣지 않는다 — 앱마다 화면이 달라지면 대조가 깨진다.
 */
static void diagnose_extension_path(void) {
    const char *im   = g_getenv("GTK_IM_MODULE");
    const char *wl   = g_getenv("WAYLAND_DISPLAY");
    const char *sess = g_getenv("XDG_SESSION_TYPE");

    if (im && *im)
        unim_log_warn("GTK_IM_MODULE=%s 가 설정돼 있다 — 이 앱은 그 모듈 경로로 "
                      "가버려서 GNOME 확장 경로를 시험하지 못한다. 비우고 띄울 것", im);
    else
        unim_log_note("GTK_IM_MODULE 없음 → GNOME Shell IME 경로로 간다");

    if (!wl || !*wl)
        unim_log_warn("WAYLAND_DISPLAY 가 없다 — X11 세션이면 확장 경로가 아니다 "
                      "(session=%s)", sess ? sess : "?");

    GError *err = NULL;
    GDBusProxy *p = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION, G_DBUS_PROXY_FLAGS_NONE, NULL,
        "org.gnome.Shell", "/org/gnome/Shell",
        "org.gnome.Shell.Extensions", NULL, &err);
    if (!p) {
        unim_log_warn("GNOME Shell Extensions DBus 접근 실패: %s",
                      err ? err->message : "?");
        if (err) g_error_free(err);
        return;
    }

    GVariant *r = g_dbus_proxy_call_sync(
        p, "GetExtensionInfo",
        g_variant_new("(s)", "unim-gnome@from104.github.io"),
        G_DBUS_CALL_FLAGS_NONE, 3000, NULL, &err);
    if (!r) {
        unim_log_warn("확장 정보 조회 실패: %s", err ? err->message : "?");
        if (err) g_error_free(err);
        g_object_unref(p);
        return;
    }

    GVariant *dict = g_variant_get_child_value(r, 0);
    double state = 0;
    const char *ver = NULL;
    g_variant_lookup(dict, "state", "d", &state);
    g_variant_lookup(dict, "version", "&s", &ver);
    /* state 1 = ENABLED (GNOME Shell 의 ExtensionState) */
    if (state == 1.0)
        unim_log_note("GNOME 확장 활성 (version=%s)", ver ? ver : "?");
    else
        unim_log_warn("GNOME 확장이 활성이 아니다 (state=%.0f) — 조합이 아예 "
                      "안 될 수 있다", state);

    g_variant_unref(dict);
    g_variant_unref(r);
    g_object_unref(p);
}

/* ─── 폰트·측정 ───────────────────────────────────────────────────────── */

static PangoFontDescription *font_of(int size) {
    static PangoFontDescription *field_fd, *label_fd;
    PangoFontDescription **slot =
        (size == UNIM_SPEC_FONT_SIZE_FIELD) ? &field_fd : &label_fd;
    if (!*slot) {
        char s[128];
        g_snprintf(s, sizeof s, "%s %d", UNIM_SPEC_FONT_UI,
                   (int)(size * A.scale));
        *slot = pango_font_description_from_string(s);
    }
    return *slot;
}

static int measure_cb(const char *utf8, size_t nbytes, void *user) {
    (void)user;
    if (!A.measure) return 0;
    pango_layout_set_text(A.measure, utf8, (int)nbytes);
    int w, h;
    pango_layout_get_pixel_size(A.measure, &w, &h);
    return w;
}

/* ─── IM 커서 위치 ────────────────────────────────────────────────────── */

static void update_cursor_location(void) {
    if (!A.im || !A.canvas_focused) return;
    UnimTestField *f = cur();
    char before[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_before_caret(f, before, sizeof before);

    GdkRectangle r = {
        .x = f->x + S(UNIM_SPEC_FIELD_PAD_X) + measure_cb(before, strlen(before), NULL),
        .y = f->y, .width = 2, .height = f->h,
    };
    gtk_im_context_set_cursor_location(A.im, &r);
}

static void after_change(void) {
    update_cursor_location();
    refresh_status();
    if (A.canvas) gtk_widget_queue_draw(A.canvas);
}

/* ─── IM 시그널 ───────────────────────────────────────────────────────── */

static void on_commit(GtkIMContext *im, const char *text, gpointer u) {
    (void)im; (void)u;
    g_snprintf(A.last_commit, sizeof A.last_commit, "%s", text ? text : "");
    unim_field_commit(cur(), text);
    after_change();
}

static void on_preedit_start(GtkIMContext *im, gpointer u) {
    (void)im; (void)u;
    unim_field_preedit_start(cur());
    after_change();
}

static void on_preedit_changed(GtkIMContext *im, gpointer u) {
    (void)u;
    char *text = NULL;
    PangoAttrList *attrs = NULL;
    int cursor = 0;
    gtk_im_context_get_preedit_string(im, &text, &attrs, &cursor);

    /* cursor 는 문자 수 단위 — 필드 엔진은 바이트를 쓴다. */
    int byte_cursor = text ? (int)(g_utf8_offset_to_pointer(text, cursor) - text) : 0;
    unim_field_set_preedit(cur(), text ? text : "", byte_cursor);

    g_free(text);
    if (attrs) pango_attr_list_unref(attrs);
    after_change();
}

static void on_preedit_end(GtkIMContext *im, gpointer u) {
    (void)im; (void)u;
    unim_field_preedit_end(cur());
    after_change();
}

static gboolean on_retrieve_surrounding(GtkIMContext *im, gpointer u) {
    (void)u;
    UnimTestField *f = cur();
    unim_log_surrounding("retrieve", f->committed, f->caret, 0, 0);
    /* GTK4 는 선택 영역까지 받는 쪽을 쓴다. 테스트 앱은 선택이 없으므로
     * anchor 를 캐럿과 같게 준다. */
    gtk_im_context_set_surrounding_with_selection(im, f->committed, -1,
                                                  f->caret, f->caret);
    return TRUE;
}

static gboolean on_delete_surrounding(GtkIMContext *im, int offset,
                                      int n_chars, gpointer u) {
    (void)im; (void)u;
    UnimTestField *f = cur();
    unim_log_surrounding("delete", f->committed, f->caret, offset, n_chars);
    for (int i = 0; i < -offset; i++) unim_field_move_caret(f, -1);
    for (int i = 0; i <  offset; i++) unim_field_move_caret(f, +1);
    for (int i = 0; i < n_chars; i++) unim_field_delete(f);
    after_change();
    return TRUE;
}

/* ─── 필드 전환 ───────────────────────────────────────────────────────── */

static void focus_field(int idx, const char *reason) {
    if (idx == A.active) return;
    UnimTestField *old = cur();
    const char *prev_id = old->id;

    if (old->composing || old->preedit[0]) {
        unim_log_reset(old->id, reason);
        gtk_im_context_reset(A.im);
    }
    unim_field_set_focus(old, 0, NULL);
    A.active = idx;
    unim_field_set_focus(cur(), 1, prev_id);
    after_change();
}

/* ─── 그리기 ──────────────────────────────────────────────────────────── */

static void draw_field(cairo_t *cr, const UnimTestField *f, PangoLayout *lay) {
    gboolean focused = A.canvas_focused && f->focused;

    set_rgb(cr, UNIM_SPEC_COL_LABEL);
    pango_layout_set_font_description(lay, font_of(UNIM_SPEC_FONT_SIZE_UI));
    pango_layout_set_width(lay, -1);
    pango_layout_set_text(lay, f->label, -1);
    cairo_move_to(cr, S(UNIM_SPEC_MARGIN), f->y + S(8));
    pango_cairo_show_layout(cr, lay);

    set_rgb(cr, focused ? UNIM_SPEC_COL_FIELD_FOCUS : UNIM_SPEC_COL_FIELD_BG);
    cairo_rectangle(cr, f->x, f->y, f->w, f->h);
    cairo_fill(cr);
    set_rgb(cr, focused ? UNIM_SPEC_COL_BORDER_FOCUS : UNIM_SPEC_COL_BORDER);
    cairo_set_line_width(cr, focused ? 2.0 : 1.0);
    cairo_rectangle(cr, f->x + 0.5, f->y + 0.5, f->w - 1, f->h - 1);
    cairo_stroke(cr);

    char shown[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_display(f, shown, sizeof shown);

    pango_layout_set_font_description(lay, font_of(UNIM_SPEC_FONT_SIZE_FIELD));
    pango_layout_set_text(lay, shown, -1);
    if (f->hint == UNIM_HINT_MULTILINE) {
        pango_layout_set_width(lay, (f->w - 2 * S(UNIM_SPEC_FIELD_PAD_X)) * PANGO_SCALE);
        pango_layout_set_wrap(lay, PANGO_WRAP_WORD_CHAR);
    } else {
        pango_layout_set_width(lay, -1);
    }

    int pre_len = (int)strlen(f->preedit);
    if (pre_len > 0) {
        int start, len;
        if (f->hint == UNIM_HINT_PASSWORD) {
            char head[UNIM_FIELD_TEXT_MAX];
            g_snprintf(head, sizeof head, "%.*s", f->caret, f->committed);
            start = (int)unim_log_utf8_len(head) * 3;
            len   = (int)unim_log_utf8_len(f->preedit) * 3;
        } else {
            start = f->caret;
            len   = pre_len;
        }
        PangoAttrList *al = pango_attr_list_new();
        PangoAttribute *ul = pango_attr_underline_new(PANGO_UNDERLINE_SINGLE);
        ul->start_index = (guint)start; ul->end_index = (guint)(start + len);
        pango_attr_list_insert(al, ul);
        PangoAttribute *fg = pango_attr_foreground_new(
            ((UNIM_SPEC_COL_PREEDIT >> 16) & 0xff) * 257,
            ((UNIM_SPEC_COL_PREEDIT >> 8)  & 0xff) * 257,
            ( UNIM_SPEC_COL_PREEDIT        & 0xff) * 257);
        fg->start_index = (guint)start; fg->end_index = (guint)(start + len);
        pango_attr_list_insert(al, fg);
        pango_layout_set_attributes(lay, al);
        pango_attr_list_unref(al);
    } else {
        pango_layout_set_attributes(lay, NULL);
    }

    set_rgb(cr, UNIM_SPEC_COL_TEXT);
    int tx = f->x + S(UNIM_SPEC_FIELD_PAD_X);
    int ty = (f->hint == UNIM_HINT_MULTILINE) ? f->y + S(6) : f->y + S(8);
    cairo_move_to(cr, tx, ty);
    pango_cairo_show_layout(cr, lay);

    if (focused) {
        int idx;
        if (f->hint == UNIM_HINT_PASSWORD) {
            char head[UNIM_FIELD_TEXT_MAX], phead[UNIM_FIELD_PREEDIT_MAX];
            g_snprintf(head, sizeof head, "%.*s", f->caret, f->committed);
            g_snprintf(phead, sizeof phead, "%.*s", f->preedit_caret, f->preedit);
            idx = (int)(unim_log_utf8_len(head) + unim_log_utf8_len(phead)) * 3;
        } else {
            idx = f->caret + f->preedit_caret;
        }
        PangoRectangle pos;
        pango_layout_index_to_pos(lay, idx, &pos);
        set_rgb(cr, UNIM_SPEC_COL_CARET);
        cairo_rectangle(cr, tx + pos.x / PANGO_SCALE, ty + pos.y / PANGO_SCALE,
                        S(2), pos.height / PANGO_SCALE);
        cairo_fill(cr);
    }
    pango_layout_set_attributes(lay, NULL);
}

static void draw_func(GtkDrawingArea *da, cairo_t *cr, int w, int h, gpointer u) {
    (void)da; (void)w; (void)h; (void)u;
    set_rgb(cr, UNIM_SPEC_COL_BG);
    cairo_paint(cr);

    PangoLayout *lay = pango_cairo_create_layout(cr);
    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        draw_field(cr, &A.fields[i], lay);
    g_object_unref(lay);
}

/* ─── 입력 ────────────────────────────────────────────────────────────── */

static gboolean on_key_pressed(GtkEventControllerKey *kc, guint keyval,
                               guint keycode, GdkModifierType state, gpointer u) {
    (void)u;
    char ubuf[8] = "";
    const char *utf8 = NULL;
    guint32 uc = gdk_keyval_to_unicode(keyval);
    if (uc >= 0x20 && uc != 0x7f) {
        int n = g_unichar_to_utf8((gunichar)uc, ubuf);
        ubuf[n] = '\0';
        utf8 = ubuf;
    }
    unim_log_key("press", keyval, keyval, keycode, state, utf8, -1);

    if (keyval == GDK_KEY_Tab || keyval == GDK_KEY_ISO_Left_Tab) {
        int dir = (state & GDK_SHIFT_MASK) ? -1 : 1;
        focus_field((A.active + dir + UNIM_SPEC_N_CORE_FIELDS)
                        % UNIM_SPEC_N_CORE_FIELDS, "tab");
        return TRUE;
    }

    GdkEvent *ev = gtk_event_controller_get_current_event(GTK_EVENT_CONTROLLER(kc));
    gint64 t0 = g_get_monotonic_time();
    unim_log_im("enter", cur()->id, "filter_keypress", 0);
    gboolean filtered = ev ? gtk_im_context_filter_keypress(A.im, ev) : FALSE;
    double ms = (double)(g_get_monotonic_time() - t0) / 1000.0;
    unim_log_im("leave", cur()->id, filtered ? "IM 삼킴" : "앱으로", ms);
    if (filtered) return TRUE;

    UnimTestField *f = cur();
    switch (keyval) {
        case GDK_KEY_BackSpace: unim_field_backspace(f);      break;
        case GDK_KEY_Delete:    unim_field_delete(f);         break;
        case GDK_KEY_Left:      unim_field_move_caret(f, -1); break;
        case GDK_KEY_Right:     unim_field_move_caret(f, +1); break;
        case GDK_KEY_Home:      unim_field_caret_home(f);     break;
        case GDK_KEY_End:       unim_field_caret_end(f);      break;
        case GDK_KEY_Escape:    unim_field_clear(f);          break;
        case GDK_KEY_Return:
        case GDK_KEY_KP_Enter:
            if (f->hint == UNIM_HINT_MULTILINE) unim_field_insert(f, "\n");
            else focus_field((A.active + 1) % UNIM_SPEC_N_CORE_FIELDS, "enter");
            break;
        default:
            if (utf8) unim_field_insert(f, utf8);
            else unim_log_note("%s: 처리하지 않은 키 keyval=0x%x", f->id, keyval);
    }
    after_change();
    return TRUE;
}

static void on_click(GtkGestureClick *g, int n_press, double x, double y,
                     gpointer u) {
    (void)g; (void)n_press; (void)u;
    gtk_widget_grab_focus(A.canvas);

    int hit = unim_field_hit(A.fields, UNIM_SPEC_N_CORE_FIELDS, (int)x, (int)y);
    if (hit < 0) {
        unim_log_click((int)x, (int)y, "(빈 곳)", -1, -1);
        return;
    }
    if (hit != A.active) {
        focus_field(hit, "click");
    } else if (cur()->composing || cur()->preedit[0]) {
        unim_log_reset(cur()->id, "click-in-field");
        gtk_im_context_reset(A.im);
    }

    UnimTestField *f = cur();
    int before = f->caret;
    f->caret = unim_field_caret_from_x(f, (int)x, measure_cb, NULL);
    unim_log_click((int)x, (int)y, f->id, before, f->caret);
    unim_field_log_render(f);
    after_change();
}

static void on_focus_enter(GtkEventControllerFocus *c, gpointer u) {
    (void)c; (void)u;
    A.canvas_focused = 1;
    gtk_im_context_focus_in(A.im);
    unim_field_set_focus(cur(), 1, "(네이티브 위젯)");
    after_change();
}

static void on_focus_leave(GtkEventControllerFocus *c, gpointer u) {
    (void)c; (void)u;
    A.canvas_focused = 0;
    unim_log_reset(cur()->id, "canvas-focus-out");
    gtk_im_context_focus_out(A.im);
    unim_field_set_focus(cur(), 0, NULL);
    after_change();
}

/* ─── 네이티브 위젯 ───────────────────────────────────────────────────── */

static void on_native_changed(GtkEditable *e, gpointer u) {
    const char *t = gtk_editable_get_text(e);
    /* GtkEntry 는 preedit 을 앱에 안 준다 — 확정된 내용만 관측된다. */
    unim_log_field_render((const char *)u, t, "", (int)strlen(t), t);
}

static void on_native_focus(GtkEventControllerFocus *c, gpointer u) {
    (void)c;
    unim_log_focus("in", (const char *)u, "(코어 필드)");
}

static GtkWidget *make_native_row(const UnimSpecNative *spec) {
    GtkWidget *row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, S(10));
    GtkWidget *lab = gtk_label_new(spec->label);
    gtk_label_set_xalign(GTK_LABEL(lab), 0);
    gtk_widget_set_size_request(lab, S(UNIM_SPEC_LABEL_COL_W), -1);
    gtk_box_append(GTK_BOX(row), lab);

    GtkWidget *wid;
    if (spec->kind == UNIM_NATIVE_MULTILINE) {
        wid = gtk_text_view_new();
        GtkWidget *sc = gtk_scrolled_window_new();
        gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(sc), wid);
        gtk_widget_set_size_request(sc, -1, S(60));
        gtk_widget_set_hexpand(sc, TRUE);
        gtk_box_append(GTK_BOX(row), sc);
    } else {
        wid = gtk_entry_new();
        if (spec->kind == UNIM_NATIVE_PASSWORD)
            gtk_entry_set_visibility(GTK_ENTRY(wid), FALSE);
        g_signal_connect(wid, "changed", G_CALLBACK(on_native_changed),
                         (gpointer)spec->id);
        gtk_widget_set_hexpand(wid, TRUE);
        gtk_box_append(GTK_BOX(row), wid);
    }
    gtk_widget_set_name(wid, spec->id);

    GtkEventController *fc = gtk_event_controller_focus_new();
    g_signal_connect(fc, "enter", G_CALLBACK(on_native_focus),
                     (gpointer)spec->id);
    gtk_widget_add_controller(wid, fc);
    return row;
}

/* ─── UI 조립 ─────────────────────────────────────────────────────────── */

static GtkWidget *section_title(const char *text) {
    GtkWidget *l = gtk_label_new(NULL);
    char *m = g_markup_printf_escaped("<b>%s</b>", text);
    gtk_label_set_markup(GTK_LABEL(l), m);
    g_free(m);
    gtk_label_set_xalign(GTK_LABEL(l), 0);
    return l;
}

static GtkWidget *build_status_panel(void) {
    GtkWidget *grid = gtk_grid_new();
    gtk_grid_set_column_spacing(GTK_GRID(grid), S(12));
    gtk_grid_set_row_spacing(GTK_GRID(grid), S(4));
    for (int i = 0; i < UNIM_STATUS_N; i++) {
        GtkWidget *key = gtk_label_new(UNIM_SPEC_STATUS_LABELS[i]);
        gtk_label_set_xalign(GTK_LABEL(key), 0);
        gtk_widget_set_size_request(key, S(UNIM_SPEC_STATUS_LABEL_W), -1);
        gtk_grid_attach(GTK_GRID(grid), key, 0, i, 1, 1);

        A.status_val[i] = gtk_label_new("…");
        gtk_label_set_xalign(GTK_LABEL(A.status_val[i]), 0);
        gtk_label_set_selectable(GTK_LABEL(A.status_val[i]), TRUE);
        gtk_grid_attach(GTK_GRID(grid), A.status_val[i], 1, i, 1, 1);
    }
    return grid;
}

/**
 * 필드 좌표를 창 내부 상대 좌표로 남긴다. GTK4 는 클라이언트에게 창의 화면
 * 위치를 알려주지 않으므로 절대 좌표는 내지 않는다 — 하네스가 `xwininfo` 로
 * 창 원점을 구해 더한다(TEST_APPS.md §5).
 */
static gboolean emit_geometry(gpointer u) {
    (void)u;
    double rx = 0, ry = 0;
    graphene_point_t out;
    if (gtk_widget_compute_point(A.canvas, GTK_WIDGET(A.window),
                                 &GRAPHENE_POINT_INIT(0, 0), &out)) {
        rx = out.x; ry = out.y;
    }
    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++) {
        const UnimTestField *f = &A.fields[i];
        char kv[640];
        g_snprintf(kv, sizeof kv,
                   "\"field\":\"%s\",\"x\":%d,\"y\":%d,\"w\":%d,\"h\":%d,"
                   "\"cx\":%d,\"cy\":%d,\"screen_cx\":-1,\"screen_cy\":-1",
                   f->id, (int)rx + f->x, (int)ry + f->y, f->w, f->h,
                   (int)rx + f->x + f->w / 2, (int)ry + f->y + f->h / 2);
        unim_log_raw("geometry", kv);
    }
    unim_log_ready();
    return G_SOURCE_REMOVE;
}

static void on_close(GtkWindow *w, gpointer u) {
    (void)w; (void)u;
    if (A.loop) g_main_loop_quit(A.loop);
}

static void build_ui(void) {
    A.window = gtk_window_new();
    char title[128];
    g_snprintf(title, sizeof title, UNIM_SPEC_WIN_TITLE_FMT, APP_NAME);
    gtk_window_set_title(GTK_WINDOW(A.window), title);
    gtk_window_set_default_size(GTK_WINDOW(A.window),
                                S(UNIM_SPEC_WIN_WIDTH), S(UNIM_SPEC_WIN_HEIGHT));
    g_signal_connect(A.window, "destroy", G_CALLBACK(on_close), NULL);

    GtkWidget *root = gtk_box_new(GTK_ORIENTATION_VERTICAL, S(UNIM_SPEC_SECTION_GAP));
    int m = S(UNIM_SPEC_MARGIN);
    gtk_widget_set_margin_start(root, m);   gtk_widget_set_margin_end(root, m);
    gtk_widget_set_margin_top(root, m);     gtk_widget_set_margin_bottom(root, m);
    gtk_window_set_child(GTK_WINDOW(A.window), root);

    gtk_box_append(GTK_BOX(root), section_title("① 상태"));
    gtk_box_append(GTK_BOX(root), build_status_panel());

    gtk_box_append(GTK_BOX(root),
                   section_title("② 코어 필드 (IM 직결 · 직접 그리기)"));
    A.canvas = gtk_drawing_area_new();
    gtk_widget_set_focusable(A.canvas, TRUE);
    gtk_drawing_area_set_draw_func(GTK_DRAWING_AREA(A.canvas), draw_func, NULL, NULL);

    GtkEventController *kc = gtk_event_controller_key_new();
    g_signal_connect(kc, "key-pressed", G_CALLBACK(on_key_pressed), NULL);
    gtk_widget_add_controller(A.canvas, kc);

    GtkGesture *gc = gtk_gesture_click_new();
    g_signal_connect(gc, "pressed", G_CALLBACK(on_click), NULL);
    gtk_widget_add_controller(A.canvas, GTK_EVENT_CONTROLLER(gc));

    GtkEventController *fc = gtk_event_controller_focus_new();
    g_signal_connect(fc, "enter", G_CALLBACK(on_focus_enter), NULL);
    g_signal_connect(fc, "leave", G_CALLBACK(on_focus_leave), NULL);
    gtk_widget_add_controller(A.canvas, fc);

    gtk_box_append(GTK_BOX(root), A.canvas);

    gtk_box_append(GTK_BOX(root), section_title("③ 네이티브 위젯 (툴킷 기본)"));
    for (int i = 0; i < UNIM_SPEC_N_NATIVE; i++)
        gtk_box_append(GTK_BOX(root), make_native_row(&UNIM_SPEC_NATIVE[i]));

    gtk_box_append(GTK_BOX(root), section_title("④ 로그"));
    A.log_view = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(A.log_view), FALSE);
    gtk_text_view_set_monospace(GTK_TEXT_VIEW(A.log_view), TRUE);
    A.log_buf = gtk_text_view_get_buffer(GTK_TEXT_VIEW(A.log_view));
    GtkWidget *sc = gtk_scrolled_window_new();
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(sc), A.log_view);
    gtk_widget_set_size_request(sc, -1, S(UNIM_SPEC_LOG_H));
    gtk_widget_set_vexpand(sc, TRUE);
    gtk_box_append(GTK_BOX(root), sc);
}

/* ─── main ────────────────────────────────────────────────────────────── */

int main(int argc, char *argv[]) {
    unim_log_init(APP_NAME, argc, argv);

    gboolean auto_mode = FALSE, verbose = FALSE;
    for (int i = 1; i < argc; i++) {
        if (g_strcmp0(argv[i], "--auto") == 0) auto_mode = TRUE;
        if (g_strcmp0(argv[i], "-v") == 0 || g_strcmp0(argv[i], "--verbose") == 0)
            verbose = TRUE;
    }

    char tv[64];
    g_snprintf(tv, sizeof tv, "GTK %d.%d.%d", gtk_get_major_version(),
               gtk_get_minor_version(), gtk_get_micro_version());
    unim_log_env(tv);

    if (auto_mode) {
        unim_log_note("--auto: DBus 스모크만 돌리고 종료한다 "
                      "(프런트엔드 경로 회귀는 tests/harness 가 본다)");
        UnimTestRunner *r = unim_test_runner_new(verbose);
        if (!r) { unim_log_error("러너 생성 실패"); unim_log_shutdown(); return 2; }
        unim_test_run_suite(r, UNIM_TEST_SUITE_AUTO);
        unim_test_print_summary(r);
        int ret = r->failed > 0 ? 1 : 0;
        unim_test_runner_free(r);
        unim_log_shutdown();
        return ret;
    }

    gtk_init();
    unim_log_set_sink(log_sink, NULL);

    A.scale = 1.0;
    unim_log_note("HiDPI 배율 %.2f (스펙 수치는 %g dpi 기준)",
                  A.scale, UNIM_SPEC_BASE_DPI);

    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        unim_field_init(&A.fields[i], &UNIM_SPEC_CORE_FIELDS[i]);

    build_ui();

    A.measure = gtk_widget_create_pango_layout(A.canvas, NULL);
    pango_layout_set_font_description(A.measure, font_of(UNIM_SPEC_FONT_SIZE_FIELD));

    int bottom = unim_field_layout(A.fields, UNIM_SPEC_N_CORE_FIELDS,
                                   0, S(UNIM_SPEC_WIN_WIDTH), A.scale);
    gtk_drawing_area_set_content_height(GTK_DRAWING_AREA(A.canvas), bottom);

    A.im = gtk_im_multicontext_new();
    gtk_im_context_set_client_widget(A.im, A.canvas);
    g_signal_connect(A.im, "commit",          G_CALLBACK(on_commit), NULL);
    g_signal_connect(A.im, "preedit-start",   G_CALLBACK(on_preedit_start), NULL);
    g_signal_connect(A.im, "preedit-changed", G_CALLBACK(on_preedit_changed), NULL);
    g_signal_connect(A.im, "preedit-end",     G_CALLBACK(on_preedit_end), NULL);
    g_signal_connect(A.im, "retrieve-surrounding",
                     G_CALLBACK(on_retrieve_surrounding), NULL);
    g_signal_connect(A.im, "delete-surrounding",
                     G_CALLBACK(on_delete_surrounding), NULL);
    unim_log_note("GtkIMMulticontext 연결 — 실제 모듈은 GTK_IM_MODULE 이 정한다");

    A.daemon = unim_daemon_connect(on_daemon_changed, NULL);
    diagnose_extension_path();

    gtk_window_present(GTK_WINDOW(A.window));
    unim_field_set_focus(&A.fields[0], 1, NULL);
    gtk_widget_grab_focus(A.canvas);
    refresh_status();

    /* 레이아웃이 한 번 돌아야 좌표가 확정된다 — 그 뒤에 geometry 를 낸다. */
    g_timeout_add(200, emit_geometry, NULL);

    A.loop = g_main_loop_new(NULL, FALSE);
    g_main_loop_run(A.loop);

    unim_daemon_free(A.daemon);
    unim_log_shutdown();
    return 0;
}
