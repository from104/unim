/**
 * UNIM DBus Client for GTK IM Modules
 *
 * GDBus를 사용하여 unim-daemon과 통신하는 클라이언트 헬퍼입니다.
 * GTK3/GTK4 IM 모듈에서 공통으로 사용됩니다.
 */

#ifndef UNIM_DBUS_CLIENT_H
#define UNIM_DBUS_CLIENT_H

#include <glib.h>
#include <gio/gio.h>

G_BEGIN_DECLS

/* DBus 서비스 정보 */
#define UNIM_DBUS_SERVICE       "org.atit.unim.InputMethod"
#define UNIM_DBUS_PATH          "/org/atit/unim/InputMethod"
#define UNIM_DBUS_INTERFACE     "org.atit.unim.InputMethod"
#define UNIM_DBUS_IC_INTERFACE  "org.atit.unim.InputContext"

/* 타임아웃 (밀리초) */
#define UNIM_DBUS_TIMEOUT_MS    500

/**
 * DBus 클라이언트 컨텍스트
 */
typedef struct _UnimDbusContext UnimDbusContext;

/**
 * 키 처리 결과
 */
typedef struct {
    gboolean consumed;            /* 키가 소비되었는지 */
    gchar *preedit;               /* preedit 문자열 (NULL 또는 빈 문자열이면 없음) */
    gchar *commit;                /* commit 문자열 (NULL 또는 빈 문자열이면 없음) */
} UnimDbusKeyResult;

/**
 * 새 DBus 컨텍스트 생성
 * 
 * @param client_name 클라이언트 이름 (예: "gtk4-unim")
 * @param window_id 창 식별자 (XID 또는 고유 문자열, NULL이면 client_name 사용)
 * @return 새 컨텍스트 또는 실패 시 NULL
 */
UnimDbusContext* unim_dbus_context_new(const gchar *client_name, const gchar *window_id);

/**
 * DBus 컨텍스트 해제
 * 
 * @param ctx 해제할 컨텍스트
 */
void unim_dbus_context_free(UnimDbusContext *ctx);

/**
 * 키 이벤트 처리
 * 
 * @param ctx 컨텍스트
 * @param keyval GDK keyval (사용되지 않음, 향후 호환성용)
 * @param keycode 키코드 (evdev 형식)
 * @param state 수정자 상태 비트필드
 * @param result 결과를 저장할 포인터 (호출자가 preedit/commit을 g_free해야 함)
 * @return 성공 시 TRUE
 */
gboolean unim_dbus_process_key(UnimDbusContext *ctx,
                                guint keyval,
                                guint keycode,
                                guint state,
                                UnimDbusKeyResult *result);

/**
 * 포커스 획득 알림
 * 
 * @param ctx 컨텍스트
 * @param window_id 창 식별자 (NULL이면 빈 문자열 전달)
 */
void unim_dbus_focus_in(UnimDbusContext *ctx, const gchar *window_id);

/**
 * 포커스 상실 알림
 * 조합 중인 문자가 있으면 커밋 문자열을 반환합니다.
 * 
 * @param ctx 컨텍스트
 * @param commit 커밋 문자열을 저장할 포인터 (호출자가 g_free해야 함)
 */
void unim_dbus_focus_out(UnimDbusContext *ctx, gchar **commit);

/**
 * 입력 상태 초기화
 * 조합 중인 문자가 있으면 커밋 문자열을 반환합니다.
 * 
 * @param ctx 컨텍스트
 * @param commit 커밋 문자열을 저장할 포인터 (호출자가 g_free해야 함)
 */
void unim_dbus_reset(UnimDbusContext *ctx, gchar **commit);

/**
 * 현재 preedit 문자열 가져오기
 * 
 * @param ctx 컨텍스트
 * @return preedit 문자열 (호출자가 g_free해야 함)
 */
gchar* unim_dbus_get_preedit(UnimDbusContext *ctx);

/**
 * 조합 중인지 확인
 * 
 * @param ctx 컨텍스트
 * @return 조합 중이면 TRUE
 */
gboolean unim_dbus_is_composing(UnimDbusContext *ctx);

/**
 * 커서 위치 보고 (팝업 포지셔닝용)
 * 
 * @param ctx 컨텍스트
 * @param x 커서 X 좌표
 * @param y 커서 Y 좌표
 * @param width 커서 너비
 * @param height 커서 높이
 */
void unim_dbus_report_cursor_rect(UnimDbusContext *ctx,
                                   gint x, gint y,
                                   gint width, gint height);

/**
 * 입력 필드 목적 설정 (비밀번호/PIN 감지용)
 *
 * @param ctx 컨텍스트
 * @param purpose 목적 (0=Normal, 1=Password, 2=Pin, 3=Email, 4=Number, 5=Url, 6=Terminal)
 */
void unim_dbus_set_content_type(UnimDbusContext *ctx, guint purpose);

/**
 * Surrounding text 설정 (커서 주변 텍스트 전달)
 *
 * @param ctx 컨텍스트
 * @param text 텍스트
 * @param cursor_pos 커서 위치 (문자 단위)
 * @param anchor_pos 앵커 위치 (문자 단위)
 */
void unim_dbus_set_surrounding_text(UnimDbusContext *ctx,
                                     const gchar *text,
                                     guint cursor_pos,
                                     guint anchor_pos);

/* =========================================
 * 한자 변환 관련 함수
 * ========================================= */

/**
 * 한자 후보 항목
 */
typedef struct {
    gchar *hanja;    /* 한자 문자열 */
    gchar *meaning;  /* 뜻풀이 */
} UnimHanjaCandidate;

