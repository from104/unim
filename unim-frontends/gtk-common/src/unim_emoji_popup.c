/**
 * UNIM 이모지 팝업 구현 (GTK-공통)
 *
 * `ShowEmojiPopupV2` DBus 시그널을 받아 9×9 그리드 + 좌측 9 탭 (Recent + 8 카테고리)
 * 으로 이모지를 표시. 키 처리는 엔진 (popup_state) 이 담당하므로 IM 모듈은
 * 시그널 수신만 하고 본 모듈은 표시·페이지/셀 갱신·셀 클릭만 처리.
 *
 * 한자/특수문자 팝업과의 차이:
 * - `handle_key` 없음 (엔진이 키 처리)
 * - 즐겨찾기 대신 'Recent' (MRU) 캐시
 * - 카테고리 전환은 `ShowEmojiPopupV2` 재발행으로 처리됨 → `show()` 재호출
 */

#include "unim_emoji_popup.h"
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
#define PAGE_SIZE (MAX_ROWS * MAX_COLS)  /* 81 */
#define TAB_COUNT 9                       /* Recent + 8 카테고리 */
#define FLASH_DURATION_MS 120

/* 디버그 로깅 */
static gboolean emoji_popup_debug_enabled = FALSE;
static gboolean emoji_popup_debug_checked = FALSE;

static void
emoji_popup_check_debug(void)
{
    if (!emoji_popup_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            emoji_popup_debug_enabled = TRUE;
        }
        emoji_popup_debug_checked = TRUE;
    }
}

#define EMOJI_DEBUG(fmt, ...) do { \
    if (emoji_popup_debug_enabled) \
        g_print("[EMOJI_POPUP] " fmt "\n", ##__VA_ARGS__); \
} while(0)

/* GTK3/4 호환 CSS class 관리 매크로 */
#if GTK_CHECK_VERSION(4, 0, 0)
#define WIDGET_ADD_CSS_CLASS(w, cls)    gtk_widget_add_css_class(w, cls)
#define WIDGET_REMOVE_CSS_CLASS(w, cls) gtk_widget_remove_css_class(w, cls)
#else
static void
emoji_widget_add_css_class(GtkWidget *w, const char *cls) {
    GtkStyleContext *ctx = gtk_widget_get_style_context(w);
    gtk_style_context_add_class(ctx, cls);
}
static void
emoji_widget_remove_css_class(GtkWidget *w, const char *cls) {
    GtkStyleContext *ctx = gtk_widget_get_style_context(w);
    gtk_style_context_remove_class(ctx, cls);
}
#define WIDGET_ADD_CSS_CLASS(w, cls)    emoji_widget_add_css_class(w, cls)
#define WIDGET_REMOVE_CSS_CLASS(w, cls) emoji_widget_remove_css_class(w, cls)
#endif

struct _UnimEmojiPopup {
    GtkWidget *window;
    GtkWidget *header_label;          /* 「카테고리」 → 이모지 */
    GtkWidget *grid;                  /* 9×9 grid */
    GtkWidget *footer_box;            /* ◀ + label + ▶ — 단일 페이지 시 hide */
    GtkWidget *footer_label;          /* 페이지 인디케이터 */
    GtkWidget *prev_page_btn;         /* ◀ */
    GtkWidget *next_page_btn;         /* ▶ */

    /* 좌측 9 탭 */
    GtkWidget *tab_buttons[TAB_COUNT];
    UnimEmojiCategory tab_meta[TAB_COUNT];  /* 깊은 복사된 카테고리 메타 */
    gsize tab_count;                  /* 실제 사용 중인 탭 개수 */

    /* 셀 위젯 (column-major: cells[col][row]) */
    GtkWidget *cells[MAX_COLS][MAX_ROWS];

    /* 컬럼 헤더 (top_row) / 행 번호 라벨 */
    GtkWidget *col_headers[MAX_COLS];
    GtkWidget *row_headers[MAX_ROWS];
    gchar top_row[10];                /* 예: "QWERTYUIO" */

    /* 데이터 */
    gchar **items;                    /* 현재 카테고리 이모지 풀 (깊은 복사) */
    gsize item_count;
    gchar **recent;                   /* 'Recent' 캐시 (깊은 복사, 미사용 가능) */
    gsize recent_count;
    gchar *current_cat_id;            /* 현재 카테고리 id */
    gint current_page;
    gint total_pages;
    gint sel_row;
    gint sel_col;

    /* 콜백 */
    UnimEmojiCommitCallback callback;
    gpointer user_data;

    /* 페이지 이동 콜백 (◀/▶ 클릭 → DBus PopupChangePage) */
    void (*page_change_callback)(gint direction, gpointer user_data);
    gpointer page_change_user_data;

