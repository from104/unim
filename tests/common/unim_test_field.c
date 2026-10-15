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

/** 글자 중간이면 그 글자의 앞 경계로 내리고, 범위를 [0,len] 로 맞춘다. */
static int utf8_align(const char *s, int pos) {
    int len = (int)strlen(s);
    if (pos <= 0) return 0;
    if (pos >= len) return len;
    while (pos > 0 && ((unsigned char)s[pos] & 0xC0) == 0x80) pos--;
    return pos;
}

/* ─── 선택 내부 도우미 ────────────────────────────────────────────────── */

typedef struct { int s, e; } SelSnap;

/** 유효한 anchor. caret 이 마지막 확정 때와 다르면(앱이 직접 대입) anchor 는 무효. */
static int eff_anchor(const UnimTestField *f) {
    int len = (int)strlen(f->committed);
    if (f->caret != f->sel_caret) return clamp(f->caret, 0, len);
    return clamp(f->anchor, 0, len);
}

static SelSnap snap_of(const UnimTestField *f) {
    int len = (int)strlen(f->committed);
    int c = clamp(f->caret, 0, len);
    int a = eff_anchor(f);
    SelSnap r = { a < c ? a : c, a < c ? c : a };
    return r;
}

/** 지금 caret 을 기준으로 anchor 를 확정한다. */
static void sel_store(UnimTestField *f, int anchor) {
    f->anchor = anchor;
    f->sel_caret = f->caret;
}

/** 연산 시작 때 부른다 — 어긋난 anchor 를 접는다. */
static void sel_sync(UnimTestField *f) { sel_store(f, eff_anchor(f)); }

/** 선택 범위가 이전과 달라졌으면 field.select 를 한 줄 낸다. */
static void sel_report(const UnimTestField *f, SelSnap old, const char *reason) {
    SelSnap n = snap_of(f);
    if (n.s == n.e && old.s == old.e) return;   /* 선택이 없던 → 없는 변화는 로그 없음 */
    if (n.s == old.s && n.e == old.e) return;
    char buf[UNIM_FIELD_TEXT_MAX];
    snprintf(buf, sizeof buf, "%.*s", n.e - n.s, f->committed + n.s);
    unim_log_select(f->id, eff_anchor(f), f->caret, buf,
                    n.e > n.s && f->hint != UNIM_HINT_PASSWORD, reason);
}

/** 선택을 지운다(caret=선택 시작). 지운 바이트 수. 로그 없음. */
static int sel_erase(UnimTestField *f) {
    sel_sync(f);
    SelSnap r = snap_of(f);
    if (r.e <= r.s) return 0;
    int tlen = (int)strlen(f->committed);
    memmove(f->committed + r.s, f->committed + r.e, (size_t)(tlen - r.e + 1));
    f->caret = r.s;
    sel_store(f, r.s);
    return r.e - r.s;
}

/** 단어 경계 글자인가 (공백·구두점). 한글·한자·영숫자·밑줄은 단어 글자. */
static int is_sep_at(const char *s, int pos) {
    const unsigned char *p = (const unsigned char *)s + pos;
    unsigned cp;
    if (p[0] < 0x80) {
        cp = p[0];
        if ((cp >= '0' && cp <= '9') || (cp >= 'A' && cp <= 'Z') ||
            (cp >= 'a' && cp <= 'z') || cp == '_') return 0;
        return 1;
    }
    if ((p[0] & 0xE0) == 0xC0)      cp = ((p[0] & 0x1Fu) << 6) | (p[1] & 0x3Fu);
    else if ((p[0] & 0xF0) == 0xE0) cp = ((p[0] & 0x0Fu) << 12) | ((p[1] & 0x3Fu) << 6) | (p[2] & 0x3Fu);
    else return 0;
    return (cp >= 0x00A0 && cp <= 0x00BF) ||
           (cp >= 0x2000 && cp <= 0x206F) ||
           (cp >= 0x3000 && cp <= 0x3003) || cp == 0x30FB ||
           (cp >= 0xFF01 && cp <= 0xFF0F) || (cp >= 0xFF1A && cp <= 0xFF20) ||
           (cp >= 0xFF3B && cp <= 0xFF40) || (cp >= 0xFF5B && cp <= 0xFF65);
}

