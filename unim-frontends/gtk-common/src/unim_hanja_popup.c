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

/* 표시 모드별 페이지 사이즈/격자 크기 (compact = 1×9, expanded = 9×9). */
#define MAX_VISIBLE_CANDIDATES 9
#define COMPACT_PAGE_SIZE      9
#define EXPANDED_PAGE_SIZE     81
#define EXPANDED_COLS          9
#define EXPANDED_ROWS          9
#define ICON_EXPAND            "\xE2\x8A\x9E"  /* ⊞ U+229E (compact 상태에서 표시) */
#define ICON_COMPACT           "\xE2\x8A\x9F"  /* ⊟ U+229F (expanded 상태에서 표시) */

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
    GtkWidget *header_label;     /* 헤더 (「X」 → 한자) */
    GtkWidget *body_container;   /* compact ListBox 또는 expanded Grid를 자식으로 보유 */
    GtkWidget *listbox;          /* compact 모드 ListBox (활성일 때만 비-NULL) */
    GtkWidget *grid;             /* expanded 모드 Grid (활성일 때만 비-NULL) */
    GtkWidget *footer_box;       /* page_label + expand_icon 가로 배치 */
    GtkWidget *page_label;       /* 페이지 표시 */
    GtkWidget *expand_icon;      /* ⊞/⊟ 모드 표시 라벨 */

    UnimHanjaCandidate *candidates;  /* 후보 배열 */
    gsize count;                     /* 전체 후보 개수 */
    gboolean *bookmarks;             /* 후보별 즐겨찾기 상태 (count 길이) */
    gsize current_page;              /* 현재 페이지 (0부터 시작) */
    gint selected_index;             /* compact 호환 — sel_row와 동일 의미 */
    gint sel_row;                    /* 현재 선택 행 (페이지 내, 0-based) */
    gint sel_col;                    /* 현재 선택 열 (compact=0, expanded=0..8) */
    gsize cols;                      /* 1=compact, >1=expanded (엔진 cols 미러) */

    UnimPopupState *popup_state;     /* C-API popup state */

    UnimHanjaSelectCallback callback;
    gpointer user_data;

    /* Space 토글 콜백 (프런트엔드 → DBus 호출) */
    UnimHanjaToggleBookmarkCallback toggle_bookmark_callback;
    gpointer toggle_bookmark_user_data;
};

/* 현재 모드의 페이지 사이즈 (compact=9, expanded=81) */
static gsize
get_page_size(UnimHanjaPopup *popup)
{
    return (popup->cols > 1) ? EXPANDED_PAGE_SIZE : COMPACT_PAGE_SIZE;
}

/* 현재 페이지의 후보 개수 반환 */
static gsize
get_page_candidate_count(UnimHanjaPopup *popup)
{
    gsize page_size = get_page_size(popup);
    gsize start = popup->current_page * page_size;
    if (start >= popup->count) return 0;
    gsize remaining = popup->count - start;
    return (remaining > page_size) ? page_size : remaining;
}

/* 전체 페이지 수 반환 */
static gsize
get_total_pages(UnimHanjaPopup *popup)
{
    gsize page_size = get_page_size(popup);
    if (popup->count == 0) return 1;
    return (popup->count + page_size - 1) / page_size;
}

/* compact↔expanded 전환을 위해 body 컨테이너의 자식(listbox/grid)을 모두 제거. */
static void
clear_body_children(UnimHanjaPopup *popup)
{
    if (!popup || !popup->body_container) return;
#if GTK_CHECK_VERSION(4, 0, 0)
    GtkWidget *child;
    while ((child = gtk_widget_get_first_child(popup->body_container)) != NULL) {
        gtk_box_remove(GTK_BOX(popup->body_container), child);
    }
#else
    GList *children = gtk_container_get_children(GTK_CONTAINER(popup->body_container));
    for (GList *l = children; l != NULL; l = l->next) {
        gtk_container_remove(GTK_CONTAINER(popup->body_container), GTK_WIDGET(l->data));
    }
    g_list_free(children);
#endif
    popup->listbox = NULL;
    popup->grid = NULL;
}

