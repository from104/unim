/*
 * UNIM XIM Input Method Test Application
 * 
 * 순수 X11/Xlib를 사용하여 XIM 입력기를 테스트하는 앱입니다.
 * GTK, Qt 등의 툴킷을 사용하지 않습니다.
 * DBus를 통해 unim-daemon과 통신하여 한/영 상태를 실시간으로 표시합니다.
 * 
 * [2026-01-19] Xft를 사용하여 폰트 렌더링 개선 (D2Coding)
 * [2026-01-22] 여러 입력 필드 지원 추가
 * [2026-01-25] DBus 통합 추가
 */

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
#include <wchar.h>

/* 창 크기 */
#define WINDOW_WIDTH 700
#define WINDOW_HEIGHT 700
#define MARGIN 20
#define LINE_HEIGHT 30
#define FIELD_HEIGHT 28
#define FIELD_PADDING 6

/* 텍스트 버퍼 */
#define TEXT_BUFFER_SIZE 1024
#define LOG_BUFFER_SIZE 8192
#define MAX_LOG_LINES 50

/* 입력 필드 개수 */
#define NUM_FIELDS 4

/* 입력 필드 구조체 */
typedef struct {
    int x, y, width, height;
    char label[64];
    char text[TEXT_BUFFER_SIZE];
    char preedit[TEXT_BUFFER_SIZE];
    int focused;
} InputField;

/* 전역 변수 */
static Display *display = NULL;
static Window window;
static int screen;
static GC gc;
static XIM xim = NULL;
static XIC xic = NULL;

/* Xft 리소스 */
static XftFont *font = NULL;
static XftDraw *xft_draw = NULL;
static XftColor color_black, color_white, color_gray, color_blue, color_bg;

/* 입력 필드들 */
static InputField fields[NUM_FIELDS];
static int active_field = 0;

/* 로그 버퍼 */
static char log_buffer[MAX_LOG_LINES][256];
static int log_count = 0;

/* DBus */
static GDBusProxy *dbus_proxy = NULL;
static int dbus_connected = 0;
static int is_korean_mode = 1;  /* 1: 한국어, 0: 영어 */

/* 상태 정보 */
static char im_status[256] = "(XIM 연결 중...)";
static char dbus_status[256] = "(DBus 연결 중...)";
static char mode_status[32] = "(감지 중...)";

/* 전방 선언 */
static void add_log(const char *format, ...);
static void redraw(void);

/* DBus 시그널 핸들러 */
static void on_dbus_signal(GDBusProxy *proxy,
                           const gchar *sender_name,
                           const gchar *signal_name,
                           GVariant *parameters,
                           gpointer user_data) {
    (void)proxy; (void)sender_name; (void)user_data;
    
    if (g_strcmp0(signal_name, "GlobalModeChanged") == 0) {
        gboolean is_korean;
        g_variant_get(parameters, "(b)", &is_korean);
        is_korean_mode = is_korean ? 1 : 0;
        snprintf(mode_status, sizeof(mode_status), is_korean ? "🇰🇷 한국어" : "🔤 영어");
        add_log("입력 모드 변경: %s", is_korean ? "한국어" : "영어");
    }
}

/* DBus 초기화 */
static void init_dbus(void) {
    GError *error = NULL;
    
    dbus_proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION,
        G_DBUS_PROXY_FLAGS_NONE,
        NULL,
        "org.atit.unim.InputMethod",
        "/org/atit/unim/InputMethod",
        "org.atit.unim.InputMethod",
        NULL,
        &error
    );
    
    if (error) {
        snprintf(dbus_status, sizeof(dbus_status), "❌ unim-daemon 없음");
        add_log("DBus 연결 실패: %s", error->message);
        g_error_free(error);
        return;
    }
    
    /* 시그널 연결 */
    g_signal_connect(dbus_proxy, "g-signal", G_CALLBACK(on_dbus_signal), NULL);
    
    dbus_connected = 1;
    snprintf(dbus_status, sizeof(dbus_status), "✅ DBus 연결됨");
    add_log("DBus 연결 성공 - GlobalModeChanged 시그널 구독");
    
    /* 초기 모드 조회 */
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
    
    if (result) {
        gboolean is_korean;
        g_variant_get(result, "(b)", &is_korean);
        is_korean_mode = is_korean ? 1 : 0;
        snprintf(mode_status, sizeof(mode_status), is_korean ? "🇰🇷 한국어" : "🔤 영어");
        add_log("초기 입력 모드: %s", is_korean ? "한국어" : "영어");
        g_variant_unref(result);
    }
}

