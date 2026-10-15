/**
 * 대상③ 교체 대조 — GLib 만 의존하는 순수 로직 (헤더 참조)
 */
#include "unim_surrounding_word.h"

#include <string.h>

gboolean
unim_replace_expect_matches(const gchar *text,
                             gint cursor_byte,
                             gint anchor_byte,
                             guint before,
                             guint after,
                             const gchar *expect)
{
    if (!text || !expect || !*expect) return FALSE;
    if (cursor_byte < 0 || anchor_byte < 0) return FALSE;
    if (anchor_byte != cursor_byte) return FALSE;            /* 선택이 있다 */
    if (!g_utf8_validate(text, -1, NULL) || !g_utf8_validate(expect, -1, NULL))
        return FALSE;

    gsize tlen = strlen(text);
    if ((gsize)cursor_byte > tlen) return FALSE;
    /* 커서는 문자 경계여야 한다(연속 바이트 위면 거부) */
    if ((gsize)cursor_byte < tlen && ((guchar)text[cursor_byte] & 0xC0) == 0x80)
        return FALSE;

    glong cursor_chars = g_utf8_pointer_to_offset(text, text + cursor_byte);
    glong total_chars = g_utf8_strlen(text, -1);
    if ((glong)before > cursor_chars) return FALSE;                  /* c < a */
    if (cursor_chars + (glong)after > total_chars) return FALSE;    /* c+b > len */

    /* 기대 길이와 구간 길이가 같아야 한다 */
    glong expect_chars = g_utf8_strlen(expect, -1);
    if ((glong)before + (glong)after != expect_chars) return FALSE;

    const gchar *start = g_utf8_offset_to_pointer(text, cursor_chars - (glong)before);
    const gchar *end = g_utf8_offset_to_pointer(text, cursor_chars + (glong)after);
    gsize span = (gsize)(end - start);
    return span == strlen(expect) && memcmp(start, expect, span) == 0;
}
