/**
 * UNIM Special Character Popup Implementation
 *
 * 초성 기반 특수문자 선택을 위한 9x9 그리드 팝업 윈도우입니다.
 * 열 우선 채움 레이아웃을 사용하며, 숫자키/화살표/Enter로 네비게이션합니다.
 */

#include "unim_special_popup.h"
#include <string.h>
#include <stdio.h>

/* X11 지원 */
#ifdef GDK_WINDOWING_X11
#include <gdk/x11/gdkx.h>
#include <X11/Xlib.h>
#endif

/* 상수 */
#define MAX_ROWS 9
#define MAX_COLS 9
#define CELL_SIZE 32
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
    GtkWidget *grid;
    GtkWidget *footer_label;

    /* 데이터 */
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
};

/* 전방 선언 */
static void update_grid(UnimSpecialPopup *popup);
static void update_selection(UnimSpecialPopup *popup);
static gint get_char_index(UnimSpecialPopup *popup, gint row, gint col);
static gboolean cell_has_char(UnimSpecialPopup *popup, gint row, gint col);
static void select_current(UnimSpecialPopup *popup);
static void on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data);

UnimSpecialPopup*
unim_special_popup_new(void)
{
    special_popup_check_debug();

    UnimSpecialPopup *popup = g_new0(UnimSpecialPopup, 1);

    /* override-redirect 팝업 윈도우 생성 */
    popup->window = gtk_window_new();
    gtk_window_set_decorated(GTK_WINDOW(popup->window), FALSE);
    gtk_window_set_resizable(GTK_WINDOW(popup->window), FALSE);
    gtk_widget_set_can_focus(popup->window, FALSE);

    /* 메인 박스 */
    GtkWidget *vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_margin_start(vbox, 0);
    gtk_widget_set_margin_end(vbox, 0);
    gtk_widget_set_margin_top(vbox, 0);
    gtk_widget_set_margin_bottom(vbox, 0);
    gtk_window_set_child(GTK_WINDOW(popup->window), vbox);

    /* 그리드 영역 */
    popup->grid = gtk_grid_new();
    gtk_grid_set_row_spacing(GTK_GRID(popup->grid), 1);
    gtk_grid_set_column_spacing(GTK_GRID(popup->grid), 1);
    gtk_widget_set_margin_start(popup->grid, 4);
    gtk_widget_set_margin_end(popup->grid, 4);
    gtk_widget_set_margin_top(popup->grid, 4);
    gtk_widget_set_margin_bottom(popup->grid, 2);

    /* 마우스 클릭 제스처 */
    GtkGesture *click = gtk_gesture_click_new();
    gtk_gesture_single_set_button(GTK_GESTURE_SINGLE(click), GDK_BUTTON_PRIMARY);
    g_signal_connect(click, "pressed", G_CALLBACK(on_cell_clicked), popup);
    gtk_widget_add_controller(popup->grid, GTK_EVENT_CONTROLLER(click));
    gtk_box_append(GTK_BOX(vbox), popup->grid);

    /* 페이지 표시 (하단) - 한자 팝업과 동일한 패턴 */
    popup->footer_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->footer_label), 0.5);
    gtk_box_append(GTK_BOX(vbox), popup->footer_label);

    /* 팝업 전용 CSS 클래스 */
    gtk_widget_add_css_class(popup->window, "unim-special-popup");
    gtk_widget_add_css_class(vbox, "unim-special-vbox");

    /* 스타일 */
    GtkCssProvider *css = gtk_css_provider_new();
    gtk_css_provider_load_from_string(css,
        "window.unim-special-popup {"
        "  background-color: #2d2d2d; border: 1px solid #555;"
        "  padding: 0; margin: 0; }"
        ".unim-special-vbox {"
        "  padding: 0; margin: 0; }"
        ".unim-special-vbox grid {"
        "  padding: 0; }"
        ".unim-special-vbox label { color: #e0e0e0; font-size: 14px; }"
        ".unim-special-vbox label.header { color: #888; font-size: 11px; font-weight: bold; min-height: 20px; }"
        ".unim-special-vbox label.header.row-header { min-width: 20px; }"
        ".unim-special-vbox label.header-active { color: #7ab8ff; font-size: 11px; font-weight: bold; min-height: 20px; }"
        ".unim-special-vbox label.cell { padding: 2px 4px; min-width: 24px; min-height: 20px; }"
        ".unim-special-vbox label.cell-col-highlight { background-color: #383838; border-radius: 2px; }"
        ".unim-special-vbox label.cell-row-highlight { background-color: #383838; border-radius: 2px; }"
        ".unim-special-vbox label.cell-selected { background-color: #4a90d9; color: white; border-radius: 3px; }"
        ".unim-special-vbox label.cell-flash { background-color: #2ecc71; color: white; border-radius: 3px; }"
        ".unim-special-vbox label.footer { color: #999; font-size: 11px; min-height: 0; padding: 0; margin: 0; }"
    );
    gtk_style_context_add_provider_for_display(
        gdk_display_get_default(),
        GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_USER
    );
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

    if (popup->window) {
        gtk_window_destroy(GTK_WINDOW(popup->window));
    }

    g_free(popup);
}

