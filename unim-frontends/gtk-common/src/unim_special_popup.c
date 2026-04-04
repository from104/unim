/**
 * UNIM Special Character Popup Implementation
 *
 * 초성 기반 특수문자 선택을 위한 9x9 그리드 팝업 윈도우입니다.
 * 열 우선 채움 레이아웃을 사용하며, 숫자키/화살표/Enter로 네비게이션합니다.
 */

#include "unim_special_popup.h"
#include "unim.h"
#include <string.h>
#include <stdio.h>

/* X11 override_redirect 설정을 위한 헤더 */
#ifdef GDK_WINDOWING_X11
#if GTK_MAJOR_VERSION >= 4
#include <gdk/x11/gdkx.h>
#else
#include <gdk/gdkx.h>
#endif
#include <X11/Xlib.h>
#endif

/* 상수 */
#define MAX_ROWS 9
#define MAX_COLS 9
#define CELL_SIZE 28
#define HEADER_SIZE 20
#define FOOTER_HEIGHT 20
#define PAGE_SIZE (MAX_ROWS * MAX_COLS)  /* 81 */
#define FLASH_DURATION_MS 120  /* 선택 플래시 지속 시간 */

/* 로깅 매크로 (unim_dbus_client.c와 동일 패턴) */
static gboolean special_popup_debug_enabled = FALSE;
static gboolean special_popup_debug_checked = FALSE;

static void
special_popup_check_debug(void)
{
    if (!special_popup_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            special_popup_debug_enabled = TRUE;
        }
        special_popup_debug_checked = TRUE;
    }
}

#define SPECIAL_DEBUG(fmt, ...) do { \
    if (special_popup_debug_enabled) \
        g_print("[SPECIAL_POPUP] " fmt "\n", ##__VA_ARGS__); \
} while(0)

struct _UnimSpecialPopup {
    GtkWidget *window;
    GtkWidget *header_label;  /* 헤더 (「X」 → 특수문자) */
    GtkWidget *grid;
    GtkWidget *footer_label;

    /* 데이터 */
    gchar *target;            /* 대상 문자 (푸터 표시용) */
    gchar **characters;       /* 전체 문자 배열 */
    gsize total_count;        /* 전체 문자 수 */
    gint current_page;        /* 현재 페이지 (0부터) */
    gint total_pages;         /* 전체 페이지 수 */
    gint rows;                /* 현재 페이지 행 수 */
    gint cols;                /* 현재 페이지 열 수 */

    /* 선택 커서 */
    gint sel_row;             /* 현재 선택 행 (0부터) */
    gint sel_col;             /* 현재 선택 열 (0부터) */

    /* 셀 위젯 배열 (grid 내부 label) */
    GtkWidget *cells[MAX_ROWS][MAX_COLS];

    /* 열 헤더 위젯 배열 (열 강조용) */
    GtkWidget *col_headers[MAX_COLS];

    /* 행 헤더 위젯 배열 (행 강조용) */
    GtkWidget *row_headers[MAX_ROWS];

    /* 콜백 */
    UnimSpecialSelectCallback callback;
    gpointer user_data;

    /* 영문 키맵 상단 행 레이블 */
    gchar top_row[10];  /* 예: "QWERTYUIO" + null */

    /* 플래시 후 숨김 대기 중 플래그 */
    gboolean pending_hide;

    /* C-API popup state */
    UnimPopupState *popup_state;
};

/* 전방 선언 */
static void update_grid(UnimSpecialPopup *popup);
static void update_selection(UnimSpecialPopup *popup);
static gint get_char_index(UnimSpecialPopup *popup, gint row, gint col);
static gboolean cell_has_char(UnimSpecialPopup *popup, gint row, gint col);
static void select_current(UnimSpecialPopup *popup);

