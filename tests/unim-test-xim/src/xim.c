/*
 * UNIM XIM Test - XIM Module
 *
 * XIM 열기, XIC 생성, preedit 콜백 처리
 */

#include "app.h"
#include "log.h"
#include "xim.h"
#include "fields.h"
#include "ui.h"

/* ─── Preedit Callbacks ───────────────────────────────────────────────── */

static int preedit_start_cb(XIC ic, XPointer client, XPointer call) {
    (void)ic; (void)client; (void)call;
    log_add("[%s] Preedit Start", app.fields[app.active_field].label);
    return -1;
}

static void preedit_done_cb(XIC ic, XPointer client, XPointer call) {
    (void)ic; (void)client; (void)call;
    log_add("[%s] Preedit Done", app.fields[app.active_field].label);
    app.fields[app.active_field].preedit[0] = '\0';
    ui_redraw();
}

static void preedit_draw_cb(XIC ic, XPointer client,
                            XIMPreeditDrawCallbackStruct *call)
{
    (void)ic; (void)client;

    InputField *f = &app.fields[app.active_field];

    if (call && call->text && call->text->string.multi_byte) {
        strncpy(f->preedit, call->text->string.multi_byte,
                sizeof(f->preedit) - 1);
        f->preedit[sizeof(f->preedit) - 1] = '\0';
        log_add("[%s] Preedit: \"%s\"", f->label, f->preedit);
    } else {
        f->preedit[0] = '\0';
        log_add("[%s] Preedit: (비움)", f->label);
    }

    fields_update_spot();
    ui_redraw();
}

static void preedit_caret_cb(XIC ic, XPointer client,
                             XIMPreeditCaretCallbackStruct *call)
{
    (void)ic; (void)client; (void)call;
}

/* ─── Public API ──────────────────────────────────────────────────────── */

int xim_init(void) {
    if (setlocale(LC_ALL, "") == NULL)
        log_add("경고: 로케일 설정 실패");

    if (!XSupportsLocale())
        log_add("경고: X 로케일 미지원");

    if (XSetLocaleModifiers("") == NULL)
        log_add("경고: XSetLocaleModifiers 실패");

    const char *xmod = getenv("XMODIFIERS");
    log_add("XMODIFIERS=%s", xmod ? xmod : "(unset)");

    app.xim = XOpenIM(app.display, NULL, NULL, NULL);
    if (!app.xim) {
        log_add("오류: XOpenIM 실패");
        strcpy(app.im_status, "연결 실패");
        return 0;
    }
    log_add("XIM 열기 성공");

    /* 지원 스타일 쿼리 */
    XIMStyles *styles = NULL;
    if (XGetIMValues(app.xim, XNQueryInputStyle, &styles, NULL) || !styles) {
        log_add("오류: 입력 스타일 쿼리 실패");
        return 0;
    }

    XIMStyle best = 0;
    XIMStyle wanted[] = {
        XIMPreeditCallbacks | XIMStatusNothing,
        XIMPreeditPosition  | XIMStatusNothing,
        XIMPreeditNothing   | XIMStatusNothing,
        0
    };

    for (int w = 0; wanted[w]; w++) {
        for (unsigned long i = 0; i < styles->count_styles; i++) {
            if (styles->supported_styles[i] == wanted[w]) {
                best = wanted[w];
                break;
            }
        }
        if (best) break;
    }
    XFree(styles);

    if (!best) {
        log_add("오류: 적합한 입력 스타일 없음");
        return 0;
    }
    log_add("입력 스타일: 0x%lx", (unsigned long)best);

    /* XIC 생성 */
    XVaNestedList preedit_attr = NULL;

    if (best & XIMPreeditCallbacks) {
        XIMCallback cb_start = { NULL, (XIMProc)preedit_start_cb };
        XIMCallback cb_done  = { NULL, (XIMProc)preedit_done_cb };
        XIMCallback cb_draw  = { NULL, (XIMProc)preedit_draw_cb };
        XIMCallback cb_caret = { NULL, (XIMProc)preedit_caret_cb };

        preedit_attr = XVaCreateNestedList(0,
            XNPreeditStartCallback, &cb_start,
            XNPreeditDoneCallback,  &cb_done,
            XNPreeditDrawCallback,  &cb_draw,
            XNPreeditCaretCallback, &cb_caret,
            NULL);

        app.xic = XCreateIC(app.xim,
            XNInputStyle,      best,
            XNClientWindow,    app.window,
            XNFocusWindow,     app.window,
            XNPreeditAttributes, preedit_attr,
            NULL);
    } else if (best & XIMPreeditPosition) {
        XPoint spot = {
            (short)(app.fields[0].x + FIELD_PADDING),
            (short)(app.fields[0].y + FIELD_HEIGHT - 8)
        };
        preedit_attr = XVaCreateNestedList(0,
            XNSpotLocation, &spot,
            NULL);

        app.xic = XCreateIC(app.xim,
            XNInputStyle,      best,
            XNClientWindow,    app.window,
            XNFocusWindow,     app.window,
            XNPreeditAttributes, preedit_attr,
            NULL);
    } else {
        app.xic = XCreateIC(app.xim,
            XNInputStyle,   best,
            XNClientWindow, app.window,
            XNFocusWindow,  app.window,
            NULL);
    }

    if (preedit_attr) XFree(preedit_attr);

    if (!app.xic) {
        log_add("오류: XCreateIC 실패");
        strcpy(app.im_status, "XIC 생성 실패");
        return 0;
    }

    log_add("XIC 생성 성공");
    strcpy(app.im_status, "연결됨");

    /* XIC가 요구하는 이벤트 마스크 추가 */
    long mask = 0;
    if (XGetICValues(app.xic, XNFilterEvents, &mask, NULL) == NULL) {
        log_add("XIC 이벤트 마스크: 0x%lx", mask);
        XSelectInput(app.display, app.window,
            ExposureMask | KeyPressMask | KeyReleaseMask |
            FocusChangeMask | StructureNotifyMask | ButtonPressMask | mask);
    }

    return 1;
}

void xim_cleanup(void) {
    if (app.xic) { XDestroyIC(app.xic); app.xic = NULL; }
    if (app.xim) { XCloseIM(app.xim);   app.xim = NULL; }
}
