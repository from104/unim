/*
 * UNIM XIM Test - UI Module
 *
 * Xft 기반 다크 테마 렌더링
 * Catppuccin Mocha 색상 팔레트 사용
 */

#include "app.h"
#include "ui.h"

/* ─── Helper: HEX → XRenderColor ─────────────────────────────────────── */

static XRenderColor hex_to_render(unsigned long hex) {
    XRenderColor rc;
    rc.red   = (unsigned short)(((hex >> 16) & 0xFF) * 0x101);
    rc.green = (unsigned short)(((hex >> 8)  & 0xFF) * 0x101);
    rc.blue  = (unsigned short)((hex & 0xFF) * 0x101);
    rc.alpha = 0xFFFF;
    return rc;
}

static void alloc_xft_color(unsigned long hex, XftColor *out) {
    XRenderColor rc = hex_to_render(hex);
    XftColorAllocValue(app.display,
                       DefaultVisual(app.display, app.screen),
                       DefaultColormap(app.display, app.screen),
                       &rc, out);
}

static void free_xft_color(XftColor *c) {
    XftColorFree(app.display,
                 DefaultVisual(app.display, app.screen),
                 DefaultColormap(app.display, app.screen), c);
}

/* ─── Helper: text width ──────────────────────────────────────────────── */

static int tw(XftFont *f, const char *text) {
    if (!f || !text || !*text) return 0;
    XGlyphInfo ext;
    XftTextExtentsUtf8(app.display, f, (const FcChar8 *)text,
                       (int)strlen(text), &ext);
    return ext.xOff;
}

/* ─── Helper: draw filled rounded rectangle (approximated) ───────────── */

static void fill_rounded_rect(unsigned long color,
                               int x, int y, int w, int h, int r)
{
    XSetForeground(app.display, app.gc, color);

    /* Center cross */
    XFillRectangle(app.display, app.window, app.gc,
                   x + r, y, w - 2 * r, h);
    /* Left strip */
    XFillRectangle(app.display, app.window, app.gc,
                   x, y + r, r, h - 2 * r);
    /* Right strip */
    XFillRectangle(app.display, app.window, app.gc,
                   x + w - r, y + r, r, h - 2 * r);

    /* Four rounded corners via arcs */
    XFillArc(app.display, app.window, app.gc,
             x, y, 2 * r, 2 * r, 64 * 90, 64 * 90);
    XFillArc(app.display, app.window, app.gc,
             x + w - 2 * r, y, 2 * r, 2 * r, 0, 64 * 90);
    XFillArc(app.display, app.window, app.gc,
             x, y + h - 2 * r, 2 * r, 2 * r, 64 * 180, 64 * 90);
    XFillArc(app.display, app.window, app.gc,
             x + w - 2 * r, y + h - 2 * r, 2 * r, 2 * r, 64 * 270, 64 * 90);
}

/* ─── Helper: draw rounded rect outline ──────────────────────────────── */

static void draw_rounded_border(unsigned long color,
                                 int x, int y, int w, int h, int r)
{
    XSetForeground(app.display, app.gc, color);

    /* Top / bottom lines */
    XDrawLine(app.display, app.window, app.gc,
              x + r, y, x + w - r, y);
    XDrawLine(app.display, app.window, app.gc,
              x + r, y + h, x + w - r, y + h);
    /* Left / right lines */
    XDrawLine(app.display, app.window, app.gc,
              x, y + r, x, y + h - r);
    XDrawLine(app.display, app.window, app.gc,
              x + w, y + r, x + w, y + h - r);

    /* Corner arcs */
    XDrawArc(app.display, app.window, app.gc,
             x, y, 2 * r, 2 * r, 64 * 90, 64 * 90);
    XDrawArc(app.display, app.window, app.gc,
             x + w - 2 * r, y, 2 * r, 2 * r, 0, 64 * 90);
    XDrawArc(app.display, app.window, app.gc,
             x, y + h - 2 * r, 2 * r, 2 * r, 64 * 180, 64 * 90);
    XDrawArc(app.display, app.window, app.gc,
             x + w - 2 * r, y + h - 2 * r, 2 * r, 2 * r, 64 * 270, 64 * 90);
}

/* ─── Helper: draw a status badge ────────────────────────────────────── */