/* GTK3/4 호환 CSS class 관리 매크로 */
#if GTK_CHECK_VERSION(4, 0, 0)
#define WIDGET_ADD_CSS_CLASS(w, cls)    gtk_widget_add_css_class(w, cls)
#define WIDGET_REMOVE_CSS_CLASS(w, cls) gtk_widget_remove_css_class(w, cls)
#else
static void
widget_add_css_class_compat(GtkWidget *w, const char *cls) {
    GtkStyleContext *ctx = gtk_widget_get_style_context(w);
    gtk_style_context_add_class(ctx, cls);
}
static void
widget_remove_css_class_compat(GtkWidget *w, const char *cls) {
    GtkStyleContext *ctx = gtk_widget_get_style_context(w);
    gtk_style_context_remove_class(ctx, cls);
}
#define WIDGET_ADD_CSS_CLASS(w, cls)    widget_add_css_class_compat(w, cls)
#define WIDGET_REMOVE_CSS_CLASS(w, cls) widget_remove_css_class_compat(w, cls)
#endif

#if GTK_CHECK_VERSION(4, 0, 0)
static void on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data);
#else
static gboolean on_grid_button_press(GtkWidget *widget, GdkEventButton *event, gpointer user_data);
#endif

UnimSpecialPopup*
unim_special_popup_new(void)
{
    special_popup_check_debug();

    UnimSpecialPopup *popup = g_new0(UnimSpecialPopup, 1);

#if GTK_CHECK_VERSION(4, 0, 0)
    /* GTK4: 포커스를 가져가지 않는 팝업 윈도우 */
    popup->window = gtk_window_new();
    gtk_window_set_decorated(GTK_WINDOW(popup->window), FALSE);
    gtk_window_set_resizable(GTK_WINDOW(popup->window), FALSE);
    gtk_widget_set_can_focus(popup->window, FALSE);

    /* 접근성: GTK4 accessible label */
    gtk_accessible_update_property(GTK_ACCESSIBLE(popup->window),
        GTK_ACCESSIBLE_PROPERTY_LABEL, "UNIM 특수문자 후보 목록",
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
            atk_object_set_name(atk_obj, "UNIM 특수문자 후보 목록");
        }
    }
#endif

    /* 메인 박스 */
    GtkWidget *vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_margin_start(vbox, 0);
    gtk_widget_set_margin_end(vbox, 0);
    gtk_widget_set_margin_top(vbox, 0);
    gtk_widget_set_margin_bottom(vbox, 0);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_window_set_child(GTK_WINDOW(popup->window), vbox);
#else
    gtk_container_add(GTK_CONTAINER(popup->window), vbox);
#endif

    /* 헤더 라벨 */
    popup->header_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->header_label), 0.0);
    WIDGET_ADD_CSS_CLASS(popup->header_label, "popup-header");
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->header_label);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->header_label, FALSE, FALSE, 0);
#endif

    /* 그리드 영역 */
    popup->grid = gtk_grid_new();
    gtk_grid_set_row_spacing(GTK_GRID(popup->grid), 1);
    gtk_grid_set_column_spacing(GTK_GRID(popup->grid), 1);
    gtk_widget_set_margin_start(popup->grid, 4);
    gtk_widget_set_margin_end(popup->grid, 4);
    gtk_widget_set_margin_top(popup->grid, 4);
    gtk_widget_set_margin_bottom(popup->grid, 2);

    /* 마우스 클릭 처리 */
#if GTK_CHECK_VERSION(4, 0, 0)
    GtkGesture *click = gtk_gesture_click_new();
    gtk_gesture_single_set_button(GTK_GESTURE_SINGLE(click), GDK_BUTTON_PRIMARY);
    g_signal_connect(click, "pressed", G_CALLBACK(on_cell_clicked), popup);
    gtk_widget_add_controller(popup->grid, GTK_EVENT_CONTROLLER(click));
    gtk_box_append(GTK_BOX(vbox), popup->grid);