/* 전방 선언 (mode dispatcher) */
static void render_compact_list(UnimHanjaPopup *popup);
static void render_expanded_grid(UnimHanjaPopup *popup);
static void on_grid_cell_clicked(GtkButton *button, gpointer user_data);
static void on_row_activated(GtkListBox *listbox, GtkListBoxRow *row, gpointer user_data);
#if GTK_CHECK_VERSION(4, 0, 0)
static void on_listbox_right_click(GtkGestureClick *gesture, gint n_press,
                                    gdouble x, gdouble y, gpointer user_data);
#else
static gboolean on_listbox_button_press(GtkWidget *widget, GdkEventButton *event,
                                         gpointer user_data);
#endif

/* 리스트 갱신 — compact / expanded 모드를 cols로 분기. */
static void
update_listbox(UnimHanjaPopup *popup)
{
    if (!popup || !popup->body_container) return;

    clear_body_children(popup);

    if (popup->cols > 1) {
        render_expanded_grid(popup);
    } else {
        render_compact_list(popup);
    }

    /* 푸터 갱신 — page_label + expand_icon */
    gsize total = get_total_pages(popup);
    if (popup->page_label) {
        if (total > 1) {
            gchar *page_text = g_strdup_printf("%zu/%zu", popup->current_page + 1, total);
            gtk_label_set_text(GTK_LABEL(popup->page_label), page_text);
            g_free(page_text);
        } else {
            gtk_label_set_text(GTK_LABEL(popup->page_label), "");
        }
    }
    if (popup->expand_icon) {
        gtk_label_set_text(GTK_LABEL(popup->expand_icon),
                            (popup->cols > 1) ? ICON_COMPACT : ICON_EXPAND);
    }
}

/* compact 모드 (1×9 ListBox) 렌더 */
static void
render_compact_list(UnimHanjaPopup *popup)
{
    popup->listbox = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(popup->listbox), GTK_SELECTION_SINGLE);
    gtk_list_box_set_activate_on_single_click(GTK_LIST_BOX(popup->listbox), TRUE);
    g_signal_connect(popup->listbox, "row-activated", G_CALLBACK(on_row_activated), popup);
#if GTK_CHECK_VERSION(4, 0, 0)
    {
        GtkGesture *right_click = gtk_gesture_click_new();
        gtk_gesture_single_set_button(GTK_GESTURE_SINGLE(right_click), 0);
        g_signal_connect(right_click, "pressed", G_CALLBACK(on_listbox_right_click), popup);
        gtk_widget_add_controller(popup->listbox, GTK_EVENT_CONTROLLER(right_click));
    }
    gtk_widget_set_focusable(popup->listbox, FALSE);
#else
    g_signal_connect(popup->listbox, "button-press-event",
                     G_CALLBACK(on_listbox_button_press), popup);
#endif
    gtk_widget_set_can_focus(popup->listbox, FALSE);

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(popup->body_container), popup->listbox);
#else
    gtk_box_pack_start(GTK_BOX(popup->body_container), popup->listbox, TRUE, TRUE, 0);
#endif

    gsize start = popup->current_page * COMPACT_PAGE_SIZE;
    gsize page_count = get_page_candidate_count(popup);

    for (gsize i = 0; i < page_count; i++) {
        gsize idx = start + i;
        UnimHanjaCandidate *cand = &popup->candidates[idx];

        GtkWidget *row_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);

        gchar *num_text = g_strdup_printf("%zu.", i + 1);
        GtkWidget *num_label = gtk_label_new(num_text);
        g_free(num_text);
        gtk_label_set_xalign(GTK_LABEL(num_label), 0.0);
        gtk_widget_set_margin_end(num_label, 6);
        WIDGET_ADD_CSS_CLASS(num_label, "item-number");

        GtkWidget *hanja_label = gtk_label_new(cand->hanja);
        gtk_label_set_xalign(GTK_LABEL(hanja_label), 0.0);
        gtk_widget_set_margin_end(hanja_label, 8);
        WIDGET_ADD_CSS_CLASS(hanja_label, "item-hanja");

        GtkWidget *meaning_label = gtk_label_new(cand->meaning ? cand->meaning : "");
        gtk_label_set_xalign(GTK_LABEL(meaning_label), 0.0);
        WIDGET_ADD_CSS_CLASS(meaning_label, "item-meaning");

        gboolean bookmarked = (popup->bookmarks && idx < popup->count)
                               ? popup->bookmarks[idx]
                               : FALSE;
        GtkWidget *star_label = gtk_label_new(bookmarked ? "★" : "☆");
        gtk_label_set_xalign(GTK_LABEL(star_label), 1.0);
        gtk_widget_set_margin_start(star_label, 6);
        WIDGET_ADD_CSS_CLASS(star_label, "item-bookmark");
        if (bookmarked) {
            WIDGET_ADD_CSS_CLASS(star_label, "bookmarked");
        }

