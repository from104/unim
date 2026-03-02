/*
 * UNIM XIM Test - Fields Module
 *
 * 입력 필드 초기화, 전환, 키/마우스 이벤트 처리
 */

#include "app.h"
#include "log.h"
#include "fields.h"
#include "dbus.h"
#include "ui.h"

/* ─── Text Width Helper ───────────────────────────────────────────────── */

static int text_width(const char *text) {
    if (!app.font || !text || !*text) return 0;
    XGlyphInfo ext;
    XftTextExtentsUtf8(app.display, app.font, (const FcChar8 *)text,
                       (int)strlen(text), &ext);
    return ext.xOff;
}

/* ─── Public API ──────────────────────────────────────────────────────── */

void fields_init(void) {
    int label_col_w = 110;
    int field_w = WINDOW_WIDTH - 2 * MARGIN - label_col_w;
    int y_start = 130;

    for (int i = 0; i < NUM_FIELDS; i++) {
        app.fields[i].x      = MARGIN + label_col_w;
        app.fields[i].y      = y_start + i * (FIELD_HEIGHT + 16);
        app.fields[i].width  = field_w;
        app.fields[i].height = FIELD_HEIGHT;
        app.fields[i].text[0]    = '\0';
        app.fields[i].preedit[0] = '\0';
        app.fields[i].focused    = (i == 0) ? 1 : 0;
    }

    strcpy(app.fields[0].label, "이름");
    strcpy(app.fields[1].label, "주소");
    strcpy(app.fields[2].label, "전화번호");
    strcpy(app.fields[3].label, "메모");

    app.active_field = 0;
}

void fields_update_spot(void) {
    if (!app.xic) return;

    InputField *f = &app.fields[app.active_field];
    char buf[TEXT_BUFFER_SIZE * 2];
    snprintf(buf, sizeof(buf), "%s%s", f->text, f->preedit);

    XPoint spot;
    spot.x = (short)(f->x + FIELD_PADDING + text_width(buf));
    spot.y = (short)(f->y + FIELD_HEIGHT - 8);

    XVaNestedList attr = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
    XSetICValues(app.xic, XNPreeditAttributes, attr, NULL);
    XFree(attr);
}

void fields_switch(int direction) {
    InputField *cur = &app.fields[app.active_field];

    /* XIC 리셋 → 서버가 조합 중이던 텍스트를 반환 */
    if (app.xic) {
        char *committed = XmbResetIC(app.xic);
        if (committed && *committed) {
            strcat(cur->text, committed);
            log_add("필드 전환 시 preedit 커밋: \"%s\"", committed);
            XFree(committed);
        }
    }
    cur->preedit[0] = '\0';

    app.active_field = (app.active_field + direction + NUM_FIELDS) % NUM_FIELDS;
    log_add("필드 전환: %s", app.fields[app.active_field].label);

    fields_update_spot();
    ui_redraw();
}

void fields_handle_key(XKeyEvent *event) {
    KeySym keysym;
    Status status;
    char buf[256] = "";
    int len = 0;

    InputField *f = &app.fields[app.active_field];

    if (app.xic) {
        len = XmbLookupString(app.xic, event, buf, sizeof(buf) - 1,
                              &keysym, &status);

        /* Tab: 필드 전환 */
        if (keysym == XK_Tab || keysym == XK_ISO_Left_Tab) {
            int dir = (event->state & ShiftMask) ? -1 : 1;
            fields_switch(dir);
            return;
        }

        /* F9: 한/영 전환 */
        if (keysym == XK_F9) {
            dbus_toggle_mode();
            return;
        }

        if (status == XLookupChars || status == XLookupBoth) {
            buf[len] = '\0';
            int cur_len = (int)strlen(f->text);
            if (cur_len + len < TEXT_BUFFER_SIZE - 1) {
                strcat(f->text, buf);
                log_add("[%s] 입력: \"%s\"", f->label, buf);
            }
            f->preedit[0] = '\0';
        }

        if (status == XLookupKeySym || status == XLookupBoth) {
            if (keysym == XK_BackSpace) {
                int tlen = (int)strlen(f->text);
                if (tlen > 0) {
                    /* UTF-8 한 글자 지우기 */
                    int i = tlen - 1;
                    while (i > 0 && (f->text[i] & 0xC0) == 0x80) i--;
                    f->text[i] = '\0';
                    log_add("[%s] Backspace", f->label);
                }
            } else if (keysym == XK_Return || keysym == XK_KP_Enter) {
                log_add("[%s] Enter → 다음 필드", f->label);
                fields_switch(1);
                return;
            } else if (keysym == XK_Escape) {
                f->preedit[0] = '\0';
                log_add("[%s] Escape - preedit 취소", f->label);
            }
        }
    } else {
        /* XIC 없이 fallback */
        len = XLookupString(event, buf, sizeof(buf) - 1, &keysym, NULL);
        if (len > 0) {
            buf[len] = '\0';
            strcat(f->text, buf);
        }
    }

    fields_update_spot();
    ui_redraw();
}

void fields_handle_click(XButtonEvent *event) {
    for (int i = 0; i < NUM_FIELDS; i++) {
        InputField *f = &app.fields[i];
        if (event->x >= f->x && event->x <= f->x + f->width &&
            event->y >= f->y && event->y <= f->y + f->height) {

            if (i != app.active_field) {
                /* XIC 리셋 → 서버가 조합 중이던 텍스트를 반환 */
                InputField *cur = &app.fields[app.active_field];
                if (app.xic) {
                    char *committed = XmbResetIC(app.xic);
                    if (committed && *committed) {
                        strcat(cur->text, committed);
                        XFree(committed);
                    }
                }
                cur->preedit[0] = '\0';

                app.active_field = i;
                log_add("마우스 클릭 → %s", f->label);
                fields_update_spot();
                ui_redraw();
            }
            break;
        }
    }
}
