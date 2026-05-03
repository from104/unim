/**
 * UNIM 이모지 팝업 (GTK-공통)
 *
 * `ShowEmojiPopupV2` DBus 시그널을 받아 9×9 그리드 + 좌측 9 탭 (Recent + 8 카테고리)
 * 으로 이모지를 표시하는 팝업 윈도우입니다. GTK3/GTK4 IM 모듈에서 공통 사용.
 *
 * 한자/특수문자 팝업과 다른 점 (PR #4 emoji overhaul, OPTION X engine-driven):
 * - 키 처리는 엔진이 담당 (`unim_dbus_process_key`) → IM 모듈은 시그널 수신만
 *   (`handle_key` 함수 없음)
 * - 페이지/셀 갱신은 `PopupNavigate` 시그널 → `unim_emoji_popup_navigate()`
 * - 카테고리 전환은 `ShowEmojiPopupV2` 재발행 → `unim_emoji_popup_show()` 재호출
 * - 즐겨찾기(bookmark) 대신 'Recent' (MRU) 캐시
 */

#ifndef UNIM_EMOJI_POPUP_H
#define UNIM_EMOJI_POPUP_H

#include <gtk/gtk.h>
#include "unim_dbus_client.h"

G_BEGIN_DECLS

/**
 * 이모지 팝업 컨텍스트 (opaque)
 */
typedef struct _UnimEmojiPopup UnimEmojiPopup;

/**
 * 좌측 9 탭 메타데이터 (id, ko, en, count) — `ShowEmojiPopupV2` payload 의
 * `categories: a(sssu)` 와 1:1.
 */
typedef struct {
    gchar *id;       /* "Recent" / "SmileysPeople" / ... / "Flags" */
    gchar *name_ko;  /* 한국어 라벨 */
    gchar *name_en;  /* 영문 라벨 */
    guint count;     /* 카테고리 emoji 개수 */
} UnimEmojiCategory;

/**
 * 이모지 커밋 콜백 (셀 클릭/Enter 시 호출)
 *
 * @param emoji 선택된 이모지 문자열 (UTF-8)
 * @param user_data 사용자 데이터
 */
typedef void (*UnimEmojiCommitCallback)(const gchar *emoji, gpointer user_data);

/**
 * 새 이모지 팝업 생성
 *
 * @return 새 팝업 또는 실패 시 NULL
 */
UnimEmojiPopup* unim_emoji_popup_new(void);

/**
 * 이모지 팝업 해제
 *
 * @param popup 해제할 팝업
 */
void unim_emoji_popup_free(UnimEmojiPopup *popup);

/**
 * 이모지 팝업 표시 — `ShowEmojiPopupV2` 시그널 핸들러용.
 *
 * @param popup 팝업
 * @param target_cat_id 시작 카테고리 id ("Recent"/"SmileysPeople"/.../"Flags")
 * @param items 시작 카테고리의 emoji 풀 (전체)
 * @param item_count items 길이
 * @param top_row 활성 영문 키맵의 상단 9 문자 (NULL 이면 "QWERTYUIO")
 * @param recent 'Recent' 탭 캐시 (현재 표시에는 직접 안 쓰지만 set_recent 와
 *               동일한 데이터 — daemon 의 popup_state 가 cat_index=0 일 때
 *               items 에 동일 내용을 이미 담아 보냄)
 * @param recent_count recent 길이
 * @param categories 좌측 9 탭 메타 (정확히 9개)
 * @param category_count categories 길이 (보통 9)
 * @param x 팝업 X 좌표 (커서 절대 위치)
 * @param y 팝업 Y 좌표 (커서 아래)
 * @param cursor_height 커서 높이 (위로 올릴 때 사용)
 * @param callback 셀 클릭/Enter 시 emoji 커밋 콜백
 * @param user_data 콜백 사용자 데이터
 */
void unim_emoji_popup_show(UnimEmojiPopup *popup,
                            const gchar *target_cat_id,
                            const gchar * const *items,
                            gsize item_count,
                            const gchar *top_row,
                            const gchar * const *recent,
                            gsize recent_count,
                            const UnimEmojiCategory *categories,
                            gsize category_count,
                            gint x,
                            gint y,
                            gint cursor_height,
                            UnimEmojiCommitCallback callback,
                            gpointer user_data);

/**
 * 이모지 팝업 숨김
 *
 * @param popup 팝업
 */
void unim_emoji_popup_hide(UnimEmojiPopup *popup);

/**
 * 이모지 팝업 표시 여부
 *
 * @param popup 팝업
 * @return 표시 중이면 TRUE
 */
gboolean unim_emoji_popup_is_visible(UnimEmojiPopup *popup);

/**
 * `PopupNavigate` 시그널 처리 — 페이지/셀만 갱신.
 *
 * 카테고리 전환은 `ShowEmojiPopupV2` 재발행으로 처리되므로 여기서는 같은
 * 카테고리 내 페이지/셀 변경만 반영한다 (한자 `replace_candidates` 와 달리
 * 후보 배열은 교체하지 않음).
 *
 * @param popup 팝업
 * @param page 새 페이지 (0-based)
 * @param total_pages 총 페이지 수
 * @param selected 페이지 내 선택 인덱스 (참고 용도, 사용 안 함)
 * @param rows 활성 행 수 (참고 용도)
 * @param cols 활성 열 수 (참고 용도)
 * @param sel_row 새 선택 행 (0-based)
 * @param sel_col 새 선택 열 (0-based)
 */
void unim_emoji_popup_navigate(UnimEmojiPopup *popup,
                                gint page,
                                gint total_pages,
                                gint selected,
                                gint rows,
                                gint cols,
                                gint sel_row,
                                gint sel_col);

/**
 * 'Recent' 탭 캐시 갱신 — 별도 시그널/RPC 응답으로 갱신할 경우 사용.
 *
 * 일반 흐름에서는 `ShowEmojiPopupV2` 재발행으로 categories[0] 의 count 와
 * 'Recent' 카테고리 items 가 함께 갱신되므로 별도 호출이 필요 없을 수 있다.
 *
 * @param popup 팝업
 * @param emojis 'Recent' 이모지 배열
 * @param count emojis 길이
 */
void unim_emoji_popup_set_recent(UnimEmojiPopup *popup,
                                  const gchar * const *emojis,
                                  gsize count);

/**
 * 페이지 이동 콜백 (◀/▶ 풋터 버튼 클릭용).
 *
 * @param direction 0=Prev, 1=Next
 */
typedef void (*UnimEmojiPageChangeCallback)(gint direction, gpointer user_data);

/**
 * 페이지 이동 콜백 설정 (◀/▶ → unim_dbus_popup_change_page 등 위임).
 */
void unim_emoji_popup_set_page_change_callback(UnimEmojiPopup *popup,
                                                UnimEmojiPageChangeCallback callback,
                                                gpointer user_data);

G_END_DECLS

#endif /* UNIM_EMOJI_POPUP_H */