    /* 클릭 후 플래시 표시용 지연 숨김 플래그 */
    gboolean pending_hide;
};

/* 전방 선언 */
static void update_grid(UnimEmojiPopup *popup);
static void update_selection(UnimEmojiPopup *popup);
static void clear_grid(UnimEmojiPopup *popup);
static void emoji_prev_page_clicked(GtkButton *button, gpointer user_data);
static void emoji_next_page_clicked(GtkButton *button, gpointer user_data);
static void free_categories(UnimEmojiPopup *popup);
static void free_items(UnimEmojiPopup *popup);
static void free_recent(UnimEmojiPopup *popup);
static const gchar* tab_label_for(const gchar *cat_id);

#if GTK_CHECK_VERSION(4, 0, 0)
static void on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data);
#else
static gboolean on_grid_button_press(GtkWidget *widget, GdkEventButton *event, gpointer user_data);
#endif

/* 카테고리 id → 한국어 라벨 (카테고리 메타 미리보기용 fallback) */
static const gchar*
tab_label_for(const gchar *cat_id)
{
    if (!cat_id) return "";
    if (g_strcmp0(cat_id, "Recent") == 0) return "최근";
    if (g_strcmp0(cat_id, "SmileysPeople") == 0) return "표정·인물";
    if (g_strcmp0(cat_id, "Animals") == 0) return "동물·자연";
    if (g_strcmp0(cat_id, "Food") == 0) return "음식·음료";
    if (g_strcmp0(cat_id, "Activities") == 0) return "활동";
    if (g_strcmp0(cat_id, "Travel") == 0) return "여행·장소";
    if (g_strcmp0(cat_id, "Objects") == 0) return "사물";
    if (g_strcmp0(cat_id, "Symbols") == 0) return "기호";
    if (g_strcmp0(cat_id, "Flags") == 0) return "국기";
    return cat_id;
}

