/**
 * UNIM GTK4 Input Method Module
 *
 * GTK4 애플리케이션에서 한글 입력을 제공하는 IM 모듈입니다.
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#include <gtk/gtk.h>
#include <gtk/gtkimmodule.h>
#include <gdk/gdk.h>
#include <gdk/gdkevents.h>
#include <gio/gio.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

/* DBus 클라이언트 및 한자 팝업 헤더 */
#include "unim_dbus_client.h"
#include "unim_hanja_popup.h"

/* 모듈 정보 */
#define UNIM_IM_CONTEXT_ID "unim"
#define UNIM_IM_CONTEXT_NAME "UNIM 한글 입력기"

#ifndef GTK_IM_MODULE_EXTENSION_POINT_NAME
#define GTK_IM_MODULE_EXTENSION_POINT_NAME "gtk-im-module"
#endif

/* 타입 정의 */
#define UNIM_TYPE_IM_CONTEXT (unim_im_context_get_type())
G_DECLARE_FINAL_TYPE(UnimIMContext, unim_im_context, UNIM, IM_CONTEXT, GtkIMContext)

struct _UnimIMContext {
    GtkIMContext parent;
    UnimDbusContext *dbus_ctx;  /* DBus 클라이언트 컨텍스트 */
    gboolean is_focused;
    gchar *window_id;           /* 창 식별자 */
    
    /* 주변 텍스트 정보 캐시 */
    gchar *surrounding_text;
    gint cursor_index;    /* 바이트 오프셋 */
    gint selection_index; /* 바이트 오프셋 */
    
    /* 한자 변환 관련 */
    UnimHanjaPopup *hanja_popup;       /* 한자 후보 팝업 */
    UnimHanjaCandidate *hanja_candidates; /* 현재 후보 목록 */
    gsize hanja_count;                  /* 후보 개수 */
    GdkRectangle cursor_area;           /* 커서 위치 */
};

G_DEFINE_DYNAMIC_TYPE(UnimIMContext, unim_im_context, GTK_TYPE_IM_CONTEXT)

/* 함수 선언 */
static gboolean unim_im_context_filter_keypress(GtkIMContext *context, GdkEvent *event);
static void unim_im_context_focus_in(GtkIMContext *context);
static void unim_im_context_focus_out(GtkIMContext *context);
static void unim_im_context_reset(GtkIMContext *context);
static void unim_im_context_get_preedit_string(GtkIMContext *context, char **str,
                                                PangoAttrList **attrs, int *cursor_pos);
static void unim_im_context_set_cursor_location(GtkIMContext *context, GdkRectangle *area);
static void unim_im_context_set_surrounding(GtkIMContext *context, const char *text,
                                             int len, int cursor_index);
static void unim_im_context_set_surrounding_with_selection(GtkIMContext *context, const char *text,
                                                            int len, int cursor_index, int selection_index);

/* 디버그 로깅 시스템 */
static gboolean unim_debug_enabled = FALSE;
static gboolean unim_debug_checked = FALSE;

#include <stdio.h>
#include <stdarg.h>
#include <time.h>

/* 중앙 로깅 함수 - 콘솔과 파일에 동시 출력 */
static void
unim_log_message(const char *module, const char *format, ...)
{
    if (!unim_debug_enabled) return;

    va_list args;
    char message[1024];
    char timestamp[32];
    char log_line[2048];
    time_t now;
    struct tm *tm_info;

    /* 메시지 포맷팅 */
    va_start(args, format);
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);

    /* 타임스탬프 생성 */
    time(&now);
    tm_info = localtime(&now);
    strftime(timestamp, sizeof(timestamp), "%Y/%m/%d %H:%M:%S", tm_info);

    /* 로그 라인 생성 */
    snprintf(log_line, sizeof(log_line), "[%s] - [%s] - %s", timestamp, module, message);

    /* 콘솔 출력 */
    g_print("%s\n", log_line);

    /* 파일 출력 */
    const gchar *home = g_get_home_dir();
    if (home) {
        gchar *log_path = g_build_filename(home, ".unim-errors.log", NULL);
        FILE *f = fopen(log_path, "a");
        if (f) {
            fprintf(f, "%s\n", log_line);
            fclose(f);
        }
        g_free(log_path);
    }
}

