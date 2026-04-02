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

/* DBus 클라이언트 및 팝업 헤더 */
#include "unim_dbus_client.h"
#include "unim_hanja_popup.h"
#include "unim_special_popup.h"

/* X11 위치 계산을 위한 헤더 */
#ifdef GDK_WINDOWING_X11
#include <gdk/x11/gdkx.h>
#include <X11/Xlib.h>
#endif

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
    GtkWidget *client_widget;   /* 입력 위젯 참조 (좌표 변환용) */
    
    /* 주변 텍스트 정보 캐시 */
    gchar *surrounding_text;
    gint cursor_index;    /* 바이트 오프셋 */
    gint selection_index; /* 바이트 오프셋 */
    
    /* 한자 변환 관련 */
    UnimHanjaPopup *hanja_popup;       /* 한자 후보 팝업 */
    UnimHanjaCandidate *hanja_candidates; /* 현재 후보 목록 */
    gsize hanja_count;                  /* 후보 개수 */
    GdkRectangle cursor_area;           /* 커서 위치 (위젯 로컬 좌표) */
    
    /* 특수문자 변환 관련 */
    UnimSpecialPopup *special_popup;   /* 특수문자 후보 팝업 */
    gchar **special_characters;        /* 현재 특수문자 목록 */
    gsize special_count;               /* 특수문자 개수 */
    
    /* 한자/특수문자 키 설정 캐시 */
    guint *hanja_keysyms;              /* 설정 기반 한자키 keysym 배열 */
    gsize n_hanja_keysyms;             /* 배열 크기 */
};

G_DEFINE_DYNAMIC_TYPE(UnimIMContext, unim_im_context, GTK_TYPE_IM_CONTEXT)

/* 함수 선언 */
/* Standalone 모드 확인: 로컬 팝업 표시 건너뜀 */
static gboolean
is_standalone_popup(UnimDbusContext *ctx)
{
    gchar *mode = unim_dbus_get_config(ctx, "popup_mode");
    gboolean standalone = (mode && g_strcmp0(mode, "Standalone") == 0);
    g_free(mode);
    return standalone;
}

static void unim_im_context_dispose(GObject *obj);
static gboolean unim_im_context_filter_keypress(GtkIMContext *context, GdkEvent *event);
static void unim_im_context_focus_in(GtkIMContext *context);
static void unim_im_context_focus_out(GtkIMContext *context);
static void unim_im_context_reset(GtkIMContext *context);
static void unim_im_context_get_preedit_string(GtkIMContext *context, char **str,
                                                PangoAttrList **attrs, int *cursor_pos);
static void unim_im_context_set_cursor_location(GtkIMContext *context, GdkRectangle *area);
static void unim_im_context_set_client_widget(GtkIMContext *context, GtkWidget *widget);
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

    object_class->dispose = unim_im_context_dispose;

    im_class->filter_keypress = (gboolean (*)(GtkIMContext *, GdkEvent *))unim_im_context_filter_keypress;
    im_class->focus_in = unim_im_context_focus_in;
    im_class->focus_out = unim_im_context_focus_out;
    im_class->reset = unim_im_context_reset;
    im_class->get_preedit_string = unim_im_context_get_preedit_string;
    im_class->set_cursor_location = unim_im_context_set_cursor_location;
    im_class->set_client_widget = unim_im_context_set_client_widget;
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
    const gchar *prgname = g_get_prgname();
    context->window_id = g_strdup_printf("%s:gtk4-ctx-%p",
        prgname ? prgname : "unknown", (void*)context);
    
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
    
    /* 특수문자 팝업 초기화 */
    context->special_popup = unim_special_popup_new();
    context->special_characters = NULL;
    context->special_count = 0;
    
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

    /* 특수문자 팝업 해제 */
    if (context->special_popup) {
        unim_special_popup_free(context->special_popup);
        context->special_popup = NULL;
    }
    
    if (context->special_characters) {
        unim_special_chars_free(context->special_characters, context->special_count);
        context->special_characters = NULL;
        context->special_count = 0;
    }

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
    
    /* preedit 클리어 먼저 (엔진에서 preedit 제거) */
    unim_dbus_cancel_hanja(unim->dbus_ctx);
    
    /* preedit-changed 시그널 (빈 preedit) */
    g_signal_emit_by_name(unim, "preedit-changed");
    
    /* 한자만 커밋 */
    g_signal_emit_by_name(unim, "commit", hanja);
}