UnimEmojiPopup*
unim_emoji_popup_new(void)
{
    emoji_popup_check_debug();

    UnimEmojiPopup *popup = g_new0(UnimEmojiPopup, 1);

#if GTK_CHECK_VERSION(4, 0, 0)
    popup->window = gtk_window_new();
    gtk_window_set_decorated(GTK_WINDOW(popup->window), FALSE);
    gtk_window_set_resizable(GTK_WINDOW(popup->window), FALSE);
    gtk_widget_set_can_focus(popup->window, FALSE);

    gtk_accessible_update_property(GTK_ACCESSIBLE(popup->window),
        GTK_ACCESSIBLE_PROPERTY_LABEL, "UNIM 이모지 팝업",
        -1);
#else
    popup->window = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_type_hint(GTK_WINDOW(popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
    gtk_widget_set_can_focus(popup->window, FALSE);

    {
        AtkObject *atk_obj = gtk_widget_get_accessible(popup->window);
        if (atk_obj) {
            atk_object_set_role(atk_obj, ATK_ROLE_POPUP_MENU);
            atk_object_set_name(atk_obj, "UNIM 이모지 팝업");
        }
    }
#endif

    /* 외곽: vbox = [header, hbox(left_tabs | grid), footer] */
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

    /* 본문 hbox: [좌측 탭 | 우측 grid] */
    GtkWidget *hbox = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(hbox, 4);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(vbox), hbox);
#else
    gtk_box_pack_start(GTK_BOX(vbox), hbox, TRUE, TRUE, 0);
#endif

    /* 좌측 세로 탭 컨테이너 */
    GtkWidget *tab_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    WIDGET_ADD_CSS_CLASS(tab_box, "emoji-tabs");
    gtk_widget_set_valign(tab_box, GTK_ALIGN_START);

    /* 9 탭 ToggleButton 미리 생성 — show() 에서 라벨 갱신 */
    for (gint i = 0; i < TAB_COUNT; i++) {
        GtkWidget *btn = gtk_toggle_button_new_with_label("");
        WIDGET_ADD_CSS_CLASS(btn, "emoji-tab-button");
        gtk_widget_set_can_focus(btn, FALSE);
        popup->tab_buttons[i] = btn;
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_box_append(GTK_BOX(tab_box), btn);
#else
        gtk_box_pack_start(GTK_BOX(tab_box), btn, FALSE, FALSE, 0);
#endif
    }
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(hbox), tab_box);
#else
    gtk_box_pack_start(GTK_BOX(hbox), tab_box, FALSE, FALSE, 0);
#endif

    /* 우측 grid */
    popup->grid = gtk_grid_new();
    gtk_grid_set_row_spacing(GTK_GRID(popup->grid), 1);
    gtk_grid_set_column_spacing(GTK_GRID(popup->grid), 1);
    gtk_widget_set_margin_start(popup->grid, 4);
    gtk_widget_set_margin_end(popup->grid, 4);
    gtk_widget_set_margin_top(popup->grid, 4);
    gtk_widget_set_margin_bottom(popup->grid, 2);

#if GTK_CHECK_VERSION(4, 0, 0)
    GtkGesture *click = gtk_gesture_click_new();
    gtk_gesture_single_set_button(GTK_GESTURE_SINGLE(click), GDK_BUTTON_PRIMARY);
    g_signal_connect(click, "pressed", G_CALLBACK(on_cell_clicked), popup);
    gtk_widget_add_controller(popup->grid, GTK_EVENT_CONTROLLER(click));
    gtk_box_append(GTK_BOX(hbox), popup->grid);
#else
    gtk_widget_set_events(popup->grid,
        gtk_widget_get_events(popup->grid) | GDK_BUTTON_PRESS_MASK);
    gtk_box_pack_start(GTK_BOX(hbox), popup->grid, TRUE, TRUE, 0);
    gtk_widget_add_events(popup->window, GDK_BUTTON_PRESS_MASK);
    g_signal_connect(popup->window, "button-press-event",
        G_CALLBACK(on_grid_button_press), popup);
#endif

    /* 페이지 인디케이터: [◀] [카테고리 n/N] [▶] — 단일 페이지 시 footer_box hide */
    popup->footer_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    WIDGET_ADD_CSS_CLASS(popup->footer_box, "popup-footer-box");

    popup->prev_page_btn = gtk_button_new_with_label("\xE2\x97\x80"); /* ◀ */
    WIDGET_ADD_CSS_CLASS(popup->prev_page_btn, "popup-page-btn");
    WIDGET_ADD_CSS_CLASS(popup->prev_page_btn, "flat");
    gtk_widget_set_can_focus(popup->prev_page_btn, FALSE);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_focusable(popup->prev_page_btn, FALSE);
#endif
    /* English fallback — IM 모듈에 i18n 인프라 부재. msgid 표준으로 통일. */
    gtk_widget_set_tooltip_text(popup->prev_page_btn, "Previous page");
    g_signal_connect(popup->prev_page_btn, "clicked",
                     G_CALLBACK(emoji_prev_page_clicked), popup);

    popup->footer_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(popup->footer_label), 0.5);
    gtk_widget_set_hexpand(popup->footer_label, TRUE);
    WIDGET_ADD_CSS_CLASS(popup->footer_label, "popup-footer");

    popup->next_page_btn = gtk_button_new_with_label("\xE2\x96\xB6"); /* ▶ */
    WIDGET_ADD_CSS_CLASS(popup->next_page_btn, "popup-page-btn");
    WIDGET_ADD_CSS_CLASS(popup->next_page_btn, "flat");
    gtk_widget_set_can_focus(popup->next_page_btn, FALSE);
#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_focusable(popup->next_page_btn, FALSE);
#endif
    gtk_widget_set_tooltip_text(popup->next_page_btn, "Next page");
    g_signal_connect(popup->next_page_btn, "clicked",
                     G_CALLBACK(emoji_next_page_clicked), popup);

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_box_append(GTK_BOX(popup->footer_box), popup->prev_page_btn);
    gtk_box_append(GTK_BOX(popup->footer_box), popup->footer_label);
    gtk_box_append(GTK_BOX(popup->footer_box), popup->next_page_btn);
    gtk_box_append(GTK_BOX(vbox), popup->footer_box);
#else
    gtk_box_pack_start(GTK_BOX(popup->footer_box), popup->prev_page_btn, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(popup->footer_box), popup->footer_label, TRUE, TRUE, 0);
    gtk_box_pack_start(GTK_BOX(popup->footer_box), popup->next_page_btn, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(vbox), popup->footer_box, FALSE, FALSE, 0);
    gtk_widget_set_no_show_all(popup->footer_box, TRUE);
