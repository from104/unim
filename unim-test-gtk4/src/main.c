/*
 * UNIM GTK4 Input Method Test Application
 * 
 * 다양한 GTK4 위젯에서 UNIM 입력기를 테스트하기 위한 앱입니다.
 * DBus를 통해 unim-daemon과 통신하여 한/영 상태를 실시간으로 표시합니다.
 */

#include <gtk/gtk.h>
#include <gio/gio.h>

/* 상태 라벨 업데이트용 전역 변수 */
static GtkWidget *status_label = NULL;
static GtkWidget *preedit_label = NULL;
static GtkWidget *commit_label = NULL;
static GtkWidget *focus_label = NULL;
static GtkWidget *dbus_status_label = NULL;

/* 입력 이벤트 로그 */
static GtkTextBuffer *log_buffer = NULL;

/* DBus */
static GDBusProxy *dbus_proxy = NULL;
static guint dbus_signal_id = 0;

static void log_message(const char *format, ...) {
    va_list args;
    va_start(args, format);
    
    char buffer[1024];
    vsnprintf(buffer, sizeof(buffer), format, args);
    
    va_end(args);
    
    if (log_buffer) {
        GtkTextIter end;
        gtk_text_buffer_get_end_iter(log_buffer, &end);
        
        /* 타임스탬프 추가 */
        GDateTime *now = g_date_time_new_now_local();
        char *timestamp = g_date_time_format(now, "[%H:%M:%S] ");
        gtk_text_buffer_insert(log_buffer, &end, timestamp, -1);
        gtk_text_buffer_insert(log_buffer, &end, buffer, -1);
        gtk_text_buffer_insert(log_buffer, &end, "\n", -1);
        g_free(timestamp);
        g_date_time_unref(now);
        
        /* 스크롤 to end */
        GtkTextMark *mark = gtk_text_buffer_get_insert(log_buffer);
        gtk_text_buffer_get_end_iter(log_buffer, &end);
        gtk_text_buffer_move_mark(log_buffer, mark, &end);
    }
}

/* 입력 모드 표시 업데이트 */
static void update_mode_display(gboolean is_korean) {
    if (status_label) {
        if (is_korean) {
            gtk_label_set_text(GTK_LABEL(status_label), "🇰🇷 한국어");
            gtk_widget_add_css_class(status_label, "hangul-mode");
            gtk_widget_remove_css_class(status_label, "english-mode");
        } else {
            gtk_label_set_text(GTK_LABEL(status_label), "🔤 영어");
            gtk_widget_add_css_class(status_label, "english-mode");
            gtk_widget_remove_css_class(status_label, "hangul-mode");
        }
    }
}

/* DBus GlobalModeChanged 시그널 핸들러 */
static void on_dbus_signal(GDBusProxy *proxy,
                           const gchar *sender_name,
                           const gchar *signal_name,
                           GVariant *parameters,
                           gpointer user_data) {
    (void)proxy;
    (void)sender_name;
    (void)user_data;
    
    if (g_strcmp0(signal_name, "GlobalModeChanged") == 0) {
        gboolean is_korean;
        g_variant_get(parameters, "(b)", &is_korean);
        update_mode_display(is_korean);
        log_message("입력 모드 변경: %s", is_korean ? "한국어" : "영어");
    }
}

/* DBus 연결 설정 */
static void setup_dbus(void) {
    GError *error = NULL;
    
    dbus_proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION,
        G_DBUS_PROXY_FLAGS_NONE,
        NULL,
        "org.atit.unim.InputMethod",
        "/org/atit/unim/InputMethod",
        "org.atit.unim.InputMethod",
        NULL,
        &error
    );
    
    if (error) {
        gtk_label_set_text(GTK_LABEL(dbus_status_label), "❌ unim-daemon 없음");
        log_message("DBus 연결 실패: %s", error->message);
        g_error_free(error);
        return;
    }
    
    /* 시그널 연결 */
    dbus_signal_id = g_signal_connect(dbus_proxy, "g-signal", 
                                       G_CALLBACK(on_dbus_signal), NULL);
    
    gtk_label_set_text(GTK_LABEL(dbus_status_label), "✅ DBus 연결됨");
    log_message("DBus 연결 성공 - GlobalModeChanged 시그널 구독");
    
    /* 초기 모드 조회 */
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy,
        "GetGlobalMode",
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        -1,
        NULL,
        &error
    );
    
    if (result) {
        gboolean is_korean;
        g_variant_get(result, "(b)", &is_korean);
        update_mode_display(is_korean);
        log_message("초기 입력 모드: %s", is_korean ? "한국어" : "영어");
        g_variant_unref(result);
    }
}

