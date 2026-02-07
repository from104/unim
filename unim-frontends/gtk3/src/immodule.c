/**
 * UNIM GTK3 Input Method Module
 *
 * GTK3 애플리케이션에서 한글 입력을 제공하는 IM 모듈입니다.
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#include <gtk/gtk.h>
#include <gtk/gtkimmodule.h>
#include <gdk/gdkkeysyms.h>
#include <gio/gio.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

/* DBus 클라이언트 헤더 */
#include "unim_dbus_client.h"

/* 모듈 정보 */
#define UNIM_IM_CONTEXT_ID "unim"
#define UNIM_IM_CONTEXT_NAME "UNIM 한글 입력기"

/* 타입 정의 */
#define UNIM_TYPE_IM_CONTEXT (unim_im_context_get_type())
#define UNIM_IM_CONTEXT(obj) \
    (G_TYPE_CHECK_INSTANCE_CAST((obj), UNIM_TYPE_IM_CONTEXT, UnimIMContext))
#define UNIM_IM_CONTEXT_CLASS(klass) \
    (G_TYPE_CHECK_CLASS_CAST((klass), UNIM_TYPE_IM_CONTEXT, UnimIMContextClass))
#define UNIM_IS_IM_CONTEXT(obj) \
    (G_TYPE_CHECK_INSTANCE_TYPE((obj), UNIM_TYPE_IM_CONTEXT))

typedef struct _UnimIMContext UnimIMContext;
typedef struct _UnimIMContextClass UnimIMContextClass;

struct _UnimIMContext {
    GtkIMContext parent;
    UnimDbusContext *dbus_ctx;  /* DBus 클라이언트 컨텍스트 */
    gboolean is_focused;
    GdkWindow *client_window;
    gchar *window_id;           /* 창 식별자 */

    /* 주변 텍스트 정보 캐시 */
    gchar *surrounding_text;
    gint cursor_index;    /* 바이트 오프셋 */
    gint selection_index; /* 바이트 오프셋 */
};

struct _UnimIMContextClass {
    GtkIMContextClass parent_class;
};

/* 함수 선언 */
static void unim_im_context_class_init(UnimIMContextClass *klass);
static void unim_im_context_init(UnimIMContext *context);
static void unim_im_context_finalize(GObject *obj);

static gboolean unim_im_context_filter_keypress(GtkIMContext *context, GdkEventKey *event);
static void unim_im_context_focus_in(GtkIMContext *context);
static void unim_im_context_focus_out(GtkIMContext *context);
static void unim_im_context_reset(GtkIMContext *context);
static void unim_im_context_set_client_window(GtkIMContext *context, GdkWindow *window);
static void unim_im_context_get_preedit_string(GtkIMContext *context, gchar **str,
                                                PangoAttrList **attrs, gint *cursor_pos);
static void unim_im_context_set_cursor_location(GtkIMContext *context, GdkRectangle *area);
static void unim_im_context_set_surrounding(GtkIMContext *context, const gchar *text,
                                             gint len, gint cursor_index);

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
    unim_log_message("GTK3_IM", fmt, ##__VA_ARGS__)

static void
unim_check_debug_env(void)
{
    if (!unim_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            unim_debug_enabled = TRUE;
            unim_log_message("GTK3_IM", "디버그 모드 활성화 (UNIM_DEVELOP=1)");
        }
        unim_debug_checked = TRUE;
    }
}


/* GType 등록 */
G_DEFINE_DYNAMIC_TYPE(UnimIMContext, unim_im_context, GTK_TYPE_IM_CONTEXT)

static void
unim_im_context_class_init(UnimIMContextClass *klass)
{
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    GtkIMContextClass *im_class = GTK_IM_CONTEXT_CLASS(klass);

    object_class->finalize = unim_im_context_finalize;

    im_class->filter_keypress = unim_im_context_filter_keypress;
    im_class->focus_in = unim_im_context_focus_in;
    im_class->focus_out = unim_im_context_focus_out;
    im_class->reset = unim_im_context_reset;
    im_class->set_client_window = unim_im_context_set_client_window;
    im_class->get_preedit_string = unim_im_context_get_preedit_string;
    im_class->set_cursor_location = unim_im_context_set_cursor_location;
    im_class->set_surrounding = unim_im_context_set_surrounding;
}