#endif

    /* 팝업 전용 CSS */
    WIDGET_ADD_CSS_CLASS(popup->window, "unim-emoji-popup");
    WIDGET_ADD_CSS_CLASS(vbox, "unim-emoji-vbox");

    GtkCssProvider *css = gtk_css_provider_new();
    const gchar *css_text =
        "window.unim-emoji-popup {"
        "  background-color: rgba(30, 30, 46, 0.97);"
        "  border: 1px solid rgba(255, 255, 255, 0.15);"
        "  border-radius: 12px;"
        "  padding: 0; margin: 0;"
        "}"
        ".unim-emoji-vbox {"
        "  padding: 8px; margin: 0;"
        "}"
        ".unim-emoji-vbox label.popup-header {"
        "  background-color: #313244;"
        "  color: #a6e3a1;"
        "  font-size: 13px;"
        "  font-weight: bold;"
        "  padding: 6px 8px;"
        "  border-radius: 4px;"
        "  margin-bottom: 6px;"
        "}"
        ".unim-emoji-vbox .emoji-tabs {"
        "  padding: 2px 0;"
        "}"
        ".unim-emoji-vbox button.emoji-tab-button {"
        "  min-width: 96px;"
        "  padding: 4px 8px;"
        "  color: #cdd6f4;"
        "  background: transparent;"
        "  border-radius: 6px;"
        "  font-size: 11px;"
        "}"
        ".unim-emoji-vbox button.emoji-tab-button:hover {"
        "  background-color: rgba(255, 255, 255, 0.06);"
        "}"
        ".unim-emoji-vbox button.emoji-tab-button:checked {"
        "  background-color: rgba(166, 227, 161, 0.25);"
        "  color: #a6e3a1;"
        "  font-weight: bold;"
        "}"
        ".unim-emoji-vbox grid {"
        "  padding: 0;"
        "}"
        ".unim-emoji-vbox label.cell {"
        "  color: #cdd6f4; font-size: 18px;"
        "  padding: 2px; min-width: 30px; min-height: 30px;"
        "  border-radius: 4px;"
        "}"
        ".unim-emoji-vbox label.cell:hover {"
        "  background-color: rgba(255, 255, 255, 0.05);"
        "}"
        ".unim-emoji-vbox label.cell-col-highlight {"
        "  background-color: rgba(166, 227, 161, 0.08);"
        "  border-radius: 4px;"
        "}"
        ".unim-emoji-vbox label.cell-row-highlight {"
        "  background-color: rgba(166, 227, 161, 0.08);"
        "  border-radius: 4px;"
        "}"
        ".unim-emoji-vbox label.cell-selected {"
        "  background-color: rgba(166, 227, 161, 0.25);"
        "  color: #cdd6f4;"
        "  font-weight: bold;"
        "  border-radius: 6px;"
        "}"
        ".unim-emoji-vbox label.cell-flash {"
        "  background-color: #a6e3a1; color: #1e1e2e;"
        "  font-weight: bold; border-radius: 6px;"
        "}"
        ".unim-emoji-vbox label.header {"
        "  color: #f9e2af; font-size: 11px; font-weight: bold;"
        "  min-height: 18px; min-width: 28px;"
        "}"
        ".unim-emoji-vbox label.header.row-header {"
        "  color: #7f849c; min-width: 20px;"
        "}"
        ".unim-emoji-vbox label.header-active {"
        "  color: #a6e3a1; font-size: 11px; font-weight: bold;"
        "  min-height: 18px;"
        "}"
        ".unim-emoji-vbox label.popup-footer {"
        "  color: #6c7086; font-size: 12px;"
        "  min-height: 0; padding: 0; margin: 0;"
        "}"
        /* 마우스 페이지 이동 ◀/▶ (Phase 4)
         * hit-target 28×28px — WCAG 2.5.5 (24×24) 초과, 셀(28px)과 시각 비례 */
        ".unim-emoji-vbox .popup-footer-box { margin-top: 4px; }"
        ".unim-emoji-vbox button.popup-page-btn {"
        "  color: #7f849c; background: transparent; border: none; box-shadow: none;"
        "  font-size: 14px; min-width: 28px; min-height: 28px;"
        "  padding: 4px 10px; margin: 0; border-radius: 4px;"
        "}"
        ".unim-emoji-vbox button.popup-page-btn:hover {"
        "  color: #89b4fa; background-color: rgba(137, 180, 250, 0.2);"
        "}"
        ".unim-emoji-vbox button.popup-page-btn:active {"
        "  background-color: rgba(137, 180, 250, 0.35);"
        "}";

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_css_provider_load_from_string(css, css_text);
    gtk_style_context_add_provider_for_display(
        gdk_display_get_default(),
        GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_USER);
#else
    gtk_css_provider_load_from_data(css, css_text, -1, NULL);
    gtk_style_context_add_provider_for_screen(
        gdk_screen_get_default(),
        GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_USER);
#endif
    g_object_unref(css);

    popup->sel_row = 0;
    popup->sel_col = 0;
    popup->current_page = 0;
    popup->total_pages = 1;
    popup->tab_count = 0;
    strncpy(popup->top_row, "QWERTYUIO", 10);

    EMOJI_DEBUG("팝업 인스턴스 생성");
    return popup;
}