static void draw_badge(int x, int y, const char *text,
                       unsigned long bg, XftColor *fg)
{
    int pad = S(8);
    int w = tw(app.font_small, text) + pad * 2;
    int h = S(22);
    fill_rounded_rect(bg, x, y, w, h, S(4));
    XftDrawStringUtf8(app.xft_draw, fg, app.font_small,
                      x + pad, y + S(16),
                      (const FcChar8 *)text, (int)strlen(text));
}

/* ─── Draw: header bar ───────────────────────────────────────────────── */

static void draw_header(void) {
    /* Dark header strip */
    XSetForeground(app.display, app.gc, COL_MANTLE);
    XFillRectangle(app.display, app.window, app.gc,
                   0, 0, app.win_width, S(56));

    /* Title */
    const char *title = "UNIM XIM Test";
    XftDrawStringUtf8(app.xft_draw, &app.c_text, app.font_bold,
                      S(MARGIN), S(36),
                      (const FcChar8 *)title, (int)strlen(title));

    /* Mode badge (right side) */
    int badge_x = app.win_width - S(MARGIN) - S(90);
    if (app.is_korean_mode) {
        draw_badge(badge_x, S(17), "한국어", COL_BLUE, &app.c_mantle);
    } else {
        draw_badge(badge_x, S(17), "English", COL_SURFACE2, &app.c_text);
    }
}

/* ─── Draw: status bar ───────────────────────────────────────────────── */

static void draw_status_bar(void) {
    int y = S(64);

    /* XIM status */
    XftDrawStringUtf8(app.xft_draw, &app.c_overlay0, app.font_small,
                      S(MARGIN), y + S(14),
                      (const FcChar8 *)"XIM:", 4);

    XftColor *xim_color = app.xim ? &app.c_green : &app.c_red;
    XftDrawStringUtf8(app.xft_draw, xim_color, app.font_small,
                      S(MARGIN) + S(40), y + S(14),
                      (const FcChar8 *)app.im_status,
                      (int)strlen(app.im_status));

    /* DBus status */
    int dbus_x = S(MARGIN) + S(170);
    XftDrawStringUtf8(app.xft_draw, &app.c_overlay0, app.font_small,
                      dbus_x, y + S(14),
                      (const FcChar8 *)"DBus:", 5);

    XftColor *dbus_color = app.dbus_connected ? &app.c_green : &app.c_red;
    XftDrawStringUtf8(app.xft_draw, dbus_color, app.font_small,
                      dbus_x + S(46), y + S(14),
                      (const FcChar8 *)app.dbus_status,
                      (int)strlen(app.dbus_status));

    /* Shortcut hints */
    const char *hint = "Tab: 이동  |  F9: 한/영  |  Enter: 다음";
    int hint_w = tw(app.font_small, hint);
    XftDrawStringUtf8(app.xft_draw, &app.c_overlay0, app.font_small,
                      app.win_width - S(MARGIN) - hint_w, y + S(14),
                      (const FcChar8 *)hint, (int)strlen(hint));

    /* Divider */
    y += S(24);
    XSetForeground(app.display, app.gc, COL_SURFACE0);
    XDrawLine(app.display, app.window, app.gc,
              S(MARGIN), y, app.win_width - S(MARGIN), y);
}

/* ─── Draw: section title ────────────────────────────────────────────── */

static int draw_section_title(int y, const char *title) {
    XftDrawStringUtf8(app.xft_draw, &app.c_blue, app.font_small,
                      S(MARGIN), y + S(14),
                      (const FcChar8 *)title, (int)strlen(title));

    /* Decorative line after the text */
    int title_w = tw(app.font_small, title);
    XSetForeground(app.display, app.gc, COL_SURFACE0);
    XDrawLine(app.display, app.window, app.gc,
              S(MARGIN) + title_w + S(10), y + S(9),
              app.win_width - S(MARGIN), y + S(9));

    return y + S(24);
}

/* ─── Draw: single field ─────────────────────────────────────────────── */

