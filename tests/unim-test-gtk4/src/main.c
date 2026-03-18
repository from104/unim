/*
 * UNIM GTK4 Input Method Test Application
 *
 * 다양한 GTK4 위젯에서 UNIM 입력기를 테스트하기 위한 앱입니다.
 * DBus를 통해 unim-daemon과 통신하여 한/영 상태를 실시간으로 표시합니다.
 *
 * 설계 원칙:
 *   - 표준 GTK4 패턴 (GtkApplication, CSS Provider, GtkBuilder 스타일)
 *   - IM 모듈과 충돌하지 않는 안전한 시그널 처리
 *   - 로그 출력은 g_idle_add로 지연하여 이벤트 루프 간섭 방지
 */

#include <gtk/gtk.h>
#include <gio/gio.h>
#include "unim_test.h"



/* ───────────────────────────────────────────── */
/*  전역 상태                                    */
/* ───────────────────────────────────────────── */

static GtkTextBuffer *log_buffer  = NULL;
static GtkWidget     *log_view    = NULL;
static GtkWidget     *mode_label  = NULL;
static GtkWidget     *dbus_label  = NULL;
static GDBusProxy    *dbus_proxy  = NULL;
static guint          dbus_sig_id = 0;

/* ───────────────────────────────────────────── */
/*  로그 시스템 (g_idle_add 기반, 안전)          */
/* ───────────────────────────────────────────── */

typedef struct {
    char *message;
} LogEntry;

static gboolean
log_flush_idle(gpointer data)
{
    LogEntry *entry = data;

    if (log_buffer && entry->message) {
        GtkTextIter end;
        gtk_text_buffer_get_end_iter(log_buffer, &end);

        GDateTime *now = g_date_time_new_now_local();
        char *ts = g_date_time_format(now, "[%H:%M:%S] ");
        gtk_text_buffer_insert(log_buffer, &end, ts, -1);
        gtk_text_buffer_insert(log_buffer, &end, entry->message, -1);
        gtk_text_buffer_insert(log_buffer, &end, "\n", -1);
        g_free(ts);
        g_date_time_unref(now);

        /* 자동 스크롤 */
        if (log_view) {
            GtkTextMark *mark = gtk_text_buffer_get_insert(log_buffer);
            gtk_text_buffer_get_end_iter(log_buffer, &end);
            gtk_text_buffer_move_mark(log_buffer, mark, &end);
        }
    }

    g_free(entry->message);
    g_free(entry);
    return G_SOURCE_REMOVE;
}

static void
app_log(const char *fmt, ...)
{
    va_list ap;
    va_start(ap, fmt);

    LogEntry *entry = g_new0(LogEntry, 1);
    entry->message = g_strdup_vprintf(fmt, ap);

    va_end(ap);

    /* 콘솔에도 출력 */
    g_print("[TESTAPP] %s\n", entry->message);

    /* 메인 루프에서 안전하게 버퍼 조작 */
    g_idle_add(log_flush_idle, entry);
}

/* ───────────────────────────────────────────── */
/*  DBus 연동                                    */
/* ───────────────────────────────────────────── */

static void
update_mode_display(gboolean is_korean)
{
    if (!mode_label) return;

    if (is_korean) {
        gtk_label_set_text(GTK_LABEL(mode_label), "🇰🇷 한국어");
        gtk_widget_remove_css_class(mode_label, "mode-english");
        gtk_widget_add_css_class(mode_label, "mode-korean");
    } else {
        gtk_label_set_text(GTK_LABEL(mode_label), "🔤 English");
        gtk_widget_remove_css_class(mode_label, "mode-korean");
        gtk_widget_add_css_class(mode_label, "mode-english");
    }
}

static void
on_dbus_signal(GDBusProxy  *proxy,
               const gchar *sender_name,
               const gchar *signal_name,
               GVariant    *parameters,
               gpointer     user_data)
{
    (void)proxy; (void)sender_name; (void)user_data;

    if (g_strcmp0(signal_name, "GlobalModeChanged") == 0) {
        gboolean is_korean;
        g_variant_get(parameters, "(b)", &is_korean);
        update_mode_display(is_korean);
        app_log("입력 모드 변경: %s", is_korean ? "한국어" : "English");
    }
}