#else
    gtk_widget_set_events(popup->grid, gtk_widget_get_events(popup->grid) | GDK_BUTTON_PRESS_MASK);
    gtk_box_pack_start(GTK_BOX(vbox), popup->grid, TRUE, TRUE, 0);
    /* GTK3: 윈도우 레벨에서 클릭 이벤트 수신 (Label이 이벤트를 가로채지 않으므로 grid 시그널이 도달하지 않는 문제 우회) */
    gtk_widget_add_events(popup->window, GDK_BUTTON_PRESS_MASK);
    g_signal_connect(popup->window, "button-press-event", G_CALLBACK(on_grid_button_press), popup);
#endif

    /* 페이지 표시 (하단) */
    popup->footer_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->footer_label), 0.5);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), popup->footer_label);
#else
    gtk_box_pack_start(GTK_BOX(vbox), popup->footer_label, FALSE, FALSE, 0);
    /* show_all이 숨긴 footer를 다시 보이게 하는 것 방지 */
    gtk_widget_set_no_show_all(popup->footer_label, TRUE);
#endif

    /* 팝업 전용 CSS 클래스 */
    WIDGET_ADD_CSS_CLASS(popup->window, "unim-special-popup");
    WIDGET_ADD_CSS_CLASS(vbox, "unim-special-vbox");

    /* 스타일 */
    GtkCssProvider *css = gtk_css_provider_new();
    const gchar *css_text =
        "window.unim-special-popup {"
        "  background-color: rgba(30, 30, 46, 0.95);"
        "  border: 1px solid rgba(255, 255, 255, 0.15);"
        "  border-radius: 12px;"
        "  padding: 0; margin: 0;"
        "}"
        ".unim-special-vbox {"
        "  padding: 4px; margin: 0;"
        "}"
        ".unim-special-vbox label.popup-header {"
        "  background-color: #313244;"
        "  color: #a6e3a1;"
        "  font-size: 13px;"
        "  font-weight: bold;"
        "  padding: 6px 8px;"
        "  border-radius: 4px;"
        "  margin-bottom: 6px;"
        "}"
        ".unim-special-vbox grid {"
        "  padding: 0;"
        "}"
        ".unim-special-vbox label {"
        "  color: #cdd6f4; font-size: 16px;"
        "}"
        ".unim-special-vbox label.header {"
        "  color: #f9e2af; font-size: 11px; font-weight: bold; min-height: 20px;"
        "}"
        ".unim-special-vbox label.header.row-header {"
        "  color: #7f849c; min-width: 20px;"
        "}"
        ".unim-special-vbox label.header-active {"
        "  color: #a6e3a1; font-size: 11px; font-weight: bold; min-height: 20px;"
        "}"
        ".unim-special-vbox label.cell {"
        "  padding: 2px 4px; min-width: 28px; min-height: 28px;"
        "}"
        ".unim-special-vbox label.cell:hover {"
        "  background-color: rgba(255, 255, 255, 0.05); border-radius: 4px;"
        "}"
        ".unim-special-vbox label.cell-col-highlight {"
        "  background-color: rgba(166, 227, 161, 0.08); border-radius: 4px;"
        "}"
        ".unim-special-vbox label.cell-row-highlight {"
        "  background-color: rgba(166, 227, 161, 0.08); border-radius: 4px;"
        "}"
        ".unim-special-vbox label.cell-selected {"
        "  background-color: rgba(166, 227, 161, 0.25); color: #cdd6f4;"
        "  font-weight: bold; border-radius: 6px;"
        "}"
        ".unim-special-vbox label.cell-flash {"
        "  background-color: #a6e3a1; color: #1e1e2e;"
        "  font-weight: bold; border-radius: 6px;"
        "}"
        ".unim-special-vbox label.footer {"
        "  color: #6c7086; font-size: 12px; min-height: 0; padding: 0; margin: 0;"
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

    popup->sel_row = 0;
    popup->sel_col = 0;
    popup->current_page = 0;

    SPECIAL_DEBUG("팝업 인스턴스 생성");

    return popup;
}