/* 한/영 토글 */
static void toggle_mode(void) {
    if (!dbus_proxy) {
        add_log("DBus 연결 없음");
        return;
    }
    
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
    
    if (result) {
        gboolean current_mode;
        g_variant_get(result, "(b)", &current_mode);
        g_variant_unref(result);
        
        g_dbus_proxy_call_sync(
            dbus_proxy, "SetGlobalMode",
            g_variant_new("(b)", !current_mode),
            G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
    }
}

/* 로그 추가 함수 */
static void add_log(const char *format, ...) {
    va_list args;
    va_start(args, format);
    
    time_t now = time(NULL);
    struct tm *tm_info = localtime(&now);
    char timestamp[32];
    strftime(timestamp, sizeof(timestamp), "[%H:%M:%S] ", tm_info);
    
    char message[224];
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);
    
    if (log_count >= MAX_LOG_LINES) {
        for (int i = 0; i < MAX_LOG_LINES - 1; i++) {
            strcpy(log_buffer[i], log_buffer[i + 1]);
        }
        log_count = MAX_LOG_LINES - 1;
    }
    snprintf(log_buffer[log_count], sizeof(log_buffer[log_count]), "%s%s", timestamp, message);
    log_count++;
    
    printf("%s%s\n", timestamp, message);
}

/* 유틸리티: 문자열 폭 계산 */
static int get_text_width(const char *text) {
    if (!font || !text || !*text) return 0;
    XGlyphInfo extents;
    XftTextExtentsUtf8(display, font, (const FcChar8 *)text, strlen(text), &extents);
    return extents.xOff;
}

/* 입력 필드 초기화 */
static void init_fields(void) {
    int y_start = 100;
    int field_width = WINDOW_WIDTH - 2 * MARGIN - 100;
    
    for (int i = 0; i < NUM_FIELDS; i++) {
        fields[i].x = MARGIN + 100;
        fields[i].y = y_start + i * (FIELD_HEIGHT + 20);
        fields[i].width = field_width;
        fields[i].height = FIELD_HEIGHT;
        fields[i].text[0] = '\0';
        fields[i].preedit[0] = '\0';
        fields[i].focused = (i == 0) ? 1 : 0;
    }
    
    strcpy(fields[0].label, "이름:");
    strcpy(fields[1].label, "주소:");
    strcpy(fields[2].label, "전화번호:");
    strcpy(fields[3].label, "메모:");
}