static int utf8_count(const char *s, int nbytes) {
    int n = 0;
    for (int i = 0; i < nbytes && s[i]; i++)
        if (((unsigned char)s[i] & 0xC0) != 0x80) n++;
    return n;
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

    /* 비밀번호 필드는 화면에 •(3B)로 그려진다 — 같은 글자 수의 • 열로 잰다.
     * 그래야 클릭·드래그 위치가 그려진 폭·선택 하이라이트와 맞는다. */
    int pw = f->hint == UNIM_HINT_PASSWORD;
    char mask[UNIM_FIELD_TEXT_MAX * 3 + 1];
    if (pw) {
        int m = 0;
        for (int i = 0; i < len && m + 3 < (int)sizeof mask; i = utf8_next(f->committed, i)) {
            memcpy(mask + m, "\xe2\x80\xa2", 3);
            m += 3;
        }
        mask[m] = '\0';
    }

    /* 문자 경계마다 폭을 재서 가장 가까운 경계를 고른다. */
    int nchar = 0;
    for (int i = 0; i <= len; i = (i < len ? utf8_next(f->committed, i) : len + 1), nchar++) {
        int w = pw ? measure(mask, (size_t)nchar * 3, user)
                   : measure(f->committed, (size_t)i, user);
        int d = w > rel ? w - rel : rel - w;
        if (best_d < 0 || d < best_d) { best_d = d; best = i; }
        if (i == len) break;
    }
    return best;
}