/* 특수문자 선택 콜백 - 팝업에서 특수문자 선택 시 호출됨 */
static void
on_special_char_selected(const gchar *character, gpointer user_data)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(user_data);
    
    if (!unim || !character || !unim->dbus_ctx) {
        return;
    }
    
    UNIM_DEBUG("특수문자 선택 콜백: char='%s'", character);
    
    /* 팝업 숨기기 (먼저) */
    if (unim->special_popup) {
        unim_special_popup_hide(unim->special_popup);
    }
    
    /* 엔진 특수문자 모드 취소 (preedit 클리어) */
    unim_dbus_cancel_special_char(unim->dbus_ctx);
    
    /* preedit-changed 시그널 (빈 preedit) */
    g_signal_emit_by_name(unim, "preedit-changed");
    
    /* 특수문자 커밋 */
    g_signal_emit_by_name(unim, "commit", character);
}

/* 커서 위치로부터 화면 절대 좌표 계산 (GTK3의 gdk_window_get_origin 방식과 동일) */
static void
calculate_popup_position(UnimIMContext *unim, gint *out_x, gint *out_y)
{
    gint popup_x = unim->cursor_area.x;
    gint popup_y = unim->cursor_area.y + unim->cursor_area.height;

    if (unim->client_widget) {
        GtkNative *native = gtk_widget_get_native(unim->client_widget);
        if (native) {
            /* 위젯→native 위젯 좌표 변환 */
            graphene_point_t p_in = GRAPHENE_POINT_INIT(
                (float)unim->cursor_area.x,
                (float)(unim->cursor_area.y + unim->cursor_area.height));
            graphene_point_t p_out;
            if (gtk_widget_compute_point(unim->client_widget,
                                          GTK_WIDGET(native),
                                          &p_in, &p_out)) {
                /* native 위젯→surface 좌표 변환 (CSD 장식/그림자 보정) */
                double surface_tx, surface_ty;
                gtk_native_get_surface_transform(native, &surface_tx, &surface_ty);
                /* surface_transform: widget→surface 변환 (GTK4 실제 동작) */
                popup_x = (gint)(p_out.x + surface_tx);
                popup_y = (gint)(p_out.y + surface_ty);
            }

#ifdef GDK_WINDOWING_X11
            /* X11: surface 절대 위치 보정 */
            {
                GdkSurface *surface = gtk_native_get_surface(native);
                if (surface && GDK_IS_X11_SURFACE(surface)) {
                    Display *xdisplay = gdk_x11_display_get_xdisplay(
                        gdk_surface_get_display(surface));
                    Window xwindow = gdk_x11_surface_get_xid(surface);
                    gint abs_x = 0, abs_y = 0;
                    Window child_return;

                    XTranslateCoordinates(xdisplay, xwindow,
                        DefaultRootWindow(xdisplay),
                        0, 0, &abs_x, &abs_y, &child_return);

                    popup_x += abs_x;
                    popup_y += abs_y;
                }
            }
#endif
        }
    }

    *out_x = popup_x;
    *out_y = popup_y;
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

    /* 매 키 입력 전 surrounding text 갱신 (TypeFIX 등에 필요) */
    {
        gboolean handled = FALSE;
        g_signal_emit_by_name(context, "retrieve-surrounding", &handled);
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
        keyval == GDK_KEY_Hyper_L || keyval == GDK_KEY_Hyper_R ||
        keyval == GDK_KEY_Caps_Lock || keyval == GDK_KEY_Num_Lock ||
        keyval == GDK_KEY_Scroll_Lock ||
        keyval == GDK_KEY_ISO_Level3_Shift) {
        return FALSE;
    }

    /* 한자 팝업이 표시 중이면 먼저 팝업에서 키 처리 */
    if (unim->hanja_popup && unim_hanja_popup_is_visible(unim->hanja_popup)) {
        /* Escape → 조합 복원 + 팝업 닫기 */
        if (keyval == GDK_KEY_Escape) {
            UNIM_DEBUG("한자 팝업 Escape -> 조합 복원 + 팝업 닫기");

            /* ProcessKey(0,0,0)로 엔진 리셋 → preedit/commit 응답 받기 */
            UnimDbusKeyResult reset_result;
            if (unim_dbus_process_key(unim->dbus_ctx, 0, 0, 0, &reset_result)) {
                /* 커밋 텍스트가 있으면 커밋 */
                if (reset_result.commit && strlen(reset_result.commit) > 0) {
                    g_signal_emit_by_name(context, "commit", reset_result.commit);
                }
                g_free(reset_result.preedit);
                g_free(reset_result.commit);
            }

            /* CancelHanja → 한자 모드 해제 */
            unim_dbus_cancel_hanja(unim->dbus_ctx);

            /* preedit-changed 시그널 (preedit 복원) */
            g_signal_emit_by_name(context, "preedit-changed");

            /* 팝업 닫기 */
            unim_hanja_popup_hide(unim->hanja_popup);
            return TRUE;
        }

        /* 팝업 내부 처리 (숫자 선택, 네비게이션, 모디파이어 등) */
        if (unim_hanja_popup_handle_key(unim->hanja_popup, keyval)) {
            return TRUE;
        }

        /* 미지원 키 → 조합 커밋 + 팝업 닫기 + fall-through (엔진에 키 전달) */
        UNIM_DEBUG("한자 팝업 미지원 키 -> 조합 커밋 + 팝업 닫고 엔진에 키 전달");

        /* 1. FocusOut으로 조합 중 한글 커밋 */
        gchar *commit_text = NULL;
        unim_dbus_focus_out(unim->dbus_ctx, &commit_text);
        if (commit_text && strlen(commit_text) > 0) {
            UNIM_DEBUG("조합 커밋: \"%s\"", commit_text);
            g_signal_emit_by_name(context, "commit", commit_text);
        }
        g_free(commit_text);

        /* preedit 클리어 */
        g_signal_emit_by_name(context, "preedit-changed");

        /* 2. CancelHanja + 트리거 커밋 + 팝업 닫기 */
        {
            gchar *trigger = unim_dbus_cancel_hanja(unim->dbus_ctx);
            if (trigger) {
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
        unim_hanja_popup_hide(unim->hanja_popup);

        /* 3. FocusIn으로 컨텍스트 복원 (FocusOut 후 필요) */
        unim_dbus_focus_in(unim->dbus_ctx, unim->window_id);

        /* fall-through → 아래 ProcessKey 경로에서 엔진이 새 키 처리 */
    }

    /* 특수문자 팝업이 표시 중이면 먼저 팝업에서 키 처리 */
    if (unim->special_popup && unim_special_popup_is_visible(unim->special_popup)) {
        /* Escape → 기존 자모 커밋 + 특수문자 모드 취소 + 팝업 닫기 */
        if (keyval == GDK_KEY_Escape) {
            UNIM_DEBUG("특수문자 팝업 Escape -> 자모 커밋 + 모드 취소 + 팝업 닫기");

            /* ProcessKey(0,0,0)으로 엔진 리셋 → 조합 중 자모 커밋 */
            UnimDbusKeyResult reset_result;
            if (unim_dbus_process_key(unim->dbus_ctx, 0, 0, 0, &reset_result)) {
                if (reset_result.commit && strlen(reset_result.commit) > 0) {
                    g_signal_emit_by_name(context, "commit", reset_result.commit);
                }
                g_free(reset_result.preedit);
                g_free(reset_result.commit);
            }

            unim_dbus_cancel_special_char(unim->dbus_ctx);
            g_signal_emit_by_name(context, "preedit-changed");
            unim_special_popup_hide(unim->special_popup);
            return TRUE;
        }

        /* 팝업 내부 처리 */
        if (unim_special_popup_handle_key(unim->special_popup, keyval)) {
            return TRUE;
        }

        /* 미지원 키 → 특수문자 취소 + 트리거 커밋 + fall-through */
        UNIM_DEBUG("특수문자 팝업 미지원 키 -> 모드 취소 + 엔진에 키 전달");
        {
            gchar *trigger = unim_dbus_cancel_special_char(unim->dbus_ctx);
            if (trigger) {
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
        unim_special_popup_hide(unim->special_popup);
        g_signal_emit_by_name(context, "preedit-changed");

        /* fall-through → 아래 ProcessKey 경로에서 엔진이 새 키 처리 */
    }

    /* 한자 키 처리 (설정 기반) */
    gboolean is_hanja = FALSE;
    for (gsize i = 0; i < unim->n_hanja_keysyms; i++) {
        if (keyval == unim->hanja_keysyms[i]) {
            is_hanja = TRUE;
            break;
        }
    }
    if (is_hanja) {
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
                
                /* 화면 좌표 계산 */
                gint popup_x, popup_y;
                calculate_popup_position(unim, &popup_x, &popup_y);
                
                /* 팝업 표시 (Standalone 모드면 건너뜀) */
                if (!is_standalone_popup(unim->dbus_ctx)) {
                    unim_hanja_popup_show(
                        unim->hanja_popup,
                        target,
                        candidates,
                        count,
                        popup_x,
                        popup_y,
                        unim->cursor_area.height,
                        on_hanja_selected,
                        unim
                    );
                }
                
                UNIM_DEBUG("한자 후보 표시: target='%s', count=%zu", target, count);
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
                
                if (unim_dbus_get_special_char_candidates(unim->dbus_ctx,
                        &sp_target, &sp_chars, &sp_count, &sp_top_row) && sp_count > 0) {
                    /* 이전 특수문자 정리 */
                    if (unim->special_characters) {
                        unim_special_chars_free(unim->special_characters, unim->special_count);
                    }
                    unim->special_characters = sp_chars;
                    unim->special_count = sp_count;
                    
                    /* 화면 좌표 계산 */
                    gint popup_x, popup_y;
                    calculate_popup_position(unim, &popup_x, &popup_y);
                    
                    /* 특수문자 팝업 표시 (Standalone 모드면 건너뜀) */
                    if (!is_standalone_popup(unim->dbus_ctx)) {
                        unim_special_popup_show(
                            unim->special_popup,
                            sp_target,
                            sp_chars,
                            sp_count,
                            sp_top_row,
                            popup_x,
                            popup_y,
                            unim->cursor_area.height,
                            on_special_char_selected,
                            unim
                        );
                    }
                    
                    UNIM_DEBUG("특수문자 후보 표시: target='%s', count=%zu, top_row='%s'",
                               sp_target, sp_count, sp_top_row ? sp_top_row : "N/A");
                } else {
                    UNIM_DEBUG("특수문자 후보도 없음");
                    if (sp_chars) {
                        unim_special_chars_free(sp_chars, sp_count);
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

    /* GDK keycode = X11 keycode = evdev + 8 */
    guint evdev_code = (keycode > 8) ? (keycode - 8) : 0;
    
    /* 디버그 로그 (바이패스 전) */
    UNIM_DEBUG("키 입력: keyval=%u, keycode=%u, evdev=%u, state=0x%x, composing=%d",
               keyval, keycode, evdev_code, (guint)state, unim_dbus_is_composing(unim->dbus_ctx));

    /* 조합 중이 아닌 경우, 특수키는 IM에서 처리하지 않고 앱으로 직접 전달 */
    /* (블랙리스트 방식: GTK3과 동일) */
    if (!unim_dbus_is_composing(unim->dbus_ctx)) {
        /* 기능키 (F1~F12, 단 F9은 한자키로 위에서 처리됨) */
        if (keyval >= GDK_KEY_F1 && keyval <= GDK_KEY_F12) {
            return FALSE;
        }
        /* 방향키 */
        if (keyval >= GDK_KEY_Left && keyval <= GDK_KEY_Down) {
            return FALSE;
        }
        /* 네비게이션 키 */
        if (keyval == GDK_KEY_Home || keyval == GDK_KEY_End ||
            keyval == GDK_KEY_Page_Up || keyval == GDK_KEY_Page_Down ||
            keyval == GDK_KEY_Insert || keyval == GDK_KEY_Delete) {
            return FALSE;
        }
        /* Escape (조합 중이 아니면 앱으로) */
        if (keyval == GDK_KEY_Escape) {
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

    /* TypeFIX 더블탭 결과 처리 (delete-surrounding + commit) */
    if (result.typefix_delete != 0 && result.typefix_replacement) {
        UNIM_DEBUG("TypeFIX: delete=%d, replacement=\"%s\"",
                   result.typefix_delete, result.typefix_replacement);
        g_signal_emit_by_name(context, "delete-surrounding",
                              result.typefix_delete, (guint)(-result.typefix_delete));
        g_signal_emit_by_name(context, "commit", result.typefix_replacement);
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
    g_free(result.typefix_replacement);

    return result.consumed;
}

/* GtkInputPurpose → UNIM ContentPurpose 변환 */
static guint
gtk_input_purpose_to_unim(GtkInputPurpose purpose)
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

        /* 입력 필드 목적 감지 및 전달 */
        if (unim->client_widget) {
            GtkInputPurpose purpose = GTK_INPUT_PURPOSE_FREE_FORM;
            if (GTK_IS_TEXT(unim->client_widget)) {
                g_object_get(unim->client_widget, "input-purpose", &purpose, NULL);
            } else if (GTK_IS_EDITABLE(unim->client_widget)) {
                GtkWidget *delegate = unim->client_widget;
                /* GtkEditable에서 input-purpose 속성 확인 */
                GParamSpec *pspec = g_object_class_find_property(
                    G_OBJECT_GET_CLASS(delegate), "input-purpose");
                if (pspec) {
                    g_object_get(delegate, "input-purpose", &purpose, NULL);
                }
            }
            guint unim_purpose = gtk_input_purpose_to_unim(purpose);
            unim_dbus_set_content_type(unim->dbus_ctx, unim_purpose);
            UNIM_DEBUG("content_type 전달: gtk_purpose=%d, unim_purpose=%u",
                       (int)purpose, unim_purpose);
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

    /* 1. 한자/특수문자 팝업이 표시 중이면 먼저 닫기 + 트리거 문자 커밋 */
    if (unim->hanja_popup && unim_hanja_popup_is_visible(unim->hanja_popup)) {
        UNIM_DEBUG("focus_out: 한자 팝업 닫기");
        unim_hanja_popup_hide(unim->hanja_popup);
        if (unim->dbus_ctx) {
            gchar *trigger = unim_dbus_cancel_hanja(unim->dbus_ctx);
            if (trigger) {
                UNIM_DEBUG("focus_out: 한자 트리거 커밋 '%s'", trigger);
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
    }
    if (unim->special_popup && unim_special_popup_is_visible(unim->special_popup)) {
        UNIM_DEBUG("focus_out: 특수문자 팝업 닫기");
        unim_special_popup_hide(unim->special_popup);
        if (unim->dbus_ctx) {
            gchar *trigger = unim_dbus_cancel_special_char(unim->dbus_ctx);
            if (trigger) {
                UNIM_DEBUG("focus_out: 특수문자 트리거 커밋 '%s'", trigger);
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
    }

    /* 2. 조합 중인 글자를 커밋 */
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
        /* 1. 조합 중인 글자를 먼저 커밋 */
        unim_dbus_reset(unim->dbus_ctx, &commit);

        if (commit && strlen(commit) > 0) {
            UNIM_DEBUG("reset 커밋: \"%s\"", commit);
            g_signal_emit_by_name(context, "commit", commit);
        }
        g_free(commit);

        g_signal_emit_by_name(context, "preedit-changed");
    }

    /* 2. 한자 팝업이 표시 중이면 닫기 + 트리거 커밋 */
    if (unim->hanja_popup && unim_hanja_popup_is_visible(unim->hanja_popup)) {
        UNIM_DEBUG("reset: 한자 팝업 닫기");
        unim_hanja_popup_hide(unim->hanja_popup);
        if (unim->dbus_ctx) {
            gchar *trigger = unim_dbus_cancel_hanja(unim->dbus_ctx);
            if (trigger) {
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
    }

    /* 3. 특수문자 팝업이 표시 중이면 닫기 + 트리거 커밋 */
    if (unim->special_popup && unim_special_popup_is_visible(unim->special_popup)) {
        UNIM_DEBUG("reset: 특수문자 팝업 닫기");
        unim_special_popup_hide(unim->special_popup);
        if (unim->dbus_ctx) {
            gchar *trigger = unim_dbus_cancel_special_char(unim->dbus_ctx);
            if (trigger) {
                g_signal_emit_by_name(context, "commit", trigger);
                g_free(trigger);
            }
        }
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
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    
    if (area) {
        unim->cursor_area = *area;

        /* 커서 위치를 데몬에 보고 (팝업 포지셔닝용) */
        if (unim->dbus_ctx) {
            gint abs_x, abs_y;
            calculate_popup_position(unim, &abs_x, &abs_y);
            /* abs_y는 cursor_area.y + height로 계산되므로, 원래 y를 복원 */
            abs_y -= area->height;
            unim_dbus_report_cursor_rect(unim->dbus_ctx,
                                          abs_x, abs_y,
                                          area->width, area->height);
        }
    }
}

static void
unim_im_context_set_client_widget(GtkIMContext *context, GtkWidget *widget)
{
    UnimIMContext *unim = UNIM_IM_CONTEXT(context);
    unim->client_widget = widget;
    UNIM_DEBUG("client_widget 설정: %p", (void*)widget);
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

    /* Surrounding text를 DBus로 전달 */
    if (unim->dbus_ctx && unim->surrounding_text) {
        /* 바이트 오프셋 → 문자 오프셋 변환 */
        guint cursor_char = (guint)g_utf8_pointer_to_offset(
            unim->surrounding_text, unim->surrounding_text + cursor_index);
        guint anchor_char = (guint)g_utf8_pointer_to_offset(
            unim->surrounding_text, unim->surrounding_text + selection_index);
        unim_dbus_set_surrounding_text(unim->dbus_ctx,
                                        unim->surrounding_text,
                                        cursor_char, anchor_char);
    }
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