/**
 * 한자 후보 목록 조회
 * 
 * @param ctx 컨텍스트
 * @param target 변환 대상 문자열을 저장할 포인터 (호출자가 g_free해야 함)
 * @param candidates 후보 배열을 저장할 포인터 (호출자가 unim_hanja_candidates_free해야 함)
 * @param count 후보 개수를 저장할 포인터
 * @return 성공 시 TRUE
 */
gboolean unim_dbus_get_hanja_candidates(UnimDbusContext *ctx,
                                         gchar **target,
                                         UnimHanjaCandidate **candidates,
                                         gsize *count);

/**
 * 한자 선택
 * 
 * @param ctx 컨텍스트
 * @param index 선택할 한자의 인덱스 (0부터 시작)
 * @param selected_hanja 선택된 한자를 저장할 포인터 (호출자가 g_free해야 함)
 * @return 성공 시 TRUE
 */
gboolean unim_dbus_select_hanja(UnimDbusContext *ctx,
                                 guint index,
                                 gchar **selected_hanja);

/**
 * 한자 모드 취소
 *
 * @param ctx 컨텍스트
 * @return 커밋할 트리거 문자 (호출자가 g_free해야 함, 없으면 NULL)
 */
gchar *unim_dbus_cancel_hanja(UnimDbusContext *ctx);

/**
 * 한자 후보 배열 해제
 * 
 * @param candidates 후보 배열
 * @param count 후보 개수
 */
void unim_hanja_candidates_free(UnimHanjaCandidate *candidates, gsize count);

/* =========================================
 * 특수문자 변환 관련 함수
 * ========================================= */

/**
 * 특수문자 후보 목록 조회
 * 
 * @param ctx 컨텍스트
 * @param target 변환 대상 초성을 저장할 포인터 (호출자가 g_free해야 함)
 * @param characters 특수문자 배열을 저장할 포인터 (호출자가 unim_special_chars_free해야 함)
 * @param count 문자 개수를 저장할 포인터
 * @return 성공 시 TRUE
 */
gboolean unim_dbus_get_special_char_candidates(UnimDbusContext *ctx,
                                                gchar **target,
                                                gchar ***characters,
                                                gsize *count,
                                                gchar **top_row);

/**
 * 특수문자 선택
 * 
 * @param ctx 컨텍스트
 * @param index 선택할 특수문자의 인덱스 (0부터 시작)
 * @param selected_char 선택된 특수문자를 저장할 포인터 (호출자가 g_free해야 함)
 * @return 성공 시 TRUE
 */
gboolean unim_dbus_select_special_char(UnimDbusContext *ctx,
                                        guint index,
                                        gchar **selected_char);

/**
 * 특수문자 모드 취소
 *
 * @param ctx 컨텍스트
 * @return 커밋할 트리거 문자 (호출자가 g_free해야 함, 없으면 NULL)
 */
gchar *unim_dbus_cancel_special_char(UnimDbusContext *ctx);

/**
 * 특수문자 배열 해제
 * 
 * @param characters 문자 배열
 * @param count 문자 개수
 */
void unim_special_chars_free(gchar **characters, gsize count);

/* =========================================
 * AutoTypeFix 콜백
 * ========================================= */

/**
 * AutoTypeFix 콜백 타입
 * @param delete_chars 삭제할 글자 수
 * @param commit_text commit할 텍스트
 * @param preedit_text preedit으로 설정할 텍스트 (빈 문자열이면 없음)
 * @param user_data 사용자 데이터
 */
typedef void (*UnimAutoTypeFixCallback)(guint delete_chars,
                                         const gchar *commit_text,
                                         const gchar *preedit_text,
                                         gpointer user_data);

/**
 * AutoTypeFix 시그널 구독 및 콜백 설정
 *
 * @param ctx 컨텍스트
 * @param callback AutoTypeFix 콜백 함수
 * @param user_data 콜백에 전달할 사용자 데이터
 */
void unim_dbus_set_auto_typefix_callback(UnimDbusContext *ctx,
                                          UnimAutoTypeFixCallback callback,
                                          gpointer user_data);

/**
 * CommitText 시그널 콜백 타입
 * Standalone 팝업에서 마우스 클릭으로 한자/특수문자 선택 시 호출됨
 *
 * @param text 커밋할 텍스트
 * @param user_data 사용자 데이터
 */
typedef void (*UnimCommitTextCallback)(const gchar *text,
                                       gpointer user_data);

/**
 * CommitText 시그널 구독 및 콜백 설정
 *
 * @param ctx 컨텍스트
 * @param callback CommitText 콜백 함수
 * @param user_data 콜백에 전달할 사용자 데이터
 */
void unim_dbus_set_commit_text_callback(UnimDbusContext *ctx,
                                         UnimCommitTextCallback callback,
                                         gpointer user_data);

/**
 * preedit 캐시를 외부에서 설정 (AutoTypeFix용)
 * @param ctx 컨텍스트
 * @param preedit 새 preedit 텍스트
 */
void unim_dbus_set_preedit_cache(UnimDbusContext *ctx, const gchar *preedit);

/* =========================================
 * 설정 조회 관련 함수
 * ========================================= */

/**
 * 설정 값 조회
 *
 * @param ctx 컨텍스트
 * @param key 설정 키 (예: "toggle_keys", "hanja_keys")
 * @return 설정 값 문자열 (호출자가 g_free해야 함), 실패 시 NULL
 */
gchar* unim_dbus_get_config(UnimDbusContext *ctx, const gchar *key);

/**
 * KeyCode 이름에서 GDK keyval로 변환
 *
 * @param name KeyCode 이름 (예: "Hanja", "F9")
 * @return GDK keyval, 알 수 없으면 0
 */
guint unim_keycode_name_to_gdk_keyval(const gchar *name);

G_END_DECLS

#endif /* UNIM_DBUS_CLIENT_H */