void unim_field_log_render(const UnimTestField *f) {
    char rendered[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
    unim_field_rendered(f, rendered, sizeof rendered);
    unim_log_field_render_sel(f->id, f->committed, f->preedit, f->caret,
                              rendered, eff_anchor(f));
}

/* ─── IM 이 부르는 것 ─────────────────────────────────────────────────── */

void unim_field_commit(UnimTestField *f, const char *text) {
    if (!text || !*text) {
        unim_log_warn("%s: 빈 commit 무시", f->id);
        return;
    }
    unim_log_commit(f->id, text);

    sel_sync(f);
    SelSnap old = snap_of(f);
    int tlen = (int)strlen(f->committed);
    int add  = (int)strlen(text);
    if (tlen - (old.e - old.s) + add >= UNIM_FIELD_TEXT_MAX) {
        unim_log_error("%s: 버퍼 초과 — commit 무시 (현재 %dB + %dB)",
                       f->id, tlen, add);
        return;
    }
    if (old.e > old.s) sel_erase(f);   /* 선택 위에 넣으면 선택을 대체한다 */
    tlen = (int)strlen(f->committed);
    int c = clamp(f->caret, 0, tlen);
    memmove(f->committed + c + add, f->committed + c, (size_t)(tlen - c + 1));
    memcpy(f->committed + c, text, (size_t)add);
    f->caret = c + add;
    sel_store(f, f->caret);
    sel_report(f, old, "edit");

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

    sel_sync(f);
    SelSnap old = snap_of(f);
    int tlen = (int)strlen(f->committed);
    int add  = (int)strlen(text);
    if (tlen - (old.e - old.s) + add >= UNIM_FIELD_TEXT_MAX) {
        unim_log_error("%s: 버퍼 초과 — 삽입 무시", f->id);
        return;
    }
    if (old.e > old.s) sel_erase(f);
    tlen = (int)strlen(f->committed);
    int c = clamp(f->caret, 0, tlen);
    memmove(f->committed + c + add, f->committed + c, (size_t)(tlen - c + 1));
    memcpy(f->committed + c, text, (size_t)add);
    f->caret = c + add;
    sel_store(f, f->caret);
    sel_report(f, old, "edit");
    unim_field_log_render(f);
}

void unim_field_backspace(UnimTestField *f) {
    /* 화면에 조합 글자가 있을 때만 IM 몫으로 본다. XIM 은 조합이 끝나도 PreeditStart/Done
     * 사이클을 열어 둔 채 내용만 비우므로(third_party/xim preedit_clear_keep_session),
     * composing 만으로 막으면 IM 이 되돌려 준 자가 주입 BackSpace(AutoTypeFix·한자 단어
     * 교체)까지 무시해 실제 앱과 달라진다 — 실제 앱은 IM 이 거르지 않은 키를 그대로 처리한다. */
    if (f->composing && f->preedit[0]) {
        unim_log_note("%s: 조합 중 백스페이스 — IM 이 처리한다 (앱 무동작)", f->id);
        return;
    }
    sel_sync(f);
    SelSnap old = snap_of(f);
    if (old.e > old.s) {
        int n = sel_erase(f);
        unim_log_note("%s: 백스페이스 — 선택 [%d,%d) %dB 삭제", f->id, old.s, old.e, n);
        sel_report(f, old, "edit");
        unim_field_log_render(f);
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
    sel_store(f, prev);
    unim_field_log_render(f);
}

void unim_field_delete(UnimTestField *f) {
    /* 화면에 조합 글자가 있을 때만 IM 몫으로 본다. XIM 은 조합이 끝나도 PreeditStart/Done
     * 사이클을 열어 둔 채 내용만 비우므로(third_party/xim preedit_clear_keep_session),
     * composing 만으로 막으면 IM 이 되돌려 준 자가 주입 BackSpace(AutoTypeFix·한자 단어
     * 교체)까지 무시해 실제 앱과 달라진다 — 실제 앱은 IM 이 거르지 않은 키를 그대로 처리한다. */
    if (f->composing && f->preedit[0]) {
        unim_log_note("%s: 조합 중 Delete — IM 이 처리한다 (앱 무동작)", f->id);
        return;
    }
    sel_sync(f);
    SelSnap old = snap_of(f);
    if (old.e > old.s) {
        int n = sel_erase(f);
        unim_log_note("%s: Delete — 선택 [%d,%d) %dB 삭제", f->id, old.s, old.e, n);
        sel_report(f, old, "edit");
        unim_field_log_render(f);
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

void unim_field_move_caret_ex(UnimTestField *f, int dir, int extend) {
    if (f->composing) {
        unim_log_note("%s: 조합 중 캐럿 이동 요청 — 무시", f->id);
        return;
    }
    sel_sync(f);
    SelSnap old = snap_of(f);
    int before = f->caret;
    if (!extend && old.e > old.s)
        f->caret = (dir < 0) ? old.s : old.e;      /* 선택을 앞/뒤 끝으로 접는다 */
    else
        f->caret = (dir < 0) ? utf8_prev(f->committed, f->caret)
                             : utf8_next(f->committed, f->caret);
    sel_store(f, extend ? f->anchor : f->caret);
    unim_log_note("%s: 캐럿 %d→%d%s", f->id, before, f->caret,
                  extend ? " (Shift 확장)" : "");
    sel_report(f, old, extend ? "shift-move" : "move");
    unim_field_log_render(f);
}

void unim_field_move_caret(UnimTestField *f, int dir) {
    unim_field_move_caret_ex(f, dir, 0);
}

void unim_field_caret_home_ex(UnimTestField *f, int extend) {
    if (f->composing) return;
    sel_sync(f);
    SelSnap old = snap_of(f);
    unim_log_note("%s: 캐럿 %d→0 (Home%s)", f->id, f->caret, extend ? ", Shift 확장" : "");
    f->caret = 0;
    sel_store(f, extend ? f->anchor : 0);
    sel_report(f, old, extend ? "shift-move" : "home");
    unim_field_log_render(f);
}

void unim_field_caret_home(UnimTestField *f) { unim_field_caret_home_ex(f, 0); }

void unim_field_caret_end_ex(UnimTestField *f, int extend) {
    if (f->composing) return;
    sel_sync(f);
    SelSnap old = snap_of(f);
    int e = (int)strlen(f->committed);
    unim_log_note("%s: 캐럿 %d→%d (End%s)", f->id, f->caret, e, extend ? ", Shift 확장" : "");
    f->caret = e;
    sel_store(f, extend ? f->anchor : e);
    sel_report(f, old, extend ? "shift-move" : "end");
    unim_field_log_render(f);
}

void unim_field_caret_end(UnimTestField *f) { unim_field_caret_end_ex(f, 0); }

void unim_field_clear(UnimTestField *f) {
    unim_log_note("%s: 필드 비움 (확정 %dB, 조합 %dB 폐기)",
                  f->id, (int)strlen(f->committed), (int)strlen(f->preedit));
    SelSnap old = snap_of(f);
    f->committed[0]  = '\0';
    f->preedit[0]    = '\0';
    f->caret         = 0;
    sel_store(f, 0);
    f->preedit_caret = 0;
    f->composing     = 0;
    sel_report(f, old, "clear");
    unim_field_log_render(f);
}

/* ─── 선택 ────────────────────────────────────────────────────────────── */

void unim_field_select_all(UnimTestField *f) {
    if (f->composing) {
        unim_log_note("%s: 조합 중 전체 선택 요청 — 무시", f->id);
        return;
    }
    sel_sync(f);
    SelSnap old = snap_of(f);
    f->caret = (int)strlen(f->committed);
    sel_store(f, 0);
    unim_log_note("%s: 전체 선택 [0,%d)", f->id, f->caret);
    sel_report(f, old, "select-all");
    unim_field_log_render(f);
}

void unim_field_select_word_at(UnimTestField *f, int off) {
    int len = (int)strlen(f->committed);
    if (len == 0) return;
    sel_sync(f);
    SelSnap old = snap_of(f);

    int pos = utf8_align(f->committed, off);
    if (pos >= len) pos = utf8_prev(f->committed, len);
    int cls = is_sep_at(f->committed, pos);
    int st = pos, en = utf8_next(f->committed, pos);
    while (st > 0) {
        int p = utf8_prev(f->committed, st);
        if (is_sep_at(f->committed, p) != cls) break;
        st = p;
    }
    while (en < len && is_sep_at(f->committed, en) == cls)
        en = utf8_next(f->committed, en);

    f->caret = en;
    sel_store(f, st);
    unim_log_note("%s: 단어 선택 [%d,%d)", f->id, st, en);
    sel_report(f, old, "word");
    unim_field_log_render(f);
}

void unim_field_set_anchor(UnimTestField *f, int off) {
    sel_sync(f);
    SelSnap old = snap_of(f);
    f->caret = utf8_align(f->committed, off);
    sel_store(f, f->caret);
    sel_report(f, old, "press");
    unim_field_log_render(f);
}

void unim_field_set_caret(UnimTestField *f, int off) {
    unim_field_set_anchor(f, off);
}

void unim_field_extend_to(UnimTestField *f, int off) {
    sel_sync(f);
    SelSnap old = snap_of(f);
    int c = utf8_align(f->committed, off);
    if (c == f->caret) return;               /* 드래그 중 같은 자리 — 침묵 */
    f->caret = c;
    sel_store(f, f->anchor);
    sel_report(f, old, "drag");
    unim_field_log_render(f);
}

int unim_field_delete_selection(UnimTestField *f) {
    sel_sync(f);
    SelSnap old = snap_of(f);
    int n = sel_erase(f);
    if (n <= 0) return 0;
    unim_log_note("%s: 선택 [%d,%d) %dB 삭제", f->id, old.s, old.e, n);
    sel_report(f, old, "edit");
    unim_field_log_render(f);
    return 1;
}

void unim_field_delete_surrounding(UnimTestField *f, int offset, int n_chars) {
    sel_sync(f);
    SelSnap old = snap_of(f);
    int before = f->caret;

    /* 기존 앱 코드(move_caret 반복 + delete 반복)와 같은 가드 — 조합 중에는
     * 캐럿을 옮기지 않고, 조합 글자가 있으면 지우지 않는다. */
    if (f->composing) {
        if (offset) unim_log_note("%s: 조합 중 delete-surrounding 이동 — 무시", f->id);
    } else {
        for (int i = 0; i < -offset; i++) f->caret = utf8_prev(f->committed, f->caret);
        for (int i = 0; i <  offset; i++) f->caret = utf8_next(f->committed, f->caret);
    }
    int deleted = 0;
    if (f->composing && f->preedit[0]) {
        unim_log_note("%s: 조합 중 delete-surrounding 삭제 — IM 이 처리한다 (앱 무동작)", f->id);
    } else {
        int tlen = (int)strlen(f->committed);
        for (int i = 0; i < n_chars && f->caret < tlen; i++) {
            int next = utf8_next(f->committed, f->caret);
            memmove(f->committed + f->caret, f->committed + next,
                    (size_t)(tlen - next + 1));
            deleted += next - f->caret;
            tlen -= next - f->caret;
        }
    }
    sel_store(f, f->caret);                  /* 지운 뒤에는 선택을 접는다 */
    unim_log_note("%s: delete-surrounding offset=%d n=%d → 캐럿 %d→%d, %dB 삭제",
                  f->id, offset, n_chars, before, f->caret, deleted);
    sel_report(f, old, "surrounding");
    unim_field_log_render(f);
}

void unim_field_delete_surrounding_bytes(UnimTestField *f,
                                         unsigned before, unsigned after) {
    sel_sync(f);
    SelSnap old = snap_of(f);
    int c0 = f->caret;
    int deleted = 0;

    if (f->composing && f->preedit[0]) {
        unim_log_note("%s: 조합 중 delete-surrounding 삭제 — IM 이 처리한다 (앱 무동작)", f->id);
    } else {
        int tlen = (int)strlen(f->committed);
        int b = f->caret, e = f->caret;
        long nb = (long)before, na = (long)after;
        while (nb > 0 && b > 0) { int p = utf8_prev(f->committed, b); nb -= b - p; b = p; }
        while (na > 0 && e < tlen) { int n = utf8_next(f->committed, e); na -= n - e; e = n; }
        if (e > b) {
            memmove(f->committed + b, f->committed + e, (size_t)(tlen - e + 1));
            deleted = e - b;
        }
        f->caret = b;
    }
    sel_store(f, f->caret);
    unim_log_note("%s: delete-surrounding before=%uB after=%uB → 캐럿 %d→%d, %dB 삭제",
                  f->id, before, after, c0, f->caret, deleted);
    sel_report(f, old, "surrounding");
    unim_field_log_render(f);
}

/* ─── 선택 조회 ───────────────────────────────────────────────────────── */

int unim_field_anchor(const UnimTestField *f) { return eff_anchor(f); }

int unim_field_selection_range(const UnimTestField *f, int *start, int *end) {
    SelSnap r = snap_of(f);
    if (start) *start = r.s;
    if (end)   *end   = r.e;
    return r.e > r.s;
}

int unim_field_has_selection(const UnimTestField *f) {
    return unim_field_selection_range(f, NULL, NULL);
}

const char *unim_field_selected_text(const UnimTestField *f, char *out, size_t n) {
    SelSnap r = snap_of(f);
    snprintf(out, n, "%.*s", r.e - r.s, f->committed + r.s);
    return out;
}

const char *unim_field_primary_text(const UnimTestField *f, char *out, size_t n) {
    if (f->hint == UNIM_HINT_PASSWORD || !unim_field_has_selection(f)) return NULL;
    return unim_field_selected_text(f, out, n);
}

int unim_field_selection_display_range(const UnimTestField *f, int *start, int *end) {
    SelSnap r = snap_of(f);
    if (r.e <= r.s) {
        if (start) *start = 0;
        if (end)   *end   = 0;
        return 0;
    }
    int plen = (int)strlen(f->preedit);
    int c = clamp(f->caret, 0, (int)strlen(f->committed));
    int rs = r.s, re = r.e;
    if (plen > 0 && c == r.s) { rs += plen; re += plen; }   /* preedit 이 선택 앞에 끼었다 */

    if (f->hint == UNIM_HINT_PASSWORD) {
        char buf[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
        unim_field_rendered(f, buf, sizeof buf);
        rs = utf8_count(buf, rs) * 3;                       /* • = 3바이트 */
        re = utf8_count(buf, re) * 3;
    }
    if (start) *start = rs;
    if (end)   *end   = re;
    return 1;
}

void unim_field_set_focus(UnimTestField *f, int focused, const char *prev_id) {
    if (f->focused == focused) return;
    f->focused = focused;
    unim_log_focus(focused ? "in" : "out", f->id, prev_id);
    unim_field_log_render(f);
}