#define UNIM_DEBUG(fmt, ...) \
    unim_log_message("GTK4_IM", fmt, ##__VA_ARGS__)

static void
unim_check_debug_env(void)
{
    if (!unim_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            unim_debug_enabled = TRUE;
            unim_log_message("GTK4_IM", "디버그 모드 활성화 (UNIM_DEVELOP=1)");
        }
        unim_debug_checked = TRUE;
    }
}


static void
unim_im_context_class_init(UnimIMContextClass *klass)
{
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    GtkIMContextClass *im_class = GTK_IM_CONTEXT_CLASS(klass);

    im_class->filter_keypress = (gboolean (*)(GtkIMContext *, GdkEvent *))unim_im_context_filter_keypress;
    im_class->focus_in = unim_im_context_focus_in;
    im_class->focus_out = unim_im_context_focus_out;
    im_class->reset = unim_im_context_reset;
    im_class->get_preedit_string = unim_im_context_get_preedit_string;
    im_class->set_cursor_location = unim_im_context_set_cursor_location;
    im_class->set_surrounding = unim_im_context_set_surrounding;
    im_class->set_surrounding_with_selection = unim_im_context_set_surrounding_with_selection;
}

static void
unim_im_context_class_finalize(UnimIMContextClass *klass)
{
}

static void
unim_im_context_init(UnimIMContext *context)
{
    unim_check_debug_env();
    
    /* 창 식별자 생성 (컨텍스트 포인터 기반) */
    context->window_id = g_strdup_printf("gtk4-ctx-%p", (void*)context);
    
    /* DBus 클라이언트 생성 (window_id 포함) */
    context->dbus_ctx = unim_dbus_context_new("gtk4-unim", context->window_id);
    context->is_focused = FALSE;
    context->surrounding_text = NULL;
    context->cursor_index = 0;
    context->selection_index = 0;
    
    /* 한자 팝업 초기화 */
    context->hanja_popup = unim_hanja_popup_new();
    context->hanja_candidates = NULL;
    context->hanja_count = 0;
    memset(&context->cursor_area, 0, sizeof(GdkRectangle));
    
    if (context->dbus_ctx) {
        UNIM_DEBUG("IMContext 초기화 완료 (window_id: %s)", context->window_id);
    } else {
        UNIM_DEBUG("IMContext 초기화 (DBus 연결 실패)");
    }
}

static void
unim_im_context_dispose(GObject *obj)
{
    UnimIMContext *context = UNIM_IM_CONTEXT(obj);

    /* 한자 팝업 해제 */
    if (context->hanja_popup) {
        unim_hanja_popup_free(context->hanja_popup);
        context->hanja_popup = NULL;
    }
    
    if (context->hanja_candidates) {
        unim_hanja_candidates_free(context->hanja_candidates, context->hanja_count);
        context->hanja_candidates = NULL;
        context->hanja_count = 0;
    }

    if (context->dbus_ctx) {
        unim_dbus_context_free(context->dbus_ctx);
        context->dbus_ctx = NULL;
    }

    g_free(context->window_id);
    context->window_id = NULL;
    
    g_free(context->surrounding_text);
    context->surrounding_text = NULL;

    G_OBJECT_CLASS(unim_im_context_parent_class)->dispose(obj);
}

/* 한자 선택 콜백 - 팝업에서 한자 선택 시 호출됨 */
static void
on_hanja_selected(const gchar *hanja, gpointer user_data)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(user_data);
    
    if (!unim || !hanja || !unim->dbus_ctx) {
        return;
    }
    
    UNIM_DEBUG("한자 선택 콜백: hanja='%s'", hanja);
    
    /* 팝업 숨기기 (먼저) */
    if (unim->hanja_popup) {
        unim_hanja_popup_hide(unim->hanja_popup);
    }
    
    /* 
     * preedit을 클리어하고 한자만 커밋
     * - 먼저 preedit을 빈 상태로 변경
     * - 그 다음 한자를 커밋
     */
    
    /* preedit 클리어 먼저 (엔진에서 preedit 제거) */
    unim_dbus_cancel_hanja(unim->dbus_ctx);
    
    /* preedit-changed 시그널 (빈 preedit) */
    g_signal_emit_by_name(unim, "preedit-changed");
    
    /* 한자만 커밋 */
    g_signal_emit_by_name(unim, "commit", hanja);
}