/* 마우스 클릭 콜백 - 셀 클릭 시 해당 문자 선택 */
static void
on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data)
{
    (void)gesture; (void)n_press; (void)x; (void)y;
    UnimSpecialPopup *popup = (UnimSpecialPopup *)user_data;

    /* 클릭된 위젯 찾기 */
    GtkWidget *picked = gtk_widget_pick(popup->grid, x, y, GTK_PICK_DEFAULT);
    if (!picked) return;

    for (gint r = 0; r < MAX_ROWS; r++) {
        for (gint c = 0; c < MAX_COLS; c++) {
            if (popup->cells[r][c] == picked) {
                popup->sel_row = r;
                popup->sel_col = c;
                update_selection(popup);
                select_current(popup);
                return;
            }
        }
    }
}

static void
clear_grid(UnimSpecialPopup *popup)
{
    /* 기존 그리드 내용 모두 제거 */
    GtkWidget *child;
    while ((child = gtk_widget_get_first_child(popup->grid)) != NULL) {
        gtk_grid_remove(GTK_GRID(popup->grid), child);
    }
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
    gtk_widget_add_css_class(corner, "header");
    gtk_grid_attach(GTK_GRID(popup->grid), corner, 0, 0, 1, 1);

    /* 열 헤더 (top_row 기반) */
    for (gint c = 0; c < popup->cols; c++) {
        gchar header_text[4];
        g_snprintf(header_text, sizeof(header_text), "%c", popup->top_row[c]);
        GtkWidget *header = gtk_label_new(header_text);
        gtk_widget_add_css_class(header, "header");
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
                gtk_widget_add_css_class(row_label, "header");
                gtk_widget_add_css_class(row_label, "row-header");
                gtk_widget_set_halign(row_label, GTK_ALIGN_CENTER);
                gtk_grid_attach(GTK_GRID(popup->grid), row_label, 0, r + 1, 1, 1);
                popup->row_headers[r] = row_label;
            }

            /* 문자 셀 */
            GtkWidget *cell = gtk_label_new(popup->characters[idx]);
            gtk_widget_add_css_class(cell, "cell");
            gtk_widget_set_halign(cell, GTK_ALIGN_CENTER);
            gtk_grid_attach(GTK_GRID(popup->grid), cell, c + 1, r + 1, 1, 1);
            popup->cells[r][c] = cell;
        }
    }

    /* 페이지 표시 (2페이지 이상일 때만) */
    if (popup->footer_label && popup->total_pages > 1) {
        gchar page_text[32];
        g_snprintf(page_text, sizeof(page_text), "%d / %d",
                   popup->current_page + 1, popup->total_pages);
        gtk_label_set_text(GTK_LABEL(popup->footer_label), page_text);
        gtk_widget_add_css_class(popup->footer_label, "footer");
        gtk_widget_set_visible(popup->footer_label, TRUE);
    } else if (popup->footer_label) {
        gtk_widget_set_visible(popup->footer_label, FALSE);
    }

    update_selection(popup);
}