#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_box_append(GTK_BOX(row_box), num_label);
        gtk_box_append(GTK_BOX(row_box), hanja_label);
        gtk_box_append(GTK_BOX(row_box), meaning_label);
        gtk_box_append(GTK_BOX(row_box), star_label);
        gtk_list_box_append(GTK_LIST_BOX(popup->listbox), row_box);
#else
        gtk_box_pack_start(GTK_BOX(row_box), num_label, FALSE, FALSE, 0);
        gtk_box_pack_start(GTK_BOX(row_box), hanja_label, FALSE, FALSE, 0);
        gtk_box_pack_start(GTK_BOX(row_box), meaning_label, TRUE, TRUE, 0);
        gtk_box_pack_end(GTK_BOX(row_box), star_label, FALSE, FALSE, 0);
        gtk_container_add(GTK_CONTAINER(popup->listbox), row_box);
        gtk_widget_show_all(row_box);
#endif
    }

    if (popup->selected_index >= 0 && popup->selected_index < (gint)page_count) {
        GtkListBoxRow *row = gtk_list_box_get_row_at_index(
            GTK_LIST_BOX(popup->listbox), popup->selected_index);
        if (row) {
            gtk_list_box_select_row(GTK_LIST_BOX(popup->listbox), row);
        }
    }
}

/* expanded 모드 (9×9 Grid) 렌더 — col 우선 인덱싱: idx = col*rows + row.
 * GNOME extension hanja_popup.js와 동일하게 col=0..8 헤더(1-9)와 셀을 함께 둔다. */
static void
render_expanded_grid(UnimHanjaPopup *popup)
{
    popup->grid = gtk_grid_new();
    WIDGET_ADD_CSS_CLASS(popup->grid, "hanja-grid");
    gtk_grid_set_row_spacing(GTK_GRID(popup->grid), 2);
    gtk_grid_set_column_spacing(GTK_GRID(popup->grid), 2);
    gtk_widget_set_can_focus(popup->grid, FALSE);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_focusable(popup->grid, FALSE);
    gtk_box_append(GTK_BOX(popup->body_container), popup->grid);
#else
    gtk_box_pack_start(GTK_BOX(popup->body_container), popup->grid, TRUE, TRUE, 0);
#endif

    gsize page_size = EXPANDED_PAGE_SIZE;
    gsize start = popup->current_page * page_size;
    gsize end = start + page_size;
    if (end > popup->count) end = popup->count;

    /* (0, 0) corner — 빈 라벨 (special과 정합) */
    GtkWidget *corner = gtk_label_new(NULL);
    WIDGET_ADD_CSS_CLASS(corner, "grid-row-number");
    gtk_grid_attach(GTK_GRID(popup->grid), corner, 0, 0, 1, 1);

    /* 행 번호 (col 0, row 1..=9). sel_row면 active CSS — UX 일관성 (special과 정합). */
    for (gint row = 0; row < EXPANDED_ROWS; row++) {
        gchar num_text[4];
        g_snprintf(num_text, sizeof(num_text), "%d", row + 1);
        GtkWidget *num_label = gtk_label_new(num_text);
        WIDGET_ADD_CSS_CLASS(num_label, "grid-row-number");
        if (row == popup->sel_row) {
            WIDGET_ADD_CSS_CLASS(num_label, "active");
        }
        gtk_grid_attach(GTK_GRID(popup->grid), num_label, 0, row + 1, 1, 1);
    }

    /* 가로 레이블 키 시퀀스(special과 동일). 엔진 SSOT는 PopupState.top_row()이지만
     * C-API 표면 확장을 피하기 위해 동일 상수를 직접 사용. */
    static const char TOP_ROW[] = "QWERTYUIO";
    for (gint col = 0; col < EXPANDED_COLS; col++) {
        gchar header_text[2] = { TOP_ROW[col], '\0' };
        GtkWidget *header = gtk_label_new(header_text);
        WIDGET_ADD_CSS_CLASS(header, "grid-header");
        if (col == popup->sel_col) {
            WIDGET_ADD_CSS_CLASS(header, "active");
        }
        gtk_grid_attach(GTK_GRID(popup->grid), header, col + 1, 0, 1, 1);

        for (gint row = 0; row < EXPANDED_ROWS; row++) {
            gsize offset = (gsize)col * EXPANDED_ROWS + (gsize)row;
            gsize global = start + offset;
            if (global >= end) continue;
            UnimHanjaCandidate *cand = &popup->candidates[global];

            GtkWidget *cell = gtk_button_new_with_label(cand->hanja);
            WIDGET_ADD_CSS_CLASS(cell, "grid-cell");
            gtk_widget_set_can_focus(cell, FALSE);
#if GTK_CHECK_VERSION(4, 0, 0)
            gtk_widget_set_focusable(cell, FALSE);
#endif
            if (col == popup->sel_col && row == popup->sel_row) {
                WIDGET_ADD_CSS_CLASS(cell, "grid-cell-selected");
            }
            if (popup->bookmarks && global < popup->count && popup->bookmarks[global]) {
                WIDGET_ADD_CSS_CLASS(cell, "bookmarked");
            }

            /* 글로벌 인덱스를 GINT_TO_POINTER로 전달해 클릭 시 SelectHanja에 사용. */
            g_object_set_data(G_OBJECT(cell), "unim-global-idx",
                              GSIZE_TO_POINTER(global));
            g_signal_connect(cell, "clicked", G_CALLBACK(on_grid_cell_clicked), popup);

            gtk_grid_attach(GTK_GRID(popup->grid), cell, col + 1, row + 1, 1, 1);
        }
    }
}

