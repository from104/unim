/**
 * UNIM GTK3 Input Method Module
 *
 * GTK3 애플리케이션에서 한글 입력을 제공하는 IM 모듈입니다.
 */

#include <gtk/gtk.h>
#include <gtk/gtkimmodule.h>
#include <gdk/gdkkeysyms.h>
#include <string.h>
#include <unim.h>

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
    UnimEngine *engine;
    UnimConfig *config;
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
    context->config = unim_config_load();
    context->engine = unim_engine_new(context->config);
    context->is_focused = FALSE;
    context->client_window = NULL;
}

static void
unim_im_context_finalize(GObject *obj)
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

    G_OBJECT_CLASS(unim_im_context_parent_class)->finalize(obj);
}

static gboolean
unim_im_context_filter_keypress(GtkIMContext *context, GdkEventKey *event)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    if (!unim->engine || !unim->config) {
        return FALSE;
    }

    /* 키 릴리스는 무시 */
    if (event->type != GDK_KEY_PRESS) {
        return FALSE;
    }

    /* 수정자 상태 변환 */
    UnimModifierState state = {
        .shift = (event->state & GDK_SHIFT_MASK) != 0,
        .control = (event->state & GDK_CONTROL_MASK) != 0,
        .alt = (event->state & GDK_MOD1_MASK) != 0,
        .super_key = (event->state & GDK_SUPER_MASK) != 0,
        .caps_lock = (event->state & GDK_LOCK_MASK) != 0,
        .num_lock = FALSE
    };

    /* 키 입력 처리 */
    UnimInputResult result = unim_engine_press_key(
        unim->engine,
        unim->config,
        event->hardware_keycode,
        state
    );

    /* 커밋 처리 */
    if (result.commit_changed) {
        UnimStr commit = unim_engine_commit_str(unim->engine);
        if (commit.len > 0) {
            gchar *str = g_strndup((const gchar *)commit.ptr, commit.len);
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
        /* 조합 중이면 커밋 */
        unim_engine_clear_preedit(unim->engine);
        UnimStr commit = unim_engine_commit_str(unim->engine);
        if (commit.len > 0) {
            gchar *str = g_strndup((const gchar *)commit.ptr, commit.len);
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

    if (unim->engine) {
        UnimStr preedit = unim_engine_preedit_str(unim->engine);
        if (preedit.len > 0) {
            *str = g_strndup((const gchar *)preedit.ptr, preedit.len);
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