static void
update_selection(UnimSpecialPopup *popup)
{
    /* 모든 셀의 스타일 제거 */
    for (gint r = 0; r < MAX_ROWS; r++) {
        for (gint c = 0; c < MAX_COLS; c++) {
            if (popup->cells[r][c]) {
                gtk_widget_remove_css_class(popup->cells[r][c], "cell-selected");
                gtk_widget_remove_css_class(popup->cells[r][c], "cell-col-highlight");
                gtk_widget_remove_css_class(popup->cells[r][c], "cell-row-highlight");
            }
        }
    }

    /* 열 헤더 스타일 초기화 */
    for (gint c = 0; c < MAX_COLS; c++) {
        if (popup->col_headers[c]) {
            gtk_widget_remove_css_class(popup->col_headers[c], "header-active");
            gtk_widget_add_css_class(popup->col_headers[c], "header");
        }
    }

    /* 행 헤더 스타일 초기화 */
    for (gint r = 0; r < MAX_ROWS; r++) {
        if (popup->row_headers[r]) {
            gtk_widget_remove_css_class(popup->row_headers[r], "header-active");
            gtk_widget_add_css_class(popup->row_headers[r], "header");
        }
    }

    if (popup->sel_col >= 0 && popup->sel_col < popup->cols) {
        /* 선택된 열 전체 강조 */
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[r][popup->sel_col]) {
                gtk_widget_add_css_class(popup->cells[r][popup->sel_col], "cell-col-highlight");
            }
        }

        /* 선택된 행 전체 강조 */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS) {
            for (gint c = 0; c < popup->cols; c++) {
                if (popup->cells[popup->sel_row][c]) {
                    gtk_widget_add_css_class(popup->cells[popup->sel_row][c], "cell-row-highlight");
                }
            }
        }

        /* 선택된 열 헤더 강조 */
        if (popup->col_headers[popup->sel_col]) {
            gtk_widget_remove_css_class(popup->col_headers[popup->sel_col], "header");
            gtk_widget_add_css_class(popup->col_headers[popup->sel_col], "header-active");
        }

        /* 선택된 행 헤더 강조 */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->row_headers[popup->sel_row]) {
            gtk_widget_remove_css_class(popup->row_headers[popup->sel_row], "header");
            gtk_widget_add_css_class(popup->row_headers[popup->sel_row], "header-active");
        }

        /* 현재 선택 셀에 선택 스타일 적용 (강조 위에 겹침) */
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->cells[popup->sel_row][popup->sel_col]) {
            gtk_widget_add_css_class(
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
            gtk_widget_remove_css_class(cell, "cell-selected");
            gtk_widget_add_css_class(cell, "cell-flash");
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

    /* top_row 저장 (기본값: QWERTYUIO) */
    if (top_row && strlen(top_row) >= 9) {
        strncpy(popup->top_row, top_row, 9);
        popup->top_row[9] = '\0';
    } else {
        strncpy(popup->top_row, "QWERTYUIO", 10);
    }

    SPECIAL_DEBUG("특수문자 팝업 표시: target='%s', count=%zu, pages=%d",
                  target, count, popup->total_pages);

    /* 그리드 업데이트 */
    update_grid(popup);

    /* realize 먼저 (창을 표시하지 않고 X11 윈도우만 생성) */
    gtk_widget_realize(popup->window);

    /* 크기 측정 */
    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    gint width = req.width;
    gint height = req.height;

    /* 위치 설정 */
    gint final_x = x;
    gint final_y = y;

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

            /* 화면 경계 보정 */
            if (final_x + width > screen_w) {
                final_x = screen_w - width;
                if (final_x < 0) final_x = 0;
            }
            if (final_y + height > screen_h) {
                final_y = y - cursor_height - height;
                if (final_y < 0) final_y = 0;
            }

            /* override_redirect 설정 (WM이 이 창을 무시하도록) */
            XSetWindowAttributes attrs;
            attrs.override_redirect = True;
            XChangeWindowAttributes(xdisplay, xwindow, CWOverrideRedirect, &attrs);

            /* 위치 이동 (표시 전에!) */
            XMoveWindow(xdisplay, xwindow, final_x, final_y);
        }
    }
#endif

    /* 위치 설정 완료 후 마지막에 표시 */
    gtk_widget_set_visible(popup->window, TRUE);

    SPECIAL_DEBUG("팝업 위치: (%d, %d), 크기: %dx%d", final_x, final_y, width, height);
}