/* 한/영 토글 버튼 핸들러 */
static void on_toggle_mode(GtkButton *button, gpointer user_data) {
    (void)button;
    (void)user_data;
    
    if (!dbus_proxy) {
        log_message("DBus 연결 없음");
        return;
    }
    
    GError *error = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy,
        "GetGlobalMode",
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        -1,
        NULL,
        &error
    );
    
    if (result) {
        gboolean current_mode;
        g_variant_get(result, "(b)", &current_mode);
        g_variant_unref(result);
        
        /* 토글 */
        g_dbus_proxy_call_sync(
            dbus_proxy,
            "SetGlobalMode",
            g_variant_new("(b)", !current_mode),
            G_DBUS_CALL_FLAGS_NONE,
            -1,
            NULL,
            NULL
        );
    }
    
    if (error) {
        g_error_free(error);
    }
}

static void update_focus_widget(GtkWidget *widget, const char *name) {
    (void)widget;
    if (focus_label) {
        char *text = g_strdup_printf("Focus: %s", name);
        gtk_label_set_text(GTK_LABEL(focus_label), text);
        g_free(text);
    }
    log_message("Focus changed: %s", name);
}

/* Entry 포커스 콜백 */
static void on_entry_focus_in(GtkEventControllerFocus *controller, gpointer user_data) {
    (void)controller;
    update_focus_widget(GTK_WIDGET(user_data), (const char *)g_object_get_data(G_OBJECT(user_data), "widget-name"));
}

/* Entry 텍스트 변경 콜백 */
static void on_entry_changed(GtkEditable *editable, gpointer user_data) {
    (void)user_data;
    const char *text = gtk_editable_get_text(editable);
    const char *name = (const char *)g_object_get_data(G_OBJECT(editable), "widget-name");
    log_message("%s: text changed -> \"%s\"", name, text);
}

/* Entry 생성 헬퍼 */
static GtkWidget *create_entry_row(const char *label_text, const char *widget_name, const char *placeholder) {
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_margin_start(box, 10);
    gtk_widget_set_margin_end(box, 10);
    gtk_widget_set_margin_top(box, 5);
    gtk_widget_set_margin_bottom(box, 5);
    
    GtkWidget *label = gtk_label_new(label_text);
    gtk_widget_set_size_request(label, 120, -1);
    gtk_label_set_xalign(GTK_LABEL(label), 0);
    gtk_box_append(GTK_BOX(box), label);
    
    GtkWidget *entry = gtk_entry_new();
    gtk_widget_set_hexpand(entry, TRUE);
    if (placeholder) {
        gtk_entry_set_placeholder_text(GTK_ENTRY(entry), placeholder);
    }
    g_object_set_data(G_OBJECT(entry), "widget-name", (gpointer)widget_name);
    
    /* 포커스 컨트롤러 추가 */
    GtkEventController *focus_controller = gtk_event_controller_focus_new();
    g_signal_connect(focus_controller, "enter", G_CALLBACK(on_entry_focus_in), entry);
    gtk_widget_add_controller(entry, focus_controller);
    
    /* 텍스트 변경 콜백 */
    g_signal_connect(entry, "changed", G_CALLBACK(on_entry_changed), NULL);
    
    gtk_box_append(GTK_BOX(box), entry);
    
    return box;
}

/* Password Entry 생성 */
static GtkWidget *create_password_entry_row(const char *label_text, const char *widget_name) {
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_margin_start(box, 10);
    gtk_widget_set_margin_end(box, 10);
    gtk_widget_set_margin_top(box, 5);
    gtk_widget_set_margin_bottom(box, 5);
    
    GtkWidget *label = gtk_label_new(label_text);
    gtk_widget_set_size_request(label, 120, -1);
    gtk_label_set_xalign(GTK_LABEL(label), 0);
    gtk_box_append(GTK_BOX(box), label);
    
    GtkWidget *entry = gtk_password_entry_new();
    gtk_widget_set_hexpand(entry, TRUE);
    gtk_password_entry_set_show_peek_icon(GTK_PASSWORD_ENTRY(entry), TRUE);
    g_object_set_data(G_OBJECT(entry), "widget-name", (gpointer)widget_name);
    
    GtkEventController *focus_controller = gtk_event_controller_focus_new();
    g_signal_connect(focus_controller, "enter", G_CALLBACK(on_entry_focus_in), entry);
    gtk_widget_add_controller(entry, focus_controller);
    
    gtk_box_append(GTK_BOX(box), entry);
    
    return box;
}

