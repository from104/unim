/**
 * unim_replace_expect_matches 단위 시험 — 대상③ 교체 전 expect 대조 (HANJA_WORD_SPEC v1.6 §2.4b.5)
 * 데몬·시험 앱 없이 glib 만으로 돈다.
 */
#include "unim_surrounding_word.h"

#include <stdio.h>
#include <string.h>

static int g_pass = 0, g_fail = 0;

#define CHECK(name, cond)                                              \
    do {                                                               \
        if (cond) { g_pass++; }                                        \
        else { g_fail++; fprintf(stderr, "FAIL: %s\n", name); }        \
    } while (0)

/* 문자 오프셋 n 의 바이트 인덱스 */
static gint
byte_of(const gchar *text, glong chars)
{
    return (gint)(g_utf8_offset_to_pointer(text, chars) - text);
}

/* ---- unim_sw_replace_exec: 모의 어댑터 ---- */
typedef struct {
    gboolean focused, sensitive, fetch_ok, has_selection, delete_ok;
    const gchar *text;
    gint cursor, anchor;
    int deletes, commits;
    gint last_offset, last_n;
    gchar committed[64];
} Mock;

static gboolean m_focused(gpointer c) { return ((Mock *)c)->focused; }
static gboolean m_sensitive(gpointer c) { return ((Mock *)c)->sensitive; }
static gboolean m_fetch(gpointer c, const gchar **t, gint *cur, gint *anc, gboolean *sel)
{
    Mock *m = c;
    if (!m->fetch_ok) return FALSE;
    *t = m->text; *cur = m->cursor; *anc = m->anchor; *sel = m->has_selection;
    return TRUE;
}
static gboolean m_delete(gpointer c, gint off, gint n)
{
    Mock *m = c;
    m->deletes++; m->last_offset = off; m->last_n = n;
    return m->delete_ok;
}
static void m_commit(gpointer c, const gchar *t)
{
    Mock *m = c;
    m->commits++;
    g_strlcpy(m->committed, t, sizeof m->committed);
}

static const UnimSwReplaceOps k_ops = { m_focused, m_sensitive, m_fetch, m_delete, m_commit };

static Mock
mock_ok(void)
{
    Mock m = { .focused = TRUE, .fetch_ok = TRUE, .delete_ok = TRUE };
    m.text = "국어교육 만세";
    m.cursor = m.anchor = byte_of(m.text, 4);
    return m;
}

static void
test_replace_exec(void)
{
    const gchar *r;
    Mock m;

    m = mock_ok();
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_ok_reason", strcmp(r, "ok") == 0);
    CHECK("exec_ok_delete_once", m.deletes == 1 && m.last_offset == -4 && m.last_n == 4);
    CHECK("exec_ok_commit", m.commits == 1 && strcmp(m.committed, "國語敎育") == 0);

    m = mock_ok();
    m.cursor = m.anchor = byte_of(m.text, 2);
    r = unim_sw_replace_exec(&k_ops, &m, 2, 2, "國語敎育", "국어교육");
    CHECK("exec_mid_range", strcmp(r, "ok") == 0 && m.last_offset == -2 && m.last_n == 4);

    m = mock_ok();
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교욱");
    CHECK("exec_verify_mismatch", strcmp(r, "verify_mismatch") == 0 && m.deletes == 0 && m.commits == 0);

    m = mock_ok(); m.fetch_ok = FALSE;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_fetch_fail", strcmp(r, "verify_mismatch") == 0 && m.deletes == 0 && m.commits == 0);

    m = mock_ok(); m.delete_ok = FALSE;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_delete_failed", strcmp(r, "delete_failed") == 0 && m.deletes == 1 && m.commits == 0);

    m = mock_ok(); m.focused = FALSE;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_nofocus", strcmp(r, "nofocus") == 0 && m.deletes == 0 && m.commits == 0);

    m = mock_ok(); m.sensitive = TRUE;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_sensitive", strcmp(r, "sensitive") == 0 && m.deletes == 0 && m.commits == 0);

    m = mock_ok();
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "", "국어교육");
    CHECK("exec_empty_commit", strcmp(r, "empty_commit") == 0 && m.deletes == 0);
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, NULL, "국어교육");
    CHECK("exec_null_commit", strcmp(r, "empty_commit") == 0 && m.deletes == 0);

    m = mock_ok(); m.has_selection = TRUE;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_selection", strcmp(r, "selection") == 0 && m.deletes == 0 && m.commits == 0);

    /* anchor != cursor (GTK4 식 선택 판정) 은 expect 대조에서 걸린다 */
    m = mock_ok(); m.anchor = 0;
    r = unim_sw_replace_exec(&k_ops, &m, 4, 0, "國語敎育", "국어교육");
    CHECK("exec_anchor_selection", strcmp(r, "verify_mismatch") == 0 && m.deletes == 0);
}