static gboolean
unim_im_context_filter_keypress(GtkIMContext *context, GdkEvent *event)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (!unim->dbus_ctx) {
        UNIM_DEBUG("DBus 컨텍스트 없음, 키 무시");
        return FALSE;
    }

    /* 키 이벤트 타입 확인 (GDK4 방식) */
    GdkEventType event_type = gdk_event_get_event_type(event);
    if (event_type != GDK_KEY_PRESS && event_type != GDK_KEY_RELEASE) {
        return FALSE;
    }

    /* Release 이벤트는 일단 무시 */
    if (event_type == GDK_KEY_RELEASE) {
        return FALSE;
    }

    /* 키 정보 추출 (GDK 4.4+ 접근자) */
#if GTK_CHECK_VERSION(4, 4, 0)
    guint keyval = gdk_key_event_get_keyval(event);
    guint keycode = gdk_key_event_get_keycode(event);
    GdkModifierType state = gdk_event_get_modifier_state(event);
#else
    guint keyval = 0;
    guint keycode = 0;
    GdkModifierType state = 0;
    g_warning("GTK version too old for key event accessors");
#endif

    /* 수정자 키만 눌린 경우 바이패스 (preedit에 영향 없이 앱으로 전달) */
    if (keyval == GDK_KEY_Shift_L || keyval == GDK_KEY_Shift_R ||
        keyval == GDK_KEY_Control_L || keyval == GDK_KEY_Control_R ||
        keyval == GDK_KEY_Alt_L || keyval == GDK_KEY_Alt_R ||
        keyval == GDK_KEY_Super_L || keyval == GDK_KEY_Super_R ||
        keyval == GDK_KEY_Meta_L || keyval == GDK_KEY_Meta_R ||
        keyval == GDK_KEY_ISO_Level3_Shift) {
        return FALSE;
    }

    /* 한자 팝업이 표시 중이면 팝업에서 키 처리 */
    if (unim->hanja_popup && unim_hanja_popup_is_visible(unim->hanja_popup)) {
        if (unim_hanja_popup_handle_key(unim->hanja_popup, keyval)) {
            return TRUE;
        }
        /* Escape로 팝업 닫기 처리됨 */
    }

    /* 한자 키 처리 (Hangul_Hanja 또는 F9) */
    if (keyval == GDK_KEY_Hangul_Hanja || keyval == GDK_KEY_F9) {
        gchar *target = NULL;
        UnimHanjaCandidate *candidates = NULL;
        gsize count = 0;
        
        if (unim_dbus_get_hanja_candidates(unim->dbus_ctx, &target, &candidates, &count)) {
            if (count > 0 && unim->hanja_popup) {
                /* 이전 후보 정리 */
                if (unim->hanja_candidates) {
                    unim_hanja_candidates_free(unim->hanja_candidates, unim->hanja_count);
                }
                unim->hanja_candidates = candidates;
                unim->hanja_count = count;
                
                /* 팝업 표시 (커서 위치 기반) */
                unim_hanja_popup_show(
                    unim->hanja_popup,
                    target,
                    candidates,
                    count,
                    unim->cursor_area.x,
                    unim->cursor_area.y + unim->cursor_area.height,
                    on_hanja_selected,
                    unim
                );
                
                UNIM_DEBUG("한자 후보 표시: target='%s', count=%zu", target, count);
            } else {
                /* 후보 없음 */
                UNIM_DEBUG("한자 후보 없음");
                if (candidates) {
                    unim_hanja_candidates_free(candidates, count);
                }
            }
        }
        g_free(target);
        return TRUE;
    }

    /* GDK keycode = X11 keycode = evdev + 8 */
    guint evdev_code = (keycode > 8) ? (keycode - 8) : 0;
    
    /* 디버그 로그 (바이패스 전) */
    UNIM_DEBUG("키 입력: keyval=%u, keycode=%u, evdev=%u, state=0x%x, composing=%d",
               keyval, keycode, evdev_code, (guint)state, unim_dbus_is_composing(unim->dbus_ctx));

    /* 조합 중이 아닌 경우, 특수키는 IM에서 처리하지 않고 앱으로 직접 전달 */
    /* (Ghostty 등 터미널에서 방향키, Backspace 등이 동작하도록 함) */
    if (!unim_dbus_is_composing(unim->dbus_ctx)) {
        /* 기능키 (F1~F12) */
        if (keyval >= GDK_KEY_F1 && keyval <= GDK_KEY_F12) {
            UNIM_DEBUG("바이패스: 기능키 (keyval=%u)", keyval);
            return FALSE;
        }
        /* 방향키 */
        if (keyval >= GDK_KEY_Left && keyval <= GDK_KEY_Down) {
            UNIM_DEBUG("바이패스: 방향키 (keyval=%u)", keyval);
            return FALSE;
        }
        /* 네비게이션 키 */
        if (keyval == GDK_KEY_Home || keyval == GDK_KEY_End ||
            keyval == GDK_KEY_Page_Up || keyval == GDK_KEY_Page_Down ||
            keyval == GDK_KEY_Insert || keyval == GDK_KEY_Delete) {
            UNIM_DEBUG("바이패스: 네비게이션 (keyval=%u)", keyval);
            return FALSE;
        }
        /* Enter, Tab */
        if (keyval == GDK_KEY_Return || keyval == GDK_KEY_KP_Enter || keyval == GDK_KEY_Tab) {
            UNIM_DEBUG("바이패스: Enter/Tab (keyval=%u)", keyval);
            return FALSE;
        }
        /* Escape */
        if (keyval == GDK_KEY_Escape) {
            UNIM_DEBUG("바이패스: Escape");
            return FALSE;
        }
        /* Backspace */
        if (keyval == GDK_KEY_BackSpace) {
            UNIM_DEBUG("바이패스: Backspace");
            return FALSE;
        }
    }

    /* 수정자 상태 변환 - DBus 호출용 비트필드 */
    guint mod_state = 0;
    if (state & GDK_SHIFT_MASK) mod_state |= (1 << 0);
    if (state & GDK_CONTROL_MASK) mod_state |= (1 << 2);
    if (state & GDK_ALT_MASK) mod_state |= (1 << 3);
    if (state & GDK_SUPER_MASK) mod_state |= (1 << 26);
    if (state & GDK_LOCK_MASK) mod_state |= (1 << 1);


    /* DBus를 통해 키 처리 */
    UnimDbusKeyResult result;
    if (!unim_dbus_process_key(unim->dbus_ctx, keyval, evdev_code, mod_state, &result)) {
        UNIM_DEBUG("DBus 키 처리 실패");
        return FALSE;
    }

    UNIM_DEBUG("엔진 결과: consumed=%d, preedit=\"%s\", commit=\"%s\"",
               result.consumed, result.preedit ? result.preedit : "(null)",
               result.commit ? result.commit : "(null)");

    /* 선택 영역 삭제 처리 */
    if (result.consumed) {
        /* 최신 주변 텍스트 획득 요청 */
        gboolean handled = FALSE;
        g_signal_emit_by_name(context, "retrieve-surrounding", &handled);

        if (handled && unim->surrounding_text && unim->cursor_index != unim->selection_index) {
            int start_index = MIN(unim->cursor_index, unim->selection_index);
            int end_index = MAX(unim->cursor_index, unim->selection_index);
            
            /* 바이트 오프셋을 문자 오프셋으로 변환 */
            int n_chars = g_utf8_strlen(unim->surrounding_text, -1);
            int start_char = g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                       unim->surrounding_text + start_index);
            int end_char = g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                     unim->surrounding_text + end_index);
            
            int offset = start_char - g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                              unim->surrounding_text + unim->cursor_index);
            int len = end_char - start_char;

            UNIM_DEBUG("선택 영역 삭제: offset=%d, len=%d", offset, len);
            gtk_im_context_delete_surrounding(context, offset, len);
            
            /* 삭제 후 캐시 무효화 (다음 조작 시 갱신됨) */
            unim->cursor_index = unim->selection_index = start_index;
        }
    }

    /* 커밋 처리 */
    if (result.commit && strlen(result.commit) > 0) {
        UNIM_DEBUG("커밋: \"%s\"", result.commit);
        g_signal_emit_by_name(context, "commit", result.commit);
    }

    /* preedit 변경 처리 */
    g_signal_emit_by_name(context, "preedit-changed");

    /* 메모리 해제 */
    g_free(result.preedit);
    g_free(result.commit);

    return result.consumed;
}