void
unim_special_popup_free(UnimSpecialPopup *popup)
{
    if (!popup) return;

    if (popup->popup_state) {
        unim_popup_free(popup->popup_state);
        popup->popup_state = NULL;
    }

    g_free(popup->target);
    popup->target = NULL;

    if (popup->window) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_window_destroy(GTK_WINDOW(popup->window));
#else
        gtk_widget_destroy(popup->window);
#endif
    }

    g_free(popup);
}

/* 마우스 클릭 콜백 */
#if GTK_CHECK_VERSION(4, 0, 0)
static void
on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data)
{
    (void)gesture; (void)n_press; (void)x; (void)y;
    UnimSpecialPopup *popup = (UnimSpecialPopup *)user_data;

    GtkWidget *picked = gtk_widget_pick(popup->grid, x, y, GTK_PICK_DEFAULT);
    if (!picked || !popup->popup_state) return;

    for (gint r = 0; r < MAX_ROWS; r++) {
        for (gint c = 0; c < MAX_COLS; c++) {
            if (popup->cells[r][c] == picked) {
                UnimPopupKeyResult result = unim_popup_handle_click(popup->popup_state, r, c);
                if (result.kind == UNIM_POPUP_RESULT_SELECT) {
                    popup->sel_row = unim_popup_get_sel_row(popup->popup_state);
                    popup->sel_col = unim_popup_get_sel_col(popup->popup_state);
                    update_selection(popup);
                    select_current(popup);
                }
                return;
            }
        }
    }
}
#else
static gboolean
on_grid_button_press(GtkWidget *widget, GdkEventButton *event, gpointer user_data)
{
    (void)widget;
    UnimSpecialPopup *popup = (UnimSpecialPopup *)user_data;
    if (event->button != 1 || !popup->popup_state) return FALSE;

    /* 클릭된 셀 찾기: 각 셀의 allocation 확인 */
    for (gint r = 0; r < MAX_ROWS; r++) {
        for (gint c = 0; c < MAX_COLS; c++) {
            if (popup->cells[r][c]) {
                GtkAllocation alloc;
                gtk_widget_get_allocation(popup->cells[r][c], &alloc);
                if (event->x >= alloc.x && event->x < alloc.x + alloc.width &&
                    event->y >= alloc.y && event->y < alloc.y + alloc.height) {
                    UnimPopupKeyResult result = unim_popup_handle_click(popup->popup_state, r, c);
                    if (result.kind == UNIM_POPUP_RESULT_SELECT) {
                        popup->sel_row = unim_popup_get_sel_row(popup->popup_state);
                        popup->sel_col = unim_popup_get_sel_col(popup->popup_state);
                        update_selection(popup);
                        select_current(popup);
                    }
                    return TRUE;
                }
            }
        }
    }
    return FALSE;
}
#endif

static void
clear_grid(UnimSpecialPopup *popup)
{
    /* 기존 그리드 내용 모두 제거 */
#if GTK_CHECK_VERSION(4, 0, 0)
    GtkWidget *child;
    while ((child = gtk_widget_get_first_child(popup->grid)) != NULL) {
        gtk_grid_remove(GTK_GRID(popup->grid), child);
    }
#else
    GList *children = gtk_container_get_children(GTK_CONTAINER(popup->grid));
    for (GList *l = children; l != NULL; l = l->next) {
        gtk_container_remove(GTK_CONTAINER(popup->grid), GTK_WIDGET(l->data));
    }
    g_list_free(children);
#endif
    memset(popup->cells, 0, sizeof(popup->cells));
    memset(popup->col_headers, 0, sizeof(popup->col_headers));
    memset(popup->row_headers, 0, sizeof(popup->row_headers));
}