static void draw_field(InputField *field, int is_active) {
    /* Label */
    XftDrawStringUtf8(app.xft_draw, &app.c_subtext1, app.font,
                      S(MARGIN), field->y + field->height - S(10),
                      (const FcChar8 *)field->label,
                      (int)strlen(field->label));

    /* Field background */
    unsigned long bg = is_active ? COL_SURFACE1 : COL_SURFACE0;
    fill_rounded_rect(bg, field->x, field->y,
                      field->width, field->height, S(FIELD_RADIUS));

    /* Border */
    if (is_active) {
        draw_rounded_border(COL_BLUE,
                            field->x, field->y,
                            field->width, field->height, S(FIELD_RADIUS));
    }

    /* Text content */
    int text_x = field->x + S(FIELD_PADDING);
    int text_y = field->y + field->height - S(10);
    int cur_pos = field->cursor_pos;

    /* 커서 앞 텍스트 */
    if (cur_pos > 0) {
        XftDrawStringUtf8(app.xft_draw, &app.c_text, app.font,
                          text_x, text_y,
                          (const FcChar8 *)field->text, cur_pos);
        XGlyphInfo ext;
        XftTextExtentsUtf8(app.display, app.font,
                           (const FcChar8 *)field->text, cur_pos, &ext);
        text_x += ext.xOff;
    }

    /* preedit (커서 위치에 표시) */
    if (field->preedit[0]) {
        int pe_w = tw(app.font, field->preedit);

        /* Preedit background stripe */
        XSetForeground(app.display, app.gc, COL_LAVENDER & 0xFFFFFF);
        XFillRectangle(app.display, app.window, app.gc,
                       text_x - 1, text_y - S(16), pe_w + 2, S(20));

        /* Preedit text (dark on lavender) */
        XftDrawStringUtf8(app.xft_draw, &app.c_mantle, app.font,
                          text_x, text_y,
                          (const FcChar8 *)field->preedit,
                          (int)strlen(field->preedit));
        text_x += pe_w;
    }

    /* 커서 (preedit 없을 때) */
    if (is_active && !field->preedit[0]) {
        XSetForeground(app.display, app.gc, COL_BLUE);
        XFillRectangle(app.display, app.window, app.gc,
                       text_x + 1, field->y + S(6), S(2), field->height - S(12));
    }

    /* 커서 뒤 텍스트 */
    if (field->text[cur_pos]) {
        XftDrawStringUtf8(app.xft_draw, &app.c_text, app.font,
                          text_x, text_y,
                          (const FcChar8 *)&field->text[cur_pos],
                          (int)strlen(&field->text[cur_pos]));
    }
}

/* ─── Draw: input fields section ─────────────────────────────────────── */

static int draw_fields_section(int y) {
    y = draw_section_title(y, "입력 필드");

    for (int i = 0; i < NUM_FIELDS; i++) {
        draw_field(&app.fields[i], i == app.active_field);
    }

    return app.fields[NUM_FIELDS - 1].y + S(FIELD_HEIGHT) + S(SECTION_GAP);
}

/* ─── Draw: log section ──────────────────────────────────────────────── */

static void draw_log_section(int y) {
    y = draw_section_title(y, "이벤트 로그");
    y += 4;

    int line_h = S(18);
    int max_visible = (app.win_height - y - S(MARGIN)) / line_h;
    if (max_visible < 1) max_visible = 1;

    int start = app.log_count > max_visible
                    ? app.log_count - max_visible : 0;

    for (int i = start; i < app.log_count && y < app.win_height - S(MARGIN); i++) {
        /* Split timestamp from message for color coding */
        const char *line = app.log_buffer[i];
        const char *close_bracket = strchr(line, ']');

        if (close_bracket) {
            int ts_len = (int)(close_bracket - line + 1);

            /* Timestamp in dim color */
            XftDrawStringUtf8(app.xft_draw, &app.c_overlay0, app.font_small,
                              S(MARGIN) + S(8), y,
                              (const FcChar8 *)line, ts_len);

            /* Message in brighter color */
            const char *msg = close_bracket + 1;
            while (*msg == ' ') msg++;
            int ts_w = tw(app.font_small, app.log_buffer[i]);
            /* Use the full-width approach: timestamp then message */
            XftDrawStringUtf8(app.xft_draw, &app.c_subtext0, app.font_small,
                              S(MARGIN) + S(8) + tw(app.font_small, line) -
                              tw(app.font_small, msg),
                              y,
                              (const FcChar8 *)msg, (int)strlen(msg));
            (void)ts_w;
        } else {
            XftDrawStringUtf8(app.xft_draw, &app.c_subtext0, app.font_small,
                              S(MARGIN) + S(8), y,
                              (const FcChar8 *)line, (int)strlen(line));
        }

        y += line_h;
    }
}

