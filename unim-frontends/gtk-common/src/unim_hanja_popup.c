/**
 * UNIM 한자 후보 팝업 구현
 *
 * 한자 변환 시 후보 목록을 표시하는 팝업 윈도우입니다.
 */

#include "unim_hanja_popup.h"
#include "unim.h"
#include <string.h>

/* X11 override_redirect 설정을 위한 헤더 */
#ifdef GDK_WINDOWING_X11
#if GTK_MAJOR_VERSION >= 4
#include <gdk/x11/gdkx.h>
#else
#include <gdk/gdkx.h>
#endif
#include <X11/Xlib.h>
#endif

/* 최대 표시 후보 수 (한 페이지) */
#define MAX_VISIBLE_CANDIDATES 9

/* 디버그 로깅 */
#include <stdio.h>
#include <stdarg.h>
#include <time.h>

static gboolean unim_popup_debug_enabled = FALSE;
static gboolean unim_popup_debug_checked = FALSE;

static void
unim_popup_log_message(const char *module, const char *format, ...)
{
    if (!unim_popup_debug_enabled) return;

    va_list args;
    char message[1024];
    char timestamp[32];
    char log_line[2048];
    time_t now;
    struct tm *tm_info;

    va_start(args, format);
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);

    time(&now);
    tm_info = localtime(&now);
    strftime(timestamp, sizeof(timestamp), "%Y/%m/%d %H:%M:%S", tm_info);

    snprintf(log_line, sizeof(log_line), "[%s] - [%s] - %s", timestamp, module, message);
    g_print("%s\n", log_line);
}

#define POPUP_DEBUG(fmt, ...) \
    unim_popup_log_message("HANJA_POPUP", fmt, ##__VA_ARGS__)

static void
unim_popup_check_debug_env(void)
{
    if (!unim_popup_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            unim_popup_debug_enabled = TRUE;
        }
        unim_popup_debug_checked = TRUE;
    }
}

/* GTK3/4 호환 CSS class 관리 매크로 */
#if GTK_CHECK_VERSION(4, 0, 0)
#define WIDGET_ADD_CSS_CLASS(w, cls)    gtk_widget_add_css_class(w, cls)
#define WIDGET_REMOVE_CSS_CLASS(w, cls) gtk_widget_remove_css_class(w, cls)
#else
static void
hanja_widget_add_css_class(GtkWidget *w, const char *cls) {
    GtkStyleContext *ctx = gtk_widget_get_style_context(w);
    gtk_style_context_add_class(ctx, cls);
}
#define WIDGET_ADD_CSS_CLASS(w, cls)    hanja_widget_add_css_class(w, cls)
#endif

/* 내부 구조체 */
struct _UnimHanjaPopup {
    GtkWidget *window;           /* 팝업 윈도우 */
    GtkWidget *listbox;          /* 후보 리스트 */
    GtkWidget *page_label;       /* 페이지 표시 */
    
    UnimHanjaCandidate *candidates;  /* 후보 배열 */
    gsize count;                     /* 전체 후보 개수 */
    gsize current_page;              /* 현재 페이지 (0부터 시작) */
    gint selected_index;             /* 현재 선택 인덱스 (페이지 내) */
    
    UnimPopupState *popup_state;     /* C-API popup state */

    UnimHanjaSelectCallback callback;
    gpointer user_data;
};

/* 현재 페이지의 후보 개수 반환 */
static gsize
get_page_candidate_count(UnimHanjaPopup *popup)
{
    gsize start = popup->current_page * MAX_VISIBLE_CANDIDATES;
    gsize remaining = popup->count - start;
    return (remaining > MAX_VISIBLE_CANDIDATES) ? MAX_VISIBLE_CANDIDATES : remaining;
}

/* 전체 페이지 수 반환 */
static gsize
get_total_pages(UnimHanjaPopup *popup)
{
    return (popup->count + MAX_VISIBLE_CANDIDATES - 1) / MAX_VISIBLE_CANDIDATES;
}

