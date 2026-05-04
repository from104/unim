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

/* X11 위치 계산을 위한 헤더 */
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#include <X11/Xlib.h>
#include <X11/extensions/XTest.h>
#endif

/* Wayland 감지를 위한 헤더 */
#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif

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
    
    GdkRectangle cursor_area;           /* 커서 위치 */

    /* 한자/특수문자 키 설정 캐시 */
    guint *hanja_keysyms;              /* 설정 기반 한자키 keysym 배열 */
    gsize n_hanja_keysyms;             /* 배열 크기 */

    /* 마지막으로 emit한 preedit (preedit-start/end 자동 발사용).
     * GTK4와 동일 패턴 — preedit-end 누락 시 ghostty 등에서 키 잠금 발생 */
    gchar *last_preedit;

    /* AutoTypeFix XTest 폴백용 (delete_surrounding 미지원 앱 — Electron 등) */
    guint autofix_bs_pending;        /* 자가 주입 BackSpace 잔여 수 */
    gchar *autofix_commit_text;      /* 지연 commit 텍스트 */
    gchar *autofix_preedit_text;     /* 지연 preedit 텍스트 */

    /* 데몬 popup 세션 활성 여부 (Show*Popup* 시그널 → TRUE, HidePopup → FALSE).
     * Emoji popup 은 idle 트리거라 is_composing=FALSE 인데, 그러면 nav 키가
     * IM 우회 (앱 전달) 되어 popup 조작 불가. popup 가시 중엔 우회 차단. */
    gboolean popup_active;
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

/* Standalone popup 시그널 핸들러 */
static void on_show_emoji_popup(const gchar *target_cat_id,
                                const gchar * const *items, gsize item_count,
                                const gchar *top_row,
                                const gchar * const *recent, gsize recent_count,
                                const UnimEmojiCategoryMeta *categories, gsize category_count,
                                gint cursor_x, gint cursor_y,
                                gint cursor_width, gint cursor_height,
                                gpointer user_data);
static void on_hide_popup(gpointer user_data);

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

/* preedit 전이를 GTK IM 프로토콜에 맞춰 안전하게 emit.
 *   "" → "" : 시그널 없음
 *   "" → "X": preedit-start + preedit-changed
 *   "X" → "Y": preedit-changed
 *   "X" → "" : preedit-changed + preedit-end
 * preedit-end를 빠뜨리면 ghostty 등 일부 앱이 IM 활성 상태로 잠겨
 * 이후 non-text 키 전파를 차단함. 모든 preedit 변경은 이 함수로 통일. */
static void
unim_emit_preedit(UnimIMContext *unim, const gchar *new_preedit)
{
    GtkIMContext *context = GTK_IM_CONTEXT(unim);
    if (new_preedit == NULL) new_preedit = "";

    if (g_strcmp0(unim->last_preedit, new_preedit) == 0) {
        return;
    }

    const gboolean was_empty = (unim->last_preedit == NULL || unim->last_preedit[0] == '\0');
    const gboolean now_empty = (new_preedit[0] == '\0');

    g_free(unim->last_preedit);
    unim->last_preedit = g_strdup(new_preedit);

    if (was_empty && !now_empty) {
        g_signal_emit_by_name(context, "preedit-start");
        g_signal_emit_by_name(context, "preedit-changed");
    } else if (!was_empty && now_empty) {
        g_signal_emit_by_name(context, "preedit-changed");
        g_signal_emit_by_name(context, "preedit-end");
    } else {
        g_signal_emit_by_name(context, "preedit-changed");
    }
}

/* AutoTypeFix 지연 commit 콜백 (XTest BackSpace 처리 완료 후 실행) */
static gboolean
autofix_deferred_commit_cb(gpointer user_data)
{
    UnimIMContext *unim = (UnimIMContext *)user_data;
    GtkIMContext *context = GTK_IM_CONTEXT(unim);

    UNIM_DEBUG("AutoTypeFix 지연 commit: '%s', preedit='%s'",
               unim->autofix_commit_text ? unim->autofix_commit_text : "",
               unim->autofix_preedit_text ? unim->autofix_preedit_text : "");

    if (unim->autofix_commit_text && unim->autofix_commit_text[0] != '\0') {
        g_signal_emit_by_name(context, "commit", unim->autofix_commit_text);
    }

    if (unim->autofix_preedit_text && unim->autofix_preedit_text[0] != '\0') {
        unim_dbus_set_preedit_cache(unim->dbus_ctx, unim->autofix_preedit_text);
        unim_emit_preedit(unim, unim->autofix_preedit_text);
    } else {
        unim_emit_preedit(unim, "");
    }

    g_clear_pointer(&unim->autofix_commit_text, g_free);
    g_clear_pointer(&unim->autofix_preedit_text, g_free);

    return G_SOURCE_REMOVE;
}