/* 리스트 아이템 선택 콜백 (compact 모드 클릭). */
static void
on_row_activated(GtkListBox *listbox, GtkListBoxRow *row, gpointer user_data)
{
    (void)listbox;
    UnimHanjaPopup *popup = (UnimHanjaPopup *)user_data;

    if (!popup || !row || !popup->callback) return;

    gint index = gtk_list_box_row_get_index(row);
    gsize actual_index = popup->current_page * COMPACT_PAGE_SIZE + (gsize)index;

    if (actual_index < popup->count) {
        const gchar *hanja = popup->candidates[actual_index].hanja;
        POPUP_DEBUG("한자 선택 (클릭): index=%zu, hanja='%s'", actual_index, hanja);
        popup->callback(hanja, popup->user_data);
    }
}

/* expanded grid 셀 클릭 → cell의 unim-global-idx 데이터를 사용해 한자 선택. */
static void
on_grid_cell_clicked(GtkButton *button, gpointer user_data)
{
    UnimHanjaPopup *popup = (UnimHanjaPopup *)user_data;
    if (!popup || !popup->callback) return;
    gpointer idx_ptr = g_object_get_data(G_OBJECT(button), "unim-global-idx");
    gsize global = GPOINTER_TO_SIZE(idx_ptr);
    if (global < popup->count) {
        const gchar *hanja = popup->candidates[global].hanja;
        POPUP_DEBUG("한자 선택 (grid 클릭): index=%zu, hanja='%s'", global, hanja);
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

    /* 접근성: GTK4 accessible role */
    gtk_accessible_update_property(GTK_ACCESSIBLE(popup->window),
        GTK_ACCESSIBLE_PROPERTY_LABEL, "UNIM 한자 후보 목록",
        -1);

#else
    /* GTK3: 팝업 윈도우 (포커스 불가) */
    popup->window = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_type_hint(GTK_WINDOW(popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
    gtk_widget_set_can_focus(popup->window, FALSE);

    /* 접근성: ATK 역할 설정 */
    {
        AtkObject *atk_obj = gtk_widget_get_accessible(popup->window);
        if (atk_obj) {
            atk_object_set_role(atk_obj, ATK_ROLE_POPUP_MENU);
            atk_object_set_name(atk_obj, "UNIM 한자 후보 목록");
        }
    }
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
            ".unim-hanja-vbox label.popup-header {"
            "  background-color: #313244;"
            "  color: #89b4fa;"
            "  font-size: 13px;"
            "  font-weight: bold;"
            "  padding: 6px 8px;"
            "  border-radius: 4px;"
            "  margin-bottom: 6px;"
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
            ".unim-hanja-vbox list row:hover {"
            "  background-color: rgba(255, 255, 255, 0.05);"
            "}"
            ".unim-hanja-vbox list row:selected:hover {"
            "  background-color: rgba(137, 180, 250, 0.35);"
            "}"
            ".unim-hanja-vbox list row label.item-number {"
            "  color: #7f849c;"
            "  font-size: 12px;"
            "}"
            ".unim-hanja-vbox list row label.item-hanja {"
            "  color: #cdd6f4;"
            "  font-size: 18px;"
            "  font-weight: bold;"
            "}"
            ".unim-hanja-vbox list row label.item-meaning {"
            "  color: #a6adc8;"
            "  font-size: 12px;"
            "}"
            ".unim-hanja-vbox list row label.item-bookmark {"
            "  color: #6c7086;"
            "  font-size: 14px;"
            "  margin-left: 8px;"
            "  min-width: 16px;"
            "}"
            ".unim-hanja-vbox list row label.item-bookmark.bookmarked {"
            "  color: #f9e2af;"
            "}"
            ".unim-hanja-vbox list row:selected label {"
            "  color: #cdd6f4;"
            "}"
            ".unim-hanja-vbox label.page-label {"
            "  color: #6c7086;"
            "  font-size: 12px;"
            "  padding: 2px 0;"
            "}"
            ".unim-hanja-vbox .popup-footer-box {"
            "  margin-top: 4px;"
            "}"
            ".unim-hanja-vbox label.popup-expand-icon {"
            "  color: #7f849c;"
            "  font-size: 14px;"
            "  padding: 2px 6px;"
            "}"
            ".unim-hanja-vbox .hanja-grid {"
            "  padding: 4px;"
            "}"
            ".unim-hanja-vbox .grid-row-number {"
            "  color: #7f849c;"
            "  font-size: 11px;"
            "  min-width: 22px;"
            "  min-height: 22px;"
            "}"
            ".unim-hanja-vbox button.grid-cell {"
            "  background: transparent;"
            "  color: #cdd6f4;"
            "  font-size: 16px;"
            "  min-width: 28px;"
            "  min-height: 28px;"
            "  border-radius: 4px;"
            "  padding: 2px;"
            "  border: none;"
            "}"
            ".unim-hanja-vbox button.grid-cell:hover {"
            "  background-color: rgba(137, 180, 250, 0.15);"
            "}"
            ".unim-hanja-vbox button.grid-cell-selected {"
            "  background-color: rgba(137, 180, 250, 0.3);"
            "}"
            ".unim-hanja-vbox button.grid-cell.bookmarked {"
            "  color: #f9e2af;"
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

    /* 헤더 라벨 */
    popup->header_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->header_label), 0.0);
    WIDGET_ADD_CSS_CLASS(popup->header_label, "popup-header");
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->header_label);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->header_label, FALSE, FALSE, 0);
#endif

    /* 본문 컨테이너 — compact는 ListBox, expanded는 Grid를 동적으로 차일드로 둔다.
     * 기존에는 vbox에 listbox를 직접 붙였으나, 모드 전환을 위해 한 단계 wrapping. */
    popup->body_container = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->body_container);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->body_container, TRUE, TRUE, 0);