/* SearchEntry 생성 */
static GtkWidget *create_search_entry_row(const char *label_text, const char *widget_name) {
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_margin_start(box, 10);
    gtk_widget_set_margin_end(box, 10);
    gtk_widget_set_margin_top(box, 5);
    gtk_widget_set_margin_bottom(box, 5);
    
    GtkWidget *label = gtk_label_new(label_text);
    gtk_widget_set_size_request(label, 120, -1);
    gtk_label_set_xalign(GTK_LABEL(label), 0);
    gtk_box_append(GTK_BOX(box), label);
    
    GtkWidget *entry = gtk_search_entry_new();
    gtk_widget_set_hexpand(entry, TRUE);
    g_object_set_data(G_OBJECT(entry), "widget-name", (gpointer)widget_name);
    
    GtkEventController *focus_controller = gtk_event_controller_focus_new();
    g_signal_connect(focus_controller, "enter", G_CALLBACK(on_entry_focus_in), entry);
    gtk_widget_add_controller(entry, focus_controller);
    
    gtk_box_append(GTK_BOX(box), entry);
    
    return box;
}

/* SpinButton 생성 */
static GtkWidget *create_spin_button_row(const char *label_text, const char *widget_name) {
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_margin_start(box, 10);
    gtk_widget_set_margin_end(box, 10);
    gtk_widget_set_margin_top(box, 5);
    gtk_widget_set_margin_bottom(box, 5);
    
    GtkWidget *label = gtk_label_new(label_text);
    gtk_widget_set_size_request(label, 120, -1);
    gtk_label_set_xalign(GTK_LABEL(label), 0);
    gtk_box_append(GTK_BOX(box), label);
    
    GtkAdjustment *adj = gtk_adjustment_new(0, 0, 100, 1, 10, 0);
    GtkWidget *spin = gtk_spin_button_new(adj, 1, 0);
    gtk_spin_button_set_numeric(GTK_SPIN_BUTTON(spin), FALSE); /* 텍스트 입력 허용 */
    gtk_widget_set_hexpand(spin, TRUE);
    g_object_set_data(G_OBJECT(spin), "widget-name", (gpointer)widget_name);
    
    GtkEventController *focus_controller = gtk_event_controller_focus_new();
    g_signal_connect(focus_controller, "enter", G_CALLBACK(on_entry_focus_in), spin);
    gtk_widget_add_controller(spin, focus_controller);
    
    gtk_box_append(GTK_BOX(box), spin);
    
    return box;
}

/* TextView 프레임 생성 */
static GtkWidget *create_text_view_frame(const char *title, const char *widget_name, int height) {
    GtkWidget *frame = gtk_frame_new(title);
    gtk_widget_set_margin_start(frame, 10);
    gtk_widget_set_margin_end(frame, 10);
    gtk_widget_set_margin_top(frame, 5);
    gtk_widget_set_margin_bottom(frame, 5);
    
    GtkWidget *scrolled = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scrolled), 
                                   GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_size_request(scrolled, -1, height);
    
    GtkWidget *text_view = gtk_text_view_new();
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(text_view), GTK_WRAP_WORD_CHAR);
    gtk_text_view_set_left_margin(GTK_TEXT_VIEW(text_view), 5);
    gtk_text_view_set_right_margin(GTK_TEXT_VIEW(text_view), 5);
    gtk_text_view_set_top_margin(GTK_TEXT_VIEW(text_view), 5);
    gtk_text_view_set_bottom_margin(GTK_TEXT_VIEW(text_view), 5);
    g_object_set_data(G_OBJECT(text_view), "widget-name", (gpointer)widget_name);
    
    GtkEventController *focus_controller = gtk_event_controller_focus_new();
    g_signal_connect(focus_controller, "enter", G_CALLBACK(on_entry_focus_in), text_view);
    gtk_widget_add_controller(text_view, focus_controller);
    
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scrolled), text_view);
    gtk_frame_set_child(GTK_FRAME(frame), scrolled);
    
    return frame;
}

