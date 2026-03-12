/*
 * UNIM XIM Test Application - Shared Types & State
 *
 * 모든 모듈에서 공유하는 타입, 상수, 전역 상태를 정의합니다.
 * 다크 테마 색상 팔레트: Catppuccin Mocha
 */

#ifndef APP_H
#define APP_H

#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xlocale.h>
#include <X11/keysym.h>
#include <X11/Xft/Xft.h>
#include <gio/gio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <locale.h>
#include <time.h>

/* ─── Window ───────────────────────────────────────────────────────────── */
#define WINDOW_WIDTH   720
#define WINDOW_HEIGHT  740
#define MARGIN         24
#define LINE_HEIGHT    28
#define FIELD_HEIGHT   36
#define FIELD_PADDING  10
#define FIELD_RADIUS   6
#define SECTION_GAP    16

/* ─── Buffers ──────────────────────────────────────────────────────────── */
#define TEXT_BUFFER_SIZE 1024
#define MAX_LOG_LINES   80

/* ─── Input Fields ─────────────────────────────────────────────────────── */
#define NUM_FIELDS 4

/* ─── Catppuccin Mocha Palette (RGB hex) ──────────────────────────────── */
#define COL_BASE       0x1e1e2e   /* 배경           */
#define COL_MANTLE     0x181825   /* 짙은 배경       */
#define COL_CRUST      0x11111b   /* 가장 짙은 배경   */
#define COL_SURFACE0   0x313244   /* 카드/필드 배경   */
#define COL_SURFACE1   0x45475a   /* 활성 필드 배경   */
#define COL_SURFACE2   0x585b70   /* 호버 배경       */
#define COL_OVERLAY0   0x6c7086   /* 비활성 텍스트    */
#define COL_OVERLAY1   0x7f849c   /* 보조 텍스트      */
#define COL_TEXT       0xcdd6f4   /* 본문 텍스트      */
#define COL_SUBTEXT1   0xbac2de   /* 라벨           */
#define COL_SUBTEXT0   0xa6adc8   /* 보조 라벨       */
#define COL_BLUE       0x89b4fa   /* 강조 / 활성     */
#define COL_LAVENDER   0xb4befe   /* preedit 표시    */
#define COL_GREEN      0xa6e3a1   /* 성공           */
#define COL_RED        0xf38ba8   /* 오류 / 경고     */
#define COL_YELLOW     0xf9e2af   /* 경고           */
#define COL_PEACH      0xfab387   /* 강조2          */
#define COL_MAUVE      0xcba6f7   /* 보라           */
#define COL_TEAL       0x94e2d5   /* 청록           */

/* ─── Input Field ─────────────────────────────────────────────────────── */
typedef struct {
    int x, y, width, height;
    char label[64];
    char text[TEXT_BUFFER_SIZE];
    char preedit[TEXT_BUFFER_SIZE];
    int  focused;
    int  cursor_pos;  /* byte offset in text (insertion point) */
} InputField;

/* ─── Global Application State ────────────────────────────────────────── */
typedef struct {
    /* X11 core */
    Display *display;
    Window   window;
    int      screen;
    GC       gc;

    /* XIM / XIC */
    XIM xim;
    XIC xic;

    /* Xft rendering */
    XftFont  *font;
    XftFont  *font_bold;
    XftFont  *font_small;
    XftDraw  *xft_draw;

    /* Theme colors (Xft) */
    XftColor c_base;
    XftColor c_mantle;
    XftColor c_surface0;
    XftColor c_surface1;
    XftColor c_surface2;
    XftColor c_overlay0;
    XftColor c_text;
    XftColor c_subtext1;
    XftColor c_subtext0;
    XftColor c_blue;
    XftColor c_lavender;
    XftColor c_green;
    XftColor c_red;
    XftColor c_yellow;
    XftColor c_peach;
    XftColor c_mauve;
    XftColor c_teal;

    /* Input fields */
    InputField fields[NUM_FIELDS];
    int        active_field;

    /* Log buffer */
    char log_buffer[MAX_LOG_LINES][256];
    int  log_count;

    /* DBus */
    GDBusProxy *dbus_proxy;
    int         dbus_connected;
    int         is_korean_mode;

    /* Status strings */
    char im_status[256];
    char dbus_status[256];
    char mode_status[64];

    /* Window dimensions (for resize) */
    int win_width;
    int win_height;
} App;

/* ─── Global app instance ─────────────────────────────────────────────── */
extern App app;

#endif /* APP_H */