/* ─── Public API ──────────────────────────────────────────────────────── */

int ui_init_fonts(void) {
    app.font = XftFontOpenName(app.display, app.screen,
                               "D2Coding:size=13");
    if (!app.font)
        app.font = XftFontOpenName(app.display, app.screen,
                                    "monospace:size=13");
    if (!app.font) return 0;

    app.font_bold = XftFontOpenName(app.display, app.screen,
                                     "D2Coding:size=16:weight=bold");
    if (!app.font_bold)
        app.font_bold = XftFontOpenName(app.display, app.screen,
                                         "sans:size=16:weight=bold");

    app.font_small = XftFontOpenName(app.display, app.screen,
                                      "D2Coding:size=11");
    if (!app.font_small)
        app.font_small = XftFontOpenName(app.display, app.screen,
                                          "monospace:size=11");

    return 1;
}

void ui_init_colors(void) {
    alloc_xft_color(COL_BASE,     &app.c_base);
    alloc_xft_color(COL_MANTLE,   &app.c_mantle);
    alloc_xft_color(COL_SURFACE0, &app.c_surface0);
    alloc_xft_color(COL_SURFACE1, &app.c_surface1);
    alloc_xft_color(COL_SURFACE2, &app.c_surface2);
    alloc_xft_color(COL_OVERLAY0, &app.c_overlay0);
    alloc_xft_color(COL_TEXT,     &app.c_text);
    alloc_xft_color(COL_SUBTEXT1, &app.c_subtext1);
    alloc_xft_color(COL_SUBTEXT0, &app.c_subtext0);
    alloc_xft_color(COL_BLUE,     &app.c_blue);
    alloc_xft_color(COL_LAVENDER, &app.c_lavender);
    alloc_xft_color(COL_GREEN,    &app.c_green);
    alloc_xft_color(COL_RED,      &app.c_red);
    alloc_xft_color(COL_YELLOW,   &app.c_yellow);
    alloc_xft_color(COL_PEACH,    &app.c_peach);
    alloc_xft_color(COL_MAUVE,    &app.c_mauve);
    alloc_xft_color(COL_TEAL,     &app.c_teal);
}

void ui_init_draw(void) {
    app.xft_draw = XftDrawCreate(app.display, app.window,
                                  DefaultVisual(app.display, app.screen),
                                  DefaultColormap(app.display, app.screen));
}

void ui_redraw(void) {
    /* Clear with base color */
    XSetForeground(app.display, app.gc, COL_BASE);
    XFillRectangle(app.display, app.window, app.gc,
                   0, 0, app.win_width, app.win_height);

    draw_header();
    draw_status_bar();

    int y = S(100);
    y = draw_fields_section(y);
    draw_log_section(y);

    XFlush(app.display);
}

void ui_cleanup(void) {
    free_xft_color(&app.c_base);
    free_xft_color(&app.c_mantle);
    free_xft_color(&app.c_surface0);
    free_xft_color(&app.c_surface1);
    free_xft_color(&app.c_surface2);
    free_xft_color(&app.c_overlay0);
    free_xft_color(&app.c_text);
    free_xft_color(&app.c_subtext1);
    free_xft_color(&app.c_subtext0);
    free_xft_color(&app.c_blue);
    free_xft_color(&app.c_lavender);
    free_xft_color(&app.c_green);
    free_xft_color(&app.c_red);
    free_xft_color(&app.c_yellow);
    free_xft_color(&app.c_peach);
    free_xft_color(&app.c_mauve);
    free_xft_color(&app.c_teal);

    if (app.xft_draw) { XftDrawDestroy(app.xft_draw); app.xft_draw = NULL; }
    if (app.font)      { XftFontClose(app.display, app.font);      app.font = NULL; }
    if (app.font_bold) { XftFontClose(app.display, app.font_bold); app.font_bold = NULL; }
    if (app.font_small){ XftFontClose(app.display, app.font_small);app.font_small = NULL; }
}
