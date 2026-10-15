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

    printf("test_surrounding_word: %d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