#endif

    /* 푸터: page_label(좌) + expand_icon(우)을 가로 배치 */
    popup->footer_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
    WIDGET_ADD_CSS_CLASS(popup->footer_box, "popup-footer-box");

    popup->page_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->page_label), 0.0);
    gtk_widget_set_hexpand(popup->page_label, TRUE);
    WIDGET_ADD_CSS_CLASS(popup->page_label, "page-label");

    popup->expand_icon = gtk_label_new(ICON_EXPAND);
    gtk_label_set_xalign(GTK_LABEL(popup->expand_icon), 1.0);
    WIDGET_ADD_CSS_CLASS(popup->expand_icon, "popup-expand-icon");

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(popup->footer_box), popup->page_label);
    gtk_box_append(GTK_BOX(popup->footer_box), popup->expand_icon);
    gtk_box_append(GTK_BOX(vbox), popup->footer_box);
#else
    gtk_box_pack_start(GTK_BOX(popup->footer_box), popup->page_label, TRUE, TRUE, 0);
    gtk_box_pack_end(GTK_BOX(popup->footer_box), popup->expand_icon, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(vbox), popup->footer_box, FALSE, FALSE, 2);
#endif

    popup->listbox = NULL;
    popup->grid = NULL;
    popup->selected_index = 0;
    popup->sel_row = 0;
    popup->sel_col = 0;
    popup->cols = 1;

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

    g_free(popup->bookmarks);
    popup->bookmarks = NULL;

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
    popup->sel_row = 0;
    popup->sel_col = 0;
    popup->cols = 1;

    /* bookmarks 배열 재할당 (기본값 FALSE) — 프런트엔드가 곧이어
     * unim_hanja_popup_set_bookmark_states()로 일괄 반영한다. */
    g_free(popup->bookmarks);
    popup->bookmarks = g_new0(gboolean, count);
    popup->callback = callback;
    popup->user_data = user_data;

    /* 헤더 텍스트 설정 */
    {
        gchar *header_text = g_strdup_printf("「%s」 → 한자", target);
        gtk_label_set_text(GTK_LABEL(popup->header_label), header_text);
        g_free(header_text);
    }

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
    gint final_x = x, final_y = y + 4;  /* 4px gap below cursor */

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

            /* 오른쪽 넘침 보정 (4px 여백) */
            if (final_x + popup_w > screen_w) {
                final_x = screen_w - popup_w - 4;
                if (final_x < 0) final_x = 0;
            }
            /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 (4px 여백) */
            if (final_y + popup_h > screen_h) {
                /* y는 커서 아래쪽이므로, 커서 위로: y - cursor_height - popup_h - 4 */
                final_y = y - cursor_height - popup_h - 4;
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

                /* 오른쪽 넘침 보정 (4px 여백) */
                if (final_x + popup_w > mon_geom.x + mon_geom.width) {
                    final_x = mon_geom.x + mon_geom.width - popup_w - 4;
                    if (final_x < mon_geom.x) final_x = mon_geom.x;
                }
                /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 (4px 여백) */
                if (final_y + popup_h > mon_geom.y + mon_geom.height) {
                    final_y = y - cursor_height - popup_h - 4;
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

    case UNIM_POPUP_RESULT_UPDATED: {
        /* Period 키 토글로 cols가 1↔9로 바뀌면 compact↔expanded 위젯 트리도
         * 함께 재구성된다 (update_listbox이 cols로 분기). */
        gint engine_cols = unim_popup_get_cols(popup->popup_state);
        popup->cols = engine_cols > 0 ? (gsize)engine_cols : 1;
        popup->current_page = unim_popup_get_current_page(popup->popup_state);
        popup->sel_row = unim_popup_get_sel_row(popup->popup_state);
        popup->sel_col = unim_popup_get_sel_col(popup->popup_state);
        popup->selected_index = popup->sel_row;
        update_listbox(popup);
        return TRUE;
    }

    case UNIM_POPUP_RESULT_CONSUMED:
        return TRUE;

    case UNIM_POPUP_RESULT_TOGGLE_BOOKMARK:
        /* 엔진은 토글을 이미 반영했다. 프런트엔드는 persist(DBus)만 수행하고,
         * UI 반영은 HanjaBookmarkChanged 시그널을 통해 일원화된다. */
        if (result.selected_index >= 0
            && popup->toggle_bookmark_callback) {
            popup->toggle_bookmark_callback((gsize)result.selected_index,
                                             popup->toggle_bookmark_user_data);
        }
        return TRUE;

    case UNIM_POPUP_RESULT_NOT_HANDLED:
    default:
        return FALSE;
    }
}

