/**
 * UNIM Special Character Popup Header
 *
 * 초성 기반 특수문자 선택을 위한 9x9 그리드 팝업 윈도우입니다.
 * 한자 팝업과 유사하지만 그리드 레이아웃을 사용합니다.
 */

#ifndef UNIM_SPECIAL_POPUP_H
#define UNIM_SPECIAL_POPUP_H

#include <gtk/gtk.h>

G_BEGIN_DECLS

typedef struct _UnimSpecialPopup UnimSpecialPopup;

/**
 * 특수문자 선택 콜백
 *
 * @param character 선택된 특수문자 (UTF-8)
 * @param user_data 사용자 데이터
 */
typedef void (*UnimSpecialSelectCallback)(const gchar *character, gpointer user_data);

/**
 * 새 특수문자 팝업 생성
 */
UnimSpecialPopup* unim_special_popup_new(void);

/**
 * 특수문자 팝업 해제
 */
void unim_special_popup_free(UnimSpecialPopup *popup);

/**
 * 특수문자 팝업 표시
 *
 * @param popup 팝업 인스턴스
 * @param target 변환 대상 초성 (예: "ㄱ")
 * @param characters 특수문자 배열 (UTF-8 문자열 배열)
 * @param count 문자 개수
 * @param top_row 영문 키맵의 상단 행 레이블 (예: "QWERTYUIO")
 * @param x 팝업 X 좌표 (화면 절대)
 * @param y 팝업 Y 좌표 (화면 절대)
 * @param cursor_height 커서 높이
 * @param callback 선택 콜백
 * @param user_data 콜백 사용자 데이터
 */
void unim_special_popup_show(UnimSpecialPopup *popup,
                              const gchar *target,
                              gchar **characters,
                              gsize count,
                              const gchar *top_row,
                              gint x, gint y, gint cursor_height,
                              UnimSpecialSelectCallback callback,
                              gpointer user_data);

/**
 * 특수문자 팝업 숨기기
 */
void unim_special_popup_hide(UnimSpecialPopup *popup);

/**
 * 특수문자 팝업 표시 여부
 */
gboolean unim_special_popup_is_visible(UnimSpecialPopup *popup);

/**
 * 팝업에서 키 이벤트 처리
 *
 * @return TRUE면 키를 소비함
 */
gboolean unim_special_popup_handle_key(UnimSpecialPopup *popup, guint keyval);

/**
 * 페이지 이동 콜백 (◀/▶ 풋터 버튼 클릭용).
 *
 * @param direction 0=Prev, 1=Next
 */
typedef void (*UnimSpecialPageChangeCallback)(gint direction, gpointer user_data);

/**
 * 페이지 이동 콜백 설정 (◀/▶ → unim_dbus_popup_change_page 등 위임).
 */
void unim_special_popup_set_page_change_callback(UnimSpecialPopup *popup,
                                                  UnimSpecialPageChangeCallback callback,
                                                  gpointer user_data);

G_END_DECLS

#endif /* UNIM_SPECIAL_POPUP_H */