static void
setup_dbus(void)
{
    GError *error = NULL;

    dbus_proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION,
        G_DBUS_PROXY_FLAGS_NONE,
        NULL,
        "org.atit.unim.InputMethod",
        "/org/atit/unim/InputMethod",
        "org.atit.unim.InputMethod",
        NULL, &error);

    if (error) {
        gtk_label_set_text(GTK_LABEL(dbus_label), "❌ 연결 실패");
        gtk_widget_add_css_class(dbus_label, "dbus-disconnected");
        app_log("DBus 연결 실패: %s", error->message);
        g_error_free(error);
        return;
    }

    dbus_sig_id = g_signal_connect(dbus_proxy, "g-signal",
                                   G_CALLBACK(on_dbus_signal), NULL);

    gtk_label_set_text(GTK_LABEL(dbus_label), "✅ 연결됨");
    gtk_widget_add_css_class(dbus_label, "dbus-connected");
    app_log("DBus 연결 성공");

    /* 초기 모드 조회 */
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

    if (result) {
        gboolean is_korean;
        g_variant_get(result, "(b)", &is_korean);
        update_mode_display(is_korean);
        app_log("초기 입력 모드: %s", is_korean ? "한국어" : "English");
        g_variant_unref(result);
    }
    if (error) g_error_free(error);
}

static void
on_toggle_mode(GtkButton *button, gpointer user_data)
{
    (void)button; (void)user_data;
    if (!dbus_proxy) { app_log("DBus 미연결"); return; }

    GError *error = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

    if (result) {
        gboolean current;
        g_variant_get(result, "(b)", &current);
        g_variant_unref(result);

        g_dbus_proxy_call_sync(
            dbus_proxy, "SetGlobalMode",
            g_variant_new("(b)", !current),
            G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
    }
    if (error) g_error_free(error);
}

/* ───────────────────────────────────────────── */
/*  위젯 팩토리                                  */
/* ───────────────────────────────────────────── */

/* 라벨 + 입력 위젯을 가로로 배치한 행 생성 */
static GtkWidget *
make_input_row(const char *label_text, GtkWidget *input)
{
    GtkWidget *row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    gtk_widget_set_margin_start(row, 12);
    gtk_widget_set_margin_end(row, 12);
    gtk_widget_set_margin_top(row, 4);
    gtk_widget_set_margin_bottom(row, 4);

    GtkWidget *label = gtk_label_new(label_text);
    gtk_label_set_xalign(GTK_LABEL(label), 0);
    gtk_widget_set_size_request(label, 110, -1);
    gtk_widget_add_css_class(label, "input-label");
    gtk_box_append(GTK_BOX(row), label);

    gtk_widget_set_hexpand(input, TRUE);
    gtk_box_append(GTK_BOX(row), input);

    return row;
}

/* 포커스 콜백 (로그용) */
static void
on_focus_enter(GtkEventControllerFocus *ctrl, gpointer user_data)
{
    (void)ctrl;
    const char *name = g_object_get_data(G_OBJECT(user_data), "wname");
    app_log("포커스 진입: %s", name ? name : "(unknown)");
}

/* 위젯에 포커스 추적 컨트롤러 연결 */
static void
attach_focus_log(GtkWidget *widget, const char *name)
{
    g_object_set_data(G_OBJECT(widget), "wname", (gpointer)name);
    GtkEventController *fc = gtk_event_controller_focus_new();
    g_signal_connect(fc, "enter", G_CALLBACK(on_focus_enter), widget);
    gtk_widget_add_controller(widget, fc);
}

/* ───────────────────────────────────────────── */
/*  UI 섹션 빌더                                 */
/* ───────────────────────────────────────────── */

static GtkWidget *
build_status_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
    gtk_widget_add_css_class(box, "status-panel");
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 16);

    /* DBus 상태 행 */
    GtkWidget *dbus_row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    GtkWidget *dbus_title = gtk_label_new("DBus");
    gtk_widget_add_css_class(dbus_title, "status-title");
    gtk_label_set_xalign(GTK_LABEL(dbus_title), 0);
    gtk_box_append(GTK_BOX(dbus_row), dbus_title);

    dbus_label = gtk_label_new("연결 중…");
    gtk_widget_add_css_class(dbus_label, "status-value");
    gtk_widget_set_hexpand(dbus_label, TRUE);
    gtk_label_set_xalign(GTK_LABEL(dbus_label), 1);
    gtk_box_append(GTK_BOX(dbus_row), dbus_label);
    gtk_box_append(GTK_BOX(box), dbus_row);

    /* 입력 모드 행 */
    GtkWidget *mode_row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    GtkWidget *mode_title = gtk_label_new("입력 모드");
    gtk_widget_add_css_class(mode_title, "status-title");
    gtk_label_set_xalign(GTK_LABEL(mode_title), 0);
    gtk_box_append(GTK_BOX(mode_row), mode_title);

    mode_label = gtk_label_new("감지 중…");
    gtk_widget_add_css_class(mode_label, "status-value");
    gtk_widget_set_hexpand(mode_label, TRUE);
    gtk_label_set_xalign(GTK_LABEL(mode_label), 1);
    gtk_box_append(GTK_BOX(mode_row), mode_label);
    gtk_box_append(GTK_BOX(box), mode_row);

    return box;
}

