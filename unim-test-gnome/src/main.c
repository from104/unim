/*
 * UNIM GNOME IME Test Application
 *
 * GNOME Shell Extension의 입력기(Clutter.InputMethod + text-input-v3)를
 * 테스트하기 위한 앱입니다. GTK_IM_MODULE을 설정하지 않고 실행하여
 * GNOME Shell의 네이티브 IME 경로를 테스트합니다.
 *
 * 설계 원칙:
 *   - GTK_IM_MODULE 미설정 → Wayland text-input-v3 경로 사용
 *   - 환경 진단 패널로 IM 경로 확인
 *   - IM Context 시그널 모니터링 (preedit-changed, commit)
 *   - DBus를 통한 한/영 상태 실시간 표시
 */

#include <gtk/gtk.h>
#include <gio/gio.h>


/* ───────────────────────────────────────────── */
/*  Catppuccin Mocha 색상 팔레트 (CSS)           */
/* ───────────────────────────────────────────── */

static const char *APP_CSS =
    "window {"
    "  background-color: #1e1e2e;"
    "  color: #cdd6f4;"
    "}"
    ".header-bar {"
    "  background-color: #181825;"
    "  color: #cdd6f4;"
    "}"
    ".status-panel {"
    "  background-color: #181825;"
    "  border-radius: 12px;"
    "  padding: 12px;"
    "}"
    ".env-panel {"
    "  background-color: #181825;"
    "  border-radius: 12px;"
    "  padding: 12px;"
    "}"
    ".status-title {"
    "  color: #a6adc8;"
    "  font-size: 0.9em;"
    "}"
    ".status-value {"
    "  font-weight: bold;"
    "}"
    ".section-title {"
    "  color: #89b4fa;"
    "  font-weight: bold;"
    "  font-size: 1.05em;"
    "}"
    ".mode-korean {"
    "  color: #a6e3a1;"
    "}"
    ".mode-english {"
    "  color: #f9e2af;"
    "}"
    ".dbus-connected {"
    "  color: #a6e3a1;"
    "}"
    ".dbus-disconnected {"
    "  color: #f38ba8;"
    "}"
    ".env-key {"
    "  color: #89b4fa;"
    "  font-family: monospace;"
    "  font-size: 0.9em;"
    "}"
    ".env-value {"
    "  color: #cdd6f4;"
    "  font-family: monospace;"
    "  font-size: 0.9em;"
    "}"
    ".env-unset {"
    "  color: #6c7086;"
    "  font-family: monospace;"
    "  font-size: 0.9em;"
    "  font-style: italic;"
    "}"
    ".ext-active {"
    "  color: #a6e3a1;"
    "}"
    ".ext-inactive {"
    "  color: #f38ba8;"
    "}"
    ".input-label {"
    "  color: #bac2de;"
    "}"
    ".input-frame {"
    "  border-radius: 8px;"
    "}"
    ".log-view {"
    "  background-color: #181825;"
    "  color: #a6adc8;"
    "  font-family: monospace;"
    "  font-size: 0.85em;"
    "  padding: 8px;"
    "}"
    "textview {"
    "  background-color: #313244;"
    "  color: #cdd6f4;"
    "  border-radius: 6px;"
    "  caret-color: #89b4fa;"
    "}"
    "entry {"
    "  background-color: #313244;"
    "  color: #cdd6f4;"
    "  border-radius: 6px;"
    "  caret-color: #89b4fa;"
    "}"
    "spinbutton {"
    "  background-color: #313244;"
    "  color: #cdd6f4;"
    "}"
    "button {"
    "  background-color: #45475a;"
    "  color: #cdd6f4;"
    "  border-radius: 6px;"
    "}"
    "button:hover {"
    "  background-color: #585b70;"
    "}"
    "headerbar {"
    "  background-color: #181825;"
    "  color: #cdd6f4;"
    "}"
    ;

/* ───────────────────────────────────────────── */
/*  전역 상태                                    */
/* ───────────────────────────────────────────── */