/* 입력 필드 그리기 */
static void draw_field(InputField *field, int is_active) {
    /* 라벨 */
    XftDrawStringUtf8(xft_draw, &color_black, font, 
        MARGIN, field->y + FIELD_HEIGHT - 8,
        (const FcChar8 *)field->label, strlen(field->label));
    
    /* 필드 배경 */
    if (is_active) {
        XSetForeground(display, gc, 0xFFFFE0);  /* 연한 노란색 */
    } else {
        XSetForeground(display, gc, 0xFFFFFF);  /* 흰색 */
    }
    XFillRectangle(display, window, gc, 
        field->x, field->y, field->width, field->height);
    
    /* 테두리 */
    if (is_active) {
        XSetForeground(display, gc, 0x4169E1);  /* 파란색 */
        XDrawRectangle(display, window, gc, 
            field->x, field->y, field->width, field->height);
        XDrawRectangle(display, window, gc, 
            field->x + 1, field->y + 1, field->width - 2, field->height - 2);
    } else {
        XSetForeground(display, gc, 0xAAAAAA);  /* 회색 */
        XDrawRectangle(display, window, gc, 
            field->x, field->y, field->width, field->height);
    }
    
    /* 텍스트 + preedit */
    char display_text[TEXT_BUFFER_SIZE * 2];
    snprintf(display_text, sizeof(display_text), "%s%s", field->text, field->preedit);
    
    XftDrawStringUtf8(xft_draw, &color_black, font,
        field->x + FIELD_PADDING, field->y + FIELD_HEIGHT - 8,
        (const FcChar8 *)display_text, strlen(display_text));
    
    /* 커서 (활성 필드만) */
    if (is_active) {
        int cursor_x = field->x + FIELD_PADDING + get_text_width(display_text);
        XSetForeground(display, gc, 0x000000);
        XDrawLine(display, window, gc, 
            cursor_x, field->y + 4, cursor_x, field->y + field->height - 4);
    }
}

/* 화면 다시 그리기 */
static void redraw(void) {
    /* 배경 지우기 */
    XSetForeground(display, gc, 0xF0F0F0);
    XFillRectangle(display, window, gc, 0, 0, WINDOW_WIDTH, WINDOW_HEIGHT);
    
    int y = MARGIN + LINE_HEIGHT;
    
    /* 제목 */
    const char *title = "UNIM XIM 입력기 테스트 - 다중 입력 필드";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y, 
        (const FcChar8 *)title, strlen(title));
    y += LINE_HEIGHT;
    
    /* 상태 표시 */
    char status_line[512];
    snprintf(status_line, sizeof(status_line), "XIM: %s | DBus: %s | 모드: %s", 
        im_status, dbus_status, mode_status);
    XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN, y, 
        (const FcChar8 *)status_line, strlen(status_line));
    y += LINE_HEIGHT;
    
    snprintf(status_line, sizeof(status_line), "현재 필드: %s (Tab: 이동, F9: 한/영 전환)", 
        fields[active_field].label);
    XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN, y, 
        (const FcChar8 *)status_line, strlen(status_line));
    y += LINE_HEIGHT + 10;
    
    /* 구분선 */
    XSetForeground(display, gc, 0xCCCCCC);
    XDrawLine(display, window, gc, MARGIN, y, WINDOW_WIDTH - MARGIN, y);
    y += 20;
    
    /* 입력 필드들 그리기 */
    for (int i = 0; i < NUM_FIELDS; i++) {
        draw_field(&fields[i], i == active_field);
    }
    
    /* 안내 메시지 */
    y = fields[NUM_FIELDS - 1].y + FIELD_HEIGHT + 30;
    const char *help_msg = "Tab: 다음 필드, Shift+Tab: 이전 필드, Ctrl+C: 종료";
    XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN, y, 
        (const FcChar8 *)help_msg, strlen(help_msg));
    y += LINE_HEIGHT + 10;
    
    /* 구분선 */
    XSetForeground(display, gc, 0xCCCCCC);
    XDrawLine(display, window, gc, MARGIN, y, WINDOW_WIDTH - MARGIN, y);
    y += 10;
    
    /* 로그 섹션 */
    const char *log_title = "=== 이벤트 로그 ===";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y + LINE_HEIGHT, 
        (const FcChar8 *)log_title, strlen(log_title));
    y += LINE_HEIGHT * 2;
    
    /* 로그 출력 */
    int visible_lines = (WINDOW_HEIGHT - y - MARGIN) / (LINE_HEIGHT - 10);
    int start_log = log_count > visible_lines ? log_count - visible_lines : 0;
    
    for (int i = start_log; i < log_count && y < WINDOW_HEIGHT - MARGIN; i++) {
        XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN + 20, y, 
            (const FcChar8 *)log_buffer[i], strlen(log_buffer[i]));
        y += LINE_HEIGHT - 10;
    }
    
    XFlush(display);
}