static void
update_grid(UnimSpecialPopup *popup)
{
    clear_grid(popup);

    gsize page_start = (gsize)popup->current_page * PAGE_SIZE;
    gsize page_chars = popup->total_count - page_start;
    if (page_chars > PAGE_SIZE) page_chars = PAGE_SIZE;

    /* 열 수 계산: ceil(page_chars / MAX_ROWS) */
    popup->cols = (gint)((page_chars + MAX_ROWS - 1) / MAX_ROWS);
    if (popup->cols > MAX_COLS) popup->cols = MAX_COLS;
    if (popup->cols < 1) popup->cols = 1;

    /* 행 수는 각 열별로 다를 수 있지만 max MAX_ROWS */
    popup->rows = (gint)(page_chars < (gsize)MAX_ROWS ? page_chars : MAX_ROWS);

    /* 열 헤더 (A, B, C, ...) */
    /* 좌상단 빈 셀 */
    GtkWidget *corner = gtk_label_new("  ");
    WIDGET_ADD_CSS_CLASS(corner, "header");
    gtk_grid_attach(GTK_GRID(popup->grid), corner, 0, 0, 1, 1);

    /* 열 헤더 (top_row 기반) */
    for (gint c = 0; c < popup->cols; c++) {
        gchar header_text[4];
        g_snprintf(header_text, sizeof(header_text), "%c", popup->top_row[c]);
        GtkWidget *header = gtk_label_new(header_text);
        WIDGET_ADD_CSS_CLASS(header, "header");
        gtk_widget_set_halign(header, GTK_ALIGN_CENTER);
        gtk_grid_attach(GTK_GRID(popup->grid), header, c + 1, 0, 1, 1);
        popup->col_headers[c] = header;
    }

    /* 셀 채움 (열 우선: col 0의 row 0~8 → col 1의 row 0~8 → ...) */
    for (gint c = 0; c < popup->cols; c++) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            gint idx = (gint)page_start + c * MAX_ROWS + r;
            if (idx >= (gint)popup->total_count) break;

            /* 행 레이블 (첫 열에서만) */
            if (c == 0) {
                gchar row_text[4];
                g_snprintf(row_text, sizeof(row_text), "%d", r + 1);
                GtkWidget *row_label = gtk_label_new(row_text);
                WIDGET_ADD_CSS_CLASS(row_label, "header");
                WIDGET_ADD_CSS_CLASS(row_label, "row-header");
                gtk_widget_set_halign(row_label, GTK_ALIGN_CENTER);
                gtk_grid_attach(GTK_GRID(popup->grid), row_label, 0, r + 1, 1, 1);
                popup->row_headers[r] = row_label;
            }

            /* 문자 셀 */
            GtkWidget *cell = gtk_label_new(popup->characters[idx]);
            WIDGET_ADD_CSS_CLASS(cell, "cell");
            gtk_widget_set_halign(cell, GTK_ALIGN_CENTER);
            gtk_grid_attach(GTK_GRID(popup->grid), cell, c + 1, r + 1, 1, 1);
            popup->cells[r][c] = cell;
        }
    }

    /* 페이지 표시 (2페이지 이상일 때만) */
    if (popup->footer_label && popup->total_pages > 1) {
        gchar page_text[64];
        g_snprintf(page_text, sizeof(page_text), "[%s]  %d/%d",
                   popup->target ? popup->target : "",
                   popup->current_page + 1, popup->total_pages);
        gtk_label_set_text(GTK_LABEL(popup->footer_label), page_text);
        WIDGET_ADD_CSS_CLASS(popup->footer_label, "footer");
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->footer_label, TRUE);
#else
        gtk_widget_show(popup->footer_label);
#endif
    } else if (popup->footer_label) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->footer_label, FALSE);
#else
        gtk_widget_hide(popup->footer_label);
#endif
    }

    update_selection(popup);

#if !GTK_CHECK_VERSION(4, 0, 0)
    /* GTK3: 윈도우가 이미 보이는 상태에서 그리드가 변경되면 크기 재조정 */
    if (gtk_widget_get_visible(popup->window)) {
        gtk_window_resize(GTK_WINDOW(popup->window), 1, 1);
        GtkRequisition req;
        gtk_widget_get_preferred_size(popup->window, NULL, &req);
        gtk_window_resize(GTK_WINDOW(popup->window), req.width, req.height);
        gtk_widget_show_all(popup->window);
    }
#endif
}