static void
unim_im_context_class_finalize(UnimIMContextClass *klass)
{
    /* 정리 작업 */
}

static void
unim_im_context_init(UnimIMContext *context)
{
    unim_check_debug_env();
    
    /* 창 식별자 생성 (컨텍스트 포인터 기반) */
    context->window_id = g_strdup_printf("gtk3-ctx-%p", (void*)context);
    
    /* DBus 클라이언트 생성 (window_id 포함) */
    context->dbus_ctx = unim_dbus_context_new("gtk3-unim", context->window_id);
    context->is_focused = FALSE;
    context->client_window = NULL;
    context->surrounding_text = NULL;
    context->cursor_index = 0;
    context->selection_index = 0;
    
    if (context->dbus_ctx) {
        UNIM_DEBUG("IMContext 초기화 완료 (window_id: %s)", context->window_id);
    } else {
        UNIM_DEBUG("IMContext 초기화 (DBus 연결 실패)");
    }
}

static void
unim_im_context_finalize(GObject *obj)
{
    UnimIMContext *context = UNIM_IM_CONTEXT(obj);

    if (context->dbus_ctx) {
        unim_dbus_context_free(context->dbus_ctx);
        context->dbus_ctx = NULL;
    }

    g_free(context->window_id);
    context->window_id = NULL;
    
    g_free(context->surrounding_text);
    context->surrounding_text = NULL;

    G_OBJECT_CLASS(unim_im_context_parent_class)->finalize(obj);
}

static gboolean
unim_im_context_filter_keypress(GtkIMContext *context, GdkEventKey *event)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (!unim->dbus_ctx) {
        UNIM_DEBUG("DBus 컨텍스트 없음, 키 무시");
        return FALSE;
    }

    /* 키 릴리스는 무시 */
    if (event->type != GDK_KEY_PRESS) {
        return FALSE;
    }

    /* 수정자 키만 눌린 경우 바이패스 (preedit에 영향 없이 앱으로 전달) */
    if (event->keyval == GDK_KEY_Shift_L || event->keyval == GDK_KEY_Shift_R ||
        event->keyval == GDK_KEY_Control_L || event->keyval == GDK_KEY_Control_R ||
        event->keyval == GDK_KEY_Alt_L || event->keyval == GDK_KEY_Alt_R ||
        event->keyval == GDK_KEY_Super_L || event->keyval == GDK_KEY_Super_R ||
        event->keyval == GDK_KEY_Meta_L || event->keyval == GDK_KEY_Meta_R ||
        event->keyval == GDK_KEY_ISO_Level3_Shift) {
        return FALSE;
    }

    /* 조합 중이 아닌 경우, 특수키는 IM에서 처리하지 않고 앱으로 직접 전달 */
    /* (터미널에서 방향키, Backspace 등이 동작하도록 함) */
    if (!unim_dbus_is_composing(unim->dbus_ctx)) {
        /* 기능키 (F1~F12) */
        if (event->keyval >= GDK_KEY_F1 && event->keyval <= GDK_KEY_F12) {
            return FALSE;
        }
        /* 방향키 */
        if (event->keyval >= GDK_KEY_Left && event->keyval <= GDK_KEY_Down) {
            return FALSE;
        }
        /* 네비게이션 키 */
        if (event->keyval == GDK_KEY_Home || event->keyval == GDK_KEY_End ||
            event->keyval == GDK_KEY_Page_Up || event->keyval == GDK_KEY_Page_Down ||
            event->keyval == GDK_KEY_Insert || event->keyval == GDK_KEY_Delete) {
            return FALSE;
        }
        /* Escape (조합 중이 아니면 앱으로) */
        if (event->keyval == GDK_KEY_Escape) {
            return FALSE;
        }
    }

    /* 수정자 상태 변환 - DBus 호출용 비트필드 */
    guint mod_state = 0;
    if (event->state & GDK_SHIFT_MASK) mod_state |= (1 << 0);
    if (event->state & GDK_CONTROL_MASK) mod_state |= (1 << 2);
    if (event->state & GDK_MOD1_MASK) mod_state |= (1 << 3);  /* Alt */
    if (event->state & GDK_SUPER_MASK) mod_state |= (1 << 26);
    if (event->state & GDK_LOCK_MASK) mod_state |= (1 << 1);  /* CapsLock */

    /* GDK hardware_keycode = X11 keycode = evdev + 8 */
    guint evdev_code = (event->hardware_keycode > 8) ? (event->hardware_keycode - 8) : 0;
    
    UNIM_DEBUG("키 입력: keyval=%u, keycode=%u, evdev=%u, state=0x%x",
               event->keyval, event->hardware_keycode, evdev_code, mod_state);

    /* DBus를 통해 키 처리 */
    UnimDbusKeyResult result;
    if (!unim_dbus_process_key(unim->dbus_ctx, event->keyval, evdev_code, mod_state, &result)) {
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
            gint start_index = MIN(unim->cursor_index, unim->selection_index);
            gint end_index = MAX(unim->cursor_index, unim->selection_index);
            
            /* 바이트 오프셋을 문자 오프셋으로 변환 */
            gint start_char = g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                       unim->surrounding_text + start_index);
            gint end_char = g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                     unim->surrounding_text + end_index);
            
            gint offset = start_char - g_utf8_pointer_to_offset(unim->surrounding_text, 
                                                              unim->surrounding_text + unim->cursor_index);
            gint len = end_char - start_char;

            UNIM_DEBUG("선택 영역 삭제: offset=%d, len=%d", offset, len);
            g_signal_emit_by_name(context, "delete-surrounding", offset, len);
            
            /* 삭제 후 캐시 무효화 */
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
unim_im_context_set_client_window(GtkIMContext *context, GdkWindow *window)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    unim->client_window = window;
}