/* CommitText 콜백: Standalone 팝업 마우스 클릭 시 커밋 */
static void
on_commit_text(const gchar *text, gpointer user_data)
{
    GtkIMContext *context = GTK_IM_CONTEXT(user_data);
    if (text && text[0] != '\0') {
        UNIM_DEBUG("CommitText 시그널 수신: '%s'", text);
        g_signal_emit_by_name(context, "commit", text);
    }
}

/* AutoTypeFix 콜백: delete_surrounding + commit + preedit */
static void
on_auto_typefix(guint delete_chars, const gchar *commit_text,
                const gchar *preedit_text, gpointer user_data)
{
    UnimIMContext *unim = (UnimIMContext *)user_data;
    GtkIMContext *context = GTK_IM_CONTEXT(unim);

    UNIM_DEBUG("AutoTypeFix 적용: delete=%u, commit='%s', preedit='%s'",
               delete_chars, commit_text, preedit_text);

    /* 포커스가 없으면 무시 (다른 프론트엔드가 처리 중) */
    if (!unim->is_focused) {
        UNIM_DEBUG("AutoTypeFix 무시: 포커스 없음");
        return;
    }

    gboolean deleted = FALSE;
    if (delete_chars > 0) {
        deleted = gtk_im_context_delete_surrounding(context,
                                                    -(gint)delete_chars,
                                                    (gint)delete_chars);
    }

    /* delete_surrounding 미지원 앱 (Electron 등) → XTest BackSpace 주입 */
    if (!deleted && delete_chars > 0) {
#ifdef GDK_WINDOWING_X11
        if (GDK_IS_X11_DISPLAY(gdk_display_get_default())) {
            Display *xdisplay = gdk_x11_display_get_xdisplay(gdk_display_get_default());
            KeyCode bs_keycode = XKeysymToKeycode(xdisplay, GDK_KEY_BackSpace);

            /* 지연 commit 데이터 저장 */
            unim->autofix_bs_pending = delete_chars;
            g_free(unim->autofix_commit_text);
            unim->autofix_commit_text = g_strdup(commit_text);
            g_free(unim->autofix_preedit_text);
            unim->autofix_preedit_text = g_strdup(preedit_text);

            /* XTest로 BackSpace 주입 */
            for (guint i = 0; i < delete_chars; i++) {
                XTestFakeKeyEvent(xdisplay, bs_keycode, True, 0);
                XTestFakeKeyEvent(xdisplay, bs_keycode, False, 0);
            }
            XFlush(xdisplay);

            UNIM_DEBUG("AutoTypeFix XTest BackSpace x%u 주입", delete_chars);
            return; /* commit/preedit은 filter_keypress에서 지연 처리 */
        }
#endif
        /* Non-X11 fallback: \b */
        gchar *bs = g_malloc(delete_chars + 1);
        memset(bs, '\b', delete_chars);
        bs[delete_chars] = '\0';
        g_signal_emit_by_name(context, "commit", bs);
        g_free(bs);
        UNIM_DEBUG("AutoTypeFix delete_surrounding 실패 → \\b x%u 폴백", delete_chars);
    }

    if (commit_text && commit_text[0] != '\0') {
        g_signal_emit_by_name(context, "commit", commit_text);
    }

    if (preedit_text && preedit_text[0] != '\0') {
        unim_dbus_set_preedit_cache(unim->dbus_ctx, preedit_text);
        unim_emit_preedit(unim, preedit_text);
    } else {
        unim_emit_preedit(unim, "");
    }
}


