/*
 * UNIM XIM Input Method Test Application
 * 
 * 순수 X11/Xlib를 사용하여 XIM 입력기를 테스트하는 앱입니다.
 * GTK, Qt 등의 툴킷을 사용하지 않습니다.
 * 
 * [2026-01-19] Xft를 사용하여 폰트 렌더링 개선 (D2Coding)
 */

#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xlocale.h>
#include <X11/keysym.h>
#include <X11/Xft/Xft.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <locale.h>
#include <time.h>
#include <wchar.h>

/* 창 크기 */
#define WINDOW_WIDTH 700
#define WINDOW_HEIGHT 600
#define MARGIN 20
#define LINE_HEIGHT 30

/* 텍스트 버퍼 */
#define TEXT_BUFFER_SIZE 4096
#define LOG_BUFFER_SIZE 8192
#define MAX_LOG_LINES 100

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
static XftColor color_black, color_white, color_gray, color_bg;

/* 입력 텍스트 버퍼 */
static char text_buffer[TEXT_BUFFER_SIZE] = "";
static int text_cursor = 0;

/* Preedit 문자열 */
static char preedit_string[TEXT_BUFFER_SIZE] = "";

/* 로그 버퍼 */
static char log_buffer[MAX_LOG_LINES][256];
static int log_count = 0;

/* 상태 정보 */
static char im_status[256] = "(XIM 연결 중...)";
static char focus_status[256] = "포커스 있음";

/* 로그 추가 함수 */
static void add_log(const char *format, ...) {
    va_list args;
    va_start(args, format);
    
    /* 시간 타임스탬프 */
    time_t now = time(NULL);
    struct tm *tm_info = localtime(&now);
    char timestamp[32];
    strftime(timestamp, sizeof(timestamp), "[%H:%M:%S] ", tm_info);
    
    /* 로그 메시지 생성 */
    char message[224];
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);
    
    /* 로그 버퍼에 추가 (순환) */
    if (log_count >= MAX_LOG_LINES) {
        /* 오래된 로그 삭제 (shift up) */
        for (int i = 0; i < MAX_LOG_LINES - 1; i++) {
            strcpy(log_buffer[i], log_buffer[i + 1]);
        }
        log_count = MAX_LOG_LINES - 1;
    }
    snprintf(log_buffer[log_count], sizeof(log_buffer[log_count]), "%s%s", timestamp, message);
    log_count++;
    
    /* 콘솔에도 출력 */
    printf("%s%s\n", timestamp, message);
}

/* 유틸리티: 문자열 폭 계산 */
static int get_text_width(const char *text) {
    if (!font || !text) return 0;
    XGlyphInfo extents;
    XftTextExtentsUtf8(display, font, (const FcChar8 *)text, strlen(text), &extents);
    return extents.xOff;
}

