/**
 * UNIM GTK4 Input Method Module
 *
 * GTK4 애플리케이션에서 한글 입력을 제공하는 IM 모듈입니다.
 * GTK4는 GtkIMContext API가 GTK3와 유사하지만, 일부 이벤트 처리가 다릅니다.
 */

#include <gtk/gtk.h>
#include <gtk/gtkimmodule.h>
#include <gdk/gdk.h>
#include <gdk/gdkevents.h>
#include <gio/gio.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <unim.h>

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
    UnimEngine *engine;
    UnimConfig *config;
    gboolean is_focused;
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

/* Unim C API 추가 선언 (IDE 린트 지원용) */
bool unim_config_reload(UnimConfig *config);
UnimHangulLayout unim_config_get_hangul_layout(const UnimConfig *config);
UnimLatinLayout unim_config_get_latin_layout(const UnimConfig *config);
void unim_engine_set_hangul_layout(UnimEngine *engine, UnimHangulLayout layout);
void unim_engine_set_latin_layout(UnimEngine *engine, UnimLatinLayout layout);

static void
unim_im_context_class_init(UnimIMContextClass *klass)
{
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    GtkIMContextClass *im_class = GTK_IM_CONTEXT_CLASS(klass);

    /* GTK4에서는 filter_keypress가 GdkEvent*를 받음 */
    im_class->filter_keypress = unim_im_context_filter_keypress;
    im_class->focus_in = unim_im_context_focus_in;
    im_class->focus_out = unim_im_context_focus_out;
    im_class->reset = unim_im_context_reset;
    im_class->get_preedit_string = unim_im_context_get_preedit_string;
    im_class->set_cursor_location = unim_im_context_set_cursor_location;
}

static void
unim_im_context_class_finalize(UnimIMContextClass *klass)
{
}

static void
unim_im_context_init(UnimIMContext *context)
{
    context->config = unim_config_load();
    context->engine = unim_engine_new(context->config);
    context->is_focused = FALSE;
}

static void
unim_im_context_dispose(GObject *obj)
{
    UnimIMContext *context = UNIM_IM_CONTEXT(obj);

    if (context->engine) {
        unim_engine_delete(context->engine);
        context->engine = NULL;
    }

    if (context->config) {
        unim_config_delete(context->config);
        context->config = NULL;
    }

    G_OBJECT_CLASS(unim_im_context_parent_class)->dispose(obj);
}

static gboolean
unim_im_context_filter_keypress(GtkIMContext *context, GdkEvent *event)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (!unim->engine || !unim->config) {
        return FALSE;
    }

    /* 키 이벤트 타입 확인 (GDK4 방식) */
    GdkEventType event_type = gdk_event_get_event_type(event);
    if (event_type != GDK_KEY_PRESS && event_type != GDK_KEY_RELEASE) {
        return FALSE;
    }

    /* Release 이벤트는 일단 무시 (필요시 확장) */
    if (event_type == GDK_KEY_RELEASE) {
        return FALSE;
    }

    /* 키 정보 추출 (GDK 4.4+ 접근자) */
#if GTK_CHECK_VERSION(4, 4, 0)
    guint keyval = gdk_key_event_get_keyval(event);
    guint keycode = gdk_key_event_get_keycode(event);
    GdkModifierType state = gdk_event_get_modifier_state(event);
#else
    /* GDK 4.4 미만에서는 이 기능을 사용할 수 없으므로 기본값 사용 */
    guint keyval = 0;
    guint keycode = 0;
    GdkModifierType state = 0;
    g_warning("GTK version too old for key event accessors");
#endif

    /* 수정자 상태 변환 (GDK4 대응) */
    UnimModifierState mod_state = {
        .shift = (state & GDK_SHIFT_MASK) != 0,
        .control = (state & GDK_CONTROL_MASK) != 0,
        .alt = (state & GDK_ALT_MASK) != 0,
        .super_key = (state & GDK_SUPER_MASK) != 0,
        .caps_lock = (state & GDK_LOCK_MASK) != 0,
        .num_lock = false
    };

    /* 설정 파일 변경 체크 (mtime 기반, 매우 가벼움) */
    if (unim->config && unim_config_reload(unim->config)) {
        /* 설정이 변경되었으면 엔진에 새 레이아웃 적용 */
        if (unim->engine) {
            unim_engine_set_hangul_layout(unim->engine, 
                unim_config_get_hangul_layout(unim->config));
            unim_engine_set_latin_layout(unim->engine,
                unim_config_get_latin_layout(unim->config));
        }
    }

    /* 키 입력 처리 */
    /* GDK keycode = X11 keycode = evdev + 8, 엔진은 evdev 형식 기대 */
    uint16_t evdev_code = (keycode > 8) ? (uint16_t)(keycode - 8) : 0;
    UnimInputResult result = unim_engine_press_key(
        unim->engine,
        unim->config,
        evdev_code,
        mod_state
    );

    /* 커밋 처리 */
    if (result.commit_changed) {
        UnimStr commit = unim_engine_commit_str(unim->engine);
        if (commit.len > 0) {
            char *str = g_strndup((const char *)commit.ptr, commit.len);
            g_signal_emit_by_name(context, "commit", str);
            g_free(str);
        }
        unim_engine_clear_commit(unim->engine);
    }

    /* preedit 변경 처리 */
    if (result.preedit_changed) {
        g_signal_emit_by_name(context, "preedit-changed");
    }

    return result.consumed;
}

static void
unim_im_context_focus_in(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    unim->is_focused = TRUE;
}

static void
unim_im_context_focus_out(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (unim->engine && unim_engine_is_composing(unim->engine)) {
        unim_engine_clear_preedit(unim->engine);
        UnimStr commit = unim_engine_commit_str(unim->engine);
        if (commit.len > 0) {
            char *str = g_strndup((const char *)commit.ptr, commit.len);
            g_signal_emit_by_name(context, "commit", str);
            g_free(str);
        }
        unim_engine_clear_commit(unim->engine);
        g_signal_emit_by_name(context, "preedit-changed");
    }

    unim->is_focused = FALSE;
}

static void
unim_im_context_reset(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (unim->engine) {
        unim_engine_reset(unim->engine);
        g_signal_emit_by_name(context, "preedit-changed");
    }
}

/* 위젯 참조 저장 (GTK4에서는 방식 변경됨) */

static void
unim_im_context_get_preedit_string(GtkIMContext *context, char **str,
                                    PangoAttrList **attrs, int *cursor_pos)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (unim->engine) {
        UnimStr preedit = unim_engine_preedit_str(unim->engine);
        if (preedit.len > 0) {
            *str = g_strndup((const char *)preedit.ptr, preedit.len);
        } else {
            *str = g_strdup("");
        }
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
    /* 커서 위치 저장 */
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