static void
unim_im_context_init(UnimIMContext *context)
{
    unim_check_debug_env();

    /* 창 식별자 생성 (컨텍스트 포인터 기반) */
    const gchar *prgname = g_get_prgname();
    context->window_id = g_strdup_printf("%s:gtk3-ctx-%p",
        prgname ? prgname : "unknown", (void*)context);

    /* DBus 클라이언트 생성 (window_id 포함) */
    context->dbus_ctx = unim_dbus_context_new("gtk3-unim", context->window_id);

    /* AutoTypeFix / CommitText 시그널 콜백 등록 */
    if (context->dbus_ctx) {
        unim_dbus_set_auto_typefix_callback(context->dbus_ctx, on_auto_typefix, context);
        unim_dbus_set_commit_text_callback(context->dbus_ctx, on_commit_text, context);
    }
    context->is_focused = FALSE;
    context->client_window = NULL;
    context->surrounding_text = NULL;
    context->cursor_index = 0;
    context->selection_index = 0;
    context->last_preedit = g_strdup("");
    memset(&context->cursor_area, 0, sizeof(GdkRectangle));

    /* Standalone popup 시그널 핸들러 — popup_active 플래그 관리용 */
    if (context->dbus_ctx) {
        unim_dbus_set_show_emoji_popup_callback(
            context->dbus_ctx, on_show_emoji_popup, context);
        unim_dbus_set_hide_popup_callback(
            context->dbus_ctx, on_hide_popup, context);
    }
    
    /* 한자키 설정 로드 */
    context->hanja_keysyms = NULL;
    context->n_hanja_keysyms = 0;
    if (context->dbus_ctx) {
        gchar *hanja_keys_str = unim_dbus_get_config(context->dbus_ctx, "hanja_keys");
        if (hanja_keys_str && hanja_keys_str[0]) {
            gchar **keys = g_strsplit(hanja_keys_str, ",", -1);
            gsize n = g_strv_length(keys);
            context->hanja_keysyms = g_new0(guint, n);
            gsize count = 0;
            for (gsize i = 0; i < n; i++) {
                g_strstrip(keys[i]);
                guint kv = unim_keycode_name_to_gdk_keyval(keys[i]);
                if (kv != 0) {
                    context->hanja_keysyms[count++] = kv;
                }
            }
            context->n_hanja_keysyms = count;
            g_strfreev(keys);
        }
        g_free(hanja_keys_str);
    }
    /* 설정 로드 실패 시 기본값 사용 */
    if (context->n_hanja_keysyms == 0) {
        context->hanja_keysyms = g_new(guint, 2);
        context->hanja_keysyms[0] = GDK_KEY_Hangul_Hanja;
        context->hanja_keysyms[1] = GDK_KEY_F9;
        context->n_hanja_keysyms = 2;
    }
    
    if (context->dbus_ctx) {
        UNIM_DEBUG("IMContext 초기화 완료 (window_id: %s)", context->window_id);
    } else {
        UNIM_DEBUG("IMContext 초기화 (DBus 연결 실패)");
    }
}

/* ShowEmojiPopupV2 시그널 핸들러 — Standalone 모드: popup_active 마킹만 */
static void
on_show_emoji_popup(const gchar *target_cat_id,
                     const gchar * const *items,
                     gsize item_count,
                     const gchar *top_row,
                     const gchar * const *recent,
                     gsize recent_count,
                     const UnimEmojiCategoryMeta *categories,
                     gsize category_count,
                     gint cursor_x,
                     gint cursor_y,
                     gint cursor_width,
                     gint cursor_height,
                     gpointer user_data)
{
    (void)target_cat_id; (void)items; (void)item_count; (void)top_row;
    (void)recent; (void)recent_count; (void)categories; (void)category_count;
    (void)cursor_x; (void)cursor_y; (void)cursor_width; (void)cursor_height;
    UnimIMContext *unim = UNIM_IM_CONTEXT(user_data);
    if (!unim) return;

    /* Standalone: unim-gui-gtk 가 팝업 전담. nav 키 우회를 막기 위해 플래그만 세운다. */
    unim->popup_active = TRUE;
}