static GtkTextBuffer *log_buffer  = NULL;
static GtkWidget     *log_view    = NULL;
static GtkWidget     *mode_label  = NULL;
static GtkWidget     *dbus_label  = NULL;
static GtkWidget     *ext_label   = NULL;
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
    g_print("[GNOME-TEST] %s\n", entry->message);

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
check_extension_status(void)
{
    GError *error = NULL;

    GDBusProxy *ext_proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION,
        G_DBUS_PROXY_FLAGS_NONE,
        NULL,
        "org.gnome.Shell",
        "/org/gnome/Shell",
        "org.gnome.Shell.Extensions",
        NULL, &error);

    if (error) {
        gtk_label_set_text(GTK_LABEL(ext_label), "⚠️ 확인 불가");
        gtk_widget_add_css_class(ext_label, "ext-inactive");
        app_log("GNOME Shell Extensions DBus 접근 실패: %s", error->message);
        g_error_free(error);
        return;
    }

    GVariant *result = g_dbus_proxy_call_sync(
        ext_proxy, "GetExtensionInfo",
        g_variant_new("(s)", "unim-indicator@from104.github.io"),
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

    if (result) {
        GVariant *info = g_variant_get_child_value(result, 0);
        GVariant *state_var = g_variant_lookup_value(info, "state", G_VARIANT_TYPE_DOUBLE);

        if (state_var) {
            double state = g_variant_get_double(state_var);
            /* GNOME Shell extension state: 1 = ENABLED */
            if ((int)state == 1) {
                gtk_label_set_text(GTK_LABEL(ext_label), "✅ 활성");
                gtk_widget_add_css_class(ext_label, "ext-active");
                app_log("GNOME Extension: 활성 상태");
            } else {
                char *ext_msg = g_strdup_printf("❌ 비활성 (state=%d)", (int)state);
                gtk_label_set_text(GTK_LABEL(ext_label), ext_msg);
                g_free(ext_msg);
                gtk_widget_add_css_class(ext_label, "ext-inactive");
                app_log("GNOME Extension: 비활성 (state=%.0f)", state);
            }
            g_variant_unref(state_var);
        } else {
            gtk_label_set_text(GTK_LABEL(ext_label), "⚠️ 상태 미확인");
            gtk_widget_add_css_class(ext_label, "ext-inactive");
        }

        g_variant_unref(info);
        g_variant_unref(result);
    } else {
        gtk_label_set_text(GTK_LABEL(ext_label), "❌ 미설치");
        gtk_widget_add_css_class(ext_label, "ext-inactive");
        if (error) {
            app_log("Extension 상태 조회 실패: %s", error->message);
            g_error_free(error);
        }
    }

    g_object_unref(ext_proxy);
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
/*  IM Context 시그널 모니터링                   */
/* ───────────────────────────────────────────── */

static void
on_im_commit(GtkIMContext *ctx, const gchar *text, gpointer user_data)
{
    (void)ctx; (void)user_data;
    app_log("IM commit: \"%s\"", text);
}

static void
on_im_preedit_changed(GtkIMContext *ctx, gpointer user_data)
{
    (void)user_data;
    char *preedit_str = NULL;
    PangoAttrList *attrs = NULL;
    int cursor_pos = 0;

    gtk_im_context_get_preedit_string(ctx, &preedit_str, &attrs, &cursor_pos);

    if (preedit_str && preedit_str[0]) {
        app_log("IM preedit: \"%s\" (cursor=%d)", preedit_str, cursor_pos);
    } else {
        app_log("IM preedit: (cleared)");
    }

    g_free(preedit_str);
    if (attrs)
        pango_attr_list_unref(attrs);
}

static void
on_im_preedit_start(GtkIMContext *ctx, gpointer user_data)
{
    (void)ctx; (void)user_data;
    app_log("IM preedit-start");
}

static void
on_im_preedit_end(GtkIMContext *ctx, gpointer user_data)
{
    (void)ctx; (void)user_data;
    app_log("IM preedit-end");
}

/* Entry의 IM Context에 시그널을 연결하기 위한 헬퍼 */
static void
attach_im_monitor(GtkWidget *widget, const char *name)
{
    /* GtkText (Entry의 내부 위젯)에서 IM Context 접근 */
    GtkIMContext *ctx = NULL;

    if (GTK_IS_TEXT(widget)) {
        g_object_get(widget, "im-context", &ctx, NULL);
    } else if (GTK_IS_ENTRY(widget)) {
        /* GtkEntry → 내부 GtkText 접근 */
        GtkWidget *text_widget = gtk_widget_get_first_child(widget);
        while (text_widget) {
            if (GTK_IS_TEXT(text_widget)) {
                g_object_get(text_widget, "im-context", &ctx, NULL);
                break;
            }
            text_widget = gtk_widget_get_next_sibling(text_widget);
        }
    } else if (GTK_IS_TEXT_VIEW(widget)) {
        /* GtkTextView는 im-module 프로퍼티를 가짐 */
        ctx = gtk_text_view_get_input_hints(GTK_TEXT_VIEW(widget)) ? NULL : NULL;
        /* GtkTextView의 IM Context는 직접 접근이 제한적 */
    }

    if (ctx) {
        g_signal_connect(ctx, "commit",
                         G_CALLBACK(on_im_commit), (gpointer)name);
        g_signal_connect(ctx, "preedit-changed",
                         G_CALLBACK(on_im_preedit_changed), (gpointer)name);
        g_signal_connect(ctx, "preedit-start",
                         G_CALLBACK(on_im_preedit_start), (gpointer)name);
        g_signal_connect(ctx, "preedit-end",
                         G_CALLBACK(on_im_preedit_end), (gpointer)name);
        app_log("IM Context 모니터 연결: %s", name);
        g_object_unref(ctx);
    }
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

/* 환경변수 행 생성 */
static GtkWidget *
make_env_row(const char *key)
{
    GtkWidget *row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    gtk_widget_set_margin_start(row, 4);
    gtk_widget_set_margin_end(row, 4);

    GtkWidget *key_label = gtk_label_new(key);
    gtk_label_set_xalign(GTK_LABEL(key_label), 0);
    gtk_widget_set_size_request(key_label, 220, -1);
    gtk_widget_add_css_class(key_label, "env-key");
    gtk_box_append(GTK_BOX(row), key_label);

    const char *val = g_getenv(key);
    GtkWidget *val_label;
    if (val) {
        val_label = gtk_label_new(val);
        gtk_widget_add_css_class(val_label, "env-value");
    } else {
        val_label = gtk_label_new("(unset)");
        gtk_widget_add_css_class(val_label, "env-unset");
    }
    gtk_label_set_xalign(GTK_LABEL(val_label), 0);
    gtk_label_set_ellipsize(GTK_LABEL(val_label), PANGO_ELLIPSIZE_END);
    gtk_widget_set_hexpand(val_label, TRUE);
    gtk_box_append(GTK_BOX(row), val_label);

    return row;
}

static GtkWidget *
build_env_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    gtk_widget_add_css_class(box, "env-panel");
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 12);

    GtkWidget *title = gtk_label_new("환경 진단");
    gtk_widget_add_css_class(title, "section-title");
    gtk_label_set_xalign(GTK_LABEL(title), 0);
    gtk_box_append(GTK_BOX(box), title);

    gtk_box_append(GTK_BOX(box), make_env_row("GDK_BACKEND"));
    gtk_box_append(GTK_BOX(box), make_env_row("GTK_IM_MODULE"));
    gtk_box_append(GTK_BOX(box), make_env_row("QT_IM_MODULE"));
    gtk_box_append(GTK_BOX(box), make_env_row("XMODIFIERS"));
    gtk_box_append(GTK_BOX(box), make_env_row("WAYLAND_DISPLAY"));
    gtk_box_append(GTK_BOX(box), make_env_row("XDG_SESSION_TYPE"));

    return box;
}