static void
free_categories(UnimEmojiPopup *popup)
{
    for (gsize i = 0; i < popup->tab_count; i++) {
        g_free(popup->tab_meta[i].id);
        g_free(popup->tab_meta[i].name_ko);
        g_free(popup->tab_meta[i].name_en);
        popup->tab_meta[i].id = NULL;
        popup->tab_meta[i].name_ko = NULL;
        popup->tab_meta[i].name_en = NULL;
        popup->tab_meta[i].count = 0;
    }
    popup->tab_count = 0;
}

static void
free_items(UnimEmojiPopup *popup)
{
    if (popup->items) {
        for (gsize i = 0; i < popup->item_count; i++) {
            g_free(popup->items[i]);
        }
        g_free(popup->items);
        popup->items = NULL;
        popup->item_count = 0;
    }
}

static void
free_recent(UnimEmojiPopup *popup)
{
    if (popup->recent) {
        for (gsize i = 0; i < popup->recent_count; i++) {
            g_free(popup->recent[i]);
        }
        g_free(popup->recent);
        popup->recent = NULL;
        popup->recent_count = 0;
    }
}

void
unim_emoji_popup_free(UnimEmojiPopup *popup)
{
    if (!popup) return;

    free_items(popup);
    free_recent(popup);
    free_categories(popup);
    g_free(popup->current_cat_id);

    if (popup->window) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_window_destroy(GTK_WINDOW(popup->window));
#else
        gtk_widget_destroy(popup->window);
#endif
    }

    g_free(popup);
}

static void
clear_grid(UnimEmojiPopup *popup)
{
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
update_grid(UnimEmojiPopup *popup)
{
    clear_grid(popup);

    gsize page_start = (gsize)popup->current_page * PAGE_SIZE;
    gsize page_chars = 0;
    if (popup->item_count > page_start) {
        page_chars = popup->item_count - page_start;
        if (page_chars > PAGE_SIZE) page_chars = PAGE_SIZE;
    }

    /* 활성 cols/rows (column-major 채움) */
    gint active_cols = (gint)((page_chars + MAX_ROWS - 1) / MAX_ROWS);
    if (active_cols > MAX_COLS) active_cols = MAX_COLS;
    if (active_cols < 1) active_cols = (page_chars > 0 ? 1 : 0);
    gint active_rows = (gint)(page_chars < (gsize)MAX_ROWS ? page_chars : MAX_ROWS);

    /* 좌상단 빈 코너 */
    GtkWidget *corner = gtk_label_new("  ");
    WIDGET_ADD_CSS_CLASS(corner, "header");
    WIDGET_ADD_CSS_CLASS(corner, "row-header");
    gtk_grid_attach(GTK_GRID(popup->grid), corner, 0, 0, 1, 1);

    /* 컬럼 헤더 (top_row 기반) */
    for (gint c = 0; c < active_cols; c++) {
        gchar header_text[4];
        g_snprintf(header_text, sizeof(header_text), "%c", popup->top_row[c]);
        GtkWidget *header = gtk_label_new(header_text);
        WIDGET_ADD_CSS_CLASS(header, "header");
        gtk_widget_set_halign(header, GTK_ALIGN_CENTER);
        gtk_grid_attach(GTK_GRID(popup->grid), header, c + 1, 0, 1, 1);
        popup->col_headers[c] = header;
    }

    /* 셀 (column-major) */
    for (gint c = 0; c < active_cols; c++) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            gint idx = (gint)page_start + c * MAX_ROWS + r;
            if (idx >= (gint)popup->item_count) break;

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

            GtkWidget *cell = gtk_label_new(popup->items[idx]);
            WIDGET_ADD_CSS_CLASS(cell, "cell");
            gtk_widget_set_halign(cell, GTK_ALIGN_CENTER);
            gtk_grid_attach(GTK_GRID(popup->grid), cell, c + 1, r + 1, 1, 1);
            popup->cells[c][r] = cell;
        }
    }

    /* 페이지 인디케이터: 단일 페이지면 footer_box 자체 hide (◀/▶ 같이 사라짐) */
    if (popup->footer_box && popup->total_pages > 1) {
        gchar page_text[128];
        g_snprintf(page_text, sizeof(page_text), "[%s]  %d/%d",
                   tab_label_for(popup->current_cat_id),
                   popup->current_page + 1, popup->total_pages);
        gtk_label_set_text(GTK_LABEL(popup->footer_label), page_text);
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->footer_box, TRUE);
        gtk_widget_set_visible(popup->footer_label, TRUE);
        gtk_widget_set_visible(popup->prev_page_btn, TRUE);
        gtk_widget_set_visible(popup->next_page_btn, TRUE);
#else
        gtk_widget_show_all(popup->footer_box);