/* 리스트 갱신 */
static void
update_listbox(UnimHanjaPopup *popup)
{
    if (!popup || !popup->listbox) return;

    /* 기존 아이템 제거 */
#if GTK_CHECK_VERSION(4, 0, 0)
    GtkWidget *child;
    while ((child = gtk_widget_get_first_child(popup->listbox)) != NULL) {
        gtk_list_box_remove(GTK_LIST_BOX(popup->listbox), child);
    }
#else
    GList *children = gtk_container_get_children(GTK_CONTAINER(popup->listbox));
    for (GList *l = children; l != NULL; l = l->next) {
        gtk_container_remove(GTK_CONTAINER(popup->listbox), GTK_WIDGET(l->data));
    }
    g_list_free(children);
#endif

    /* 현재 페이지 후보 추가 */
    gsize start = popup->current_page * MAX_VISIBLE_CANDIDATES;
    gsize page_count = get_page_candidate_count(popup);

    for (gsize i = 0; i < page_count; i++) {
        gsize idx = start + i;
        UnimHanjaCandidate *cand = &popup->candidates[idx];
        
        /* 라벨 생성: "1. 漢 한자" */
        gchar *label_text = g_strdup_printf("%zu. %s  %s", 
                                             i + 1, 
                                             cand->hanja, 
                                             cand->meaning ? cand->meaning : "");
        
        GtkWidget *label = gtk_label_new(label_text);
        gtk_label_set_xalign(GTK_LABEL(label), 0.0);
        g_free(label_text);

#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_list_box_append(GTK_LIST_BOX(popup->listbox), label);
#else
        gtk_container_add(GTK_CONTAINER(popup->listbox), label);
        gtk_widget_show(label);
#endif
    }

    /* 페이지 라벨 업데이트 */
    if (popup->page_label && get_total_pages(popup) > 1) {
        gchar *page_text = g_strdup_printf("%zu / %zu", 
                                            popup->current_page + 1, 
                                            get_total_pages(popup));
        gtk_label_set_text(GTK_LABEL(popup->page_label), page_text);
        g_free(page_text);
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->page_label, TRUE);
#else
        gtk_widget_show(popup->page_label);
#endif
    } else if (popup->page_label) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->page_label, FALSE);
#else
        gtk_widget_hide(popup->page_label);
#endif
    }

    /* 선택 업데이트 */
    if (popup->selected_index >= 0 && popup->selected_index < (gint)page_count) {
        GtkListBoxRow *row = gtk_list_box_get_row_at_index(
            GTK_LIST_BOX(popup->listbox), popup->selected_index);
        if (row) {
            gtk_list_box_select_row(GTK_LIST_BOX(popup->listbox), row);
        }
    }
}

/* 리스트 아이템 선택 콜백 */
static void
on_row_activated(GtkListBox *listbox, GtkListBoxRow *row, gpointer user_data)
{
    UnimHanjaPopup *popup = (UnimHanjaPopup *)user_data;
    
    if (!popup || !row || !popup->callback) return;

    gint index = gtk_list_box_row_get_index(row);
    gsize actual_index = popup->current_page * MAX_VISIBLE_CANDIDATES + index;

    if (actual_index < popup->count) {
        const gchar *hanja = popup->candidates[actual_index].hanja;
        POPUP_DEBUG("한자 선택 (클릭): index=%zu, hanja='%s'", actual_index, hanja);
        popup->callback(hanja, popup->user_data);
    }
}