/* 화면 다시 그리기 */
static void redraw(void) {
    /* 배경 지우기 */
    XSetForeground(display, gc, WhitePixel(display, screen));
    XFillRectangle(display, window, gc, 0, 0, WINDOW_WIDTH, WINDOW_HEIGHT);
    
    /* XftColor 할당 (매번 redraw 때 하기보다 초기화 때 하는게 좋지만, 간단하게 여기서 처리하거나 전역 사용) */
    
    int y = MARGIN + LINE_HEIGHT;
    int line_num = 0;
    
    /* 제목 */
    const char *title = "UNIM XIM 입력기 테스트";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y, (const FcChar8 *)title, strlen(title));
    y += LINE_HEIGHT + 10;
    line_num++;
    
    /* 구분선 */
    XSetForeground(display, gc, 0xCCCCCC);
    XDrawLine(display, window, gc, MARGIN, y - LINE_HEIGHT/2, WINDOW_WIDTH - MARGIN, y - LINE_HEIGHT/2);
    y += 10;
    
    /* 입력기 상태 섹션 */
    const char *status_title = "=== 입력기 상태 ===";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y, (const FcChar8 *)status_title, strlen(status_title));
    y += LINE_HEIGHT;
    
    /* XIM 상태 */
    char status_line[512];
    snprintf(status_line, sizeof(status_line), "XIM 상태: %s", im_status);
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN + 20, y, (const FcChar8 *)status_line, strlen(status_line));
    y += LINE_HEIGHT;
    
    /* 포커스 상태 */
    snprintf(status_line, sizeof(status_line), "포커스: %s", focus_status);
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN + 20, y, (const FcChar8 *)status_line, strlen(status_line));
    y += LINE_HEIGHT;
    
    /* Preedit 문자열 */
    snprintf(status_line, sizeof(status_line), "Preedit: [%s]", preedit_string);
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN + 20, y, (const FcChar8 *)status_line, strlen(status_line));
    y += LINE_HEIGHT + 10;
    
    /* 구분선 */
    XSetForeground(display, gc, 0xCCCCCC);
    XDrawLine(display, window, gc, MARGIN, y - LINE_HEIGHT/2, WINDOW_WIDTH - MARGIN, y - LINE_HEIGHT/2);
    y += 10;
    
    /* 입력 영역 섹션 */
    const char *input_title = "=== 입력 영역 ===";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y, (const FcChar8 *)input_title, strlen(input_title));
    y += LINE_HEIGHT;
    
    /* 입력 박스 배경 */
    XSetForeground(display, gc, 0xF5F5F5); // 연한 회색 배경
    XFillRectangle(display, window, gc, MARGIN, y - LINE_HEIGHT + 10, WINDOW_WIDTH - 2 * MARGIN, LINE_HEIGHT * 4);
    XSetForeground(display, gc, 0x888888); // 테두리
    XDrawRectangle(display, window, gc, MARGIN, y - LINE_HEIGHT + 10, WINDOW_WIDTH - 2 * MARGIN, LINE_HEIGHT * 4);
    
    /* 입력된 텍스트 + preedit */
    char display_text[TEXT_BUFFER_SIZE * 2];
    snprintf(display_text, sizeof(display_text), "%s%s", text_buffer, preedit_string);
    
    /* 텍스트 그리기 */
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN + 10, y + 10, (const FcChar8 *)display_text, strlen(display_text));
    
    /* 커서 표시 (preedit 뒤에) */
    int cursor_x = MARGIN + 10 + get_text_width(display_text);
    XSetForeground(display, gc, 0x000000); // 검은색 커서
    XDrawLine(display, window, gc, cursor_x, y - LINE_HEIGHT + 15, cursor_x, y + 15);
    
    y += LINE_HEIGHT * 4 + 10;
    
    /* 안내 메시지 */
    const char *help_msg = "(여기에 한글/영문을 입력하세요. Backspace: 삭제, Ctrl+C: 종료)";
    XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN, y, (const FcChar8 *)help_msg, strlen(help_msg));
    y += LINE_HEIGHT + 10;
    
    /* 구분선 */
    XSetForeground(display, gc, 0xCCCCCC);
    XDrawLine(display, window, gc, MARGIN, y, WINDOW_WIDTH - MARGIN, y);
    y += 10;
    
    /* 로그 섹션 */
    const char *log_title = "=== 이벤트 로그 ===";
    XftDrawStringUtf8(xft_draw, &color_black, font, MARGIN, y + LINE_HEIGHT, (const FcChar8 *)log_title, strlen(log_title));
    y += LINE_HEIGHT * 2;
    
    /* 로그 출력 (최근 것만) */
    int visible_lines = (WINDOW_HEIGHT - y - MARGIN) / (LINE_HEIGHT - 8);
    int start_log = log_count > visible_lines ? log_count - visible_lines : 0;
    
    for (int i = start_log; i < log_count && y < WINDOW_HEIGHT - MARGIN; i++) {
        XftDrawStringUtf8(xft_draw, &color_gray, font, MARGIN + 20, y, (const FcChar8 *)log_buffer[i], strlen(log_buffer[i]));
        y += LINE_HEIGHT - 8;
    }
    
    XFlush(display);
}