#endif
    } else if (popup->footer_box) {
#if GTK_CHECK_VERSION(4, 0, 0)
        gtk_widget_set_visible(popup->footer_box, FALSE);
#else
        gtk_widget_hide(popup->footer_box);
#endif
    }

    update_selection(popup);

#if !GTK_CHECK_VERSION(4, 0, 0)
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
update_selection(UnimEmojiPopup *popup)
{
    /* 모든 셀 스타일 초기화 */
    for (gint c = 0; c < MAX_COLS; c++) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[c][r]) {
                WIDGET_REMOVE_CSS_CLASS(popup->cells[c][r], "cell-selected");
                WIDGET_REMOVE_CSS_CLASS(popup->cells[c][r], "cell-col-highlight");
                WIDGET_REMOVE_CSS_CLASS(popup->cells[c][r], "cell-row-highlight");
            }
        }
    }
    for (gint c = 0; c < MAX_COLS; c++) {
        if (popup->col_headers[c]) {
            WIDGET_REMOVE_CSS_CLASS(popup->col_headers[c], "header-active");
            WIDGET_ADD_CSS_CLASS(popup->col_headers[c], "header");
        }
    }
    for (gint r = 0; r < MAX_ROWS; r++) {
        if (popup->row_headers[r]) {
            WIDGET_REMOVE_CSS_CLASS(popup->row_headers[r], "header-active");
            WIDGET_ADD_CSS_CLASS(popup->row_headers[r], "header");
        }
    }

    if (popup->sel_col >= 0 && popup->sel_col < MAX_COLS) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[popup->sel_col][r]) {
                WIDGET_ADD_CSS_CLASS(popup->cells[popup->sel_col][r], "cell-col-highlight");
            }
        }
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS) {
            for (gint c = 0; c < MAX_COLS; c++) {
                if (popup->cells[c][popup->sel_row]) {
                    WIDGET_ADD_CSS_CLASS(popup->cells[c][popup->sel_row], "cell-row-highlight");
                }
            }
        }
        if (popup->col_headers[popup->sel_col]) {
            WIDGET_REMOVE_CSS_CLASS(popup->col_headers[popup->sel_col], "header");
            WIDGET_ADD_CSS_CLASS(popup->col_headers[popup->sel_col], "header-active");
        }
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->row_headers[popup->sel_row]) {
            WIDGET_REMOVE_CSS_CLASS(popup->row_headers[popup->sel_row], "header");
            WIDGET_ADD_CSS_CLASS(popup->row_headers[popup->sel_row], "header-active");
        }
        if (popup->sel_row >= 0 && popup->sel_row < MAX_ROWS &&
            popup->cells[popup->sel_col][popup->sel_row]) {
            WIDGET_ADD_CSS_CLASS(
                popup->cells[popup->sel_col][popup->sel_row], "cell-selected");
        }
    }
}

/* 플래시 후 숨김 */
static gboolean
on_flash_timeout(gpointer user_data)
{
    UnimEmojiPopup *popup = (UnimEmojiPopup *)user_data;
    unim_emoji_popup_hide(popup);
    popup->pending_hide = FALSE;
    return G_SOURCE_REMOVE;
}

static void
emit_select(UnimEmojiPopup *popup, gint col, gint row)
{
    gint page_start = popup->current_page * PAGE_SIZE;
    gint idx = page_start + col * MAX_ROWS + row;
    if (idx < 0 || idx >= (gint)popup->item_count) return;
    if (!popup->items[idx]) return;

    /* 플래시 */
    GtkWidget *cell = popup->cells[col][row];
    if (cell) {
        WIDGET_REMOVE_CSS_CLASS(cell, "cell-selected");
        WIDGET_ADD_CSS_CLASS(cell, "cell-flash");
    }

    if (popup->callback) {
        popup->callback(popup->items[idx], popup->user_data);
    }
    popup->pending_hide = TRUE;
    g_timeout_add(FLASH_DURATION_MS, on_flash_timeout, popup);
}