static void
unim_im_context_focus_in(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    
    UNIM_DEBUG("focus_in 호출 (window_id: %s)", unim->window_id);
    
    if (unim->dbus_ctx) {
        unim_dbus_focus_in(unim->dbus_ctx, unim->window_id);
    }
    
    unim->is_focused = TRUE;
}

static void
unim_im_context_focus_out(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    gchar *commit = NULL;

    UNIM_DEBUG("focus_out 호출");

    /* 한자 팝업이 표시 중이면 아무것도 하지 않음 (팝업 유지) */
    if (unim->hanja_popup && unim_hanja_popup_is_visible(unim->hanja_popup)) {
        UNIM_DEBUG("focus_out: 한자 팝업 표시 중 - 무시");
        return;  /* 팝업 숨기지 않고 그냥 무시 */
    }

    if (unim->dbus_ctx) {
        unim_dbus_focus_out(unim->dbus_ctx, &commit);
        
        /* 조합 중이던 문자 커밋 */
        if (commit && strlen(commit) > 0) {
            UNIM_DEBUG("focus_out 커밋: \"%s\"", commit);
            g_signal_emit_by_name(context, "commit", commit);
        }
        g_free(commit);
        
        /* preedit 업데이트 */
        g_signal_emit_by_name(context, "preedit-changed");
    }

    unim->is_focused = FALSE;
}