/* XIM preedit 콜백들 */
static int preedit_start_callback(XIC ic, XPointer client_data, XPointer call_data) {
    (void)ic;
    (void)client_data;
    (void)call_data;
    add_log("Preedit Start");
    return -1; /* 제한 없음 */
}

static void preedit_done_callback(XIC ic, XPointer client_data, XPointer call_data) {
    (void)ic;
    (void)client_data;
    (void)call_data;
    add_log("Preedit Done");
    preedit_string[0] = '\0';
    redraw();
}

static void preedit_draw_callback(XIC ic, XPointer client_data, XIMPreeditDrawCallbackStruct *call_data) {
    (void)ic;
    (void)client_data;
    
    if (call_data && call_data->text && call_data->text->string.multi_byte) {
        strncpy(preedit_string, call_data->text->string.multi_byte, sizeof(preedit_string) - 1);
        preedit_string[sizeof(preedit_string) - 1] = '\0';
        add_log("Preedit Draw: \"%s\"", preedit_string);
    } else {
        preedit_string[0] = '\0';
        add_log("Preedit Draw: (empty)");
    }
    redraw();
}

static void preedit_caret_callback(XIC ic, XPointer client_data, XIMPreeditCaretCallbackStruct *call_data) {
    (void)ic;
    (void)client_data;
    (void)call_data;
    add_log("Preedit Caret");
}