static void
update_selection(UnimSpecialPopup *popup)
{
    /* 모든 셀의 스타일 제거 */
    for (gint r = 0; r < MAX_ROWS; r++) {
        for (gint c = 0; c < MAX_COLS; c++) {
            if (popup->cells[r][c]) {
                WIDGET_REMOVE_CSS_CLASS(popup->cells[r][c], "cell-selected");
                WIDGET_REMOVE_CSS_CLASS(popup->cells[r][c], "cell-col-highlight");
                WIDGET_REMOVE_CSS_CLASS(popup->cells[r][c], "cell-row-highlight");
            }
        }
    }

    /* 열 헤더 스타일 초기화 */
    for (gint c = 0; c < MAX_COLS; c++) {
        if (popup->col_headers[c]) {
            WIDGET_REMOVE_CSS_CLASS(popup->col_headers[c], "header-active");
            WIDGET_ADD_CSS_CLASS(popup->col_headers[c], "header");
        }
    }

    /* 행 헤더 스타일 초기화 */
    for (gint r = 0; r < MAX_ROWS; r++) {
        if (popup->row_headers[r]) {
            WIDGET_REMOVE_CSS_CLASS(popup->row_headers[r], "header-active");
            WIDGET_ADD_CSS_CLASS(popup->row_headers[r], "header");
        }
    }

    if (popup->sel_col >= 0 && popup->sel_col < popup->cols) {
        /* 선택된 열 전체 강조 */
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[r][popup->sel_col]) {
                WIDGET_ADD_CSS_CLASS(popup->cells[r][popup->sel_col], "cell-col-highlight");
            }
        }

        /* 선택된 행 전체 강조 */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS) {
            for (gint c = 0; c < popup->cols; c++) {
                if (popup->cells[popup->sel_row][c]) {
                    WIDGET_ADD_CSS_CLASS(popup->cells[popup->sel_row][c], "cell-row-highlight");
                }
            }
        }

        /* 선택된 열 헤더 강조 */
        if (popup->col_headers[popup->sel_col]) {
            WIDGET_REMOVE_CSS_CLASS(popup->col_headers[popup->sel_col], "header");
            WIDGET_ADD_CSS_CLASS(popup->col_headers[popup->sel_col], "header-active");
        }

        /* 선택된 행 헤더 강조 */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->row_headers[popup->sel_row]) {
            WIDGET_REMOVE_CSS_CLASS(popup->row_headers[popup->sel_row], "header");
            WIDGET_ADD_CSS_CLASS(popup->row_headers[popup->sel_row], "header-active");
        }

        /* 현재 선택 셀에 선택 스타일 적용 (강조 위에 겹침) */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->cells[popup->sel_row][popup->sel_col]) {
            WIDGET_ADD_CSS_CLASS(
                popup->cells[popup->sel_row][popup->sel_col], "cell-selected");
        }
    }
}

static gint
get_char_index(UnimSpecialPopup *popup, gint row, gint col)
{
    return popup->current_page * PAGE_SIZE + col * MAX_ROWS + row;
}

static gboolean
cell_has_char(UnimSpecialPopup *popup, gint row, gint col)
{
    gint idx = get_char_index(popup, row, col);
    return (idx >= 0 && idx < (gint)popup->total_count);
}

/* 플래시 후 팝업 숨김 타이머 */
static gboolean
on_flash_timeout(gpointer user_data)
{
    UnimSpecialPopup *popup = (UnimSpecialPopup *)user_data;
    unim_special_popup_hide(popup);
    popup->pending_hide = FALSE;
    return G_SOURCE_REMOVE;
}