/* 우클릭 → 다음 페이지 전환 */
#if GTK_CHECK_VERSION(4, 0, 0)
static void
on_listbox_right_click(GtkGestureClick *gesture, gint n_press,
                       gdouble x, gdouble y, gpointer user_data)
{
    (void)n_press; (void)x; (void)y;
    UnimHanjaPopup *popup = (UnimHanjaPopup *)user_data;
    if (!popup) return;

    guint button = gtk_gesture_single_get_current_button(GTK_GESTURE_SINGLE(gesture));
    if (button == 3) { /* 우클릭 */
        gsize total = get_total_pages(popup);
        if (total > 1) {
            if (popup->current_page < total - 1) {
                popup->current_page++;
            } else {
                popup->current_page = 0;
            }
            popup->selected_index = 0;
            update_listbox(popup);
        }
        POPUP_DEBUG("우클릭 → 다음 페이지: %zu/%zu", popup->current_page + 1, total);
    }
}
#else
static gboolean
on_listbox_button_press(GtkWidget *widget, GdkEventButton *event, gpointer user_data)
{
    (void)widget;
    UnimHanjaPopup *popup = (UnimHanjaPopup *)user_data;
    if (!popup) return FALSE;

    if (event->button == 3) { /* 우클릭 */
        gsize total = get_total_pages(popup);
        if (total > 1) {
            if (popup->current_page < total - 1) {
                popup->current_page++;
            } else {
                popup->current_page = 0;
            }
            popup->selected_index = 0;
            update_listbox(popup);
        }
        POPUP_DEBUG("우클릭 → 다음 페이지: %zu/%zu", popup->current_page + 1, total);
        return TRUE;
    }
    return FALSE;
}
#endif

/* X11에서 override_redirect 설정 (포커스 방지) */
#ifdef GDK_WINDOWING_X11
#if GTK_CHECK_VERSION(4, 0, 0)
static void
on_popup_realize_x11(GtkWidget *widget, gpointer user_data)
{
    (void)user_data;
    GdkSurface *surface = gtk_native_get_surface(GTK_NATIVE(widget));
    if (surface && GDK_IS_X11_SURFACE(surface)) {
        Display *xdisplay = gdk_x11_display_get_xdisplay(
            gdk_surface_get_display(surface));
        Window xwindow = gdk_x11_surface_get_xid(surface);
        
        XSetWindowAttributes attrs;
        attrs.override_redirect = True;
        XChangeWindowAttributes(xdisplay, xwindow, CWOverrideRedirect, &attrs);
        
        POPUP_DEBUG("X11 override_redirect 설정 완료");
    }
}
#endif
#endif

