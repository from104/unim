/**
 * UNIM 테스트 앱 — 코어 필드 엔진 구현
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §7
 */

#include "unim_test_field.h"
#include "unim_test_log.h"

#include <stdio.h>
#include <string.h>

/* ─── UTF-8 경계 ──────────────────────────────────────────────────────── */

static int utf8_prev(const char *s, int pos) {
    if (pos <= 0) return 0;
    int i = pos - 1;
    while (i > 0 && ((unsigned char)s[i] & 0xC0) == 0x80) i--;
    return i;
}

static int utf8_next(const char *s, int pos) {
    int len = (int)strlen(s);
    if (pos >= len) return len;
    int i = pos + 1;
    while (i < len && ((unsigned char)s[i] & 0xC0) == 0x80) i++;
    return i;
}

static int clamp(int v, int lo, int hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

/* ─── 스케일 ──────────────────────────────────────────────────────────── */

static int S(int v, double scale) { return (int)(v * scale + 0.5); }

/* ─── 수명·레이아웃 ───────────────────────────────────────────────────── */

void unim_field_init(UnimTestField *f, const UnimSpecField *spec) {
    memset(f, 0, sizeof *f);
    f->id    = spec->id;
    f->label = spec->label;
    f->hint  = spec->hint;
    unim_log_note("필드 초기화: %s (%s, hint=%d) — %s",
                  spec->id, spec->label, (int)spec->hint, spec->purpose);
}

int unim_field_layout(UnimTestField *fields, int n,
                      int top, int width, double scale) {
    int label_w = S(UNIM_SPEC_LABEL_COL_W, scale);
    int margin  = S(UNIM_SPEC_MARGIN, scale);
    int gap     = S(UNIM_SPEC_ROW_GAP, scale);
    int y       = top;

    for (int i = 0; i < n; i++) {
        UnimTestField *f = &fields[i];
        int h = (f->hint == UNIM_HINT_MULTILINE)
                    ? S(UNIM_SPEC_FIELD_H_MULTI, scale)
                    : S(UNIM_SPEC_FIELD_H, scale);
        f->x = margin + label_w;
        f->y = y;
        f->w = width - 2 * margin - label_w;
        f->h = h;
        y += h + gap;

        unim_log_note("레이아웃 %s: x=%d y=%d w=%d h=%d (scale=%.2f)",
                      f->id, f->x, f->y, f->w, f->h, scale);
    }
    return y;
}

int unim_field_hit(const UnimTestField *fields, int n, int x, int y) {
    for (int i = 0; i < n; i++) {
        const UnimTestField *f = &fields[i];
        if (x >= f->x && x < f->x + f->w && y >= f->y && y < f->y + f->h)
            return i;
    }
    return -1;
}

/* ─── 조회 ────────────────────────────────────────────────────────────── */

const char *unim_field_rendered(const UnimTestField *f, char *out, size_t n) {
    int c = clamp(f->caret, 0, (int)strlen(f->committed));
    snprintf(out, n, "%.*s%s%s", c, f->committed, f->preedit, f->committed + c);
    return out;
}

const char *unim_field_display(const UnimTestField *f, char *out, size_t n) {
    if (f->hint != UNIM_HINT_PASSWORD)
        return unim_field_rendered(f, out, n);

    /* 비밀번호는 화면에만 마스킹한다. 로그의 rendered 는 실물 그대로 —
     * 테스트 앱이므로 검증 가능성이 우선이다. */
    char buf[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_rendered(f, buf, sizeof buf);

    size_t chars = unim_log_utf8_len(buf);
    size_t o = 0;
    for (size_t i = 0; i < chars && o + 4 < n; i++) {
        memcpy(out + o, "•", 3);   /* U+2022 */
        o += 3;
    }
    out[o] = '\0';
    return out;
}

const char *unim_field_before_caret(const UnimTestField *f, char *out, size_t n) {
    int c = clamp(f->caret, 0, (int)strlen(f->committed));
    snprintf(out, n, "%.*s%.*s", c, f->committed,
             f->preedit_caret, f->preedit);
    return out;
}

int unim_field_caret_from_x(const UnimTestField *f, int x,
                            UnimTextWidthFn measure, void *user) {
    int rel = x - f->x - UNIM_SPEC_FIELD_PAD_X;
    if (rel <= 0) return 0;

    int len = (int)strlen(f->committed);
    int best = 0, best_d = -1;

    /* 문자 경계마다 폭을 재서 가장 가까운 경계를 고른다. */
    for (int i = 0; i <= len; i = (i < len ? utf8_next(f->committed, i) : len + 1)) {
        int w = measure(f->committed, (size_t)i, user);
        int d = w > rel ? w - rel : rel - w;
        if (best_d < 0 || d < best_d) { best_d = d; best = i; }
        if (i == len) break;
    }
    return best;
}

void unim_field_log_render(const UnimTestField *f) {
    char rendered[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_rendered(f, rendered, sizeof rendered);
    unim_log_field_render(f->id, f->committed, f->preedit, f->caret, rendered);
}

/* ─── IM 이 부르는 것 ─────────────────────────────────────────────────── */

void unim_field_commit(UnimTestField *f, const char *text) {
    if (!text || !*text) {
        unim_log_warn("%s: 빈 commit 무시", f->id);
        return;
    }
    unim_log_commit(f->id, text);

    int tlen = (int)strlen(f->committed);
    int add  = (int)strlen(text);
    if (tlen + add >= UNIM_FIELD_TEXT_MAX) {
        unim_log_error("%s: 버퍼 초과 — commit 무시 (현재 %dB + %dB)",
                       f->id, tlen, add);
        return;
    }
    int c = clamp(f->caret, 0, tlen);
    memmove(f->committed + c + add, f->committed + c, (size_t)(tlen - c + 1));
    memcpy(f->committed + c, text, (size_t)add);
    f->caret = c + add;

    /* preedit 은 건드리지 않는다 — 헤더의 경고 참조. */
    unim_field_log_render(f);
}

void unim_field_preedit_start(UnimTestField *f) {
    f->composing = 1;
    unim_log_preedit("start", f->id, f->preedit, f->preedit_caret, NULL);
    unim_field_log_render(f);
}

void unim_field_set_preedit(UnimTestField *f, const char *text, int cursor) {
    const char *t = text ? text : "";
    int len = (int)strlen(t);
    if (len >= UNIM_FIELD_PREEDIT_MAX) {
        unim_log_error("%s: preedit 버퍼 초과 (%dB) — 잘라냄", f->id, len);
        len = UNIM_FIELD_PREEDIT_MAX - 1;
    }
    memcpy(f->preedit, t, (size_t)len);
    f->preedit[len] = '\0';
    f->preedit_caret = (cursor < 0) ? len : clamp(cursor, 0, len);
    if (len > 0) f->composing = 1;

    unim_log_preedit("changed", f->id, f->preedit, f->preedit_caret,
                     len > 0 ? "underline" : NULL);
    unim_field_log_render(f);
}

void unim_field_preedit_end(UnimTestField *f) {
    f->preedit[0]    = '\0';
    f->preedit_caret = 0;
    f->composing     = 0;
    unim_log_preedit("end", f->id, "", 0, NULL);
    unim_field_log_render(f);
}

/* ─── 앱이 부르는 것 ──────────────────────────────────────────────────── */

void unim_field_insert(UnimTestField *f, const char *text) {
    if (!text || !*text) return;
    unim_log_note("%s: IM 미필터 문자 직접 삽입 \"%s\"", f->id, text);

    int tlen = (int)strlen(f->committed);
    int add  = (int)strlen(text);
    if (tlen + add >= UNIM_FIELD_TEXT_MAX) {
        unim_log_error("%s: 버퍼 초과 — 삽입 무시", f->id);
        return;
    }
    int c = clamp(f->caret, 0, tlen);
    memmove(f->committed + c + add, f->committed + c, (size_t)(tlen - c + 1));
    memcpy(f->committed + c, text, (size_t)add);
    f->caret = c + add;
    unim_field_log_render(f);
}

void unim_field_backspace(UnimTestField *f) {
    if (f->composing) {
        unim_log_note("%s: 조합 중 백스페이스 — IM 이 처리한다 (앱 무동작)", f->id);
        return;
    }
    if (f->caret <= 0) {
        unim_log_note("%s: 백스페이스 — 캐럿이 맨 앞, 무동작", f->id);
        return;
    }
    int prev = utf8_prev(f->committed, f->caret);
    int tlen = (int)strlen(f->committed);
    unim_log_note("%s: 백스페이스 %d→%d (%dB 삭제)",
                  f->id, f->caret, prev, f->caret - prev);
    memmove(f->committed + prev, f->committed + f->caret,
            (size_t)(tlen - f->caret + 1));
    f->caret = prev;
    unim_field_log_render(f);
}

void unim_field_delete(UnimTestField *f) {
    if (f->composing) {
        unim_log_note("%s: 조합 중 Delete — IM 이 처리한다 (앱 무동작)", f->id);
        return;
    }
    int tlen = (int)strlen(f->committed);
    if (f->caret >= tlen) {
        unim_log_note("%s: Delete — 캐럿이 맨 뒤, 무동작", f->id);
        return;
    }
    int next = utf8_next(f->committed, f->caret);
    memmove(f->committed + f->caret, f->committed + next,
            (size_t)(tlen - next + 1));
    unim_log_note("%s: Delete (%dB 삭제)", f->id, next - f->caret);
    unim_field_log_render(f);
}

void unim_field_move_caret(UnimTestField *f, int dir) {
    if (f->composing) {
        unim_log_note("%s: 조합 중 캐럿 이동 요청 — 무시", f->id);
        return;
    }
    int before = f->caret;
    f->caret = (dir < 0) ? utf8_prev(f->committed, f->caret)
                         : utf8_next(f->committed, f->caret);
    unim_log_note("%s: 캐럿 %d→%d", f->id, before, f->caret);
    unim_field_log_render(f);
}

void unim_field_caret_home(UnimTestField *f) {
    if (f->composing) return;
    unim_log_note("%s: 캐럿 %d→0 (Home)", f->id, f->caret);
    f->caret = 0;
    unim_field_log_render(f);
}

void unim_field_caret_end(UnimTestField *f) {
    if (f->composing) return;
    int e = (int)strlen(f->committed);
    unim_log_note("%s: 캐럿 %d→%d (End)", f->id, f->caret, e);
    f->caret = e;
    unim_field_log_render(f);
}

void unim_field_clear(UnimTestField *f) {
    unim_log_note("%s: 필드 비움 (확정 %dB, 조합 %dB 폐기)",
                  f->id, (int)strlen(f->committed), (int)strlen(f->preedit));
    f->committed[0]  = '\0';
    f->preedit[0]    = '\0';
    f->caret         = 0;
    f->preedit_caret = 0;
    f->composing     = 0;
    unim_field_log_render(f);
}

void unim_field_set_focus(UnimTestField *f, int focused, const char *prev_id) {
    if (f->focused == focused) return;
    f->focused = focused;
    unim_log_focus(focused ? "in" : "out", f->id, prev_id);
    unim_field_log_render(f);
}