static void
select_current(UnimSpecialPopup *popup)
{
    if (!popup->callback) return;

    gint idx = get_char_index(popup, popup->sel_row, popup->sel_col);
    if (idx >= 0 && idx < (gint)popup->total_count) {
        SPECIAL_DEBUG("문자 선택: [%d] '%s'", idx, popup->characters[idx]);

        /* 플래시 스타일 적용 */
        GtkWidget *cell = popup->cells[popup->sel_row][popup->sel_col];
        if (cell) {
            WIDGET_REMOVE_CSS_CLASS(cell, "cell-selected");
            WIDGET_ADD_CSS_CLASS(cell, "cell-flash");
        }

        /* 콜백 즉시 실행 (문자 커밋) */
        popup->callback(popup->characters[idx], popup->user_data);

        /* 팝업 숨김만 지연 (플래시 표시용) */
        popup->pending_hide = TRUE;
        g_timeout_add(FLASH_DURATION_MS, on_flash_timeout, popup);
    }
}

void
unim_special_popup_show(UnimSpecialPopup *popup,
                         const gchar *target,
                         gchar **characters,
                         gsize count,
                         const gchar *top_row,
                         gint x, gint y, gint cursor_height,
                         UnimSpecialSelectCallback callback,
                         gpointer user_data)
{
    if (!popup || !characters || count == 0) return;

    popup->characters = characters;
    popup->total_count = count;
    popup->current_page = 0;
    popup->total_pages = (gint)((count + PAGE_SIZE - 1) / PAGE_SIZE);
    popup->callback = callback;
    popup->user_data = user_data;
    popup->sel_row = 0;
    popup->sel_col = 0;
    popup->pending_hide = FALSE;  /* 이전 세션의 플래시 상태 초기화 */

    /* target 저장 (푸터 표시용) */
    g_free(popup->target);
    popup->target = g_strdup(target);

    /* 헤더 텍스트 설정 */
    {
        gchar *header_text = g_strdup_printf("「%s」 → 특수문자", target);
        gtk_label_set_text(GTK_LABEL(popup->header_label), header_text);
        g_free(header_text);
    }

    /* top_row 저장 (기본값: QWERTYUIO) */
    if (top_row && strlen(top_row) >= 9) {
        strncpy(popup->top_row, top_row, 9);
        popup->top_row[9] = '\0';
    } else {
        strncpy(popup->top_row, "QWERTYUIO", 10);
    }

    /* Build arrays for C-API */
    {
        const uint8_t **char_ptrs = g_malloc(sizeof(uint8_t*) * count);
        size_t *char_lens = g_malloc(sizeof(size_t) * count);
        for (gsize i = 0; i < count; i++) {
            char_ptrs[i] = (const uint8_t *)popup->characters[i];
            char_lens[i] = strlen(popup->characters[i]);
        }
        if (popup->popup_state) unim_popup_free(popup->popup_state);
        popup->popup_state = unim_popup_new_special(
            (const uint8_t *)target, strlen(target),
            char_ptrs, char_lens, count,
            (const uint8_t *)popup->top_row, strlen(popup->top_row)
        );
        g_free(char_ptrs);
        g_free(char_lens);
    }

    SPECIAL_DEBUG("특수문자 팝업 표시: target='%s', count=%zu, pages=%d",
                  target, count, popup->total_pages);

    /* 그리드 업데이트 */
    update_grid(popup);

    /* 크기 측정 및 위치 설정 */
    gint final_x = x;
    gint final_y = y + 4;  /* 4px gap below cursor */

#if GTK_CHECK_VERSION(4, 0, 0)
    /* GTK4: realize하여 X11 윈도우를 생성하되 아직 표시하지 않음 */
    gtk_widget_realize(popup->window);

    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    gint width = req.width;
    gint height = req.height;

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

            if (final_x + width > screen_w) {
                final_x = screen_w - width - 4;
                if (final_x < 0) final_x = 0;
            }
            if (final_y + height > screen_h) {
                final_y = y - cursor_height - height - 4;
                if (final_y < 0) final_y = 0;
            }

            XSetWindowAttributes attrs;
            attrs.override_redirect = True;
            XChangeWindowAttributes(xdisplay, xwindow, CWOverrideRedirect, &attrs);
            XMoveWindow(xdisplay, xwindow, final_x, final_y);
        }
    }