/* 상태 패널 생성 */
static GtkWidget *create_status_panel(void) {
    GtkWidget *frame = gtk_frame_new("입력기 상태");
    gtk_widget_set_margin_start(frame, 10);
    gtk_widget_set_margin_end(frame, 10);
    gtk_widget_set_margin_top(frame, 10);
    gtk_widget_set_margin_bottom(frame, 5);
    
    GtkWidget *grid = gtk_grid_new();
    gtk_grid_set_column_spacing(GTK_GRID(grid), 10);
    gtk_grid_set_row_spacing(GTK_GRID(grid), 5);
    gtk_widget_set_margin_start(grid, 10);
    gtk_widget_set_margin_end(grid, 10);
    gtk_widget_set_margin_top(grid, 10);
    gtk_widget_set_margin_bottom(grid, 10);
    
    /* DBus 상태 */
    GtkWidget *dbus_title = gtk_label_new("DBus 연결:");
    gtk_label_set_xalign(GTK_LABEL(dbus_title), 0);
    gtk_grid_attach(GTK_GRID(grid), dbus_title, 0, 0, 1, 1);
    
    dbus_status_label = gtk_label_new("(연결 중...)");
    gtk_label_set_xalign(GTK_LABEL(dbus_status_label), 0);
    gtk_widget_set_hexpand(dbus_status_label, TRUE);
    gtk_grid_attach(GTK_GRID(grid), dbus_status_label, 1, 0, 1, 1);
    
    /* Focus */
    GtkWidget *focus_title = gtk_label_new("현재 포커스:");
    gtk_label_set_xalign(GTK_LABEL(focus_title), 0);
    gtk_grid_attach(GTK_GRID(grid), focus_title, 0, 1, 1, 1);
    
    focus_label = gtk_label_new("(없음)");
    gtk_label_set_xalign(GTK_LABEL(focus_label), 0);
    gtk_widget_set_hexpand(focus_label, TRUE);
    gtk_grid_attach(GTK_GRID(grid), focus_label, 1, 1, 1, 1);
    
    /* IM 상태 */
    GtkWidget *status_title = gtk_label_new("입력 모드:");
    gtk_label_set_xalign(GTK_LABEL(status_title), 0);
    gtk_grid_attach(GTK_GRID(grid), status_title, 0, 2, 1, 1);
    
    status_label = gtk_label_new("(감지 중...)");
    gtk_label_set_xalign(GTK_LABEL(status_label), 0);
    gtk_grid_attach(GTK_GRID(grid), status_label, 1, 2, 1, 1);
    
    /* Preedit */
    GtkWidget *preedit_title = gtk_label_new("Preedit:");
    gtk_label_set_xalign(GTK_LABEL(preedit_title), 0);
    gtk_grid_attach(GTK_GRID(grid), preedit_title, 0, 3, 1, 1);
    
    preedit_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(preedit_label), 0);
    gtk_grid_attach(GTK_GRID(grid), preedit_label, 1, 3, 1, 1);
    
    /* Commit */
    GtkWidget *commit_title = gtk_label_new("Last Commit:");
    gtk_label_set_xalign(GTK_LABEL(commit_title), 0);
    gtk_grid_attach(GTK_GRID(grid), commit_title, 0, 4, 1, 1);
    
    commit_label = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(commit_label), 0);
    gtk_grid_attach(GTK_GRID(grid), commit_label, 1, 4, 1, 1);
    
    gtk_frame_set_child(GTK_FRAME(frame), grid);
    
    return frame;
}

/* 로그 패널 생성 */
static GtkWidget *create_log_panel(void) {
    GtkWidget *frame = gtk_frame_new("이벤트 로그");
    gtk_widget_set_margin_start(frame, 10);
    gtk_widget_set_margin_end(frame, 10);
    gtk_widget_set_margin_top(frame, 5);
    gtk_widget_set_margin_bottom(frame, 10);
    gtk_widget_set_vexpand(frame, TRUE);
    
    GtkWidget *scrolled = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scrolled), 
                                   GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    
    GtkWidget *text_view = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(text_view), FALSE);
    gtk_text_view_set_monospace(GTK_TEXT_VIEW(text_view), TRUE);
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(text_view), GTK_WRAP_WORD_CHAR);
    gtk_text_view_set_left_margin(GTK_TEXT_VIEW(text_view), 5);
    gtk_text_view_set_top_margin(GTK_TEXT_VIEW(text_view), 5);
    
    log_buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(text_view));
    
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scrolled), text_view);
    gtk_frame_set_child(GTK_FRAME(frame), scrolled);
    
    return frame;
}

/* Clear 버튼 핸들러 */
static void on_clear_log(GtkButton *button, gpointer user_data) {
    (void)button;
    (void)user_data;
    if (log_buffer) {
        gtk_text_buffer_set_text(log_buffer, "", -1);
    }
}