UnimHanjaPopup*
unim_hanja_popup_new(void)
{
    UnimHanjaPopup *popup;

    unim_popup_check_debug_env();

    popup = g_new0(UnimHanjaPopup, 1);

#if GTK_CHECK_VERSION(4, 0, 0)
    /* GTK4: 포커스를 가져가지 않는 팝업 윈도우 */
    popup->window = gtk_window_new();
    gtk_window_set_decorated(GTK_WINDOW(popup->window), FALSE);
    gtk_window_set_resizable(GTK_WINDOW(popup->window), FALSE);
    gtk_window_set_default_size(GTK_WINDOW(popup->window), 300, -1);
    
    /* GTK4 수준 포커스 비활성화 */
    gtk_widget_set_focusable(popup->window, FALSE);
    gtk_widget_set_can_focus(popup->window, FALSE);
    
    /* X11에서 override_redirect 설정 (realize 후) */
#ifdef GDK_WINDOWING_X11
    g_signal_connect(popup->window, "realize", G_CALLBACK(on_popup_realize_x11), NULL);
#endif

#else
    /* GTK3: 팝업 윈도우 (포커스 불가) */
    popup->window = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_type_hint(GTK_WINDOW(popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
    gtk_widget_set_can_focus(popup->window, FALSE);
#endif

    /* Catppuccin Mocha 스타일 */
    {
        GtkCssProvider *css = gtk_css_provider_new();
        const gchar *css_text =
            "window.unim-hanja-popup {"
            "  background-color: rgba(30, 30, 46, 0.95);"
            "  border: 1px solid rgba(255, 255, 255, 0.15);"
            "  border-radius: 12px;"
            "  padding: 12px;"
            "}"
            ".unim-hanja-vbox {"
            "  padding: 0; margin: 0;"
            "}"
            ".unim-hanja-vbox list {"
            "  background: transparent;"
            "  border-radius: 6px;"
            "}"
            ".unim-hanja-vbox list row {"
            "  background: transparent;"
            "  border-radius: 6px;"
            "  min-height: 28px;"
            "  padding: 0 8px;"
            "}"
            ".unim-hanja-vbox list row:selected {"
            "  background-color: rgba(137, 180, 250, 0.2);"
            "}"
            ".unim-hanja-vbox list row label {"
            "  color: #cdd6f4;"
            "  font-size: 14px;"
            "}"
            ".unim-hanja-vbox list row:selected label {"
            "  color: #cdd6f4;"
            "}"
            ".unim-hanja-vbox label.page-label {"
            "  color: #6c7086;"
            "  font-size: 12px;"
            "  padding: 2px 0;"
            "}";
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_css_provider_load_from_string(css, css_text);
        gtk_style_context_add_provider_for_display(
            gdk_display_get_default(),
            GTK_STYLE_PROVIDER(css),
            GTK_STYLE_PROVIDER_PRIORITY_USER
        );
#else
        gtk_css_provider_load_from_data(css, css_text, -1, NULL);
        gtk_style_context_add_provider_for_screen(
            gdk_screen_get_default(),
            GTK_STYLE_PROVIDER(css),
            GTK_STYLE_PROVIDER_PRIORITY_USER
        );
#endif
        g_object_unref(css);
    }

    /* CSS 클래스 적용 */
    WIDGET_ADD_CSS_CLASS(popup->window, "unim-hanja-popup");

    /* 메인 박스 */
    GtkWidget *vbox;
#if GTK_CHECK_VERSION(4, 0, 0)
    vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    gtk_window_set_child(GTK_WINDOW(popup->window), vbox);
#else
    vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    gtk_container_add(GTK_CONTAINER(popup->window), vbox);
#endif
    WIDGET_ADD_CSS_CLASS(vbox, "unim-hanja-vbox");

    /* 리스트박스 */
    popup->listbox = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(popup->listbox), GTK_SELECTION_SINGLE);
    gtk_list_box_set_activate_on_single_click(GTK_LIST_BOX(popup->listbox), TRUE);
    g_signal_connect(popup->listbox, "row-activated", G_CALLBACK(on_row_activated), popup);
    
    /* 우클릭 핸들러 */
#if GTK_CHECK_VERSION(4, 0, 0)
    {
        GtkGesture *right_click = gtk_gesture_click_new();
        gtk_gesture_single_set_button(GTK_GESTURE_SINGLE(right_click), 0); /* 모든 버튼 */
        g_signal_connect(right_click, "pressed", G_CALLBACK(on_listbox_right_click), popup);
        gtk_widget_add_controller(popup->listbox, GTK_EVENT_CONTROLLER(right_click));
    }
#else
    g_signal_connect(popup->listbox, "button-press-event", G_CALLBACK(on_listbox_button_press), popup);
#endif

    /* 리스트박스도 포커스 불가 */
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_focusable(popup->listbox, FALSE);
#endif
    gtk_widget_set_can_focus(popup->listbox, FALSE);

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->listbox);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->listbox, TRUE, TRUE, 0);
#endif

    /* 페이지 라벨 */
    popup->page_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->page_label), 0.5);
    WIDGET_ADD_CSS_CLASS(popup->page_label, "page-label");

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->page_label);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->page_label, FALSE, FALSE, 2);
#endif

    popup->selected_index = 0;

    POPUP_DEBUG("한자 팝업 생성");

    return popup;
}