#endif

    gtk_widget_set_visible(popup->window, TRUE);

#else
    /* GTK3: 초기 위치 설정 후 show_all (realize를 위해) */
    gtk_window_resize(GTK_WINDOW(popup->window), 1, 1);
    gtk_window_move(GTK_WINDOW(popup->window), final_x, final_y);
    gtk_widget_show_all(popup->window);

    /* show_all 후 정확한 크기 측정 가능 */
    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    gint width = req.width;
    gint height = req.height;

    /* 커서가 위치한 모니터 기준으로 화면 경계 보정 */
    {
        GdkScreen *screen = gtk_window_get_screen(GTK_WINDOW(popup->window));
        if (screen) {
            GdkDisplay *display = gdk_screen_get_display(screen);
            GdkMonitor *monitor = gdk_display_get_monitor_at_point(display, x, y);
            if (monitor) {
                GdkRectangle mon_geom;
                gdk_monitor_get_geometry(monitor, &mon_geom);

                if (final_x + width > mon_geom.x + mon_geom.width) {
                    final_x = mon_geom.x + mon_geom.width - width - 4;
                    if (final_x < mon_geom.x) final_x = mon_geom.x;
                }
                if (final_y + height > mon_geom.y + mon_geom.height) {
                    final_y = y - cursor_height - height - 4;
                    if (final_y < mon_geom.y) final_y = mon_geom.y;
                }
            }
        }
    }

    gtk_window_resize(GTK_WINDOW(popup->window), width, height);
    gtk_window_move(GTK_WINDOW(popup->window), final_x, final_y);
#endif

    SPECIAL_DEBUG("팝업 위치: (%d, %d), 크기: %dx%d", final_x, final_y, width, height);
}

void
unim_special_popup_hide(UnimSpecialPopup *popup)
{
    if (!popup || !popup->window) return;

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_visible(popup->window, FALSE);
#else
    gtk_widget_hide(popup->window);
#endif
    popup->characters = NULL;
    popup->total_count = 0;

    SPECIAL_DEBUG("팝업 숨김");
}

gboolean
unim_special_popup_is_visible(UnimSpecialPopup *popup)
{
    if (!popup || !popup->window) return FALSE;
    /* 플래시 후 숨김 대기 중이면 이미 선택 완료 → 비표시로 취급 */
    if (popup->pending_hide) return FALSE;
    return gtk_widget_get_visible(popup->window);
}

gboolean
unim_special_popup_handle_key(UnimSpecialPopup *popup, guint keyval)
{
    if (!popup || !popup->popup_state || popup->pending_hide) return FALSE;

    uint32_t popup_key = unim_popup_key_from_gdk((uint32_t)keyval);
    UnimPopupKeyResult result = unim_popup_handle_key(popup->popup_state, popup_key);

    switch (result.kind) {
    case UNIM_POPUP_RESULT_SELECT:
        popup->sel_row = unim_popup_get_sel_row(popup->popup_state);
        popup->sel_col = unim_popup_get_sel_col(popup->popup_state);
        update_selection(popup);
        select_current(popup);
        return TRUE;
    case UNIM_POPUP_RESULT_CANCEL:
        return FALSE;  /* Let caller handle cancel */
    case UNIM_POPUP_RESULT_UPDATED:
        /* Sync state from PopupState */
        popup->current_page = unim_popup_get_current_page(popup->popup_state);
        popup->total_pages = unim_popup_get_total_pages(popup->popup_state);
        popup->rows = unim_popup_get_rows(popup->popup_state);
        popup->cols = unim_popup_get_cols(popup->popup_state);
        popup->sel_row = unim_popup_get_sel_row(popup->popup_state);
        popup->sel_col = unim_popup_get_sel_col(popup->popup_state);
        update_grid(popup);
        update_selection(popup);
        return TRUE;
    case UNIM_POPUP_RESULT_CONSUMED:
        return TRUE;
    case UNIM_POPUP_RESULT_NOT_HANDLED:
    default:
        return FALSE;
    }
}