/* HidePopup 시그널 핸들러 — Standalone 모드: popup_active 해제만 */
static void
on_hide_popup(gpointer user_data)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(user_data);
    if (!unim) return;

    /* Popup 세션 종료 — nav 키 우회 차단 해제 */
    unim->popup_active = FALSE;
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
    
    g_free(context->hanja_keysyms);
    context->hanja_keysyms = NULL;
    context->n_hanja_keysyms = 0;

    g_free(context->surrounding_text);
    context->surrounding_text = NULL;

    g_free(context->last_preedit);
    context->last_preedit = NULL;

    g_free(context->autofix_commit_text);
    context->autofix_commit_text = NULL;
    g_free(context->autofix_preedit_text);
    context->autofix_preedit_text = NULL;

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

    /* AutoTypeFix 자가 주입 BackSpace 패스스루 */
    if (event->keyval == GDK_KEY_BackSpace && unim->autofix_bs_pending > 0) {
        unim->autofix_bs_pending--;
        UNIM_DEBUG("AutoTypeFix self-BackSpace 패스스루 (남은=%u)",
                   unim->autofix_bs_pending);
        if (unim->autofix_bs_pending == 0) {
            /* 마지막 BackSpace → idle에서 지연 commit */
            g_idle_add(autofix_deferred_commit_cb, unim);
        }
        return FALSE; /* 앱이 BackSpace 처리 */
    }

    /* 매 키 입력 전 surrounding text 갱신 (TypeFIX 등에 필요) */
    {
        gboolean handled = FALSE;
        g_signal_emit_by_name(context, "retrieve-surrounding", &handled);
    }

    /* 수정자 키만 눌린 경우 바이패스 (preedit에 영향 없이 앱으로 전달) */
    if (event->keyval == GDK_KEY_Shift_L || event->keyval == GDK_KEY_Shift_R ||
        event->keyval == GDK_KEY_Control_L || event->keyval == GDK_KEY_Control_R ||
        event->keyval == GDK_KEY_Alt_L || event->keyval == GDK_KEY_Alt_R ||
        event->keyval == GDK_KEY_Super_L || event->keyval == GDK_KEY_Super_R ||
        event->keyval == GDK_KEY_Meta_L || event->keyval == GDK_KEY_Meta_R ||
        event->keyval == GDK_KEY_Hyper_L || event->keyval == GDK_KEY_Hyper_R ||
        event->keyval == GDK_KEY_Caps_Lock || event->keyval == GDK_KEY_Num_Lock ||
        event->keyval == GDK_KEY_Scroll_Lock ||
        event->keyval == GDK_KEY_ISO_Level3_Shift) {
        return FALSE;
    }

    /* 한자 키 처리 (설정 기반) */
    gboolean is_hanja = FALSE;
    for (gsize i = 0; i < unim->n_hanja_keysyms; i++) {
        if (event->keyval == unim->hanja_keysyms[i]) {
            is_hanja = TRUE;
            break;
        }
    }
    if (is_hanja) {
        gchar *target = NULL;
        UnimHanjaCandidate *candidates = NULL;
        gsize count = 0;

        /* Wayland: Hanja 키를 ProcessKeyEvent로 보내서 Push 시그널 발행
         * → GNOME extension이 팝업 처리 */
        gboolean is_wayland = FALSE;
#ifdef GDK_WINDOWING_WAYLAND
        GdkDisplay *display = gdk_display_get_default();
        if (GDK_IS_WAYLAND_DISPLAY(display)) {
            is_wayland = TRUE;
        }
#endif
        if (is_wayland) {
            UNIM_DEBUG("Wayland: Hanja 키 → ProcessKeyEvent (GNOME ext 팝업 위임)");
            UnimDbusKeyResult hanja_result;
            guint evdev = event->hardware_keycode - 8;
            if (unim_dbus_process_key(unim->dbus_ctx, event->keyval, evdev, event->state, &hanja_result)) {
                if (hanja_result.preedit) {
                    unim_emit_preedit(unim, hanja_result.preedit);
                }
                if (hanja_result.commit && strlen(hanja_result.commit) > 0) {
                    g_signal_emit_by_name(context, "commit", hanja_result.commit);
                }
                g_free(hanja_result.preedit);
                g_free(hanja_result.commit);
            }
            return TRUE;
        }

        /* X11: Standalone — unim-dbus RPC 호출 → unim-gui-gtk 가 팝업 전담 */
        if (unim_dbus_get_hanja_candidates(unim->dbus_ctx, &target, &candidates, &count)) {
            if (count > 0) {
                /* 후보 즉시 해제 (Standalone 모드: IM 모듈이 직접 표시 안 함) */
                unim_hanja_candidates_free(candidates, count);
                UNIM_DEBUG("한자 후보 %zu개 — Standalone popup 위임", count);
            } else {
                /* 한자 후보 없음 → 특수문자 후보 확인 */
                UNIM_DEBUG("한자 후보 없음, 특수문자 확인...");
                if (candidates) {
                    unim_hanja_candidates_free(candidates, count);
                }
                g_free(target);
                target = NULL;

                /* 특수문자 후보 조회 */
                gchar *sp_target = NULL;
                gchar **sp_chars = NULL;
                gsize sp_count = 0;
                gchar *sp_top_row = NULL;

                gboolean sp_ok = unim_dbus_get_special_char_candidates(unim->dbus_ctx,
                        &sp_target, &sp_chars, &sp_count, &sp_top_row);
                UNIM_DEBUG("특수문자 조회 결과: ok=%d, target='%s', count=%zu",
                           sp_ok, sp_target ? sp_target : "(null)", sp_count);
                if (sp_ok && sp_count > 0) {
                    /* 후보 즉시 해제 (Standalone popup 위임) */
                    unim_special_chars_free(sp_chars, sp_count);
                    UNIM_DEBUG("특수문자 후보 %zu개 — Standalone popup 위임", sp_count);
                } else {
                    UNIM_DEBUG("특수문자 후보도 없음 → idle Hanja: emoji 트리거 위임");
                    if (sp_chars) {
                        unim_special_chars_free(sp_chars, sp_count);
                    }
                    /* idle (preedit/조합 비어있음) → 엔진의 dual-purpose
                     * Hanja 분기가 emoji popup 트리거. ShowEmojiPopupV2
                     * signal handler 가 popup_active 마킹. */
                    UnimDbusKeyResult emoji_result = {0};
                    guint evdev = event->hardware_keycode > 0
                                      ? event->hardware_keycode - 8
                                      : 0;
                    if (unim_dbus_process_key(unim->dbus_ctx,
                                              event->keyval,
                                              evdev,
                                              event->state,
                                              &emoji_result)) {
                        if (emoji_result.preedit) {
                            unim_emit_preedit(unim, emoji_result.preedit);
                        }
                        if (emoji_result.commit
                            && strlen(emoji_result.commit) > 0) {
                            g_signal_emit_by_name(context, "commit",
                                                  emoji_result.commit);
                        }
                        g_free(emoji_result.preedit);
                        g_free(emoji_result.commit);
                    }
                }
                g_free(sp_target);
                g_free(sp_top_row);
                return TRUE;
            }
        }
        g_free(target);
        return TRUE;
    }

    /* 조합 중이 아닌 경우, 특수키는 IM에서 처리하지 않고 앱으로 직접 전달 */
    /* (터미널에서 방향키, Backspace 등이 동작하도록 함)
     * 단 emoji popup 등 idle 트리거 popup 가시 중엔 우회 차단 — 그렇지 않으면
     * 화살표/Esc/Home/End/PgUp/PgDn 이 popup 으로 가지 않고 앱에 전달된다. */
    if (!unim_dbus_is_composing(unim->dbus_ctx) && !unim->popup_active) {
        /* 기능키 (F1~F12, 단 F9은 한자키로 위에서 처리됨) */
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

    /* preedit 전이는 헬퍼로 통일 (preedit-start/changed/end 자동 발사) */
    unim_emit_preedit(unim, result.preedit);

    /* 메모리 해제 */
    g_free(result.preedit);
    g_free(result.commit);

    return result.consumed;
}

