/**
 * UNIM 테스트 앱 — 데몬 연결·상태 패널 구현
 */

#include "unim_test_dbus.h"
#include "unim_test_log.h"

#include <gio/gio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define DBUS_NAME     "org.atit.unim.InputMethod"
#define DBUS_IM_PATH  "/org/atit/unim/InputMethod"
#define DBUS_IM_IFACE "org.atit.unim.InputMethod"

struct UnimTestDaemon {
    GDBusProxy         *proxy;
    gboolean            connected;
    gboolean            korean;
    char                layout[64];
    char                error[256];
    UnimDaemonChangedFn cb;
    void               *user;
};

/* ─── 내부 ────────────────────────────────────────────────────────────── */

static double elapsed_ms(gint64 start_us) {
    return (double)(g_get_monotonic_time() - start_us) / 1000.0;
}

static void on_g_signal(GDBusProxy *proxy, const char *sender,
                        const char *signal, GVariant *params, gpointer user) {
    (void)proxy; (void)sender;
    UnimTestDaemon *d = user;

    char *dump = g_variant_print(params, FALSE);
    unim_log_dbus("signal", DBUS_IM_IFACE, signal, dump ? dump : "", 0);
    g_free(dump);

    if (g_strcmp0(signal, "GlobalModeChanged") == 0) {
        gboolean korean = FALSE;
        g_variant_get(params, "(b)", &korean);
        if (d->korean != korean) {
            unim_log_note("엔진 모드 변경: %s → %s",
                          d->korean ? "한글" : "영문", korean ? "한글" : "영문");
            d->korean = korean;
        }
        if (d->cb) d->cb(d->user);
    }
}

static void fetch_mode(UnimTestDaemon *d) {
    if (!d->proxy) return;
    gint64 t0 = g_get_monotonic_time();
    GError *err = NULL;
    GVariant *r = g_dbus_proxy_call_sync(d->proxy, "GetGlobalMode", NULL,
                                         G_DBUS_CALL_FLAGS_NONE, 3000, NULL, &err);
    if (r) {
        g_variant_get(r, "(b)", &d->korean);
        unim_log_dbus("call", DBUS_IM_IFACE, "GetGlobalMode",
                      d->korean ? "한글" : "영문", elapsed_ms(t0));
        g_variant_unref(r);
    } else {
        unim_log_dbus("error", DBUS_IM_IFACE, "GetGlobalMode",
                      err ? err->message : "?", elapsed_ms(t0));
        if (err) g_error_free(err);
    }
}

static void fetch_layout(UnimTestDaemon *d) {
    if (!d->proxy) return;
    gint64 t0 = g_get_monotonic_time();
    GError *err = NULL;
    GVariant *r = g_dbus_proxy_call_sync(
        d->proxy, "GetConfig", g_variant_new("(s)", "korean_layout"),
        G_DBUS_CALL_FLAGS_NONE, 3000, NULL, &err);
    if (r) {
        const char *s = NULL;
        g_variant_get(r, "(&s)", &s);
        snprintf(d->layout, sizeof d->layout, "%s", s ? s : "?");
        unim_log_dbus("call", DBUS_IM_IFACE, "GetConfig(korean_layout)",
                      d->layout, elapsed_ms(t0));
        g_variant_unref(r);
    } else {
        snprintf(d->layout, sizeof d->layout, "?");
        unim_log_dbus("error", DBUS_IM_IFACE, "GetConfig(korean_layout)",
                      err ? err->message : "?", elapsed_ms(t0));
        if (err) g_error_free(err);
    }
}

/* ─── 공개 ────────────────────────────────────────────────────────────── */

UnimTestDaemon *unim_daemon_connect(UnimDaemonChangedFn cb, void *user_data) {
    UnimTestDaemon *d = g_new0(UnimTestDaemon, 1);
    d->cb   = cb;
    d->user = user_data;
    snprintf(d->layout, sizeof d->layout, "?");

    gint64 t0 = g_get_monotonic_time();
    GError *err = NULL;
    d->proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION, G_DBUS_PROXY_FLAGS_NONE, NULL,
        DBUS_NAME, DBUS_IM_PATH, DBUS_IM_IFACE, NULL, &err);

    if (!d->proxy) {
        snprintf(d->error, sizeof d->error, "%s", err ? err->message : "unknown");
        unim_log_dbus("error", DBUS_IM_IFACE, "connect", d->error, elapsed_ms(t0));
        unim_log_error("데몬 연결 실패 — 상태 패널은 '연결 안 됨'으로 표시된다: %s",
                       d->error);
        if (err) g_error_free(err);
        return d;
    }

    /* 프록시는 이름 소유자가 없어도 만들어진다 — 실제 소유자를 확인해야 한다. */
    char *owner = g_dbus_proxy_get_name_owner(d->proxy);
    if (!owner) {
        snprintf(d->error, sizeof d->error, "%s 소유자 없음 (데몬 미실행)",
                 DBUS_NAME);
        unim_log_dbus("error", DBUS_IM_IFACE, "connect", d->error, elapsed_ms(t0));
        unim_log_error("unim-daemon 이 떠 있지 않다");
        return d;
    }
    unim_log_dbus("connect", DBUS_IM_IFACE, "", owner, elapsed_ms(t0));
    g_free(owner);

    d->connected = TRUE;
    g_signal_connect(d->proxy, "g-signal", G_CALLBACK(on_g_signal), d);
    unim_log_note("GlobalModeChanged 시그널 구독");

    fetch_mode(d);
    fetch_layout(d);
    return d;
}