void
unim_hanja_popup_set_toggle_bookmark_callback(UnimHanjaPopup *popup,
                                               UnimHanjaToggleBookmarkCallback callback,
                                               gpointer user_data)
{
    if (!popup) return;
    popup->toggle_bookmark_callback = callback;
    popup->toggle_bookmark_user_data = user_data;
}

void
unim_hanja_popup_set_bookmark_states(UnimHanjaPopup *popup,
                                      const gboolean *states,
                                      gsize count)
{
    if (!popup) return;
    g_free(popup->bookmarks);
    popup->bookmarks = g_new0(gboolean, popup->count > 0 ? popup->count : 1);
    gsize n = count < popup->count ? count : popup->count;
    if (states) {
        for (gsize i = 0; i < n; i++) {
            popup->bookmarks[i] = states[i];
        }
    }
    if (unim_hanja_popup_is_visible(popup)) {
        update_listbox(popup);
    }
}

void
unim_hanja_popup_set_bookmark(UnimHanjaPopup *popup,
                               gsize global_index,
                               gboolean bookmarked)
{
    if (!popup) return;
    if (global_index >= popup->count) return;
    if (!popup->bookmarks) {
        popup->bookmarks = g_new0(gboolean, popup->count);
    }
    popup->bookmarks[global_index] = bookmarked;

    /* 현재 페이지에 포함된 인덱스일 때만 즉시 재렌더 (페이지 크기는 모드 의존) */
    gsize page_size = get_page_size(popup);
    gsize start = popup->current_page * page_size;
    gsize end = start + get_page_candidate_count(popup);
    if (global_index >= start && global_index < end
        && unim_hanja_popup_is_visible(popup)) {
        update_listbox(popup);
    }
}