static void
unim_im_context_reset(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    gchar *commit = NULL;

    UNIM_DEBUG("reset 호출");

    if (unim->dbus_ctx) {
        unim_dbus_reset(unim->dbus_ctx, &commit);
        
        /* 조합 중이던 문자 커밋 */
        if (commit && strlen(commit) > 0) {
            UNIM_DEBUG("reset 커밋: \"%s\"", commit);
            g_signal_emit_by_name(context, "commit", commit);
        }
        g_free(commit);
        
        /* preedit 업데이트 */
        g_signal_emit_by_name(context, "preedit-changed");
    }
}

static void
unim_im_context_get_preedit_string(GtkIMContext *context, char **str,
                                    PangoAttrList **attrs, int *cursor_pos)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (unim->dbus_ctx) {
        *str = unim_dbus_get_preedit(unim->dbus_ctx);
    } else {
        *str = g_strdup("");
    }

    if (attrs) {
        *attrs = pango_attr_list_new();
        if (strlen(*str) > 0) {
            PangoAttribute *attr = pango_attr_underline_new(PANGO_UNDERLINE_SINGLE);
            attr->start_index = 0;
            attr->end_index = strlen(*str);
            pango_attr_list_insert(*attrs, attr);
        }
    }

    if (cursor_pos) {
        *cursor_pos = g_utf8_strlen(*str, -1);
    }
}

static void
unim_im_context_set_cursor_location(GtkIMContext *context, GdkRectangle *area)
{
    /* 커서 위치 저장 (현재 사용하지 않음) */
}

static void
unim_im_context_set_surrounding(GtkIMContext *context, const char *text,
                                 int len, int cursor_index)
{
    unim_im_context_set_surrounding_with_selection(context, text, len, cursor_index, cursor_index);
}

static void
unim_im_context_set_surrounding_with_selection(GtkIMContext *context, const char *text,
                                                int len, int cursor_index, int selection_index)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    g_free(unim->surrounding_text);
    if (len < 0) {
        unim->surrounding_text = g_strdup(text);
    } else {
        unim->surrounding_text = g_strndup(text, len);
    }
    unim->cursor_index = cursor_index;
    unim->selection_index = selection_index;

    UNIM_DEBUG("surrounding 업데이트: cursor=%d, selection=%d, text=\"%s\"",
               cursor_index, selection_index, unim->surrounding_text);
}

/* GTK4 IM 모듈 엔트리 포인트 (GIO 모듈로 등록) */

G_MODULE_EXPORT void
g_io_module_load(GIOModule *module)
{
    unim_im_context_register_type(G_TYPE_MODULE(module));

    g_io_extension_point_implement(
        GTK_IM_MODULE_EXTENSION_POINT_NAME,
        UNIM_TYPE_IM_CONTEXT,
        UNIM_IM_CONTEXT_ID,
        10  /* priority */
    );
}

G_MODULE_EXPORT void
g_io_module_unload(GIOModule *module)
{
    /* 언로드 시 정리 */
}

G_MODULE_EXPORT char **
g_io_module_query(void)
{
    char *eps[] = {
        (char *)GTK_IM_MODULE_EXTENSION_POINT_NAME,
        NULL
    };
    return g_strdupv(eps);
}