/* 필드 변경 시 XIC 갱신 */
static void update_spot_location(void) {
    if (!xic) return;
    
    InputField *field = &fields[active_field];
    char display_text[TEXT_BUFFER_SIZE * 2];
    snprintf(display_text, sizeof(display_text), "%s%s", field->text, field->preedit);
    
    XPoint spot;
    spot.x = field->x + FIELD_PADDING + get_text_width(display_text);
    spot.y = field->y + FIELD_HEIGHT - 4;
    
    XVaNestedList preedit_attr = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
    XSetICValues(xic, XNPreeditAttributes, preedit_attr, NULL);
    XFree(preedit_attr);
}

/* 필드 전환 */
static void switch_field(int direction) {
    /* 현재 필드의 preedit 커밋 */
    InputField *current = &fields[active_field];
    if (current->preedit[0] != '\0') {
        strcat(current->text, current->preedit);
        current->preedit[0] = '\0';
        add_log("필드 전환 시 preedit 커밋: \"%s\"", current->text);
    }
    
    /* XIC 리셋 */
    if (xic) {
        char *committed = XmbResetIC(xic);
        if (committed && *committed) {
            strcat(current->text, committed);
            XFree(committed);
        }
    }
    
    /* 다음/이전 필드로 이동 */
    active_field = (active_field + direction + NUM_FIELDS) % NUM_FIELDS;
    
    add_log("필드 전환: %s", fields[active_field].label);
    
    /* spot location 갱신 */
    update_spot_location();
    
    redraw();
}

/* XIM preedit 콜백들 */
static int preedit_start_callback(XIC ic, XPointer client_data, XPointer call_data) {
    (void)ic; (void)client_data; (void)call_data;
    add_log("[%s] Preedit Start", fields[active_field].label);
    return -1;
}

static void preedit_done_callback(XIC ic, XPointer client_data, XPointer call_data) {
    (void)ic; (void)client_data; (void)call_data;
    add_log("[%s] Preedit Done", fields[active_field].label);
    fields[active_field].preedit[0] = '\0';
    redraw();
}

static void preedit_draw_callback(XIC ic, XPointer client_data, XIMPreeditDrawCallbackStruct *call_data) {
    (void)ic; (void)client_data;
    
    InputField *field = &fields[active_field];
    
    if (call_data && call_data->text && call_data->text->string.multi_byte) {
        strncpy(field->preedit, call_data->text->string.multi_byte, sizeof(field->preedit) - 1);
        field->preedit[sizeof(field->preedit) - 1] = '\0';
        add_log("[%s] Preedit: \"%s\"", field->label, field->preedit);
    } else {
        field->preedit[0] = '\0';
    }
    
    update_spot_location();
    redraw();
}

static void preedit_caret_callback(XIC ic, XPointer client_data, XIMPreeditCaretCallbackStruct *call_data) {
    (void)ic; (void)client_data; (void)call_data;
}