void
unim_hanja_popup_replace_candidates(UnimHanjaPopup *popup,
                                     UnimHanjaCandidate *candidates,
                                     gsize count,
                                     const gboolean *bookmarks,
                                     gsize bookmarks_count,
                                     gint page,
                                     gint sel_row,
                                     gint sel_col)
{
    if (!popup) return;

    /* 이전 candidates 메모리는 호출자(im 모듈)가 해제. 본 함수는 새 포인터를 보관. */
    popup->candidates = candidates;
    popup->count = count;

    g_free(popup->bookmarks);
    popup->bookmarks = g_new0(gboolean, count > 0 ? count : 1);
    gsize n = bookmarks_count < count ? bookmarks_count : count;
    if (bookmarks) {
        for (gsize i = 0; i < n; i++) {
            popup->bookmarks[i] = bookmarks[i];
        }
    }

    popup->current_page = page < 0 ? 0 : (gsize)page;
    popup->sel_row = sel_row < 0 ? 0 : (gsize)sel_row;
    popup->sel_col = sel_col < 0 ? 0 : (gsize)sel_col;
    popup->selected_index = popup->sel_row;

    /* PopupState도 재구성해서 keyboard 처리와 후보 텍스트 동기화 */
    if (popup->popup_state) {
        unim_popup_free(popup->popup_state);
        popup->popup_state = NULL;
    }
    if (count > 0) {
        const uint8_t **hanja_ptrs = g_malloc(sizeof(uint8_t*) * count);
        size_t *hanja_lens = g_malloc(sizeof(size_t) * count);
        const uint8_t **meaning_ptrs = g_malloc(sizeof(uint8_t*) * count);
        size_t *meaning_lens = g_malloc(sizeof(size_t) * count);
        for (gsize i = 0; i < count; i++) {
            hanja_ptrs[i] = (const uint8_t *)candidates[i].hanja;
            hanja_lens[i] = candidates[i].hanja ? strlen(candidates[i].hanja) : 0;
            meaning_ptrs[i] = (const uint8_t *)candidates[i].meaning;
            meaning_lens[i] = candidates[i].meaning ? strlen(candidates[i].meaning) : 0;
        }
        /* target은 헤더 라벨에서 추출하지 않고, im 모듈이 같은 target을 알고 있어
         * 재구성 가능하지만 여기선 빈 문자열로 두고 키 처리 동기화만 신경 쓴다. */
        popup->popup_state = unim_popup_new_hanja(
            (const uint8_t *)"", 0,
            hanja_ptrs, hanja_lens, meaning_ptrs, meaning_lens, count
        );
        g_free(hanja_ptrs); g_free(hanja_lens); g_free(meaning_ptrs); g_free(meaning_lens);
    }

    if (unim_hanja_popup_is_visible(popup)) {
        update_listbox(popup);
    }
}