static void activate(GtkApplication *app, gpointer user_data) {
    (void)user_data;
    
    GtkWidget *window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "UNIM GTK4 입력기 테스트");
    gtk_window_set_default_size(GTK_WINDOW(window), 700, 800);
    
    /* 메인 박스 */
    GtkWidget *main_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_window_set_child(GTK_WINDOW(window), main_box);
    
    /* 헤더 바 */
    GtkWidget *header = gtk_header_bar_new();
    gtk_header_bar_set_show_title_buttons(GTK_HEADER_BAR(header), TRUE);
    
    GtkWidget *toggle_btn = gtk_button_new_with_label("한/영 전환");
    g_signal_connect(toggle_btn, "clicked", G_CALLBACK(on_toggle_mode), NULL);
    gtk_header_bar_pack_start(GTK_HEADER_BAR(header), toggle_btn);
    
    GtkWidget *clear_btn = gtk_button_new_with_label("로그 지우기");
    g_signal_connect(clear_btn, "clicked", G_CALLBACK(on_clear_log), NULL);
    gtk_header_bar_pack_end(GTK_HEADER_BAR(header), clear_btn);
    
    gtk_window_set_titlebar(GTK_WINDOW(window), header);
    
    /* 스크롤 영역 */
    GtkWidget *scrolled = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scrolled), 
                                   GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_vexpand(scrolled, TRUE);
    
    GtkWidget *content_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    
    /* 상태 패널 */
    gtk_box_append(GTK_BOX(content_box), create_status_panel());
    
    /* 구분선 */
    gtk_box_append(GTK_BOX(content_box), gtk_separator_new(GTK_ORIENTATION_HORIZONTAL));
    
    /* Entry 위젯들 */
    GtkWidget *entry_label = gtk_label_new(NULL);
    gtk_label_set_markup(GTK_LABEL(entry_label), "<b>Entry 위젯</b>");
    gtk_label_set_xalign(GTK_LABEL(entry_label), 0);
    gtk_widget_set_margin_start(entry_label, 10);
    gtk_widget_set_margin_top(entry_label, 10);
    gtk_box_append(GTK_BOX(content_box), entry_label);
    
    gtk_box_append(GTK_BOX(content_box), 
        create_entry_row("일반 Entry:", "Entry-일반", "한국어/영어 입력 테스트"));
    gtk_box_append(GTK_BOX(content_box), 
        create_entry_row("숫자 Entry:", "Entry-숫자", "123-456"));
    gtk_box_append(GTK_BOX(content_box), 
        create_password_entry_row("비밀번호:", "PasswordEntry"));
    gtk_box_append(GTK_BOX(content_box), 
        create_search_entry_row("검색:", "SearchEntry"));
    gtk_box_append(GTK_BOX(content_box), 
        create_spin_button_row("SpinButton:", "SpinButton"));
    
    /* 구분선 */
    gtk_box_append(GTK_BOX(content_box), gtk_separator_new(GTK_ORIENTATION_HORIZONTAL));
    
    /* TextView */
    GtkWidget *textview_label = gtk_label_new(NULL);
    gtk_label_set_markup(GTK_LABEL(textview_label), "<b>TextView 위젯</b>");
    gtk_label_set_xalign(GTK_LABEL(textview_label), 0);
    gtk_widget_set_margin_start(textview_label, 10);
    gtk_widget_set_margin_top(textview_label, 10);
    gtk_box_append(GTK_BOX(content_box), textview_label);
    
    gtk_box_append(GTK_BOX(content_box), 
        create_text_view_frame("멀티라인 입력", "TextView-멀티라인", 120));
    
    /* 구분선 */
    gtk_box_append(GTK_BOX(content_box), gtk_separator_new(GTK_ORIENTATION_HORIZONTAL));
    
    /* 로그 패널 */
    gtk_box_append(GTK_BOX(content_box), create_log_panel());
    
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scrolled), content_box);
    gtk_box_append(GTK_BOX(main_box), scrolled);
    
    /* 초기 로그 메시지 */
    log_message("UNIM GTK4 입력기 테스트 앱 시작");
    log_message("GTK_IM_MODULE=%s", g_getenv("GTK_IM_MODULE") ? g_getenv("GTK_IM_MODULE") : "(unset)");
    
    /* DBus 설정 */
    setup_dbus();
    
    gtk_window_present(GTK_WINDOW(window));
}

static void cleanup(void) {
    if (dbus_proxy) {
        if (dbus_signal_id > 0) {
            g_signal_handler_disconnect(dbus_proxy, dbus_signal_id);
        }
        g_object_unref(dbus_proxy);
    }
}

int main(int argc, char *argv[]) {
    GtkApplication *app = gtk_application_new("io.github.from104.unim.test.gtk4", 
                                               G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
    
    int status = g_application_run(G_APPLICATION(app), argc, argv);
    
    cleanup();
    g_object_unref(app);
    
    return status;
}