int
main(void)
{
    const gchar *doc = "국어교육 만세";

    /* 일치 — 커서 끝 */
    {
        const gchar *t = "국어교육";
        gint c = byte_of(t, 4);
        CHECK("end_match", unim_replace_expect_matches(t, c, c, 4, 0, "국어교육"));
        CHECK("end_match_partial", unim_replace_expect_matches(t, c, c, 2, 0, "교육"));
    }
    /* 일치 — 커서 중간(앞 2·뒤 2) */
    {
        gint c = byte_of(doc, 2);
        CHECK("mid_match", unim_replace_expect_matches(doc, c, c, 2, 2, "국어교육"));
        CHECK("mid_match_after_only", unim_replace_expect_matches(doc, c, c, 0, 2, "교육"));
        CHECK("mid_match_before_only", unim_replace_expect_matches(doc, c, c, 2, 0, "국어"));
    }
    /* 불일치 */
    {
        gint c = byte_of(doc, 2);
        CHECK("mismatch_text", !unim_replace_expect_matches(doc, c, c, 2, 2, "국어교욱"));
        CHECK("mismatch_shifted", !unim_replace_expect_matches(doc, c, c, 1, 2, "국어교육"));
        CHECK("mismatch_len_short", !unim_replace_expect_matches(doc, c, c, 2, 2, "국어교"));
    }
    /* c < a */
    {
        gint c = byte_of(doc, 1);
        CHECK("c_less_than_a", !unim_replace_expect_matches(doc, c, c, 2, 0, "국어"));
    }
    /* c + b > len */
    {
        const gchar *t = "국어";
        gint c = byte_of(t, 1);
        CHECK("c_plus_b_over", !unim_replace_expect_matches(t, c, c, 1, 2, "국어교"));
    }
    /* anchor != cursor (선택 있음) */
    {
        gint c = byte_of(doc, 2);
        gint a = byte_of(doc, 0);
        CHECK("selection", !unim_replace_expect_matches(doc, c, a, 2, 2, "국어교육"));
    }
    /* 커서 앞 4바이트 문자(😀) — 오프셋은 문자 단위 */
    {
        const gchar *t = "😀국어";
        gint c = byte_of(t, 3);
        CHECK("emoji_prefix_match", unim_replace_expect_matches(t, c, c, 2, 0, "국어"));
        CHECK("emoji_prefix_include", !unim_replace_expect_matches(t, c, c, 3, 0, "국어"));
        gint c1 = byte_of(t, 1);
        CHECK("emoji_cursor_after_emoji", unim_replace_expect_matches(t, c1, c1, 0, 2, "국어"));
    }
    /* 빈 expect */
    {
        gint c = byte_of(doc, 2);
        CHECK("empty_expect", !unim_replace_expect_matches(doc, c, c, 0, 0, ""));
        CHECK("null_expect", !unim_replace_expect_matches(doc, c, c, 0, 0, NULL));
        CHECK("null_text", !unim_replace_expect_matches(NULL, 0, 0, 0, 2, "국어"));
    }
    /* 잘못된 UTF-8 */
    {
        const gchar bad[] = "\xEA\xB5\xAD\xEC\x96";   /* 잘린 두 번째 문자 */
        CHECK("invalid_utf8_text", !unim_replace_expect_matches(bad, 3, 3, 1, 0, "국"));
        const gchar bad_expect[] = "\xEA\xB5";
        CHECK("invalid_utf8_expect", !unim_replace_expect_matches("국", 3, 3, 1, 0, bad_expect));
    }
    /* 커서 바이트가 문자 경계가 아님 / 범위 밖 / 음수 */
    {
        CHECK("not_char_boundary", !unim_replace_expect_matches("국어", 1, 1, 0, 1, "국"));
        CHECK("cursor_out_of_range", !unim_replace_expect_matches("국어", 100, 100, 1, 0, "어"));
        CHECK("negative_cursor", !unim_replace_expect_matches("국어", -1, -1, 0, 1, "국"));
    }
    /* 한글 외 문자도 코드포인트 단위로 동일하게 */
    {
        const gchar *t = "abc국어";
        gint c = byte_of(t, 5);
        CHECK("ascii_prefix_match", unim_replace_expect_matches(t, c, c, 2, 0, "국어"));
    }

    test_replace_exec();

    printf("test_surrounding_word: %d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
