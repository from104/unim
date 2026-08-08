/**
 * UNIM GTK3 테스트 앱
 *
 * 화면·필드 동작·로그는 전부 tests/common 의 공용 코드가 정한다. 이 파일이
 * 하는 일은 (a) GTK3 IM 시그널을 공용 필드 엔진으로 옮기고 (b) 필드 엔진이
 * 준 문자열을 cairo/pango 로 그리는 것뿐이다.
 *
 * 코어 필드는 `GtkDrawingArea` + `GtkIMMulticontext` 직결이다. `GtkEntry` 는
 * 내부 IM 컨텍스트를 숨겨서 preedit 을 앱이 볼 수 없기 때문이다 — 화면의
 * 진실을 로그로 남기려면 우리가 직접 그려야 한다(TEST_APPS.md §2).
 *
 * 실행:
 *   GTK_IM_MODULE=unim unim-test-gtk3
 *   unim-test-gtk3 --auto          DBus 스모크만 돌리고 종료
 */

#include <gtk/gtk.h>
#include <string.h>

#include "unim_test.h"
#include "unim_test_dbus.h"
#include "unim_test_field.h"
#include "unim_test_log.h"
#include "unim_test_spec.h"

#define APP_NAME "gtk3"

/* ─── 앱 상태 ─────────────────────────────────────────────────────────── */

static struct {
    GtkWidget      *window;
    GtkWidget      *canvas;        /* 코어 필드 6개를 직접 그리는 면 */
    GtkTextBuffer  *log_buf;
    GtkWidget      *log_view;
    GtkWidget      *status_val[UNIM_STATUS_N];
    GtkIMContext   *im;
    UnimTestDaemon *daemon;

    UnimTestField   fields[UNIM_SPEC_N_CORE_FIELDS];
    int             active;        /* 포커스 중인 코어 필드 index */
    int             canvas_focused;

    double          scale;
    char            last_commit[512];
    PangoLayout    *measure;       /* 폭 측정 전용 (그리기와 분리) */
} A;

static UnimTestField *cur(void) { return &A.fields[A.active]; }

/* ─── 색 ──────────────────────────────────────────────────────────────── */

static void set_rgb(cairo_t *cr, unsigned rgb) {
    cairo_set_source_rgb(cr, ((rgb >> 16) & 0xff) / 255.0,
                             ((rgb >> 8)  & 0xff) / 255.0,
                             ( rgb        & 0xff) / 255.0);
}

static int S(int v) { return (int)(v * A.scale + 0.5); }

/* ─── 로그 패널 ───────────────────────────────────────────────────────── */