void unim_daemon_free(UnimTestDaemon *d) {
    if (!d) return;
    if (d->proxy) g_object_unref(d->proxy);
    g_free(d);
}

gboolean unim_daemon_connected(const UnimTestDaemon *d) {
    return d && d->connected;
}
gboolean unim_daemon_korean(const UnimTestDaemon *d) {
    return d && d->korean;
}
const char *unim_daemon_layout(const UnimTestDaemon *d) {
    return d ? d->layout : "?";
}
const char *unim_daemon_error(const UnimTestDaemon *d) {
    return d ? d->error : "";
}

void unim_daemon_refresh(UnimTestDaemon *d) {
    if (!d || !d->connected) return;
    fetch_mode(d);
    fetch_layout(d);
    if (d->cb) d->cb(d->user);
}

void unim_daemon_toggle(UnimTestDaemon *d) {
    if (!d || !d->connected) {
        unim_log_warn("한/영 토글 요청 — 데몬 연결 없음, 무시");
        return;
    }
    gint64 t0 = g_get_monotonic_time();
    GError *err = NULL;
    GVariant *r = g_dbus_proxy_call_sync(
        d->proxy, "SetGlobalMode", g_variant_new("(b)", !d->korean),
        G_DBUS_CALL_FLAGS_NONE, 3000, NULL, &err);
    if (r) {
        unim_log_dbus("call", DBUS_IM_IFACE, "SetGlobalMode",
                      !d->korean ? "한글" : "영문", elapsed_ms(t0));
        g_variant_unref(r);
    } else {
        unim_log_dbus("error", DBUS_IM_IFACE, "SetGlobalMode",
                      err ? err->message : "?", elapsed_ms(t0));
        if (err) g_error_free(err);
    }
}

/* ─── 상태 패널 ───────────────────────────────────────────────────────── */

const char *unim_status_im_path(const char *frontend) {
    static char buf[256];
    const char *gtk = g_getenv("GTK_IM_MODULE");
    const char *qt  = g_getenv("QT_IM_MODULE");
    const char *xm  = g_getenv("XMODIFIERS");
    const char *be  = g_getenv("GDK_BACKEND");
    const char *st  = g_getenv("XDG_SESSION_TYPE");

    if (frontend && g_str_has_prefix(frontend, "qt"))
        snprintf(buf, sizeof buf, "QT_IM_MODULE=%s  XMODIFIERS=%s",
                 qt ? qt : "(없음)", xm ? xm : "(없음)");
    else if (frontend && g_strcmp0(frontend, "xim") == 0)
        snprintf(buf, sizeof buf, "XMODIFIERS=%s  DISPLAY=%s",
                 xm ? xm : "(없음)", g_getenv("DISPLAY") ? g_getenv("DISPLAY") : "");
    else if (frontend && g_strcmp0(frontend, "wayland") == 0)
        snprintf(buf, sizeof buf, "text-input-v3  SESSION=%s", st ? st : "?");
    else
        snprintf(buf, sizeof buf, "GTK_IM_MODULE=%s  BACKEND=%s",
                 gtk ? gtk : "(없음)", be ? be : (st ? st : "?"));
    return buf;
}

void unim_status_render(const UnimTestDaemon *d, const UnimStatusInput *in,
                        char out[UNIM_STATUS_N][UNIM_STATUS_VALUE_MAX]) {
    /* ① DBus */
    if (unim_daemon_connected(d))
        snprintf(out[UNIM_STATUS_DBUS], UNIM_STATUS_VALUE_MAX,
                 "✅ 연결됨 (%s)", DBUS_NAME);
    else
        snprintf(out[UNIM_STATUS_DBUS], UNIM_STATUS_VALUE_MAX,
                 "❌ 연결 안 됨 — %.180s", unim_daemon_error(d));

    /* ② 프런트엔드 + 결정된 IM 경로 */
    snprintf(out[UNIM_STATUS_FRONTEND], UNIM_STATUS_VALUE_MAX, "%s   %s",
             in->frontend ? in->frontend : "?",
             in->im_path ? in->im_path : unim_status_im_path(in->frontend));

    /* ③ 엔진 모드 + 레이아웃 */
    snprintf(out[UNIM_STATUS_MODE], UNIM_STATUS_VALUE_MAX, "%s   레이아웃 %s",
             unim_daemon_connected(d)
                 ? (unim_daemon_korean(d) ? "🇰🇷 한글" : "🔤 영문")
                 : "(알 수 없음)",
             unim_daemon_layout(d));

    /* ④ 포커스 */
    snprintf(out[UNIM_STATUS_FOCUS], UNIM_STATUS_VALUE_MAX, "%s",
             (in->focus_field && *in->focus_field) ? in->focus_field : "(없음)");

    /* ⑤ preedit */
    if (in->preedit && *in->preedit)
        snprintf(out[UNIM_STATUS_PREEDIT], UNIM_STATUS_VALUE_MAX,
                 "\"%s\"  (%zu자 / %zuB / 커서 %d)", in->preedit,
                 unim_log_utf8_len(in->preedit), strlen(in->preedit),
                 in->preedit_caret);
    else
        snprintf(out[UNIM_STATUS_PREEDIT], UNIM_STATUS_VALUE_MAX, "(없음)");

    /* ⑥ 최근 commit */
    if (in->last_commit && *in->last_commit)
        snprintf(out[UNIM_STATUS_COMMIT], UNIM_STATUS_VALUE_MAX,
                 "\"%s\"  (%zu자)", in->last_commit,
                 unim_log_utf8_len(in->last_commit));
    else
        snprintf(out[UNIM_STATUS_COMMIT], UNIM_STATUS_VALUE_MAX, "(없음)");
}