static GtkWidget *
build_entry_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 12);

    /* 섹션 제목 */
    GtkWidget *title = gtk_label_new("Entry 위젯");
    gtk_widget_add_css_class(title, "section-title");
    gtk_label_set_xalign(GTK_LABEL(title), 0);
    gtk_box_append(GTK_BOX(box), title);

    /* 일반 Entry */
    GtkWidget *entry1 = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(entry1), "한국어/영어 입력 테스트");
    attach_focus_log(entry1, "일반 Entry");
    gtk_box_append(GTK_BOX(box), make_input_row("일반 Entry", entry1));

    /* 숫자 Entry */
    GtkWidget *entry2 = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(entry2), "123-456");
    attach_focus_log(entry2, "숫자 Entry");
    gtk_box_append(GTK_BOX(box), make_input_row("숫자 Entry", entry2));

    /* 비밀번호 */
    GtkWidget *pw = gtk_password_entry_new();
    gtk_password_entry_set_show_peek_icon(GTK_PASSWORD_ENTRY(pw), TRUE);
    attach_focus_log(pw, "비밀번호");
    gtk_box_append(GTK_BOX(box), make_input_row("비밀번호", pw));

    /* 검색 */
    GtkWidget *search = gtk_search_entry_new();
    attach_focus_log(search, "검색");
    gtk_box_append(GTK_BOX(box), make_input_row("검색", search));

    /* SpinButton */
    GtkAdjustment *adj = gtk_adjustment_new(0, 0, 100, 1, 10, 0);
    GtkWidget *spin = gtk_spin_button_new(adj, 1, 0);
    gtk_spin_button_set_numeric(GTK_SPIN_BUTTON(spin), FALSE);
    attach_focus_log(spin, "SpinButton");
    gtk_box_append(GTK_BOX(box), make_input_row("SpinButton", spin));

    return box;
}

static GtkWidget *
build_textview_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 12);

    GtkWidget *title = gtk_label_new("TextView 위젯");
    gtk_widget_add_css_class(title, "section-title");
    gtk_label_set_xalign(GTK_LABEL(title), 0);
    gtk_box_append(GTK_BOX(box), title);

    GtkWidget *frame = gtk_frame_new(NULL);
    gtk_widget_add_css_class(frame, "input-frame");

    GtkWidget *sw = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(sw),
                                   GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_size_request(sw, -1, 120);

    GtkWidget *tv = gtk_text_view_new();
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(tv), GTK_WRAP_WORD_CHAR);
    gtk_text_view_set_left_margin(GTK_TEXT_VIEW(tv), 8);
    gtk_text_view_set_right_margin(GTK_TEXT_VIEW(tv), 8);
    gtk_text_view_set_top_margin(GTK_TEXT_VIEW(tv), 8);
    gtk_text_view_set_bottom_margin(GTK_TEXT_VIEW(tv), 8);
    attach_focus_log(tv, "멀티라인 TextView");

    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(sw), tv);
    gtk_frame_set_child(GTK_FRAME(frame), sw);
    gtk_box_append(GTK_BOX(box), frame);

    return box;
}

static GtkWidget *
build_log_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 12);
    gtk_widget_set_margin_bottom(box, 16);
    gtk_widget_set_vexpand(box, TRUE);

    GtkWidget *title = gtk_label_new("이벤트 로그");
    gtk_widget_add_css_class(title, "section-title");
    gtk_label_set_xalign(GTK_LABEL(title), 0);
    gtk_box_append(GTK_BOX(box), title);

    GtkWidget *frame = gtk_frame_new(NULL);
    gtk_widget_add_css_class(frame, "input-frame");
    gtk_widget_set_vexpand(frame, TRUE);

    GtkWidget *sw = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(sw),
                                   GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_vexpand(sw, TRUE);

    log_view = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(log_view), FALSE);
    gtk_text_view_set_cursor_visible(GTK_TEXT_VIEW(log_view), FALSE);
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(log_view), GTK_WRAP_WORD_CHAR);
    gtk_widget_add_css_class(log_view, "log-view");

    log_buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(log_view));

    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(sw), log_view);
    gtk_frame_set_child(GTK_FRAME(frame), sw);
    gtk_box_append(GTK_BOX(box), frame);

    return box;
}