/* XIM 초기화 */
static int init_xim(void) {
    if (setlocale(LC_ALL, "") == NULL) {
        add_log("경고: 로케일 설정 실패");
    }
    
    if (!XSupportsLocale()) {
        add_log("경고: X에서 현재 로케일을 지원하지 않음");
    }
    
    if (XSetLocaleModifiers("") == NULL) {
        add_log("경고: XSetLocaleModifiers 실패");
    }
    
    const char *xmodifiers = getenv("XMODIFIERS");
    add_log("XMODIFIERS=%s", xmodifiers ? xmodifiers : "(unset)");
    
    xim = XOpenIM(display, NULL, NULL, NULL);
    if (xim == NULL) {
        add_log("오류: XOpenIM 실패");
        strcpy(im_status, "연결 실패");
        return 0;
    }
    
    add_log("XIM 열기 성공");
    
    XIMStyles *im_styles = NULL;
    if (XGetIMValues(xim, XNQueryInputStyle, &im_styles, NULL) != NULL || im_styles == NULL) {
        add_log("오류: 입력 스타일 쿼리 실패");
        return 0;
    }
    
    XIMStyle best_style = 0;
    XIMStyle wanted_styles[] = {
        XIMPreeditCallbacks | XIMStatusNothing,
        XIMPreeditPosition | XIMStatusNothing,
        XIMPreeditNothing | XIMStatusNothing,
        0
    };
    
    for (int w = 0; wanted_styles[w] != 0; w++) {
        for (unsigned long i = 0; i < im_styles->count_styles; i++) {
            if (im_styles->supported_styles[i] == wanted_styles[w]) {
                best_style = wanted_styles[w];
                break;
            }
        }
        if (best_style != 0) break;
    }
    
    XFree(im_styles);
    
    if (best_style == 0) {
        add_log("오류: 적합한 입력 스타일 없음");
        return 0;
    }
    
    add_log("입력 스타일: 0x%lx", (unsigned long)best_style);
    
    XVaNestedList preedit_attr = NULL;
    
    if (best_style & XIMPreeditCallbacks) {
        XIMCallback preedit_start_cb = { NULL, (XIMProc)preedit_start_callback };
        XIMCallback preedit_done_cb = { NULL, (XIMProc)preedit_done_callback };
        XIMCallback preedit_draw_cb = { NULL, (XIMProc)preedit_draw_callback };
        XIMCallback preedit_caret_cb = { NULL, (XIMProc)preedit_caret_callback };
        
        preedit_attr = XVaCreateNestedList(0,
            XNPreeditStartCallback, &preedit_start_cb,
            XNPreeditDoneCallback, &preedit_done_cb,
            XNPreeditDrawCallback, &preedit_draw_cb,
            XNPreeditCaretCallback, &preedit_caret_cb,
            NULL);
        
        xic = XCreateIC(xim,
            XNInputStyle, best_style,
            XNClientWindow, window,
            XNFocusWindow, window,
            XNPreeditAttributes, preedit_attr,
            NULL);
    } else if (best_style & XIMPreeditPosition) {
        /* Over-the-spot 스타일 */
        XPoint spot = { fields[0].x + FIELD_PADDING, fields[0].y + FIELD_HEIGHT - 4 };
        XFontSet font_set = NULL;  /* 필요시 설정 */
        
        preedit_attr = XVaCreateNestedList(0,
            XNSpotLocation, &spot,
            NULL);
        
        xic = XCreateIC(xim,
            XNInputStyle, best_style,
            XNClientWindow, window,
            XNFocusWindow, window,
            XNPreeditAttributes, preedit_attr,
            NULL);
    } else {
        xic = XCreateIC(xim,
            XNInputStyle, best_style,
            XNClientWindow, window,
            XNFocusWindow, window,
            NULL);
    }
    
    if (preedit_attr) {
        XFree(preedit_attr);
    }
    
    if (xic == NULL) {
        add_log("오류: XCreateIC 실패");
        strcpy(im_status, "XIC 생성 실패");
        return 0;
    }
    
    add_log("XIC 생성 성공");
    strcpy(im_status, "연결됨");
    
    long im_event_mask = 0;
    if (XGetICValues(xic, XNFilterEvents, &im_event_mask, NULL) == NULL) {
        add_log("XIC 이벤트 마스크: 0x%lx", im_event_mask);
        XSelectInput(display, window, 
            ExposureMask | KeyPressMask | KeyReleaseMask | 
            FocusChangeMask | StructureNotifyMask | ButtonPressMask | im_event_mask);
    }
    
    return 1;
}