void
unim_hanja_popup_free(UnimHanjaPopup *popup)
{
    if (!popup) return;

    if (popup->popup_state) {
        unim_popup_free(popup->popup_state);
        popup->popup_state = NULL;
    }

    if (popup->window) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_window_destroy(GTK_WINDOW(popup->window));
#else
        gtk_widget_destroy(popup->window);
#endif
    }

    g_free(popup);
    POPUP_DEBUG("한자 팝업 해제");
}

void
unim_hanja_popup_show(UnimHanjaPopup *popup,
                       const gchar *target,
                       UnimHanjaCandidate *candidates,
                       gsize count,
                       gint x,
                       gint y,
                       gint cursor_height,
                       UnimHanjaSelectCallback callback,
                       gpointer user_data)
{
    if (!popup || !candidates || count == 0) return;

    popup->candidates = candidates;
    popup->count = count;
    popup->current_page = 0;
    popup->selected_index = 0;
    popup->callback = callback;
    popup->user_data = user_data;

    /* Build arrays for C-API PopupState */
    {
        const uint8_t **hanja_ptrs = g_malloc(sizeof(uint8_t*) * count);
        size_t *hanja_lens = g_malloc(sizeof(size_t) * count);
        const uint8_t **meaning_ptrs = g_malloc(sizeof(uint8_t*) * count);
        size_t *meaning_lens = g_malloc(sizeof(size_t) * count);
        for (gsize i = 0; i < count; i++) {
            hanja_ptrs[i] = (const uint8_t *)candidates[i].hanja;
            hanja_lens[i] = strlen(candidates[i].hanja);
            meaning_ptrs[i] = (const uint8_t *)candidates[i].meaning;
            meaning_lens[i] = candidates[i].meaning ? strlen(candidates[i].meaning) : 0;
        }
        if (popup->popup_state) unim_popup_free(popup->popup_state);
        popup->popup_state = unim_popup_new_hanja(
            (const uint8_t *)target, strlen(target),
            hanja_ptrs, hanja_lens, meaning_ptrs, meaning_lens, count
        );
        g_free(hanja_ptrs); g_free(hanja_lens); g_free(meaning_ptrs); g_free(meaning_lens);
    }

    update_listbox(popup);

    /* 팝업 크기 측정 및 화면 경계 보정 */
    gint popup_w = 0, popup_h = 0;
    gint final_x = x, final_y = y;

#if GTK_CHECK_VERSION(4, 0, 0)
    /* GTK4: realize하여 X11 윈도우를 생성하되 아직 표시하지 않음 */
    gtk_widget_realize(popup->window);

    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    popup_w = req.width;
    popup_h = req.height;

    /* 화면 크기 가져오기 (X11) */
#ifdef GDK_WINDOWING_X11
    {
        GdkSurface *surface = gtk_native_get_surface(GTK_NATIVE(popup->window));
        if (surface && GDK_IS_X11_SURFACE(surface)) {
            Display *xdisplay = gdk_x11_display_get_xdisplay(
                gdk_surface_get_display(surface));
            Window xwindow = gdk_x11_surface_get_xid(surface);
            int screen_num = DefaultScreen(xdisplay);
            gint screen_w = DisplayWidth(xdisplay, screen_num);
            gint screen_h = DisplayHeight(xdisplay, screen_num);

            /* 오른쪽 넘침 보정 */
            if (final_x + popup_w > screen_w) {
                final_x = screen_w - popup_w;
                if (final_x < 0) final_x = 0;
            }
            /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 */
            if (final_y + popup_h > screen_h) {
                /* y는 커서 아래쪽이므로, 커서 위로: y - cursor_height - popup_h */
                final_y = y - cursor_height - popup_h;
                if (final_y < 0) final_y = 0;
            }

            POPUP_DEBUG("화면 경계 보정: screen=(%d,%d), popup=(%d,%d), req=(%d,%d) -> final=(%d,%d)",
                         screen_w, screen_h, x, y, popup_w, popup_h, final_x, final_y);

            /* 위치 설정 후 표시 (GTK3과 동일 순서: move → show) */
            XMoveWindow(xdisplay, xwindow, final_x, final_y);
        }
    }
#endif

    /* 위치 설정 완료 후 마지막에 표시 */
    gtk_widget_set_visible(popup->window, TRUE);

#else
    /* GTK3: 초기 위치 설정 후 show_all (realize를 위해) */
    gtk_window_move(GTK_WINDOW(popup->window), final_x, final_y);
    gtk_widget_show_all(popup->window);

    /* show_all 후 정확한 크기 측정 가능 */
    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    popup_w = req.width;
    popup_h = req.height;

    /* 커서가 위치한 모니터 기준으로 화면 경계 보정 */
    {
        GdkScreen *screen = gtk_window_get_screen(GTK_WINDOW(popup->window));
        if (screen) {
            GdkDisplay *display = gdk_screen_get_display(screen);
            GdkMonitor *monitor = gdk_display_get_monitor_at_point(display, x, y);
            if (monitor) {
                GdkRectangle mon_geom;
                gdk_monitor_get_geometry(monitor, &mon_geom);

                /* 오른쪽 넘침 보정 */
                if (final_x + popup_w > mon_geom.x + mon_geom.width) {
                    final_x = mon_geom.x + mon_geom.width - popup_w;
                    if (final_x < mon_geom.x) final_x = mon_geom.x;
                }
                /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 */
                if (final_y + popup_h > mon_geom.y + mon_geom.height) {
                    final_y = y - cursor_height - popup_h;
                    if (final_y < mon_geom.y) final_y = mon_geom.y;
                }
            }

            POPUP_DEBUG("화면 경계 보정: popup=(%d,%d), req=(%d,%d) -> final=(%d,%d)",
                         x, y, popup_w, popup_h, final_x, final_y);
        }
    }

    gtk_window_move(GTK_WINDOW(popup->window), final_x, final_y);
#endif

    POPUP_DEBUG("한자 팝업 표시: target='%s', count=%zu, pos=(%d,%d)", 
                 target, count, final_x, final_y);
}

