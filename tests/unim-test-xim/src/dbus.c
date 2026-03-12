/*
 * UNIM XIM Test - DBus Module
 *
 * unim-daemon과의 DBus 통신:
 * - GlobalModeChanged 시그널 구독
 * - 한/영 모드 조회/토글
 */

#include "app.h"
#include "log.h"
#include "dbus.h"

/* ─── Signal Handler ──────────────────────────────────────────────────── */

static void on_dbus_signal(GDBusProxy *proxy,
                           const gchar *sender_name,
                           const gchar *signal_name,
                           GVariant *parameters,
                           gpointer user_data)
{
    (void)proxy; (void)sender_name; (void)user_data;

    if (g_strcmp0(signal_name, "GlobalModeChanged") == 0) {
        gboolean is_korean;
        g_variant_get(parameters, "(b)", &is_korean);
        app.is_korean_mode = is_korean ? 1 : 0;
        snprintf(app.mode_status, sizeof(app.mode_status),
                 is_korean ? "한국어" : "영어");
        log_add("입력 모드 변경: %s", is_korean ? "한국어" : "영어");
    }
}

/* ─── Public API ──────────────────────────────────────────────────────── */

void dbus_setup(void) {
    GError *error = NULL;

    app.dbus_proxy = g_dbus_proxy_new_for_bus_sync(
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
        snprintf(app.dbus_status, sizeof(app.dbus_status), "연결 실패");
        log_add("DBus 연결 실패: %s", error->message);
        g_error_free(error);
        return;
    }

    g_signal_connect(app.dbus_proxy, "g-signal",
                     G_CALLBACK(on_dbus_signal), NULL);

    app.dbus_connected = 1;
    snprintf(app.dbus_status, sizeof(app.dbus_status), "연결됨");
    log_add("DBus 연결 성공");

    /* 초기 모드 조회 */
    GVariant *result = g_dbus_proxy_call_sync(
        app.dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);

    if (result) {
        gboolean is_korean;
        g_variant_get(result, "(b)", &is_korean);
        app.is_korean_mode = is_korean ? 1 : 0;
        snprintf(app.mode_status, sizeof(app.mode_status),
                 is_korean ? "한국어" : "영어");
        log_add("초기 입력 모드: %s", is_korean ? "한국어" : "영어");
        g_variant_unref(result);
    }
}

void dbus_toggle_mode(void) {
    if (!app.dbus_proxy) {
        log_add("DBus 연결 없음 - 토글 불가");
        return;
    }

    GVariant *result = g_dbus_proxy_call_sync(
        app.dbus_proxy, "GetGlobalMode", NULL,
        G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);

    if (result) {
        gboolean current;
        g_variant_get(result, "(b)", &current);
        g_variant_unref(result);

        g_dbus_proxy_call_sync(
            app.dbus_proxy, "SetGlobalMode",
            g_variant_new("(b)", !current),
            G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
    }
}

void dbus_process_pending(void) {
    GMainContext *ctx = g_main_context_default();
    while (g_main_context_pending(ctx)) {
        g_main_context_iteration(ctx, FALSE);
    }
}

void dbus_cleanup(void) {
    if (app.dbus_proxy) {
        g_object_unref(app.dbus_proxy);
        app.dbus_proxy = NULL;
    }
}