static GtkWidget *
build_status_section(void)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
    gtk_widget_add_css_class(box, "status-panel");
    gtk_widget_set_margin_start(box, 16);
    gtk_widget_set_margin_end(box, 16);
    gtk_widget_set_margin_top(box, 12);

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

    /* Extension 상태 행 */
    GtkWidget *ext_row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    GtkWidget *ext_title = gtk_label_new("GNOME Extension");
    gtk_widget_add_css_class(ext_title, "status-title");
    gtk_label_set_xalign(GTK_LABEL(ext_title), 0);
    gtk_box_append(GTK_BOX(ext_row), ext_title);

    ext_label = gtk_label_new("확인 중…");
    gtk_widget_add_css_class(ext_label, "status-value");
    gtk_widget_set_hexpand(ext_label, TRUE);
    gtk_label_set_xalign(GTK_LABEL(ext_label), 1);
    gtk_box_append(GTK_BOX(ext_row), ext_label);
    gtk_box_append(GTK_BOX(box), ext_row);

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

    /* IM Context 모니터링을 첫 번째 Entry에 연결 */
    attach_im_monitor(entry1, "일반 Entry");

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

    /* ── CSS ── */
    GtkCssProvider *css = gtk_css_provider_new();
    gtk_css_provider_load_from_string(css, APP_CSS);
    gtk_style_context_add_provider_for_display(
        gdk_display_get_default(),
        GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
    g_object_unref(css);

    /* ── 윈도우 ── */
    GtkWidget *window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "UNIM GNOME IME 테스트");
    gtk_window_set_default_size(GTK_WINDOW(window), 680, 900);

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

    gtk_box_append(GTK_BOX(content), build_env_section());
    gtk_box_append(GTK_BOX(content), build_status_section());
    gtk_box_append(GTK_BOX(content), build_entry_section());
    gtk_box_append(GTK_BOX(content), build_textview_section());
    gtk_box_append(GTK_BOX(content), build_log_section());

    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroll), content);
    gtk_window_set_child(GTK_WINDOW(window), scroll);

    /* ── 초기화 ── */
    app_log("UNIM GNOME IME 테스트 앱 시작");
    app_log("GDK_BACKEND=%s",
            g_getenv("GDK_BACKEND") ? g_getenv("GDK_BACKEND") : "(unset)");
    app_log("GTK_IM_MODULE=%s",
            g_getenv("GTK_IM_MODULE") ? g_getenv("GTK_IM_MODULE") : "(unset → GNOME Shell IME 경로)");
    app_log("WAYLAND_DISPLAY=%s",
            g_getenv("WAYLAND_DISPLAY") ? g_getenv("WAYLAND_DISPLAY") : "(unset)");

    if (g_getenv("GTK_IM_MODULE")) {
        app_log("⚠️ GTK_IM_MODULE이 설정되어 있습니다! GNOME Shell IME 테스트를 위해 해제하세요.");
        app_log("   실행: unset GTK_IM_MODULE && ./unim-test-gnome");
    }

    if (!g_getenv("WAYLAND_DISPLAY")) {
        app_log("⚠️ Wayland 세션이 아닙니다. GNOME Shell IME는 Wayland에서만 동작합니다.");
    }

    setup_dbus();
    check_extension_status();

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

int
main(int argc, char *argv[])
{
    /* GNOME Shell IME 테스트를 위해 GTK_IM_MODULE을 의도적으로 해제 */
    g_unsetenv("GTK_IM_MODULE");

    GtkApplication *app = gtk_application_new(
        "io.github.from104.unim.test.gnome",
        G_APPLICATION_DEFAULT_FLAGS);

    g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);

    int status = g_application_run(G_APPLICATION(app), argc, argv);

    cleanup();
    g_object_unref(app);

    return status;
}
