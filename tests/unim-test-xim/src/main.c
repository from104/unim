/**
 * UNIM XIM 테스트 앱
 *
 * 화면·필드 동작·로그는 tests/common 의 공용 코드가 정한다. GTK·Qt 판과
 * 화면이 같아야 하며(같은 `unim_test_spec.h`), 다른 것은 툴킷 API 뿐이다.
 *
 * XIM 은 애초에 위젯 툴킷이 아니라 앱이 전부 직접 그린다 — 그래서 preedit
 * 관측이 원래부터 100% 다. 다른 앱들이 캔버스 직접 그리기로 맞춘 그 지점에
 * 이 앱은 처음부터 있었다(TEST_APPS.md §2).
 *
 * 스타일은 ON-THE-SPOT(`XIMPreeditCallbacks`)을 최우선으로 잡는다 — Obsidian
 * 등 실제 앱이 타는 경로이자 2026-08-07 회귀가 났던 자리다.
 *
 * 실행:
 *   XMODIFIERS=@im=unim unim-test-xim
 *   unim-test-xim --auto           DBus 스모크만 돌리고 종료
 */

#define _POSIX_C_SOURCE 200809L

#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>
#include <X11/Xft/Xft.h>

#include <glib.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>

#include "unim_test.h"
#include "unim_test_dbus.h"
#include "unim_test_field.h"
#include "unim_test_log.h"
#include "unim_test_spec.h"

#define APP_NAME "xim"

#define LOG_VIEW_LINES 14
#define LOG_LINE_LEN   256

/* ─── 앱 상태 ─────────────────────────────────────────────────────────── */

static struct {
    Display        *dpy;
    int             screen;
    Window          win;
    XIM             xim;
    XIC             xic;
    XIMStyle        style;

    XftDraw        *draw;
    XftFont        *font_field;
    XftFont        *font_ui;
    XftFont        *font_log;

    UnimTestField   fields[UNIM_SPEC_N_CORE_FIELDS];
    int             active;
    int             focused;

    UnimTestDaemon *daemon;
    char            last_commit[512];

    /* 화면 로그 패널 — 순환 버퍼 */
    char            loglines[LOG_VIEW_LINES][LOG_LINE_LEN];
    int             log_count;

    int             fields_top;
    int             log_top;
    int             running;
    int             ready;
} A;

static UnimTestField *cur(void) { return &A.fields[A.active]; }

/* ─── 색 ──────────────────────────────────────────────────────────────── */

static XftColor xft_col(unsigned rgb) {
    XftColor c;
    XRenderColor rc = {
        .red   = (unsigned short)(((rgb >> 16) & 0xff) * 257),
        .green = (unsigned short)(((rgb >> 8)  & 0xff) * 257),
        .blue  = (unsigned short)(( rgb        & 0xff) * 257),
        .alpha = 0xffff,
    };
    XftColorAllocValue(A.dpy, DefaultVisual(A.dpy, A.screen),
                       DefaultColormap(A.dpy, A.screen), &rc, &c);
    return c;
}

static void fill_rect(unsigned rgb, int x, int y, int w, int h) {
    XSetForeground(A.dpy, DefaultGC(A.dpy, A.screen), rgb);
    XFillRectangle(A.dpy, A.win, DefaultGC(A.dpy, A.screen), x, y, w, h);
}

static void frame_rect(unsigned rgb, int x, int y, int w, int h, int lw) {
    XSetForeground(A.dpy, DefaultGC(A.dpy, A.screen), rgb);
    XSetLineAttributes(A.dpy, DefaultGC(A.dpy, A.screen), lw,
                       LineSolid, CapButt, JoinMiter);
    XDrawRectangle(A.dpy, A.win, DefaultGC(A.dpy, A.screen), x, y, w - 1, h - 1);
}

static void draw_text(XftFont *f, unsigned rgb, int x, int y, const char *s) {
    if (!s || !*s) return;
    XftColor c = xft_col(rgb);
    XftDrawStringUtf8(A.draw, &c, f, x, y, (const FcChar8 *)s, (int)strlen(s));
    XftColorFree(A.dpy, DefaultVisual(A.dpy, A.screen),
                 DefaultColormap(A.dpy, A.screen), &c);
}

