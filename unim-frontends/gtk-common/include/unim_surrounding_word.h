/**
 * 대상③(커서 주변 단어) 교체 대조 — GLib 만 의존하는 순수 로직
 *
 * ReplaceSurroundingText 를 받은 GTK 프런트가 surrounding 을 다시 조회한 뒤,
 * 커서 앞 a자·뒤 b자 구간이 데몬이 보낸 expect_text 와 완전 일치하는지 가른다
 * (HANJA_WORD_SPEC v1.6 §2.4b.5). 오프셋 단위는 유니코드 코드포인트(문자).
 */
#ifndef UNIM_SURROUNDING_WORD_H
#define UNIM_SURROUNDING_WORD_H

#include <glib.h>

/**
 * 커서 구간이 expect 와 일치하는지 본다.
 *
 * @param text         surrounding 전문 (UTF-8)
 * @param cursor_byte  커서의 바이트 인덱스 (GTK get_surrounding 의 cursor_index)
 * @param anchor_byte  선택 앵커의 바이트 인덱스 (선택이 없으면 cursor 와 같다)
 * @param before       커서 앞 삭제 문자 수 a
 * @param after        커서 뒤 삭제 문자 수 b
 * @param expect       지울 원문 (UTF-8)
 * @return 일치하면 TRUE. anchor!=cursor·범위 초과·잘못된 UTF-8·빈 expect·
 *         커서가 문자 경계가 아님·구간 불일치는 FALSE
 */
gboolean unim_replace_expect_matches(const gchar *text,
                                      gint cursor_byte,
                                      gint anchor_byte,
                                      guint before,
                                      guint after,
                                      const gchar *expect);

#endif /* UNIM_SURROUNDING_WORD_H */