/* 로그 지우기 */
static void
on_clear_log(GtkButton *button, gpointer user_data)
{
    (void)button; (void)user_data;
    if (log_buffer)
        gtk_text_buffer_set_text(log_buffer, "", -1);
}

/* ───────────────────────────────────────────── */
/*  Application                                  */
/* ───────────────────────────────────────────── */

static void
activate(GtkApplication *app, gpointer user_data)
{
    (void)user_data;


    /* ── 윈도우 ── */
    GtkWidget *window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "UNIM GTK4 입력기 테스트");
    gtk_window_set_default_size(GTK_WINDOW(window), 640, 780);

    /* ── 헤더바 ── */
    GtkWidget *header = gtk_header_bar_new();
    gtk_header_bar_set_show_title_buttons(GTK_HEADER_BAR(header), TRUE);

    GtkWidget *toggle_btn = gtk_button_new_with_label("한/영 전환");
    g_signal_connect(toggle_btn, "clicked", G_CALLBACK(on_toggle_mode), NULL);
    gtk_header_bar_pack_start(GTK_HEADER_BAR(header), toggle_btn);

    GtkWidget *clear_btn = gtk_button_new_with_label("로그 지우기");
    g_signal_connect(clear_btn, "clicked",
        G_CALLBACK(on_clear_log), NULL);
    gtk_header_bar_pack_end(GTK_HEADER_BAR(header), clear_btn);

    gtk_window_set_titlebar(GTK_WINDOW(window), header);

    /* ── 콘텐츠 ── */
    GtkWidget *scroll = gtk_scrolled_window_new();
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scroll),
                                   GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_vexpand(scroll, TRUE);

    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    gtk_box_append(GTK_BOX(content), build_status_section());
    gtk_box_append(GTK_BOX(content), build_entry_section());
    gtk_box_append(GTK_BOX(content), build_textview_section());
    gtk_box_append(GTK_BOX(content), build_log_section());

    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroll), content);
    gtk_window_set_child(GTK_WINDOW(window), scroll);

    /* ── 초기화 ── */
    app_log("UNIM GTK4 입력기 테스트 앱 시작");
    app_log("GTK_IM_MODULE=%s",
            g_getenv("GTK_IM_MODULE") ? g_getenv("GTK_IM_MODULE") : "(unset)");

    setup_dbus();

    gtk_window_present(GTK_WINDOW(window));
}

static void
cleanup(void)
{
    if (dbus_proxy) {
        if (dbus_sig_id > 0)
            g_signal_handler_disconnect(dbus_proxy, dbus_sig_id);
        g_object_unref(dbus_proxy);
        dbus_proxy = NULL;
    }
}

/* ─── 자동 테스트 ─────────────────────────────────────────────── */

static int
run_auto_test(gboolean verbose)
{
    long log_mark = unim_test_log_mark();

    UnimTestRunner *runner = unim_test_runner_new(verbose);
    if (!runner) return 1;

    unim_test_run_suite(runner, UNIM_TEST_SUITE_AUTO);
    unim_test_log_check(runner, log_mark);

    int ret = runner->failed > 0 ? 1 : 0;
    unim_test_runner_free(runner);
    return ret;
}

int
main(int argc, char *argv[])
{
    /* --auto 모드: GUI 없이 자동 테스트 */
    gboolean auto_mode = FALSE, verbose = FALSE;
    for (int i = 1; i < argc; i++) {
        if (g_strcmp0(argv[i], "--auto") == 0) auto_mode = TRUE;
        if (g_strcmp0(argv[i], "--verbose") == 0 || g_strcmp0(argv[i], "-v") == 0) verbose = TRUE;
    }

    if (auto_mode)
        return run_auto_test(verbose);

    /* GUI 모드 */
    GtkApplication *app = gtk_application_new(
        "io.github.from104.unim.test.gtk4",
        G_APPLICATION_DEFAULT_FLAGS);

    g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);

    int status = g_application_run(G_APPLICATION(app), argc, argv);

    cleanup();
    g_object_unref(app);

    return status;
}