/* GtkInputPurpose → UNIM ContentPurpose 변환 */
static guint
gtk3_input_purpose_to_unim(GtkInputPurpose purpose)
{
    switch (purpose) {
        case GTK_INPUT_PURPOSE_PASSWORD: return 1; /* Password */
        case GTK_INPUT_PURPOSE_PIN:      return 2; /* Pin */
        case GTK_INPUT_PURPOSE_EMAIL:    return 3; /* Email */
        case GTK_INPUT_PURPOSE_NUMBER:   return 4; /* Number */
        case GTK_INPUT_PURPOSE_URL:      return 5; /* Url */
        case GTK_INPUT_PURPOSE_TERMINAL: return 6; /* Terminal */
        default:                         return 0; /* Normal */
    }
}

static void
unim_im_context_focus_in(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);

    UNIM_DEBUG("focus_in 호출 (window_id: %s)", unim->window_id);

    if (unim->dbus_ctx) {
        unim_dbus_focus_in(unim->dbus_ctx, unim->window_id);

        /* GTK3: GtkEntry에서 input-purpose 속성 감지 */
        if (unim->client_window) {
            GtkWidget *widget = NULL;
            gdk_window_get_user_data(unim->client_window, (gpointer*)&widget);
            if (widget && GTK_IS_WIDGET(widget)) {
                GtkInputPurpose purpose = GTK_INPUT_PURPOSE_FREE_FORM;
                GParamSpec *pspec = g_object_class_find_property(
                    G_OBJECT_GET_CLASS(widget), "input-purpose");
                if (pspec) {
                    g_object_get(widget, "input-purpose", &purpose, NULL);
                }
                guint unim_purpose = gtk3_input_purpose_to_unim(purpose);
                unim_dbus_set_content_type(unim->dbus_ctx, unim_purpose);
                UNIM_DEBUG("content_type 전달: gtk_purpose=%d, unim_purpose=%u",
                           (int)purpose, unim_purpose);
            }
        }
    }

    unim->is_focused = TRUE;
}

