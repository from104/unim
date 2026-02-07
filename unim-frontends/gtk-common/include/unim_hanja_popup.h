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
 * @param y 팝업 Y 위치
 * @param callback 선택 콜백
 * @param user_data 콜백 사용자 데이터
 */
void unim_hanja_popup_show(UnimHanjaPopup *popup,
                            const gchar *target,
                            UnimHanjaCandidate *candidates,
                            gsize count,
                            gint x,
                            gint y,
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

G_END_DECLS

#endif /* UNIM_HANJA_POPUP_H */
