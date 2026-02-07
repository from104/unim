/**
 * UNIM 한자 후보 팝업 구현
 *
 * 한자 변환 시 후보 목록을 표시하는 팝업 윈도우입니다.
 */

#include "unim_hanja_popup.h"
#include <string.h>

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

/* 내부 구조체 */
struct _UnimHanjaPopup {
    GtkWidget *window;           /* 팝업 윈도우 */
    GtkWidget *listbox;          /* 후보 리스트 */
    GtkWidget *page_label;       /* 페이지 표시 */
    
    UnimHanjaCandidate *candidates;  /* 후보 배열 */
    gsize count;                     /* 전체 후보 개수 */
    gsize current_page;              /* 현재 페이지 (0부터 시작) */
    gint selected_index;             /* 현재 선택 인덱스 (페이지 내) */
    
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
    
    /* 포커스를 가져가지 않도록 설정 */
    gtk_widget_set_focusable(popup->window, FALSE);
    gtk_widget_set_can_focus(popup->window, FALSE);
#else
    /* GTK3: 팝업 윈도우 (포커스 불가) */
    popup->window = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_type_hint(GTK_WINDOW(popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
    gtk_widget_set_can_focus(popup->window, FALSE);
#endif

    /* 메인 박스 */
    GtkWidget *vbox;
#if GTK_CHECK_VERSION(4, 0, 0)
    vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    gtk_window_set_child(GTK_WINDOW(popup->window), vbox);
#else
    vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    gtk_container_add(GTK_CONTAINER(popup->window), vbox);
#endif

    /* 리스트박스 */
    popup->listbox = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(popup->listbox), GTK_SELECTION_SINGLE);
    g_signal_connect(popup->listbox, "row-activated", G_CALLBACK(on_row_activated), popup);
    
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

    update_listbox(popup);

#if GTK_CHECK_VERSION(4, 0, 0)
    /* GTK4: visible로 표시 (포커스 가져가지 않음) */
    gtk_widget_set_visible(popup->window, TRUE);
#else
    gtk_window_move(GTK_WINDOW(popup->window), x, y);
    gtk_widget_show_all(popup->window);
#endif

    POPUP_DEBUG("한자 팝업 표시: target='%s', count=%zu, pos=(%d,%d)", 
                 target, count, x, y);
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
    if (!popup || !unim_hanja_popup_is_visible(popup)) return FALSE;

    gsize page_count = get_page_candidate_count(popup);

    /* 숫자키 1-9 선택 */
    if (keyval >= GDK_KEY_1 && keyval <= GDK_KEY_9) {
        gsize idx = keyval - GDK_KEY_1;
        if (idx < page_count) {
            gsize actual_index = popup->current_page * MAX_VISIBLE_CANDIDATES + idx;
            if (actual_index < popup->count && popup->callback) {
                const gchar *hanja = popup->candidates[actual_index].hanja;
                POPUP_DEBUG("한자 선택 (숫자): index=%zu, hanja='%s'", actual_index, hanja);
                popup->callback(hanja, popup->user_data);
                return TRUE;
            }
        }
    }

    /* 위/아래 화살표 네비게이션 */
    if (keyval == GDK_KEY_Up) {
        if (popup->selected_index > 0) {
            popup->selected_index--;
            update_listbox(popup);
        }
        return TRUE;
    }

    if (keyval == GDK_KEY_Down) {
        if (popup->selected_index < (gint)page_count - 1) {
            popup->selected_index++;
            update_listbox(popup);
        }
        return TRUE;
    }

    /* 좌/우 화살표 페이지 전환 */
    if (keyval == GDK_KEY_Left || keyval == GDK_KEY_Page_Up) {
        if (popup->current_page > 0) {
            popup->current_page--;
            popup->selected_index = 0;
            update_listbox(popup);
        }
        return TRUE;
    }

    if (keyval == GDK_KEY_Right || keyval == GDK_KEY_Page_Down) {
        if (popup->current_page < get_total_pages(popup) - 1) {
            popup->current_page++;
            popup->selected_index = 0;
            update_listbox(popup);
        }
        return TRUE;
    }

    /* Enter로 현재 선택 확정 */
    if (keyval == GDK_KEY_Return || keyval == GDK_KEY_KP_Enter) {
        if (popup->selected_index >= 0 && popup->selected_index < (gint)page_count) {
            gsize actual_index = popup->current_page * MAX_VISIBLE_CANDIDATES + popup->selected_index;
            if (actual_index < popup->count && popup->callback) {
                const gchar *hanja = popup->candidates[actual_index].hanja;
                POPUP_DEBUG("한자 선택 (Enter): index=%zu, hanja='%s'", actual_index, hanja);
                popup->callback(hanja, popup->user_data);
                return TRUE;
            }
        }
    }

    /* Escape로 취소 */
    if (keyval == GDK_KEY_Escape) {
        unim_hanja_popup_hide(popup);
        return TRUE;
    }

    return FALSE;
}