static void
unim_im_context_get_preedit_string(GtkIMContext *context, gchar **str,
                                    PangoAttrList **attrs, gint *cursor_pos)
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
    /* 커서 위치 저장 (팝업 후보창 등에 사용) */
}

static void
unim_im_context_set_surrounding(GtkIMContext *context, const gchar *text,
                                 gint len, gint cursor_index)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    g_free(unim->surrounding_text);
    if (len < 0) {
        unim->surrounding_text = g_strdup(text);
    } else {
        unim->surrounding_text = g_strndup(text, len);
    }
    unim->cursor_index = cursor_index;
    
    /* GTK3에서는 anchor 정보가 별도로 없으므로 cursor_index와 동일하게 설정 */
    /* 다만 retrieve-surrounding 신호 시 위젯이 정보를 업데이트해주길 기대함 */
    unim->selection_index = cursor_index;

    UNIM_DEBUG("surrounding 업데이트: cursor=%d, text=\"%s\"",
               cursor_index, unim->surrounding_text);
}

/* 모듈 정보 */
static const GtkIMContextInfo unim_info = {
    .context_id = UNIM_IM_CONTEXT_ID,
    .context_name = UNIM_IM_CONTEXT_NAME,
    .domain = "unim",
    .domain_dirname = "",
    .default_locales = "ko:*"
};

static const GtkIMContextInfo *info_list[] = {
    &unim_info
};

/* GTK3 IM 모듈 엔트리 포인트 */
G_MODULE_EXPORT void
im_module_init(GTypeModule *module)
{
    unim_im_context_register_type(module);
}

G_MODULE_EXPORT void
im_module_exit(void)
{
    /* 정리 작업 */
}

G_MODULE_EXPORT void
im_module_list(const GtkIMContextInfo ***contexts, int *n_contexts)
{
    *contexts = info_list;
    *n_contexts = G_N_ELEMENTS(info_list);
}

G_MODULE_EXPORT GtkIMContext *
im_module_create(const gchar *context_id)
{
    if (g_strcmp0(context_id, UNIM_IM_CONTEXT_ID) == 0) {
        return g_object_new(UNIM_TYPE_IM_CONTEXT, NULL);
    }
    return NULL;
}