void
unim_special_popup_hide(UnimSpecialPopup *popup)
{
    if (!popup || !popup->window) return;

    gtk_widget_set_visible(popup->window, FALSE);
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
    if (!popup || !popup->characters || popup->pending_hide) return FALSE;

    /* top_row 키: 열 점프 (영문 키맵의 상단 행 키) */
    {
        guint lower = gdk_keyval_to_lower(keyval);
        for (gint i = 0; i < 9; i++) {
            if (popup->top_row[i] == '\0') break;
            guint expected = gdk_unicode_to_keyval(
                g_unichar_tolower((gunichar)popup->top_row[i]));
            if (lower == expected) {
                if (i < popup->cols && cell_has_char(popup, 0, i)) {
                    popup->sel_col = i;
                    if (!cell_has_char(popup, popup->sel_row, popup->sel_col)) {
                        popup->sel_row = 0;
                    }
                    update_selection(popup);
                    SPECIAL_DEBUG("열 점프: %c → 열 %d", (char)popup->top_row[i], i);
                }
                return TRUE;
            }
        }
    }

    switch (keyval) {    /* 숫자 1-9: 현재 열의 N번째 행 선택 */
    case GDK_KEY_1: case GDK_KEY_2: case GDK_KEY_3:
    case GDK_KEY_4: case GDK_KEY_5: case GDK_KEY_6:
    case GDK_KEY_7: case GDK_KEY_8: case GDK_KEY_9:
    {
        gint row = keyval - GDK_KEY_1;
        if (cell_has_char(popup, row, popup->sel_col)) {
            popup->sel_row = row;
            update_selection(popup);
            select_current(popup);
        }
        return TRUE;
    }

    /* 화살표 키: 네비게이션 (열 내 무한 루프) */
    case GDK_KEY_Up:
        if (popup->sel_row > 0) {
            popup->sel_row--;
        } else {
            /* 위 가장자리 → 같은 열 마지막 행 (무한 루프) */
            gint last_row = MAX_ROWS - 1;
            while (last_row > 0 && !cell_has_char(popup, last_row, popup->sel_col)) {
                last_row--;
            }
            popup->sel_row = last_row;
        }
        update_selection(popup);
        return TRUE;

    case GDK_KEY_Down:
        if (popup->sel_row < MAX_ROWS - 1 &&
            cell_has_char(popup, popup->sel_row + 1, popup->sel_col)) {
            popup->sel_row++;
        } else {
            /* 아래 가장자리 → 같은 열 첫 행 (무한 루프) */
            popup->sel_row = 0;
        }
        update_selection(popup);
        return TRUE;

    case GDK_KEY_Left:
        if (popup->sel_col > 0) {
            popup->sel_col--;
        } else {
            /* 왼쪽 가장자리 → 마지막 열 (무한 루프) */
            popup->sel_col = popup->cols - 1;
        }
        if (!cell_has_char(popup, popup->sel_row, popup->sel_col)) {
            popup->sel_row = 0;
        }
        update_selection(popup);
        return TRUE;

    case GDK_KEY_Right:
        if (popup->sel_col < popup->cols - 1) {
            popup->sel_col++;
        } else {
            /* 오른쪽 가장자리 → 첫 열 (무한 루프) */
            popup->sel_col = 0;
        }
        if (!cell_has_char(popup, popup->sel_row, popup->sel_col)) {
            popup->sel_row = 0;
        }
        update_selection(popup);
        return TRUE;

    /* 페이지 이동 */
    case GDK_KEY_Page_Up:
        if (popup->current_page > 0) {
            popup->current_page--;
            popup->sel_row = 0;
            popup->sel_col = 0;
            update_grid(popup);
        }
        return TRUE;

    case GDK_KEY_Page_Down:
    case GDK_KEY_space:
        if (popup->current_page < popup->total_pages - 1) {
            popup->current_page++;
            popup->sel_row = 0;
            popup->sel_col = 0;
            update_grid(popup);
        }
        return TRUE;

    /* Enter: 선택 확정 */
    case GDK_KEY_Return:
    case GDK_KEY_KP_Enter:
        select_current(popup);
        return TRUE;

    /* Tab: 다음 페이지 (무한 루프), Shift+Tab: 이전 페이지 (무한 루프) */
    case GDK_KEY_Tab:
    case GDK_KEY_ISO_Left_Tab:  /* Shift+Tab */
        if (keyval == GDK_KEY_ISO_Left_Tab) {
            if (popup->current_page > 0) {
                popup->current_page--;
            } else {
                popup->current_page = popup->total_pages - 1;
            }
        } else {
            if (popup->current_page < popup->total_pages - 1) {
                popup->current_page++;
            } else {
                popup->current_page = 0;
            }
        }
        popup->sel_col = 0;
        popup->sel_row = 0;
        update_grid(popup);
        update_selection(popup);
        return TRUE;

    /* 수정자 키 무시 (소비) */
    case GDK_KEY_Shift_L: case GDK_KEY_Shift_R:
    case GDK_KEY_Control_L: case GDK_KEY_Control_R:
    case GDK_KEY_Alt_L: case GDK_KEY_Alt_R:
    case GDK_KEY_Super_L: case GDK_KEY_Super_R:
        return TRUE;

    default:
        /* 미처리 키 → 팝업 밖으로 전달 */
        return FALSE;
    }
}