/* XIM 초기화 */
static int init_xim(void) {
    /* 로케일 설정 */
    if (setlocale(LC_ALL, "") == NULL) {
        add_log("경고: 로케일 설정 실패");
    }
    
    if (!XSupportsLocale()) {
        add_log("경고: X에서 현재 로케일을 지원하지 않음");
    }
    
    if (XSetLocaleModifiers("") == NULL) {
        add_log("경고: XSetLocaleModifiers 실패");
    }
    
    /* XMODIFIERS 환경변수 확인 */
    const char *xmodifiers = getenv("XMODIFIERS");
    add_log("XMODIFIERS=%s", xmodifiers ? xmodifiers : "(unset)");
    
    /* XIM 열기 */
    xim = XOpenIM(display, NULL, NULL, NULL);
    if (xim == NULL) {
        add_log("오류: XOpenIM 실패 - XIM 서버 연결 불가");
        strcpy(im_status, "연결 실패 (XIM 서버 없음)");
        return 0;
    }
    
    add_log("XIM 열기 성공");
    
    /* 지원되는 입력 스타일 확인 */
    XIMStyles *im_styles = NULL;
    if (XGetIMValues(xim, XNQueryInputStyle, &im_styles, NULL) != NULL || im_styles == NULL) {
        add_log("오류: 입력 스타일 쿼리 실패");
        return 0;
    }
    
    /* 원하는 스타일: On-the-spot (preedit callbacks) */
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
    
    /* XIC 생성 */
    XVaNestedList preedit_attr = NULL;
    
    if (best_style & XIMPreeditCallbacks) {
        /* Preedit 콜백 설정 */
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
    } else {
        /* 그 외 스타일 (fallback) */
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
    strcpy(im_status, "XIM 연결됨");
    
    /* XIC에서 처리할 이벤트 마스크 가져오기 */
    long im_event_mask = 0;
    if (XGetICValues(xic, XNFilterEvents, &im_event_mask, NULL) == NULL) {
        add_log("XIC 이벤트 마스크: 0x%lx", im_event_mask);
        XSelectInput(display, window, 
            ExposureMask | KeyPressMask | KeyReleaseMask | 
            FocusChangeMask | StructureNotifyMask | im_event_mask);
    }
    
    return 1;
}

/* 키 이벤트 처리 */
static void handle_key_event(XKeyEvent *event) {
    KeySym keysym;
    Status status;
    char buf[256] = "";
    int len = 0;
    
    if (xic) {
        /* XIM을 통한 키 처리 */
        len = XmbLookupString(xic, event, buf, sizeof(buf) - 1, &keysym, &status);
        
        const char *status_str;
        switch (status) {
            case XLookupNone: status_str = "None"; break;
            case XLookupChars: status_str = "Chars"; break;
            case XLookupKeySym: status_str = "KeySym"; break;
            case XLookupBoth: status_str = "Both"; break;
            case XBufferOverflow: status_str = "Overflow"; break;
            default: status_str = "Unknown"; break;
        }
        add_log("XmbLookupString: status=%s, len=%d, keysym=0x%lx", status_str, len, keysym);
        
        if (status == XLookupChars || status == XLookupBoth) {
            buf[len] = '\0';
            add_log("  -> 문자: \"%s\"", buf);
            
            /* 텍스트 버퍼에 추가 */
            int cur_len = strlen(text_buffer);
            if (cur_len + len < TEXT_BUFFER_SIZE - 1) {
                strcat(text_buffer, buf);
                add_log("버퍼 업데이트: \"%s\"", text_buffer);
            }
            
            /* preedit이 있으면 비우기 (commit 후) */
            preedit_string[0] = '\0';
        }
        
        if (status == XLookupKeySym || status == XLookupBoth) {
            /* 특수 키 처리 */
            if (keysym == XK_BackSpace) {
                int text_len = strlen(text_buffer);
                if (text_len > 0) {
                    /* UTF-8 문자 경계 찾기 */
                    int i = text_len - 1;
                    while (i > 0 && (text_buffer[i] & 0xC0) == 0x80) {
                        i--;
                    }
                    text_buffer[i] = '\0';
                    add_log("Backspace: \"%s\"", text_buffer);
                }
            } else if (keysym == XK_Return || keysym == XK_KP_Enter) {
                // Enter: 줄바꿈 대신 그냥 로그만 남김 (한 줄 입력기 컨셉)
                add_log("Enter 키 (Commit)");
            } else if (keysym == XK_Escape) {
                preedit_string[0] = '\0';
                add_log("Escape 키 - preedit 취소");
            }
        }
    } else {
        /* XIM 없이 기본 처리 */
        len = XLookupString(event, buf, sizeof(buf) - 1, &keysym, NULL);
        if (len > 0) {
            buf[len] = '\0';
            strcat(text_buffer, buf);
            add_log("직접 입력 (XIM 없음): \"%s\"", buf);
        }
    }
    
    redraw();
}

int main(int argc, char *argv[]) {
    (void)argc;
    (void)argv;
    
    /* 로케일 초기화 */
    setlocale(LC_ALL, "");
    
    /* X 디스플레이 열기 */
    display = XOpenDisplay(NULL);
    if (display == NULL) {
        fprintf(stderr, "오류: X 디스플레이를 열 수 없습니다.\n");
        return 1;
    }
    
    screen = DefaultScreen(display);
    
    add_log("UNIM XIM 입력기 테스트 앱 시작 (Xft 적용)");
    add_log("DISPLAY=%s", getenv("DISPLAY") ? getenv("DISPLAY") : "(unset)");
    
    Window root = RootWindow(display, screen);
    
    /* 폰트 초기화 (Xft) */
    /* 1순위: D2Coding, 2순위: NanumGothicCoding, 3순위: Monospace */
    const char *font_names[] = {
        "D2Coding:size=14",
        "NanumGothicCoding:size=14",
        "Monospace:size=14", 
        NULL
    };
    
    for (int i = 0; font_names[i] != NULL; i++) {
        font = XftFontOpenName(display, screen, font_names[i]);
        if (font) {
            add_log("폰트 로드 성공: %s", font_names[i]);
            break;
        }
    }
    
    if (!font) {
        add_log("오류: 적절한 폰트를 찾을 수 없습니다. 기본값 사용 시도...");
        font = XftFontOpenName(display, screen, "fixed");
    }
    
    /* 색상 할당 */
    Visual *visual = DefaultVisual(display, screen);
    Colormap cmap = DefaultColormap(display, screen);
    
    XftColorAllocName(display, visual, cmap, "black", &color_black);
    XftColorAllocName(display, visual, cmap, "white", &color_white);
    XftColorAllocName(display, visual, cmap, "#666666", &color_gray);
    XftColorAllocName(display, visual, cmap, "#F5F5F5", &color_bg);
    
    /* 윈도우 속성 */
    XSetWindowAttributes attrs;
    attrs.background_pixel = WhitePixel(display, screen);
    attrs.event_mask = ExposureMask | KeyPressMask | KeyReleaseMask | 
                       FocusChangeMask | StructureNotifyMask;
    
    /* 윈도우 생성 */
    window = XCreateWindow(display, root, 
        100, 100, WINDOW_WIDTH, WINDOW_HEIGHT, 
        1, CopyFromParent, InputOutput, CopyFromParent,
        CWBackPixel | CWEventMask, &attrs);
    
    /* 윈도우 제목 설정 */
    XTextProperty window_name;
    const char *title = "UNIM XIM 입력기 테스트";
    XmbTextListToTextProperty(display, (char **)&title, 1, XStdICCTextStyle, &window_name);
    XSetWMName(display, window, &window_name);
    XFree(window_name.value);
    
    /* WM_DELETE_WINDOW 프로토콜 등록 */
    Atom wm_delete = XInternAtom(display, "WM_DELETE_WINDOW", False);
    XSetWMProtocols(display, window, &wm_delete, 1);
    
    /* GC 생성 */
    gc = XCreateGC(display, window, 0, NULL);
    
    /* Xft Draw Context 생성 */
    xft_draw = XftDrawCreate(display, window, visual, cmap);
    
    /* 윈도우 표시 */
    XMapWindow(display, window);
    XFlush(display);
    
    /* XIM 초기화 */
    init_xim();
    
    /* 포커스 설정 */
    if (xic) {
        XSetICFocus(xic);
    }
    
    add_log("이벤트 루프 시작");
    
    /* 이벤트 루프 */
    int running = 1;
    XEvent event;
    
    while (running && XPending(display) >= 0) {
        XNextEvent(display, &event);
        
        /* XIM 필터링 */
        if (XFilterEvent(&event, None)) {
            continue;
        }
        
        switch (event.type) {
            case Expose:
                if (event.xexpose.count == 0) {
                    redraw();
                }
                break;
                
            case KeyPress:
                handle_key_event(&event.xkey);
                
                /* Ctrl+C로 종료 */
                if ((event.xkey.state & ControlMask) && 
                    XLookupKeysym(&event.xkey, 0) == XK_c) {
                    add_log("Ctrl+C - 종료");
                    running = 0;
                }
                break;
                
            case FocusIn:
                add_log("FocusIn");
                strcpy(focus_status, "포커스 있음");
                if (xic) {
                    XSetICFocus(xic);
                }
                redraw();
                break;
                
            case FocusOut:
                add_log("FocusOut");
                strcpy(focus_status, "포커스 없음");
                if (xic) {
                    XUnsetICFocus(xic);
                }
                redraw();
                break;
                
            case ClientMessage:
                if ((Atom)event.xclient.data.l[0] == wm_delete) {
                    add_log("창 닫기 요청");
                    running = 0;
                }
                break;
                
            case DestroyNotify:
                running = 0;
                break;
        }
    }
    
    add_log("앱 종료");
    
    /* 정리 */
    if (xic) {
        XDestroyIC(xic);
    }
    if (xim) {
        XCloseIM(xim);
    }
    
    /* Xft 리소스 해제 */
    if (font) XftFontClose(display, font);
    if (xft_draw) XftDrawDestroy(xft_draw);
    XftColorFree(display, DefaultVisual(display, screen), DefaultColormap(display, screen), &color_black);
    XftColorFree(display, DefaultVisual(display, screen), DefaultColormap(display, screen), &color_white);
    
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    
    return 0;
}