static int text_width(XftFont *f, const char *s, size_t n) {
    if (!s || n == 0) return 0;
    XGlyphInfo gi;
    XftTextExtentsUtf8(A.dpy, f, (const FcChar8 *)s, (int)n, &gi);
    return gi.xOff;
}

static int measure_cb(const char *utf8, size_t n, void *user) {
    (void)user;
    return text_width(A.font_field, utf8, n);
}

/* ─── 로그 패널 ───────────────────────────────────────────────────────── */

static void log_sink(const char *line, void *user) {
    (void)user;
    int slot = A.log_count % LOG_VIEW_LINES;
    snprintf(A.loglines[slot], LOG_LINE_LEN, "%s", line);
    A.log_count++;
    /* 그리기는 다음 Expose/redraw 에서 한다 — 로그마다 전체 다시 그리면
     * 키 처리가 느려져 IM 왕복 측정이 흐려진다. */
}

/* ─── IM 커서 위치 ────────────────────────────────────────────────────── */

static void update_spot(void) {
    if (!A.xic) return;
    UnimTestField *f = cur();

    char before[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_before_caret(f, before, sizeof before);

    XPoint spot = {
        .x = (short)(f->x + UNIM_SPEC_FIELD_PAD_X +
                     text_width(A.font_field, before, strlen(before))),
        .y = (short)(f->y + f->h - 10),
    };
    XVaNestedList attr = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
    XSetICValues(A.xic, XNPreeditAttributes, attr, NULL);
    XFree(attr);
}

static void redraw(void);

static void after_change(void) {
    update_spot();
    redraw();
}

/* ─── XIM preedit 콜백 (ON-THE-SPOT) ──────────────────────────────────── */

static int preedit_start_cb(XIC ic, XPointer client, XPointer call) {
    (void)ic; (void)client; (void)call;
    unim_field_preedit_start(cur());
    after_change();
    return -1;   /* 길이 제한 없음 */
}

static void preedit_done_cb(XIC ic, XPointer client, XPointer call) {
    (void)ic; (void)client; (void)call;
    unim_field_preedit_end(cur());
    after_change();
}

static void preedit_draw_cb(XIC ic, XPointer client,
                            XIMPreeditDrawCallbackStruct *call) {
    (void)ic; (void)client;
    UnimTestField *f = cur();

    const char *s = (call && call->text && call->text->string.multi_byte)
                        ? call->text->string.multi_byte : "";

    /* caret 은 **문자 단위**다 — 필드 엔진은 바이트를 쓴다. */
    int byte_caret = -1;
    if (call && call->caret >= 0 && *s) {
        const char *p = s;
        for (int i = 0; i < call->caret && *p; i++) {
            p++;
            while ((*p & 0xC0) == 0x80) p++;
        }
        byte_caret = (int)(p - s);
    }
    unim_field_set_preedit(f, s, byte_caret);

    /* 앱이 preedit 을 임의로 비우지 않는다는 계약을 그대로 지킨다 —
     * 소유자는 IM 이고, 비움도 IM 이 빈 문자열로 알려준다(2026-08-07 회귀). */
    after_change();
}

static void preedit_caret_cb(XIC ic, XPointer client,
                             XIMPreeditCaretCallbackStruct *call) {
    (void)ic; (void)client;
    if (call)
        unim_log_note("%s: PreeditCaret position=%d direction=%d style=%d",
                      cur()->id, call->position, call->direction, call->style);
}

/* ─── XIM 설정 ────────────────────────────────────────────────────────── */

static int xim_init(void) {
    if (!setlocale(LC_ALL, "")) unim_log_warn("로케일 설정 실패");
    if (!XSupportsLocale())     unim_log_warn("X 로케일 미지원");
    if (!XSetLocaleModifiers("")) unim_log_warn("XSetLocaleModifiers 실패");

    A.xim = XOpenIM(A.dpy, NULL, NULL, NULL);
    if (!A.xim) {
        unim_log_error("XOpenIM 실패 — XMODIFIERS=%s, IM 서버가 떠 있는지 확인",
                       getenv("XMODIFIERS") ? getenv("XMODIFIERS") : "(없음)");
        return 0;
    }
    unim_log_note("XOpenIM 성공");

    XIMStyles *styles = NULL;
    if (XGetIMValues(A.xim, XNQueryInputStyle, &styles, NULL) || !styles) {
        unim_log_error("입력 스타일 쿼리 실패");
        return 0;
    }
    for (unsigned long i = 0; i < styles->count_styles; i++)
        unim_log_note("IM 지원 스타일 [%lu] 0x%lx", i,
                      (unsigned long)styles->supported_styles[i]);

    /* ON-THE-SPOT 을 최우선으로 — 실제 앱(Obsidian 등)이 타는 경로다. */
    XIMStyle wanted[] = {
        XIMPreeditCallbacks | XIMStatusNothing,
        XIMPreeditPosition  | XIMStatusNothing,
        XIMPreeditNothing   | XIMStatusNothing,
        0
    };
    XIMStyle best = 0;
    for (int w = 0; wanted[w] && !best; w++)
        for (unsigned long i = 0; i < styles->count_styles; i++)
            if (styles->supported_styles[i] == wanted[w]) { best = wanted[w]; break; }
    XFree(styles);

    if (!best) { unim_log_error("적합한 입력 스타일 없음"); return 0; }
    A.style = best;
    unim_log_note("선택한 스타일 0x%lx (%s)", (unsigned long)best,
                  (best & XIMPreeditCallbacks) ? "ON-THE-SPOT"
                  : (best & XIMPreeditPosition) ? "OVER-THE-SPOT" : "ROOT");

    XVaNestedList pattr = NULL;
    if (best & XIMPreeditCallbacks) {
        static XIMCallback cb_start, cb_done, cb_draw, cb_caret;
        cb_start.client_data = NULL; cb_start.callback = (XIMProc)preedit_start_cb;
        cb_done.client_data  = NULL; cb_done.callback  = (XIMProc)preedit_done_cb;
        cb_draw.client_data  = NULL; cb_draw.callback  = (XIMProc)preedit_draw_cb;
        cb_caret.client_data = NULL; cb_caret.callback = (XIMProc)preedit_caret_cb;

        pattr = XVaCreateNestedList(0,
            XNPreeditStartCallback, &cb_start,
            XNPreeditDoneCallback,  &cb_done,
            XNPreeditDrawCallback,  &cb_draw,
            XNPreeditCaretCallback, &cb_caret,
            NULL);
    } else if (best & XIMPreeditPosition) {
        XPoint spot = { (short)(A.fields[0].x + UNIM_SPEC_FIELD_PAD_X),
                        (short)(A.fields[0].y + A.fields[0].h - 10) };
        pattr = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
    }

    if (pattr)
        A.xic = XCreateIC(A.xim, XNInputStyle, best,
                          XNClientWindow, A.win, XNFocusWindow, A.win,
                          XNPreeditAttributes, pattr, NULL);
    else
        A.xic = XCreateIC(A.xim, XNInputStyle, best,
                          XNClientWindow, A.win, XNFocusWindow, A.win, NULL);
    if (pattr) XFree(pattr);

    if (!A.xic) { unim_log_error("XCreateIC 실패"); return 0; }
    unim_log_note("XCreateIC 성공");

    long mask = 0;
    if (XGetICValues(A.xic, XNFilterEvents, &mask, NULL) == NULL) {
        unim_log_note("XIC 가 요구하는 이벤트 마스크 0x%lx", mask);
        XSelectInput(A.dpy, A.win,
                     ExposureMask | KeyPressMask | KeyReleaseMask |
                     FocusChangeMask | StructureNotifyMask |
                     ButtonPressMask | mask);
    }
    return 1;
}

/* ─── 그리기 ──────────────────────────────────────────────────────────── */

static void draw_status(void) {
    UnimStatusInput in = {
        .frontend      = APP_NAME,
        .im_path       = NULL,
        .focus_field   = cur()->id,
        .preedit       = cur()->preedit,
        .preedit_caret = cur()->preedit_caret,
        .last_commit   = A.last_commit,
    };
    char vals[UNIM_STATUS_N][UNIM_STATUS_VALUE_MAX];
    unim_status_render(A.daemon, &in, vals);

    int y = UNIM_SPEC_MARGIN + A.font_ui->ascent;
    int rowh = A.font_ui->height + 4;
    for (int i = 0; i < UNIM_STATUS_N; i++) {
        draw_text(A.font_ui, UNIM_SPEC_COL_LABEL, UNIM_SPEC_MARGIN, y,
                  UNIM_SPEC_STATUS_LABELS[i]);
        draw_text(A.font_ui, UNIM_SPEC_COL_TEXT,
                  UNIM_SPEC_MARGIN + UNIM_SPEC_STATUS_LABEL_W, y, vals[i]);
        y += rowh;
    }
}

static void draw_field(const UnimTestField *f) {
    int focused = A.focused && f->focused;

    draw_text(A.font_ui, UNIM_SPEC_COL_LABEL, UNIM_SPEC_MARGIN,
              f->y + A.font_ui->ascent + 8, f->label);

    fill_rect(focused ? UNIM_SPEC_COL_FIELD_FOCUS : UNIM_SPEC_COL_FIELD_BG,
              f->x, f->y, f->w, f->h);
    frame_rect(focused ? UNIM_SPEC_COL_BORDER_FOCUS : UNIM_SPEC_COL_BORDER,
               f->x, f->y, f->w, f->h, focused ? 2 : 1);

    char shown[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_display(f, shown, sizeof shown);

    int tx = f->x + UNIM_SPEC_FIELD_PAD_X;
    int ty = f->y + A.font_field->ascent + 8;

    /* 확정 부분과 조합 부분을 나눠 그린다 — 조합은 색과 밑줄로 구분한다. */
    int head_bytes, pre_bytes;
    if (f->hint == UNIM_HINT_PASSWORD) {
        char h[UNIM_FIELD_TEXT_MAX];
        snprintf(h, sizeof h, "%.*s", f->caret, f->committed);
        head_bytes = (int)unim_log_utf8_len(h) * 3;          /* • = 3B */
        pre_bytes  = (int)unim_log_utf8_len(f->preedit) * 3;
    } else {
        head_bytes = f->caret;
        pre_bytes  = (int)strlen(f->preedit);
    }

    char buf[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    int x = tx;

    snprintf(buf, sizeof buf, "%.*s", head_bytes, shown);
    draw_text(A.font_field, UNIM_SPEC_COL_TEXT, x, ty, buf);
    x += text_width(A.font_field, buf, strlen(buf));

    if (pre_bytes > 0) {
        snprintf(buf, sizeof buf, "%.*s", pre_bytes, shown + head_bytes);
        draw_text(A.font_field, UNIM_SPEC_COL_PREEDIT, x, ty, buf);
        int pw = text_width(A.font_field, buf, strlen(buf));
        fill_rect(UNIM_SPEC_COL_PREEDIT_UL, x, ty + 3, pw, 2);   /* 밑줄 */
        x += pw;
    }

    snprintf(buf, sizeof buf, "%s", shown + head_bytes + pre_bytes);
    draw_text(A.font_field, UNIM_SPEC_COL_TEXT, x, ty, buf);

    if (focused) {
        int caret_bytes = head_bytes;
        if (f->hint == UNIM_HINT_PASSWORD) {
            char p[UNIM_FIELD_PREEDIT_MAX];
            snprintf(p, sizeof p, "%.*s", f->preedit_caret, f->preedit);
            caret_bytes += (int)unim_log_utf8_len(p) * 3;
        } else {
            caret_bytes += f->preedit_caret;
        }
        snprintf(buf, sizeof buf, "%.*s", caret_bytes, shown);
        int cx = tx + text_width(A.font_field, buf, strlen(buf));
        fill_rect(UNIM_SPEC_COL_CARET, cx, f->y + 6, 2, f->h - 12);
    }
}

static void draw_log_panel(void) {
    int y = A.log_top;
    draw_text(A.font_ui, UNIM_SPEC_COL_LABEL, UNIM_SPEC_MARGIN,
              y + A.font_ui->ascent, "④ 로그");
    y += A.font_ui->height + 6;

    fill_rect(UNIM_SPEC_COL_PANEL, UNIM_SPEC_MARGIN, y,
              UNIM_SPEC_WIN_WIDTH - 2 * UNIM_SPEC_MARGIN,
              LOG_VIEW_LINES * (A.font_log->height + 2) + 8);

    int start = (A.log_count > LOG_VIEW_LINES) ? A.log_count - LOG_VIEW_LINES : 0;
    int ly = y + 4 + A.font_log->ascent;
    for (int i = start; i < A.log_count; i++) {
        draw_text(A.font_log, UNIM_SPEC_COL_TEXT, UNIM_SPEC_MARGIN + 6, ly,
                  A.loglines[i % LOG_VIEW_LINES]);
        ly += A.font_log->height + 2;
    }
}

static void redraw(void) {
    if (!A.draw) return;
    fill_rect(UNIM_SPEC_COL_BG, 0, 0, UNIM_SPEC_WIN_WIDTH, UNIM_SPEC_WIN_HEIGHT);

    draw_text(A.font_ui, UNIM_SPEC_COL_LABEL, UNIM_SPEC_MARGIN,
              UNIM_SPEC_MARGIN - 4, "① 상태");
    draw_status();

    draw_text(A.font_ui, UNIM_SPEC_COL_LABEL, UNIM_SPEC_MARGIN,
              A.fields_top - 8, "② 코어 필드 (IM 직결 · 직접 그리기)");
    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        draw_field(&A.fields[i]);

    /* ③ 네이티브 위젯 섹션은 없다 — Xlib 에는 툴킷 기본 위젯이 존재하지
     * 않는다. 다른 앱과 화면이 다른 유일한 지점이며 의도된 것이다. */

    draw_log_panel();
    XFlush(A.dpy);
}

/* ─── 필드 전환 ───────────────────────────────────────────────────────── */

static void focus_field(int idx, const char *reason) {
    if (idx == A.active) return;
    UnimTestField *old = cur();
    const char *prev_id = old->id;

    if (old->composing || old->preedit[0]) {
        unim_log_reset(old->id, reason);
        /* XIM 리셋은 조합 중이던 문자열을 **동기로 되돌려준다** — 그것을
         * 원래 필드에 확정해야 클릭 자리로 새지 않는다. */
        if (A.xic) {
            char *committed = XmbResetIC(A.xic);
            if (committed && *committed) {
                unim_log_note("XmbResetIC 반환 \"%s\" → %s 에 확정",
                              committed, old->id);
                snprintf(A.last_commit, sizeof A.last_commit, "%s", committed);
                unim_field_commit(old, committed);
            }
            if (committed) XFree(committed);
        }
        /* 조합 상태는 여기서 정리한다 — reset 뒤에는 IM 이 PreeditDone 을
         * 보내지 않을 수도 있다. */
        unim_field_preedit_end(old);
    }

    unim_field_set_focus(old, 0, NULL);
    A.active = idx;
    unim_field_set_focus(cur(), 1, prev_id);
    after_change();
}

/* ─── 이벤트 ──────────────────────────────────────────────────────────── */

static void handle_key(XKeyEvent *ev) {
    KeySym raw = XLookupKeysym(ev, 0);

    /* Tab 은 IM 에 넘기기 전에 가로챈다 — 필드 순환이 우선이다. */
    if (raw == XK_Tab || raw == XK_ISO_Left_Tab) {
        unim_log_key("press", (unsigned)raw, (unsigned)raw, ev->keycode,
                     ev->state, NULL, 0);
        int dir = (ev->state & ShiftMask) ? -1 : 1;
        focus_field((A.active + dir + UNIM_SPEC_N_CORE_FIELDS)
                        % UNIM_SPEC_N_CORE_FIELDS, "tab");
        return;
    }

    char buf[256] = "";
    KeySym keysym = NoSymbol;
    Status status = 0;
    int len = 0;

    gint64 t0 = g_get_monotonic_time();
    unim_log_im("enter", cur()->id, "XmbLookupString", 0);
    if (A.xic)
        len = XmbLookupString(A.xic, ev, buf, sizeof buf - 1, &keysym, &status);
    else
        len = XLookupString(ev, buf, sizeof buf - 1, &keysym, NULL);
    buf[len] = '\0';
    double ms = (double)(g_get_monotonic_time() - t0) / 1000.0;

    unim_log_key("press", (unsigned)keysym, (unsigned)raw, ev->keycode,
                 ev->state, len ? buf : NULL, 0);
    unim_log_im("leave", cur()->id,
                (status == XLookupChars || status == XLookupBoth)
                    ? "문자 반환" : "문자 없음", ms);

    UnimTestField *f = cur();

    /* IM 이 만들어 준 문자 — 제어코드는 걸러낸다. */
    if ((status == XLookupChars || status == XLookupBoth) && len > 0 &&
        (unsigned char)buf[0] >= 0x20 && (unsigned char)buf[0] != 0x7f) {
        snprintf(A.last_commit, sizeof A.last_commit, "%s", buf);
        unim_field_commit(f, buf);
    }

    /* 편집·이동 키는 raw keysym 으로 처리한다 (IM 상태와 무관). */
    switch (raw) {
        case XK_BackSpace: unim_field_backspace(f);      break;
        case XK_Delete:    unim_field_delete(f);         break;
        case XK_Left:      unim_field_move_caret(f, -1); break;
        case XK_Right:     unim_field_move_caret(f, +1); break;
        case XK_Home:      unim_field_caret_home(f);     break;
        case XK_End:       unim_field_caret_end(f);      break;
        case XK_Escape:    unim_field_clear(f);          break;
        case XK_Return:
        case XK_KP_Enter:
            if (f->hint == UNIM_HINT_MULTILINE) unim_field_insert(f, "\n");
            else focus_field((A.active + 1) % UNIM_SPEC_N_CORE_FIELDS, "enter");
            break;
        default: break;
    }
    after_change();
}

static void handle_click(XButtonEvent *ev) {
    int hit = unim_field_hit(A.fields, UNIM_SPEC_N_CORE_FIELDS, ev->x, ev->y);
    if (hit < 0) {
        unim_log_click(ev->x, ev->y, "(빈 곳)", -1, -1);
        return;
    }
    if (hit != A.active) {
        focus_field(hit, "click");
    } else if (cur()->composing || cur()->preedit[0]) {
        /* 같은 필드 안에서 조합 중 클릭 — 2026-08-06 회귀의 재현 지점. */
        unim_log_reset(cur()->id, "click-in-field");
        if (A.xic) {
            char *committed = XmbResetIC(A.xic);
            if (committed && *committed) {
                snprintf(A.last_commit, sizeof A.last_commit, "%s", committed);
                unim_field_commit(cur(), committed);
                XFree(committed);
            }
        }
        unim_field_preedit_end(cur());
    }

    UnimTestField *f = cur();
    int before = f->caret;
    f->caret = unim_field_caret_from_x(f, ev->x, measure_cb, NULL);
    unim_log_click(ev->x, ev->y, f->id, before, f->caret);
    unim_field_log_render(f);
    after_change();
}

/** 필드의 화면 절대 좌표 — Xlib 은 정확히 알려준다. */
static void emit_geometry(void) {
    int sx = 0, sy = 0;
    Window child;
    XTranslateCoordinates(A.dpy, A.win, DefaultRootWindow(A.dpy), 0, 0,
                          &sx, &sy, &child);

    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++) {
        const UnimTestField *f = &A.fields[i];
        char kv[640];
        snprintf(kv, sizeof kv,
                 "\"field\":\"%s\",\"x\":%d,\"y\":%d,\"w\":%d,\"h\":%d,"
                 "\"cx\":%d,\"cy\":%d,\"screen_cx\":%d,\"screen_cy\":%d",
                 f->id, f->x, f->y, f->w, f->h,
                 f->x + f->w / 2, f->y + f->h / 2,
                 sx + f->x + f->w / 2, sy + f->y + f->h / 2);
        unim_log_raw("geometry", kv);
    }
}

/* ─── 메인 루프 ───────────────────────────────────────────────────────── */

/**
 * X 이벤트와 GMainContext(공용 DBus 모듈이 쓰는 gio)를 함께 돌린다.
 * X fd 를 select 로 기다리되 타임아웃을 두어 glib 쪽도 굶지 않게 한다.
 */
static void main_loop(void) {
    int xfd = ConnectionNumber(A.dpy);
    A.running = 1;

    while (A.running) {
        while (XPending(A.dpy)) {
            XEvent ev;
            XNextEvent(A.dpy, &ev);

            /* XFilterEvent 앞에서 원본 키를 먼저 남긴다 — 앱이 그 키를
             * "받기는 했는지" 와 "IM 이 삼켰는지" 를 구분해야 XTest 재주입
             * 같은 문제를 추적할 수 있다. */
            if (ev.type == KeyPress || ev.type == KeyRelease)
                unim_log_note("X 수신: type=%d keycode=%u state=0x%x",
                              ev.type, ev.xkey.keycode, ev.xkey.state);

            /* IM 이 먼저 볼 기회를 준다. 삼키면 여기서 끝. */
            if (XFilterEvent(&ev, None)) {
                if (ev.type == KeyPress || ev.type == KeyRelease)
                    unim_log_note("XFilterEvent 삼킴: type=%d keycode=%u",
                                  ev.type, ev.xkey.keycode);
                else
                    unim_log_note("XFilterEvent 삼킴: type=%d", ev.type);
                continue;
            }

            switch (ev.type) {
            case Expose:
                if (ev.xexpose.count == 0) redraw();
                break;
            case KeyPress:
                handle_key(&ev.xkey);
                break;
            case ButtonPress:
                handle_click(&ev.xbutton);
                break;
            case FocusIn:
                A.focused = 1;
                if (A.xic) XSetICFocus(A.xic);
                unim_field_set_focus(cur(), 1, NULL);
                redraw();
                break;
            case FocusOut:
                A.focused = 0;
                unim_log_reset(cur()->id, "window-focus-out");
                if (A.xic) XUnsetICFocus(A.xic);
                unim_field_set_focus(cur(), 0, NULL);
                redraw();
                break;
            case ClientMessage:
                unim_log_note("ClientMessage — 창 닫기로 본다");
                A.running = 0;
                break;
            case MapNotify:
                if (!A.ready) {
                    A.ready = 1;
                    emit_geometry();
                    unim_log_ready();
                }
                break;
            default:
                break;
            }
        }

        while (g_main_context_pending(NULL))
            g_main_context_iteration(NULL, FALSE);

        fd_set fds;
        FD_ZERO(&fds);
        FD_SET(xfd, &fds);
        struct timeval tv = { .tv_sec = 0, .tv_usec = 20000 };  /* 20ms */
        select(xfd + 1, &fds, NULL, NULL, &tv);
    }
}

/* ─── main ────────────────────────────────────────────────────────────── */

static void on_daemon_changed(void *user) { (void)user; redraw(); }

int main(int argc, char *argv[]) {
    unim_log_init(APP_NAME, argc, argv);

    int auto_mode = 0, verbose = 0;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--auto")) auto_mode = 1;
        if (!strcmp(argv[i], "-v") || !strcmp(argv[i], "--verbose")) verbose = 1;
    }
    unim_log_env("Xlib/Xft");

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

    A.dpy = XOpenDisplay(NULL);
    if (!A.dpy) {
        unim_log_error("XOpenDisplay 실패 — DISPLAY=%s",
                       getenv("DISPLAY") ? getenv("DISPLAY") : "(없음)");
        unim_log_shutdown();
        return 1;
    }
    A.screen = DefaultScreen(A.dpy);
    unim_log_set_sink(log_sink, NULL);

    A.win = XCreateSimpleWindow(A.dpy, RootWindow(A.dpy, A.screen), 0, 0,
                                UNIM_SPEC_WIN_WIDTH, UNIM_SPEC_WIN_HEIGHT, 0,
                                UNIM_SPEC_COL_BORDER, UNIM_SPEC_COL_BG);

    char title[128];
    snprintf(title, sizeof title, UNIM_SPEC_WIN_TITLE_FMT, APP_NAME);
    XStoreName(A.dpy, A.win, title);
    /* XStoreName 은 Latin-1 이라 한글 제목이 깨진다. 창 관리자와 `xdotool
     * search --name` 이 보는 것은 _NET_WM_NAME(UTF8_STRING) 쪽이다. */
    XChangeProperty(A.dpy, A.win,
                    XInternAtom(A.dpy, "_NET_WM_NAME", False),
                    XInternAtom(A.dpy, "UTF8_STRING", False), 8,
                    PropModeReplace, (const unsigned char *)title,
                    (int)strlen(title));
    Atom wm_delete = XInternAtom(A.dpy, "WM_DELETE_WINDOW", False);
    XSetWMProtocols(A.dpy, A.win, &wm_delete, 1);

    XSelectInput(A.dpy, A.win,
                 ExposureMask | KeyPressMask | FocusChangeMask |
                 StructureNotifyMask | ButtonPressMask);

    char fname[128];
    snprintf(fname, sizeof fname, "%s-%d", UNIM_SPEC_FONT_UI,
             UNIM_SPEC_FONT_SIZE_FIELD);
    A.font_field = XftFontOpenName(A.dpy, A.screen, fname);
    snprintf(fname, sizeof fname, "%s-%d", UNIM_SPEC_FONT_UI,
             UNIM_SPEC_FONT_SIZE_UI);
    A.font_ui = XftFontOpenName(A.dpy, A.screen, fname);
    snprintf(fname, sizeof fname, "%s-%d", UNIM_SPEC_FONT_MONO,
             UNIM_SPEC_FONT_SIZE_LOG);
    A.font_log = XftFontOpenName(A.dpy, A.screen, fname);

    if (!A.font_field || !A.font_ui || !A.font_log) {
        unim_log_error("폰트 열기 실패 (%s / %s)",
                       UNIM_SPEC_FONT_UI, UNIM_SPEC_FONT_MONO);
        unim_log_shutdown();
        return 1;
    }

    A.draw = XftDrawCreate(A.dpy, A.win, DefaultVisual(A.dpy, A.screen),
                           DefaultColormap(A.dpy, A.screen));

    for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
        unim_field_init(&A.fields[i], &UNIM_SPEC_CORE_FIELDS[i]);

    A.fields_top = UNIM_SPEC_MARGIN + UNIM_STATUS_N * (A.font_ui->height + 4)
                   + UNIM_SPEC_SECTION_GAP + 12;
    int bottom = unim_field_layout(A.fields, UNIM_SPEC_N_CORE_FIELDS,
                                   A.fields_top, UNIM_SPEC_WIN_WIDTH, 1.0);
    A.log_top = bottom + UNIM_SPEC_SECTION_GAP;

    XMapWindow(A.dpy, A.win);

    if (!xim_init())
        unim_log_warn("XIM 없이 계속한다 — 조합 없이 raw 키만 들어온다");

    A.daemon = unim_daemon_connect(on_daemon_changed, NULL);
    unim_field_set_focus(&A.fields[0], 1, NULL);
    A.focused = 1;

    main_loop();

    if (A.xic) XDestroyIC(A.xic);
    if (A.xim) XCloseIM(A.xim);
    unim_daemon_free(A.daemon);
    unim_log_shutdown();
    return 0;
}
