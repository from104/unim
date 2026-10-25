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

/**
 * 대상③ 교체 확정 실행 — GTK3/GTK4 IM 모듈 공통 (계획 F7 규약).
 *
 * GTK3/4 의 API 차이(문서 재조회·선택 판정·삭제·커밋)는 콜백으로 주입한다.
 * 순서: 포커스 → 민감 필드 → 빈 커밋 → 문서 재조회 → 선택 → expect 대조 →
 * delete_surrounding → (성공일 때만) commit. XTest 폴백은 없다(오삭제 방지).
 */
typedef struct {
    /** 포커스가 있으면 TRUE */
    gboolean (*is_focused)(gpointer ctx);
    /** 비밀번호·PIN 등 민감 필드면 TRUE */
    gboolean (*is_sensitive)(gpointer ctx);
    /**
     * 문서를 다시 조회한다(검증 조회는 데몬에 보내지 않는다). 성공하면 TRUE 와 함께
     * text(소유권은 ctx 에 남는다)·cursor/anchor 바이트 인덱스·선택 유무를 채운다.
     * 조회 실패·text 없음이면 FALSE.
     */
    gboolean (*fetch_surrounding)(gpointer ctx, const gchar **text, gint *cursor_byte,
                                  gint *anchor_byte, gboolean *has_selection);
    /** gtk_im_context_delete_surrounding 에 해당. 성공이면 TRUE */
    gboolean (*delete_surrounding)(gpointer ctx, gint offset, gint n_chars);
    /** 확정 문자열 커밋 */
    void (*commit)(gpointer ctx, const gchar *text);
} UnimSwReplaceOps;

/** unim_sw_replace_exec 가 성공했을 때의 reason */
#define UNIM_SW_REASON_OK "ok"

/**
 * 확정 실행. 평문을 로그에 남기지 않도록 결과를 reason 코드로만 돌려준다.
 *
 * @return 적용했으면 "ok"(UNIM_SW_REASON_OK), 버렸으면 이유:
 *         "nofocus"·"sensitive"·"empty_commit"·"verify_mismatch"·"selection"·"delete_failed"
 *         (항상 정적 문자열). 계획 F7 코드는 ok·verify_mismatch·delete_failed·nofocus.
 */
const gchar *unim_sw_replace_exec(const UnimSwReplaceOps *ops,
                                   gpointer ctx,
                                   guint before,
                                   guint after,
                                   const gchar *commit_text,
                                   const gchar *expect_text);

#endif /* UNIM_SURROUNDING_WORD_H */
