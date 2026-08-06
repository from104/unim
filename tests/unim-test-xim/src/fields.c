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
    int label_col_w = S(110);
    int field_w = app.win_width - 2 * S(MARGIN) - label_col_w;
    int y_start = S(130);

    for (int i = 0; i < NUM_FIELDS; i++) {
        app.fields[i].x          = S(MARGIN) + label_col_w;
        app.fields[i].y          = y_start + i * (S(FIELD_HEIGHT) + S(16));
        app.fields[i].width      = field_w;
        app.fields[i].height     = S(FIELD_HEIGHT);
        app.fields[i].text[0]    = '\0';
        app.fields[i].preedit[0] = '\0';
        app.fields[i].focused    = (i == 0) ? 1 : 0;
        app.fields[i].cursor_pos = 0;
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

    /* spot = cursor 위치(text[0..cursor_pos]) + preedit 너비 */
    char before[TEXT_BUFFER_SIZE];
    memcpy(before, f->text, f->cursor_pos);
    before[f->cursor_pos] = '\0';

    char buf[TEXT_BUFFER_SIZE * 2];
    snprintf(buf, sizeof(buf), "%s%s", before, f->preedit);

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
            int tlen = (int)strlen(cur->text);
            int clen = (int)strlen(committed);
            if (tlen + clen < TEXT_BUFFER_SIZE - 1) {
                strcat(cur->text, committed);
                cur->cursor_pos = tlen + clen;
            }
            log_add("필드 전환 시 preedit 커밋: \"%s\"", committed);
            XFree(committed);
        }
    }
    cur->preedit[0] = '\0';

    app.active_field = (app.active_field + direction + NUM_FIELDS) % NUM_FIELDS;
    /* 새 필드의 커서를 끝으로 */
    app.fields[app.active_field].cursor_pos =
        (int)strlen(app.fields[app.active_field].text);
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

    /* 제어키는 XIM 상태와 무관하게 항상 raw keysym으로 직접 처리 */
    KeySym raw_ks = XLookupKeysym(event, 0);

    /* Tab: 필드 전환 */
    if (raw_ks == XK_Tab || raw_ks == XK_ISO_Left_Tab) {
        fields_switch((event->state & ShiftMask) ? -1 : 1);
        return;
    }
    /* F9: 한/영 전환 */
    if (raw_ks == XK_F9) {
        dbus_toggle_mode();
        return;
    }

    if (app.xic) {
        len = XmbLookupString(app.xic, event, buf, sizeof(buf) - 1,
                              &keysym, &status);

        /* 문자 커밋: printable UTF-8만 삽입 (0x00-0x1f, 0x7f 제어코드 제외) */
        if (status == XLookupChars || status == XLookupBoth) {
            buf[len] = '\0';
            if (len > 0 &&
                (unsigned char)buf[0] >= 0x20 &&
                (unsigned char)buf[0] != 0x7f) {
                int cur_len = (int)strlen(f->text);
                if (cur_len + len < TEXT_BUFFER_SIZE - 1) {
                    memmove(&f->text[f->cursor_pos + len],
                            &f->text[f->cursor_pos],
                            cur_len - f->cursor_pos + 1);
                    memcpy(&f->text[f->cursor_pos], buf, len);
                    f->cursor_pos += len;
                    log_add("[%s] 입력: \"%s\"", f->label, buf);
                }
            }
            /* preedit 은 여기서 지우지 않는다.
             *
             * ON-THE-SPOT(PREEDIT_CALLBACKS) 에서 preedit 의 소유자는 IM 이다.
             * 앱은 IM 이 PreeditDraw(빈 문자열) 이나 PreeditDone 을 보냈을 때만
             * 지워야 하고, 그건 preedit_draw_cb / preedit_done_cb 가 이미 한다.
             * 커밋을 받았다고 앱이 임의로 비우면, 같은 키에서 IM 이 방금 그린
             * **새 조합**까지 지워져 화면에서 사라진다(커밋과 새 preedit 이 한
             * 키에 함께 오는 경우 — 예: ㄹ 연타). */
        }
    } else {
        /* XIC 없이 fallback */
        len = XLookupString(event, buf, sizeof(buf) - 1, &keysym, NULL);
        if (len > 0 &&
            (unsigned char)buf[0] >= 0x20 &&
            (unsigned char)buf[0] != 0x7f) {
            buf[len] = '\0';
            int cur_len = (int)strlen(f->text);
            if (cur_len + len < TEXT_BUFFER_SIZE - 1) {
                memmove(&f->text[f->cursor_pos + len],
                        &f->text[f->cursor_pos],
                        cur_len - f->cursor_pos + 1);
                memcpy(&f->text[f->cursor_pos], buf, len);
                f->cursor_pos += len;
            }
        }
    }

    /* 탐색/편집 키: raw keysym으로 처리 (XIM 상태 무관) */
    if (raw_ks == XK_BackSpace) {
        /* preedit 조합 중이면 XIM이 처리 */
        if (f->preedit[0] == '\0' && f->cursor_pos > 0) {
            int i = f->cursor_pos - 1;
            while (i > 0 && (f->text[i] & 0xC0) == 0x80) i--;
            int tlen = (int)strlen(f->text);
            memmove(&f->text[i], &f->text[f->cursor_pos],
                    tlen - f->cursor_pos + 1);
            f->cursor_pos = i;
            log_add("[%s] Backspace", f->label);
        }
    } else if (raw_ks == XK_Left) {
        if (f->preedit[0] == '\0' && f->cursor_pos > 0) {
            int i = f->cursor_pos - 1;
            while (i > 0 && (f->text[i] & 0xC0) == 0x80) i--;
            f->cursor_pos = i;
        }
    } else if (raw_ks == XK_Right) {
        if (f->preedit[0] == '\0' && f->text[f->cursor_pos]) {
            f->cursor_pos++;
            while (f->text[f->cursor_pos] &&
                   (f->text[f->cursor_pos] & 0xC0) == 0x80)
                f->cursor_pos++;
        }
    } else if (raw_ks == XK_Home) {
        if (f->preedit[0] == '\0') f->cursor_pos = 0;
    } else if (raw_ks == XK_End) {
        if (f->preedit[0] == '\0') f->cursor_pos = (int)strlen(f->text);
    } else if (raw_ks == XK_Return || raw_ks == XK_KP_Enter) {
        log_add("[%s] Enter → 다음 필드", f->label);
        fields_switch(1);
        return;
    } else if (raw_ks == XK_Escape) {
        f->preedit[0] = '\0';
        log_add("[%s] Escape - preedit 취소", f->label);
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
                f->cursor_pos = (int)strlen(f->text);
                log_add("마우스 클릭 → %s", f->label);
                fields_update_spot();
                ui_redraw();
            }
            break;
        }
    }
}