static void
unim_im_context_focus_out(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    gchar *commit = NULL;

    UNIM_DEBUG("focus_out 호출");

    /* 1. 조합 중인 글자를 커밋 */
    if (unim->dbus_ctx) {
        unim_dbus_focus_out(unim->dbus_ctx, &commit);
        
        /* 조합 중이던 문자 커밋 */
        if (commit && strlen(commit) > 0) {
            UNIM_DEBUG("focus_out 커밋: \"%s\"", commit);
            g_signal_emit_by_name(context, "commit", commit);
        }
        g_free(commit);

        /* preedit 클리어 (preedit-end까지 발사) */
        unim_emit_preedit(unim, "");
    }

    unim->is_focused = FALSE;
}

static void
unim_im_context_reset(GtkIMContext *context)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    gchar *commit = NULL;

    UNIM_DEBUG("reset 호출");

    /* 1. 조합 중인 글자를 먼저 커밋 */
    if (unim->dbus_ctx) {
        unim_dbus_reset(unim->dbus_ctx, &commit);
        
        if (commit && strlen(commit) > 0) {
            UNIM_DEBUG("reset 커밋: \"%s\"", commit);
            g_signal_emit_by_name(context, "commit", commit);
        }
        g_free(commit);

        /* preedit 클리어 (preedit-end까지 발사) */
        unim_emit_preedit(unim, "");
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
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    
    if (area) {
        unim->cursor_area = *area;

        /* 커서 위치를 데몬에 보고 (팝업 포지셔닝용)
         *
         * GTK3 좌표 단위 (GTK4 immodule.c calculate_popup_position 와 동일 패턴):
         *   area->x, area->y           → 논리 픽셀 (window-local, scale_factor 미반영)
         *   gdk_window_get_origin 결과 → X11 framebuffer 물리 픽셀 (root coords, device px)
         *   unim_dbus_report_cursor_rect → 물리 픽셀 기대 (popup_positioning.rs)
         *
         * gdk_window_get_root_coords 는 일부 fractional scaling 환경에서 입력 단위 처리가
         * 모호 → gdk_window_get_origin + 명시적 scale 곱하기로 일관성 보장.
         * GDK_SCALE=1 (일반 환경) 에서는 scale=1 이라 곱해도 결과 동일.
         */
        if (unim->dbus_ctx && unim->client_window) {
            gint scale = gdk_window_get_scale_factor(unim->client_window);
            if (scale < 1) scale = 1;

            gint origin_x = 0, origin_y = 0;
            gdk_window_get_origin(unim->client_window, &origin_x, &origin_y);

            /* 논리 좌표를 물리 픽셀로 변환한 뒤 window root offset 더하기 */
            gint abs_x = (area->x * scale) + origin_x;
            gint abs_y = (area->y * scale) + origin_y;
            gint abs_w = area->width * scale;
            gint abs_h = area->height * scale;

            unim_dbus_report_cursor_rect(unim->dbus_ctx,
                                          abs_x, abs_y,
                                          abs_w, abs_h);
        }
    }
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

    /* Surrounding text를 DBus로 전달 */
    if (unim->dbus_ctx && unim->surrounding_text) {
        /* 바이트 오프셋 → 문자 오프셋 변환 */
        guint cursor_char = (guint)g_utf8_pointer_to_offset(
            unim->surrounding_text, unim->surrounding_text + cursor_index);
        unim_dbus_set_surrounding_text(unim->dbus_ctx,
                                        unim->surrounding_text,
                                        cursor_char, cursor_char);
    }
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