/* 키 이벤트 처리 */
static void handle_key_event(XKeyEvent *event) {
    KeySym keysym;
    Status status;
    char buf[256] = "";
    int len = 0;
    
    InputField *field = &fields[active_field];
    
    if (xic) {
        len = XmbLookupString(xic, event, buf, sizeof(buf) - 1, &keysym, &status);
        
        /* Tab 키: 필드 전환 */
        if (keysym == XK_Tab || keysym == XK_ISO_Left_Tab) {
            int direction = (event->state & ShiftMask) ? -1 : 1;
            switch_field(direction);
            return;
        }
        
        /* F9: 한/영 전환 */
        if (keysym == XK_F9) {
            toggle_mode();
            return;
        }
        
        if (status == XLookupChars || status == XLookupBoth) {
            buf[len] = '\0';
            
            int cur_len = strlen(field->text);
            if (cur_len + len < TEXT_BUFFER_SIZE - 1) {
                strcat(field->text, buf);
                add_log("[%s] 입력: \"%s\"", field->label, buf);
            }
            
            field->preedit[0] = '\0';
        }
        
        if (status == XLookupKeySym || status == XLookupBoth) {
            if (keysym == XK_BackSpace) {
                int text_len = strlen(field->text);
                if (text_len > 0) {
                    int i = text_len - 1;
                    while (i > 0 && (field->text[i] & 0xC0) == 0x80) {
                        i--;
                    }
                    field->text[i] = '\0';
                    add_log("[%s] Backspace", field->label);
                }
            } else if (keysym == XK_Return || keysym == XK_KP_Enter) {
                add_log("[%s] Enter", field->label);
                switch_field(1);  /* 다음 필드로 이동 */
                return;
            } else if (keysym == XK_Escape) {
                field->preedit[0] = '\0';
                add_log("[%s] Escape - preedit 취소", field->label);
            }
        }
    } else {
        len = XLookupString(event, buf, sizeof(buf) - 1, &keysym, NULL);
        if (len > 0) {
            buf[len] = '\0';
            strcat(field->text, buf);
        }
    }
    
    update_spot_location();
    redraw();
}

/* 마우스 클릭으로 필드 선택 */
static void handle_button_press(XButtonEvent *event) {
    for (int i = 0; i < NUM_FIELDS; i++) {
        InputField *field = &fields[i];
        if (event->x >= field->x && event->x <= field->x + field->width &&
            event->y >= field->y && event->y <= field->y + field->height) {
            
            if (i != active_field) {
                /* 현재 필드 preedit 커밋 */
                InputField *current = &fields[active_field];
                if (current->preedit[0] != '\0') {
                    strcat(current->text, current->preedit);
                    current->preedit[0] = '\0';
                }
                
                if (xic) {
                    char *committed = XmbResetIC(xic);
                    if (committed && *committed) {
                        strcat(current->text, committed);
                        XFree(committed);
                    }
                }
                
                active_field = i;
                add_log("마우스로 필드 선택: %s", fields[active_field].label);
                update_spot_location();
                redraw();
            }
            break;
        }
    }
}

