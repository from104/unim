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

/* 디버그 로깅 시스템 */
static gboolean unim_debug_enabled = FALSE;

#define UNIM_DEBUG(fmt, ...) \
    do { \
        if (unim_debug_enabled) { \
            g_print("[UNIM-GTK3] " fmt "\n", ##__VA_ARGS__); \
        } \
    } while (0)

static void
unim_check_debug_env(void)
{
    static gboolean checked = FALSE;
    if (!checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            unim_debug_enabled = TRUE;
            g_print("[UNIM-GTK3] 디버그 모드 활성화 (UNIM_DEVELOP=1)\n");
        }
        checked = TRUE;
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
    
    /* DBus 클라이언트 생성 */
    context->dbus_ctx = unim_dbus_context_new("gtk3-unim");
    context->is_focused = FALSE;
    context->client_window = NULL;
    
    if (context->dbus_ctx) {
        UNIM_DEBUG("IMContext 초기화 완료 (DBus 연결됨)");
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
    
    UNIM_DEBUG("focus_in 호출");
    
    if (unim->dbus_ctx) {
        unim_dbus_focus_in(unim->dbus_ctx);
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