#if GTK_CHECK_VERSION(4, 0, 0)
static void
on_cell_clicked(GtkGestureClick *gesture, gint n_press, gdouble x, gdouble y, gpointer user_data)
{
    (void)gesture; (void)n_press;
    UnimEmojiPopup *popup = (UnimEmojiPopup *)user_data;

    GtkWidget *picked = gtk_widget_pick(popup->grid, x, y, GTK_PICK_DEFAULT);
    if (!picked) return;

    for (gint c = 0; c < MAX_COLS; c++) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[c][r] == picked) {
                popup->sel_col = c;
                popup->sel_row = r;
                update_selection(popup);
                emit_select(popup, c, r);
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
    UnimEmojiPopup *popup = (UnimEmojiPopup *)user_data;
    if (event->button != 1) return FALSE;

    for (gint c = 0; c < MAX_COLS; c++) {
        for (gint r = 0; r < MAX_ROWS; r++) {
            if (popup->cells[c][r]) {
                GtkAllocation alloc;
                gtk_widget_get_allocation(popup->cells[c][r], &alloc);
                if (event->x >= alloc.x && event->x < alloc.x + alloc.width &&
                    event->y >= alloc.y && event->y < alloc.y + alloc.height) {
                    popup->sel_col = c;
                    popup->sel_row = r;
                    update_selection(popup);
                    emit_select(popup, c, r);
                    return TRUE;
                }
            }
        }
    }
    return FALSE;
}
#endif

void
unim_emoji_popup_show(UnimEmojiPopup *popup,
                       const gchar *target_cat_id,
                       const gchar * const *items,
                       gsize item_count,
                       const gchar *top_row,
                       const gchar * const *recent,
                       gsize recent_count,
                       const UnimEmojiCategory *categories,
                       gsize category_count,
                       gint x,
                       gint y,
                       gint cursor_height,
                       UnimEmojiCommitCallback callback,
                       gpointer user_data)
{
    if (!popup) return;

    /* 상태 갱신 */
    popup->callback = callback;
    popup->user_data = user_data;
    popup->pending_hide = FALSE;

    /* current_cat_id */
    g_free(popup->current_cat_id);
    popup->current_cat_id = g_strdup(target_cat_id ? target_cat_id : "");

    /* items 깊은 복사 */
    free_items(popup);
    if (item_count > 0 && items) {
        popup->items = g_new0(gchar*, item_count);
        for (gsize i = 0; i < item_count; i++) {
            popup->items[i] = g_strdup(items[i] ? items[i] : "");
        }
        popup->item_count = item_count;
    }

    /* recent 깊은 복사 */
    free_recent(popup);
    if (recent_count > 0 && recent) {
        popup->recent = g_new0(gchar*, recent_count);
        for (gsize i = 0; i < recent_count; i++) {
            popup->recent[i] = g_strdup(recent[i] ? recent[i] : "");
        }
        popup->recent_count = recent_count;
    }

    /* top_row */
    if (top_row && strlen(top_row) >= 9) {
        strncpy(popup->top_row, top_row, 9);
        popup->top_row[9] = '\0';
    } else {
        strncpy(popup->top_row, "QWERTYUIO", 10);
    }

    /* categories 갱신 (최대 TAB_COUNT) */
    free_categories(popup);
    gsize n_tabs = category_count;
    if (n_tabs > TAB_COUNT) n_tabs = TAB_COUNT;
    for (gsize i = 0; i < n_tabs; i++) {
        popup->tab_meta[i].id = g_strdup(categories[i].id ? categories[i].id : "");
        popup->tab_meta[i].name_ko = g_strdup(categories[i].name_ko ? categories[i].name_ko : "");
        popup->tab_meta[i].name_en = g_strdup(categories[i].name_en ? categories[i].name_en : "");
        popup->tab_meta[i].count = categories[i].count;
    }
    popup->tab_count = n_tabs;

    /* 좌측 탭 라벨/active 동기화 */
    gint active_idx = 0;
    for (gint i = 0; i < TAB_COUNT; i++) {
        GtkWidget *btn = popup->tab_buttons[i];
        if (!btn) continue;
        if ((gsize)i < popup->tab_count) {
            const gchar *label = popup->tab_meta[i].name_ko;
            if (!label || !*label) label = tab_label_for(popup->tab_meta[i].id);
            gtk_button_set_label(GTK_BUTTON(btn), label);
            gtk_widget_set_visible(btn, TRUE);
            if (g_strcmp0(popup->tab_meta[i].id, popup->current_cat_id) == 0) {
                active_idx = i;
            }
        } else {
            gtk_button_set_label(GTK_BUTTON(btn), "");
            gtk_widget_set_visible(btn, FALSE);
        }
    }
    /* active 토글 — 그룹 분리 (한 번에 하나만 active) */
    for (gint i = 0; i < TAB_COUNT; i++) {
        GtkWidget *btn = popup->tab_buttons[i];
        if (!btn) continue;
        gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(btn), i == active_idx);
    }

    /* 헤더 */
    {
        const gchar *label = (active_idx < (gint)popup->tab_count) ?
            (popup->tab_meta[active_idx].name_ko && *popup->tab_meta[active_idx].name_ko ?
                popup->tab_meta[active_idx].name_ko :
                tab_label_for(popup->tab_meta[active_idx].id)) :
            tab_label_for(popup->current_cat_id);
        gchar *header_text = g_strdup_printf("「%s」 → 이모지", label);
        gtk_label_set_text(GTK_LABEL(popup->header_label), header_text);
        g_free(header_text);
    }

    /* 페이지 계산 + 그리드 렌더 */
    popup->current_page = 0;
    popup->sel_row = 0;
    popup->sel_col = 0;
    if (popup->item_count == 0) {
        popup->total_pages = 1;
    } else {
        popup->total_pages = (gint)((popup->item_count + PAGE_SIZE - 1) / PAGE_SIZE);
    }

    update_grid(popup);

    /* 위치 + 표시 (특수문자 패턴) */
    gint final_x = x;
    gint final_y = y + 4;