int main(int argc, char *argv[]) {
    (void)argc; (void)argv;
    
    setlocale(LC_ALL, "");
    
    display = XOpenDisplay(NULL);
    if (display == NULL) {
        fprintf(stderr, "오류: X 디스플레이를 열 수 없습니다.\n");
        return 1;
    }
    
    screen = DefaultScreen(display);
    
    add_log("UNIM XIM 입력기 테스트 앱 시작 (다중 필드)");
    add_log("DISPLAY=%s", getenv("DISPLAY") ? getenv("DISPLAY") : "(unset)");
    
    /* 창 생성 */
    window = XCreateSimpleWindow(display, RootWindow(display, screen),
        100, 100, WINDOW_WIDTH, WINDOW_HEIGHT,
        1, BlackPixel(display, screen), WhitePixel(display, screen));
    
    XStoreName(display, window, "UNIM XIM Test - Multiple Fields");
    
    /* GC 생성 */
    gc = XCreateGC(display, window, 0, NULL);
    
    /* Xft 초기화 */
    font = XftFontOpenName(display, screen, "D2Coding:size=14");
    if (!font) {
        font = XftFontOpenName(display, screen, "monospace:size=14");
    }
    
    if (!font) {
        fprintf(stderr, "오류: 폰트를 열 수 없습니다.\n");
        return 1;
    }
    add_log("폰트 로드 성공");
    
    xft_draw = XftDrawCreate(display, window, 
        DefaultVisual(display, screen), DefaultColormap(display, screen));
    
    /* 색상 할당 */
    XRenderColor render_black = { 0, 0, 0, 0xFFFF };
    XRenderColor render_white = { 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF };
    XRenderColor render_gray = { 0x6666, 0x6666, 0x6666, 0xFFFF };
    XRenderColor render_blue = { 0x4141, 0x6969, 0xE1E1, 0xFFFF };
    
    XftColorAllocValue(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &render_black, &color_black);
    XftColorAllocValue(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &render_white, &color_white);
    XftColorAllocValue(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &render_gray, &color_gray);
    XftColorAllocValue(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &render_blue, &color_blue);
    
    /* 입력 필드 초기화 */
    init_fields();
    
    /* 창 표시 */
    XMapWindow(display, window);
    XFlush(display);
    
    /* DBus 초기화 */
    init_dbus();
    
    /* XIM 초기화 */
    init_xim();
    
    /* WM_DELETE_WINDOW 처리 */
    Atom wm_delete = XInternAtom(display, "WM_DELETE_WINDOW", False);
    XSetWMProtocols(display, window, &wm_delete, 1);
    
    add_log("이벤트 루프 시작");
    
    /* 이벤트 루프 */
    XEvent xevent;
    int running = 1;
    int x11_fd = ConnectionNumber(display);
    
    while (running) {
        /* DBus 이벤트 처리 */
        GMainContext *context = g_main_context_default();
        while (g_main_context_pending(context)) {
            g_main_context_iteration(context, FALSE);
        }
        
        /* X11 이벤트 대기 (타임아웃: 100ms) */
        fd_set fds;
        struct timeval tv;
        FD_ZERO(&fds);
        FD_SET(x11_fd, &fds);
        tv.tv_sec = 0;
        tv.tv_usec = 100000;  /* 100ms */
        
        if (select(x11_fd + 1, &fds, NULL, NULL, &tv) <= 0) {
            /* 타임아웃 또는 에러 - 다시 DBus 체크 */
            continue;
        }
        
        while (XPending(display)) {
            XNextEvent(display, &xevent);
        
            if (XFilterEvent(&xevent, None)) {
                continue;
            }
            
            switch (xevent.type) {
                case Expose:
                    if (xevent.xexpose.count == 0) {
                        redraw();
                    }
                    break;
                    
                case KeyPress:
                    handle_key_event(&xevent.xkey);
                    break;
                    
                case ButtonPress:
                    handle_button_press(&xevent.xbutton);
                    break;
                    
                case FocusIn:
                    add_log("FocusIn");
                    if (xic) XSetICFocus(xic);
                    redraw();
                    break;
                    
                case FocusOut:
                    add_log("FocusOut");
                    if (xic) XUnsetICFocus(xic);
                    redraw();
                    break;
                    
                case ClientMessage:
                    if ((Atom)xevent.xclient.data.l[0] == wm_delete) {
                        add_log("창 닫기 요청");
                        running = 0;
                    }
                    break;
            }
        }
    }
    
    /* 정리 */
    add_log("앱 종료");
    
    if (xic) XDestroyIC(xic);
    if (xim) XCloseIM(xim);
    if (dbus_proxy) g_object_unref(dbus_proxy);
    
    XftColorFree(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &color_black);
    XftColorFree(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &color_white);
    XftColorFree(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &color_gray);
    XftColorFree(display, DefaultVisual(display, screen),
        DefaultColormap(display, screen), &color_blue);
    
    if (xft_draw) XftDrawDestroy(xft_draw);
    if (font) XftFontClose(display, font);
    
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    
    return 0;
}