void
unim_hanja_popup_hide(UnimHanjaPopup *popup)
{
    if (!popup || !popup->window) return;

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_visible(popup->window, FALSE);
#else
    gtk_widget_hide(popup->window);
#endif

    popup->candidates = NULL;
    popup->count = 0;

    POPUP_DEBUG("한자 팝업 숨김");
}

gboolean
unim_hanja_popup_is_visible(UnimHanjaPopup *popup)
{
    if (!popup || !popup->window) return FALSE;
    return gtk_widget_is_visible(popup->window);
}

gboolean
unim_hanja_popup_handle_key(UnimHanjaPopup *popup, guint keyval)
{
    if (!popup || !popup->popup_state || !unim_hanja_popup_is_visible(popup)) return FALSE;

    uint32_t popup_key = unim_popup_key_from_gdk(keyval);
    UnimPopupKeyResult result = unim_popup_handle_key(popup->popup_state, popup_key);

    switch (result.kind) {
    case UNIM_POPUP_RESULT_SELECT:
        if (result.selected_index >= 0 && (gsize)result.selected_index < popup->count && popup->callback) {
            const gchar *hanja = popup->candidates[result.selected_index].hanja;
            POPUP_DEBUG("한자 선택 (C-API): index=%d, hanja='%s'", result.selected_index, hanja);
            popup->callback(hanja, popup->user_data);
            return TRUE;
        }
        return FALSE;

    case UNIM_POPUP_RESULT_CANCEL:
        return FALSE;

    case UNIM_POPUP_RESULT_UPDATED:
        popup->current_page = unim_popup_get_current_page(popup->popup_state);
        popup->selected_index = unim_popup_get_sel_row(popup->popup_state);
        update_listbox(popup);
        return TRUE;

    case UNIM_POPUP_RESULT_CONSUMED:
        return TRUE;

    case UNIM_POPUP_RESULT_NOT_HANDLED:
    default:
        return FALSE;
    }
}
