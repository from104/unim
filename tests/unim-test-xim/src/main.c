/*
 * UNIM XIM Input Method Test Application
 *
 * 순수 X11/Xlib + Xft 기반 XIM 입력기 테스트 앱
 * DBus를 통해 unim-daemon과 통신하여 한/영 상태를 실시간 표시
 *
 * Catppuccin Mocha 다크 테마 사용
 *
 * [2026-01-19] 초기 구현
 * [2026-01-22] 다중 입력 필드
 * [2026-01-25] DBus 통합
 * [2026-02-13] 현대적 재작성: 모듈 분리, 다크 테마
 */

#include "app.h"
#include "log.h"
#include "ui.h"
#include "xim.h"
#include "dbus.h"
#include "unim_test.h"
#include "fields.h"

#include <sys/select.h>

/* ─── 자동 테스트 (--auto) ──────────────────────────────────────── */

static int
run_auto_test(gboolean verbose)
{
    long log_mark = unim_test_log_mark();
    UnimTestRunner *runner = unim_test_runner_new(verbose);
    if (!runner) return 1;
    unim_test_run_suite(runner, UNIM_TEST_SUITE_AUTO);
    unim_test_log_check(runner, log_mark);
    int ret = runner->failed > 0 ? 1 : 0;
    unim_test_runner_free(runner);
    return ret;
}

/* ─── Global App Instance ─────────────────────────────────────────────── */

App app;

/* ─── Main ────────────────────────────────────────────────────────────── */

int main(int argc, char *argv[]) {
    /* --auto 모드 */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--auto") == 0) {
            gboolean verbose = FALSE;
            for (int j = 1; j < argc; j++)
                if (strcmp(argv[j], "-v") == 0 || strcmp(argv[j], "--verbose") == 0) verbose = TRUE;
            return run_auto_test(verbose);
        }
    }

    /* Zero-init */
    memset(&app, 0, sizeof(App));
    app.win_width  = WINDOW_WIDTH;
    app.win_height = WINDOW_HEIGHT;
    app.is_korean_mode = 1;
    strcpy(app.im_status,   "(연결 중...)");
    strcpy(app.dbus_status, "(연결 중...)");
    strcpy(app.mode_status, "(감지 중...)");

    setlocale(LC_ALL, "");

    /* Log init */
    log_init();

    /* X11 Display */
    app.display = XOpenDisplay(NULL);
    if (!app.display) {
        fprintf(stderr, "오류: X 디스플레이를 열 수 없습니다.\n");
        return 1;
    }
    app.screen = DefaultScreen(app.display);

    log_add("UNIM XIM Test 시작");
    log_add("DISPLAY=%s", getenv("DISPLAY") ? getenv("DISPLAY") : "(unset)");

    /* Window */
    app.window = XCreateSimpleWindow(
        app.display, RootWindow(app.display, app.screen),
        100, 100, WINDOW_WIDTH, WINDOW_HEIGHT,
        0, BlackPixel(app.display, app.screen),
        0x1e1e2e  /* COL_BASE */
    );

    XStoreName(app.display, app.window, "UNIM XIM Test");

    /* Size hints (min size) */
    XSizeHints *hints = XAllocSizeHints();
    if (hints) {
        hints->flags = PMinSize;
        hints->min_width  = 500;
        hints->min_height = 400;
        XSetWMNormalHints(app.display, app.window, hints);
        XFree(hints);
    }

    /* GC */
    app.gc = XCreateGC(app.display, app.window, 0, NULL);

    /* Fonts */
    if (!ui_init_fonts()) {
        fprintf(stderr, "오류: 폰트를 열 수 없습니다.\n");
        return 1;
    }
    log_add("폰트 로드 성공");

    /* Colors & draw context */
    ui_init_colors();

    /* Fields */
    fields_init();

    /* Map + flush */
    XMapWindow(app.display, app.window);
    XFlush(app.display);

    /* XftDraw (after map) */
    ui_init_draw();

    /* DBus */
    dbus_setup();

    /* XIM */
    xim_init();

    /* WM_DELETE_WINDOW */
    Atom wm_delete = XInternAtom(app.display, "WM_DELETE_WINDOW", False);
    XSetWMProtocols(app.display, app.window, &wm_delete, 1);

    log_add("이벤트 루프 시작");

    /* ─── Event Loop ──────────────────────────────────────────────────── */
    int running = 1;
    int x11_fd = ConnectionNumber(app.display);
    XEvent ev;

    while (running) {
        /* Process pending DBus events */
        dbus_process_pending();

        /* Wait for X11 events (100ms timeout) */
        fd_set fds;
        struct timeval tv;
        FD_ZERO(&fds);
        FD_SET(x11_fd, &fds);
        tv.tv_sec  = 0;
        tv.tv_usec = 100000;

        if (select(x11_fd + 1, &fds, NULL, NULL, &tv) <= 0)
            continue;

        while (XPending(app.display)) {
            XNextEvent(app.display, &ev);

            if (XFilterEvent(&ev, None))
                continue;

            switch (ev.type) {
            case Expose:
                if (ev.xexpose.count == 0)
                    ui_redraw();
                break;

            case KeyPress:
                fields_handle_key(&ev.xkey);
                break;

            case ButtonPress:
                fields_handle_click(&ev.xbutton);
                break;

            case FocusIn:
                log_add("FocusIn");
                if (app.xic) XSetICFocus(app.xic);
                ui_redraw();
                break;

            case FocusOut:
                log_add("FocusOut");
                if (app.xic) XUnsetICFocus(app.xic);
                ui_redraw();
                break;

            case ConfigureNotify:
                if (ev.xconfigure.width  != app.win_width ||
                    ev.xconfigure.height != app.win_height) {
                    app.win_width  = ev.xconfigure.width;
                    app.win_height = ev.xconfigure.height;
                    ui_redraw();
                }
                break;

            case ClientMessage:
                if ((Atom)ev.xclient.data.l[0] == wm_delete) {
                    log_add("창 닫기");
                    running = 0;
                }
                break;
            }
        }
    }

    /* ─── Cleanup ─────────────────────────────────────────────────────── */
    log_add("앱 종료");

    xim_cleanup();
    dbus_cleanup();
    ui_cleanup();

    XFreeGC(app.display, app.gc);
    XDestroyWindow(app.display, app.window);
    XCloseDisplay(app.display);

    return 0;
}
