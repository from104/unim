/**
 * UNIM 한자 후보 팝업 (GTK-공통)
 *
 * 한자 변환 시 후보 목록을 표시하는 팝업 윈도우입니다.
 * GTK3/GTK4 모두에서 사용할 수 있도록 설계되었습니다.
 */

#ifndef UNIM_HANJA_POPUP_H
#define UNIM_HANJA_POPUP_H

#include <gtk/gtk.h>
#include "unim_dbus_client.h"

G_BEGIN_DECLS

/**
 * 한자 팝업 컨텍스트
 */
typedef struct _UnimHanjaPopup UnimHanjaPopup;

/**
 * 한자 선택 콜백
 *
 * @param hanja 선택된 한자 문자열
 * @param user_data 사용자 데이터
 */
typedef void (*UnimHanjaSelectCallback)(const gchar *hanja, gpointer user_data);

/**
 * 새 한자 팝업 생성
 *
 * @return 새 팝업 또는 실패 시 NULL
 */
UnimHanjaPopup* unim_hanja_popup_new(void);

/**
 * 한자 팝업 해제
 *
 * @param popup 해제할 팝업
 */
void unim_hanja_popup_free(UnimHanjaPopup *popup);

/**
 * 한자 후보 표시
 *
 * @param popup 팝업
 * @param target 변환 대상 문자열
 * @param candidates 후보 배열
 * @param count 후보 개수
 * @param x 팝업 X 위치
 * @param y 팝업 Y 위치 (커서 아래)
 * @param cursor_height 커서(preedit 줄) 높이 (넘침 시 위로 올릴 때 사용)
 * @param callback 선택 콜백
 * @param user_data 콜백 사용자 데이터
 */
void unim_hanja_popup_show(UnimHanjaPopup *popup,
                            const gchar *target,
                            UnimHanjaCandidate *candidates,
                            gsize count,
                            gint x,
                            gint y,
                            gint cursor_height,
                            UnimHanjaSelectCallback callback,
                            gpointer user_data);

/**
 * 한자 팝업 숨기기
 *
 * @param popup 팝업
 */
void unim_hanja_popup_hide(UnimHanjaPopup *popup);

/**
 * 한자 팝업이 표시 중인지 확인
 *
 * @param popup 팝업
 * @return 표시 중이면 TRUE
 */
gboolean unim_hanja_popup_is_visible(UnimHanjaPopup *popup);

/**
 * 키 이벤트 처리 (숫자 선택, 화살표 네비게이션 등)
 *
 * @param popup 팝업
 * @param keyval GDK keyval
 * @return 키가 소비되었으면 TRUE
 */
gboolean unim_hanja_popup_handle_key(UnimHanjaPopup *popup, guint keyval);

/**
 * 한자 즐겨찾기 토글 콜백
 *
 * 팝업이 Space 키로 ToggleBookmark 결과를 받으면 호출된다.
 * 프런트엔드는 보통 unim_dbus_toggle_hanja_bookmark()를 엮어둔다.
 *
 * @param global_index 토글할 후보의 전체 인덱스
 * @param user_data 사용자 데이터
 */
typedef void (*UnimHanjaToggleBookmarkCallback)(gsize global_index,
                                                 gpointer user_data);

/**
 * 즐겨찾기 토글 콜백 설정 (Space → 엔진 DBus 호출 연결용)
 */
void unim_hanja_popup_set_toggle_bookmark_callback(UnimHanjaPopup *popup,
                                                    UnimHanjaToggleBookmarkCallback callback,
                                                    gpointer user_data);

/**
 * 전체 즐겨찾기 상태를 일괄 반영 (초기 fetch 결과용)
 *
 * @param popup 팝업
 * @param states 상태 배열 (candidates와 동일 길이여야 함)
 * @param count 상태 배열 길이
 */
void unim_hanja_popup_set_bookmark_states(UnimHanjaPopup *popup,
                                           const gboolean *states,
                                           gsize count);

/**
 * 특정 후보의 즐겨찾기 상태 갱신 (HanjaBookmarkChanged 시그널에서 호출)
 *
 * @param popup 팝업
 * @param global_index 전체 후보 인덱스
 * @param bookmarked 새 상태
 */
void unim_hanja_popup_set_bookmark(UnimHanjaPopup *popup,
                                    gsize global_index,
                                    gboolean bookmarked);

/**
 * 한자 후보를 즐겨찾기 정렬 결과로 일괄 교체하고 커서를 점프시킨다.
 * (HanjaCandidatesReordered 시그널)
 *
 * candidates는 (hanja, meaning) 페어를 가리키는 UnimHanjaCandidate 배열.
 * 호출자(im 모듈)는 이전 candidates 메모리를 해제할 책임이 있고, 본 함수는
 * 내부적으로 새 candidates 포인터를 그대로 보관한다 (소유권 이관).
 */
void unim_hanja_popup_replace_candidates(UnimHanjaPopup *popup,
                                          UnimHanjaCandidate *candidates,
                                          gsize count,
                                          const gboolean *bookmarks,
                                          gsize bookmarks_count,
                                          gint page,
                                          gint sel_row,
                                          gint sel_col);

G_END_DECLS

#endif /* UNIM_HANJA_POPUP_H */