#if GTK_CHECK_VERSION(4, 0, 0)
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
#else
    (void)cursor_height;
#endif

    gtk_widget_set_visible(popup->window, TRUE);
#else
    gtk_window_resize(GTK_WINDOW(popup->window), 1, 1);
    gtk_window_move(GTK_WINDOW(popup->window), final_x, final_y);
    gtk_widget_show_all(popup->window);

    GtkRequisition req;
    gtk_widget_get_preferred_size(popup->window, NULL, &req);
    gint width = req.width;
    gint height = req.height;

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

    EMOJI_DEBUG("팝업 표시: cat='%s', items=%zu, recent=%zu, cats=%zu, pos=(%d,%d), size=%dx%d",
                popup->current_cat_id, popup->item_count, popup->recent_count,
                popup->tab_count, final_x, final_y, width, height);
}

void
unim_emoji_popup_hide(UnimEmojiPopup *popup)
{
    if (!popup || !popup->window) return;

#if GTK_CHECK_VERSION(4, 0, 0)
    gtk_widget_set_visible(popup->window, FALSE);
#else
    gtk_widget_hide(popup->window);
#endif

    EMOJI_DEBUG("팝업 숨김");
}

gboolean
unim_emoji_popup_is_visible(UnimEmojiPopup *popup)
{
    if (!popup || !popup->window) return FALSE;
    if (popup->pending_hide) return FALSE;
    return gtk_widget_get_visible(popup->window);
}

void
unim_emoji_popup_navigate(UnimEmojiPopup *popup,
                           gint page,
                           gint total_pages,
                           gint selected,
                           gint rows,
                           gint cols,
                           gint sel_row,
                           gint sel_col)
{
    (void)selected; (void)rows; (void)cols;
    if (!popup) return;

    if (total_pages > 0) {
        popup->total_pages = total_pages;
    }

    gint new_page = page < 0 ? 0 : page;
    gboolean page_changed = (new_page != popup->current_page);
    popup->current_page = new_page;
    popup->sel_row = sel_row < 0 ? 0 : sel_row;
    popup->sel_col = sel_col < 0 ? 0 : sel_col;

    if (page_changed) {
        update_grid(popup);
    } else {
        update_selection(popup);
    }
}

void
unim_emoji_popup_set_recent(UnimEmojiPopup *popup,
                             const gchar * const *emojis,
                             gsize count)
{
    if (!popup) return;

    free_recent(popup);
    if (count > 0 && emojis) {
        popup->recent = g_new0(gchar*, count);
        for (gsize i = 0; i < count; i++) {
            popup->recent[i] = g_strdup(emojis[i] ? emojis[i] : "");
        }
        popup->recent_count = count;
    }

    /* 'Recent' 탭 라벨에 count 표시 등은 향후 — 현재는 데이터만 보관 */
    EMOJI_DEBUG("Recent 캐시 갱신: count=%zu", popup->recent_count);
}

/* ===== Phase 4: 마우스 페이지 이동 ◀/▶ ===== */

void
unim_emoji_popup_set_page_change_callback(UnimEmojiPopup *popup,
                                            UnimEmojiPageChangeCallback callback,
                                            gpointer user_data)
{
    if (!popup) return;
    popup->page_change_callback = callback;
    popup->page_change_user_data = user_data;
}

static void
emoji_prev_page_clicked(GtkButton *button G_GNUC_UNUSED, gpointer user_data)
{
    UnimEmojiPopup *popup = (UnimEmojiPopup *)user_data;
    if (!popup || !popup->page_change_callback) return;
    popup->page_change_callback(0, popup->page_change_user_data);
}

static void
emoji_next_page_clicked(GtkButton *button G_GNUC_UNUSED, gpointer user_data)
{
    UnimEmojiPopup *popup = (UnimEmojiPopup *)user_data;
    if (!popup || !popup->page_change_callback) return;
    popup->page_change_callback(1, popup->page_change_user_data);
}