/** 로거가 만든 사람용 한 줄을 그대로 패널에 붙인다 — stdout 과 항상 같다. */
static void log_sink(const char *line, void *user) {
    (void)user;
    if (!A.log_buf) return;

    GtkTextIter end;
    gtk_text_buffer_get_end_iter(A.log_buf, &end);
    gtk_text_buffer_insert(A.log_buf, &end, line, -1);
    gtk_text_buffer_insert(A.log_buf, &end, "\n", -1);

    /* 오래된 줄 버리기 */
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

/* ─── 폰트·측정 ───────────────────────────────────────────────────────── */

static PangoFontDescription *font_field(void) {
    static PangoFontDescription *fd;
    if (!fd) {
        char s[128];
        g_snprintf(s, sizeof s, "%s %d", UNIM_SPEC_FONT_UI,
                   (int)(UNIM_SPEC_FONT_SIZE_FIELD * A.scale));
        fd = pango_font_description_from_string(s);
    }
    return fd;
}

static PangoFontDescription *font_label(void) {
    static PangoFontDescription *fd;
    if (!fd) {
        char s[128];
        g_snprintf(s, sizeof s, "%s %d", UNIM_SPEC_FONT_UI,
                   (int)(UNIM_SPEC_FONT_SIZE_UI * A.scale));
        fd = pango_font_description_from_string(s);
    }
    return fd;
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

/**
 * IM 에게 조합 창을 띄울 자리를 알려준다. 이게 틀리면 한자 팝업이 엉뚱한
 * 곳에 뜨고, XIM 계열에서는 preedit 자체가 어긋난다.
 */
static void update_cursor_location(void) {
    if (!A.im || !A.canvas_focused) return;

    UnimTestField *f = cur();
    char before[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_before_caret(f, before, sizeof before);

    GdkRectangle r = {
        .x      = f->x + S(UNIM_SPEC_FIELD_PAD_X) + measure_cb(before, strlen(before), NULL),
        .y      = f->y,
        .width  = 2,
        .height = f->h,
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
    gint cursor = 0;
    gtk_im_context_get_preedit_string(im, &text, &attrs, &cursor);

    /* cursor 는 **문자 수** 단위다 — 필드 엔진은 바이트를 쓴다. */
    int byte_cursor = text ? (int)(g_utf8_offset_to_pointer(text, cursor) - text) : 0;
    unim_field_set_preedit(cur(), text ? text : "", byte_cursor);

    if (text) g_free(text);
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
    gtk_im_context_set_surrounding(im, f->committed, -1, f->caret);
    return TRUE;
}

static gboolean on_delete_surrounding(GtkIMContext *im, gint offset,
                                      gint n_chars, gpointer u) {
    (void)im; (void)u;
    UnimTestField *f = cur();
    unim_log_surrounding("delete", f->committed, f->caret, offset, n_chars);

    /* offset 은 캐럿 기준 문자 단위. 캐럿을 옮긴 뒤 n_chars 만큼 지운다. */
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

    /* 전환 전에 조합을 끊는다. 안 그러면 조합이 새 필드로 새어 나간다. */
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

    /* 라벨 */
    set_rgb(cr, UNIM_SPEC_COL_LABEL);
    pango_layout_set_font_description(lay, font_label());
    pango_layout_set_width(lay, -1);
    pango_layout_set_text(lay, f->label, -1);
    cairo_move_to(cr, S(UNIM_SPEC_MARGIN), f->y + S(8));
    pango_cairo_show_layout(cr, lay);

    /* 상자 */
    set_rgb(cr, focused ? UNIM_SPEC_COL_FIELD_FOCUS : UNIM_SPEC_COL_FIELD_BG);
    cairo_rectangle(cr, f->x, f->y, f->w, f->h);
    cairo_fill(cr);
    set_rgb(cr, focused ? UNIM_SPEC_COL_BORDER_FOCUS : UNIM_SPEC_COL_BORDER);
    cairo_set_line_width(cr, focused ? 2.0 : 1.0);
    cairo_rectangle(cr, f->x + 0.5, f->y + 0.5, f->w - 1, f->h - 1);
    cairo_stroke(cr);

    /* 텍스트 — 화면 표시용(비밀번호면 마스킹) */
    char shown[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_display(f, shown, sizeof shown);

    pango_layout_set_font_description(lay, font_field());
    pango_layout_set_text(lay, shown, -1);
    if (f->hint == UNIM_HINT_MULTILINE) {
        pango_layout_set_width(lay, (f->w - 2 * S(UNIM_SPEC_FIELD_PAD_X)) * PANGO_SCALE);
        pango_layout_set_wrap(lay, PANGO_WRAP_WORD_CHAR);
    } else {
        pango_layout_set_width(lay, -1);
    }

    /* 조합 구간 강조 — 화면에서 preedit 이 어디인지 눈으로 확인할 수 있어야
     * 한다. 마스킹된 필드는 문자 수 기준으로 구간을 다시 잡는다. */
    int pre_len = (int)strlen(f->preedit);
    if (pre_len > 0) {
        int start, len;
        if (f->hint == UNIM_HINT_PASSWORD) {
            char head[UNIM_FIELD_TEXT_MAX];
            g_snprintf(head, sizeof head, "%.*s", f->caret, f->committed);
            start = (int)unim_log_utf8_len(head) * 3;              /* • = 3B */
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

    /* 캐럿 */
    if (focused) {
        int idx;
        if (f->hint == UNIM_HINT_PASSWORD) {
            char head[UNIM_FIELD_TEXT_MAX];
            g_snprintf(head, sizeof head, "%.*s", f->caret, f->committed);
            char phead[UNIM_FIELD_PREEDIT_MAX];
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

static gboolean on_draw(GtkWidget *w, cairo_t *cr, gpointer u) {
    (void)u;
    GtkAllocation al;
    gtk_widget_get_allocation(w, &al);

    set_rgb(cr, UNIM_SPEC_COL_BG);
    cairo_paint(cr);

    PangoLayout *lay = pango_cairo_create_layout(cr);
    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        draw_field(cr, &A.fields[i], lay);
    g_object_unref(lay);
    return FALSE;
}

/* ─── 입력 ────────────────────────────────────────────────────────────── */

static gboolean on_key_press(GtkWidget *w, GdkEventKey *ev, gpointer u) {
    (void)w; (void)u;

    char *utf8 = NULL;
    guint32 uc = gdk_keyval_to_unicode(ev->keyval);
    char ubuf[8] = "";
    if (uc >= 0x20 && uc != 0x7f) {
        int n = g_unichar_to_utf8((gunichar)uc, ubuf);
        ubuf[n] = '\0';
        utf8 = ubuf;
    }
    unim_log_key("press", ev->keyval, ev->keyval, ev->hardware_keycode,
                 ev->state, utf8, -1);

    /* Tab 은 IM 에 넘기기 전에 가로챈다 — 필드 순환이 우선이다. */
    if (ev->keyval == GDK_KEY_Tab || ev->keyval == GDK_KEY_ISO_Left_Tab) {
        int dir = (ev->state & GDK_SHIFT_MASK) ? -1 : 1;
        focus_field((A.active + dir + UNIM_SPEC_N_CORE_FIELDS)
                        % UNIM_SPEC_N_CORE_FIELDS, "tab");
        return TRUE;
    }

    gint64 t0 = g_get_monotonic_time();
    unim_log_im("enter", cur()->id, "filter_keypress", 0);
    gboolean filtered = gtk_im_context_filter_keypress(A.im, ev);
    double ms = (double)(g_get_monotonic_time() - t0) / 1000.0;
    unim_log_im("leave", cur()->id, filtered ? "IM 삼킴" : "앱으로", ms);

    if (filtered) return TRUE;

    UnimTestField *f = cur();
    switch (ev->keyval) {
        case GDK_KEY_BackSpace: unim_field_backspace(f);        break;
        case GDK_KEY_Delete:    unim_field_delete(f);           break;
        case GDK_KEY_Left:      unim_field_move_caret(f, -1);   break;
        case GDK_KEY_Right:     unim_field_move_caret(f, +1);   break;
        case GDK_KEY_Home:      unim_field_caret_home(f);       break;
        case GDK_KEY_End:       unim_field_caret_end(f);        break;
        case GDK_KEY_Escape:    unim_field_clear(f);            break;
        case GDK_KEY_Return:
        case GDK_KEY_KP_Enter:
            if (f->hint == UNIM_HINT_MULTILINE) unim_field_insert(f, "\n");
            else focus_field((A.active + 1) % UNIM_SPEC_N_CORE_FIELDS, "enter");
            break;
        default:
            if (utf8) unim_field_insert(f, utf8);
            else unim_log_note("%s: 처리하지 않은 키 keyval=0x%x", f->id, ev->keyval);
    }
    after_change();
    return TRUE;
}

static gboolean on_button_press(GtkWidget *w, GdkEventButton *ev, gpointer u) {
    (void)u;
    gtk_widget_grab_focus(w);

    int hit = unim_field_hit(A.fields, UNIM_SPEC_N_CORE_FIELDS,
                             (int)ev->x, (int)ev->y);
    if (hit < 0) {
        unim_log_click((int)ev->x, (int)ev->y, "(빈 곳)", -1, -1);
        return TRUE;
    }

    if (hit != A.active) {
        focus_field(hit, "click");
    } else if (cur()->composing || cur()->preedit[0]) {
        /* 같은 필드 안에서 조합 중 클릭 — 2026-08-06 회귀의 재현 지점이다.
         * 조합을 그 자리에서 끊고 캐럿을 옮긴다. */
        unim_log_reset(cur()->id, "click-in-field");
        gtk_im_context_reset(A.im);
    }

    UnimTestField *f = cur();
    int before = f->caret;
    f->caret = unim_field_caret_from_x(f, (int)ev->x, measure_cb, NULL);
    unim_log_click((int)ev->x, (int)ev->y, f->id, before, f->caret);
    unim_field_log_render(f);

    after_change();
    return TRUE;
}

static gboolean on_canvas_focus_in(GtkWidget *w, GdkEventFocus *ev, gpointer u) {
    (void)w; (void)ev; (void)u;
    A.canvas_focused = 1;
    gtk_im_context_focus_in(A.im);
    unim_field_set_focus(cur(), 1, "(네이티브 위젯)");
    after_change();
    return FALSE;
}

static gboolean on_canvas_focus_out(GtkWidget *w, GdkEventFocus *ev, gpointer u) {
    (void)w; (void)ev; (void)u;
    A.canvas_focused = 0;
    unim_log_reset(cur()->id, "canvas-focus-out");
    gtk_im_context_focus_out(A.im);
    unim_field_set_focus(cur(), 0, NULL);
    after_change();
    return FALSE;
}

/* ─── 네이티브 위젯 (툴킷 기본 경로 감시) ────────────────────────────── */

static gboolean on_native_focus_in(GtkWidget *w, GdkEventFocus *ev, gpointer u) {
    (void)w; (void)ev;
    unim_log_focus("in", (const char *)u, "(코어 필드)");
    return FALSE;
}

static void on_native_changed(GtkEditable *e, gpointer u) {
    const char *t = gtk_entry_get_text(GTK_ENTRY(e));
    /* GtkEntry 는 preedit 을 앱에 안 준다 — 확정된 내용만 관측된다.
     * 이 한계 때문에 코어 필드를 따로 두었다(TEST_APPS.md §2). */
    unim_log_field_render((const char *)u, t, "", (int)strlen(t), t);
}

static GtkWidget *make_native_row(const UnimSpecNative *spec) {
    GtkWidget *row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, S(10));
    GtkWidget *lab = gtk_label_new(spec->label);
    gtk_label_set_xalign(GTK_LABEL(lab), 0);
    gtk_widget_set_size_request(lab, S(UNIM_SPEC_LABEL_COL_W), -1);
    gtk_box_pack_start(GTK_BOX(row), lab, FALSE, FALSE, 0);

    GtkWidget *wid;
    if (spec->kind == UNIM_NATIVE_MULTILINE) {
        wid = gtk_text_view_new();
        GtkWidget *sc = gtk_scrolled_window_new(NULL, NULL);
        gtk_widget_set_size_request(sc, -1, S(60));
        gtk_container_add(GTK_CONTAINER(sc), wid);
        gtk_box_pack_start(GTK_BOX(row), sc, TRUE, TRUE, 0);
    } else {
        wid = gtk_entry_new();
        if (spec->kind == UNIM_NATIVE_PASSWORD)
            gtk_entry_set_visibility(GTK_ENTRY(wid), FALSE);
        g_signal_connect(wid, "changed", G_CALLBACK(on_native_changed),
                         (gpointer)spec->id);
        gtk_box_pack_start(GTK_BOX(row), wid, TRUE, TRUE, 0);
    }
    gtk_widget_set_name(wid, spec->id);
    g_signal_connect(wid, "focus-in-event", G_CALLBACK(on_native_focus_in),
                     (gpointer)spec->id);
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
 * 필드 좌표를 남긴다 — 하네스가 이 값으로 클릭한다.
 *
 * 두 벌을 낸다: 창 내부 상대(`x`,`y`,`cx`,`cy`)와, 화면 절대
 * (`screen_cx`,`screen_cy`). Wayland 클라이언트는 자기 창의 화면 위치를 알
 * 수 없어 절대 좌표를 못 낸다. 하네스는 절대가 있으면 그걸 쓰고, 없으면
 * 창 원점을 스스로 구해 상대에 더한다(TEST_APPS.md §5).
 */
static void on_map_event_done(GtkWidget *w, gpointer u) {
    (void)w; (void)u;
    gint rx = 0, ry = 0;       /* canvas → toplevel 오프셋 */
    GtkWidget *top = gtk_widget_get_toplevel(A.canvas);
    if (top)
        gtk_widget_translate_coordinates(A.canvas, top, 0, 0, &rx, &ry);

    gint sx = -1, sy = -1;     /* canvas 의 화면 절대 좌표 (X11 에서만) */
    GdkWindow *gw = gtk_widget_get_window(A.canvas);
    if (gw) gdk_window_get_origin(gw, &sx, &sy);

    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++) {
        const UnimTestField *f = &A.fields[i];
        char kv[640];
        g_snprintf(kv, sizeof kv,
                   "\"field\":\"%s\",\"x\":%d,\"y\":%d,\"w\":%d,\"h\":%d,"
                   "\"cx\":%d,\"cy\":%d,\"screen_cx\":%d,\"screen_cy\":%d",
                   f->id, rx + f->x, ry + f->y, f->w, f->h,
                   rx + f->x + f->w / 2, ry + f->y + f->h / 2,
                   sx + f->x + f->w / 2, sy + f->y + f->h / 2);
        unim_log_raw("geometry", kv);
    }
    unim_log_ready();
}

static void build_ui(void) {
    A.window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    char title[128];
    g_snprintf(title, sizeof title, UNIM_SPEC_WIN_TITLE_FMT, APP_NAME);
    gtk_window_set_title(GTK_WINDOW(A.window), title);
    gtk_window_set_default_size(GTK_WINDOW(A.window),
                                S(UNIM_SPEC_WIN_WIDTH), S(UNIM_SPEC_WIN_HEIGHT));
    g_signal_connect(A.window, "destroy", G_CALLBACK(gtk_main_quit), NULL);

    GtkWidget *root = gtk_box_new(GTK_ORIENTATION_VERTICAL, S(UNIM_SPEC_SECTION_GAP));
    gtk_container_set_border_width(GTK_CONTAINER(root), S(UNIM_SPEC_MARGIN));
    gtk_container_add(GTK_CONTAINER(A.window), root);

    /* ① 상태 */
    gtk_box_pack_start(GTK_BOX(root), section_title("① 상태"), FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(root), build_status_panel(), FALSE, FALSE, 0);

    /* ② 코어 필드 — 직접 그리기 */
    gtk_box_pack_start(GTK_BOX(root),
                       section_title("② 코어 필드 (IM 직결 · 직접 그리기)"),
                       FALSE, FALSE, 0);

    A.canvas = gtk_drawing_area_new();
    gtk_widget_set_can_focus(A.canvas, TRUE);
    gtk_widget_add_events(A.canvas, GDK_KEY_PRESS_MASK | GDK_BUTTON_PRESS_MASK |
                                    GDK_FOCUS_CHANGE_MASK);
    g_signal_connect(A.canvas, "draw", G_CALLBACK(on_draw), NULL);
    g_signal_connect(A.canvas, "key-press-event", G_CALLBACK(on_key_press), NULL);
    g_signal_connect(A.canvas, "button-press-event", G_CALLBACK(on_button_press), NULL);
    g_signal_connect(A.canvas, "focus-in-event", G_CALLBACK(on_canvas_focus_in), NULL);
    g_signal_connect(A.canvas, "focus-out-event", G_CALLBACK(on_canvas_focus_out), NULL);
    gtk_box_pack_start(GTK_BOX(root), A.canvas, FALSE, FALSE, 0);

    /* ③ 네이티브 위젯 */
    gtk_box_pack_start(GTK_BOX(root),
                       section_title("③ 네이티브 위젯 (툴킷 기본)"),
                       FALSE, FALSE, 0);
    for (int i = 0; i < UNIM_SPEC_N_NATIVE; i++)
        gtk_box_pack_start(GTK_BOX(root), make_native_row(&UNIM_SPEC_NATIVE[i]),
                           FALSE, FALSE, 0);

    /* ④ 로그 */
    gtk_box_pack_start(GTK_BOX(root), section_title("④ 로그"), FALSE, FALSE, 0);
    A.log_view = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(A.log_view), FALSE);
    gtk_text_view_set_monospace(GTK_TEXT_VIEW(A.log_view), TRUE);
    A.log_buf = gtk_text_view_get_buffer(GTK_TEXT_VIEW(A.log_view));
    GtkWidget *sc = gtk_scrolled_window_new(NULL, NULL);
    gtk_container_add(GTK_CONTAINER(sc), A.log_view);
    gtk_widget_set_size_request(sc, -1, S(UNIM_SPEC_LOG_H));
    gtk_box_pack_start(GTK_BOX(root), sc, TRUE, TRUE, 0);

    g_signal_connect_after(A.window, "map-event",
                           G_CALLBACK(on_map_event_done), NULL);
}

/* ─── main ────────────────────────────────────────────────────────────── */

int main(int argc, char *argv[]) {
    unim_log_init(APP_NAME, argc, argv);   /* 첫 줄 — 시작 실패도 기록된다 */

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

    gtk_init(&argc, &argv);
    unim_log_set_sink(log_sink, NULL);

    A.scale = 1.0;
    GdkScreen *screen = gdk_screen_get_default();
    if (screen) {
        double dpi = gdk_screen_get_resolution(screen);
        if (dpi > 0) A.scale = dpi / UNIM_SPEC_BASE_DPI;
    }
    unim_log_note("HiDPI 배율 %.2f (스펙 수치는 %g dpi 기준)",
                  A.scale, UNIM_SPEC_BASE_DPI);

    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        unim_field_init(&A.fields[i], &UNIM_SPEC_CORE_FIELDS[i]);

    build_ui();

    A.measure = gtk_widget_create_pango_layout(A.canvas, NULL);
    pango_layout_set_font_description(A.measure, font_field());

    int bottom = unim_field_layout(A.fields, UNIM_SPEC_N_CORE_FIELDS,
                                   0, S(UNIM_SPEC_WIN_WIDTH), A.scale);
    gtk_widget_set_size_request(A.canvas, -1, bottom);

    /* IM 은 창이 realize 된 뒤에 붙인다 (client window 가 필요하다). */
    gtk_widget_show_all(A.window);
    A.im = gtk_im_multicontext_new();
    gtk_im_context_set_client_window(A.im, gtk_widget_get_window(A.canvas));
    g_signal_connect(A.im, "commit",             G_CALLBACK(on_commit), NULL);
    g_signal_connect(A.im, "preedit-start",      G_CALLBACK(on_preedit_start), NULL);
    g_signal_connect(A.im, "preedit-changed",    G_CALLBACK(on_preedit_changed), NULL);
    g_signal_connect(A.im, "preedit-end",        G_CALLBACK(on_preedit_end), NULL);
    g_signal_connect(A.im, "retrieve-surrounding",
                     G_CALLBACK(on_retrieve_surrounding), NULL);
    g_signal_connect(A.im, "delete-surrounding",
                     G_CALLBACK(on_delete_surrounding), NULL);
    unim_log_note("GtkIMMulticontext 연결 — 실제 모듈은 GTK_IM_MODULE 이 정한다");

    A.daemon = unim_daemon_connect(on_daemon_changed, NULL);

    unim_field_set_focus(&A.fields[0], 1, NULL);
    gtk_widget_grab_focus(A.canvas);
    refresh_status();

    gtk_main();

    unim_daemon_free(A.daemon);
    unim_log_shutdown();
    return 0;
}
