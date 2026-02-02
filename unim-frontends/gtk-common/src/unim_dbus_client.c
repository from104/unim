/**
 * UNIM DBus Client Implementation
 *
 * GDBus를 사용하여 unim-daemon과 통신하는 클라이언트 구현입니다.
 */

#include "unim_dbus_client.h"
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <time.h>

/* 디버그 로깅 */
static gboolean unim_dbus_debug_enabled = FALSE;
static gboolean unim_dbus_debug_checked = FALSE;

/* 중앙 로깅 함수 - 콘솔과 파일에 동시 출력 */
static void
unim_log_message(const char *module, const char *format, ...)
{
    if (!unim_dbus_debug_enabled) return;

    va_list args;
    char message[1024];
    char timestamp[32];
    char log_line[2048];
    time_t now;
    struct tm *tm_info;

    /* 메시지 포맷팅 */
    va_start(args, format);
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);

    /* 타임스탬프 생성 */
    time(&now);
    tm_info = localtime(&now);
    strftime(timestamp, sizeof(timestamp), "%Y/%m/%d %H:%M:%S", tm_info);

    /* 로그 라인 생성 */
    snprintf(log_line, sizeof(log_line), "[%s] - [%s] - %s", timestamp, module, message);

    /* 콘솔 출력 */
    g_print("%s\n", log_line);

    /* 파일 출력 */
    const gchar *home = g_get_home_dir();
    if (home) {
        gchar *log_path = g_build_filename(home, ".unim-errors.log", NULL);
        FILE *f = fopen(log_path, "a");
        if (f) {
            fprintf(f, "%s\n", log_line);
            fclose(f);
        }
        g_free(log_path);
    }
}

#define UNIM_DBUS_DEBUG(fmt, ...) \
    unim_log_message("GTK_DBUS", fmt, ##__VA_ARGS__)

static void
unim_dbus_check_debug_env(void)
{
    if (!unim_dbus_debug_checked) {
        const char *env = g_getenv("UNIM_DEVELOP");
        if (env && g_strcmp0(env, "1") == 0) {
            unim_dbus_debug_enabled = TRUE;
        }
        unim_dbus_debug_checked = TRUE;
    }
}


/* 내부 구조체 */
struct _UnimDbusContext {
    GDBusConnection *connection;
    gchar *context_path;
    gchar *preedit_cache;      /* 현재 preedit 캐시 */
    gboolean is_composing;     /* 조합 중인지 */
};

UnimDbusContext*
unim_dbus_context_new(const gchar *client_name, const gchar *window_id)
{
    GError *error = NULL;
    GVariant *result;
    UnimDbusContext *ctx;

    unim_dbus_check_debug_env();

    ctx = g_new0(UnimDbusContext, 1);

    /* 세션 버스 연결 */
    ctx->connection = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (error) {
        UNIM_DBUS_DEBUG("DBus 연결 실패: %s", error->message);
        g_error_free(error);
        g_free(ctx);
        return NULL;
    }

    UNIM_DBUS_DEBUG("DBus 세션 버스 연결 성공");

    /* window_id가 NULL이면 빈 문자열 사용 */
    const gchar *effective_window_id = window_id ? window_id : "";

    /* InputContext 생성 요청 (window_id 포함) */
    result = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_PATH,
        UNIM_DBUS_INTERFACE,
        "CreateInputContext",
        g_variant_new("(ss)", client_name, effective_window_id),
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("CreateInputContext 실패: %s", error->message);
        g_error_free(error);
        g_object_unref(ctx->connection);
        g_free(ctx);
        return NULL;
    }

    g_variant_get(result, "(s)", &ctx->context_path);
    g_variant_unref(result);

    ctx->preedit_cache = g_strdup("");
    ctx->is_composing = FALSE;

    UNIM_DBUS_DEBUG("InputContext 생성: %s (window_id: %s)", ctx->context_path, effective_window_id);

    return ctx;
}

void
unim_dbus_context_free(UnimDbusContext *ctx)
{
    GError *error = NULL;

    if (!ctx) return;

    /* Destroy 호출 */
    if (ctx->connection && ctx->context_path) {
        g_dbus_connection_call_sync(
            ctx->connection,
            UNIM_DBUS_SERVICE,
            ctx->context_path,
            UNIM_DBUS_IC_INTERFACE,
            "Destroy",
            NULL,
            NULL,
            G_DBUS_CALL_FLAGS_NONE,
            UNIM_DBUS_TIMEOUT_MS,
            NULL,
            &error
        );
        if (error) {
            UNIM_DBUS_DEBUG("Destroy 실패: %s", error->message);
            g_error_free(error);
        }
    }

    if (ctx->connection) {
        g_object_unref(ctx->connection);
    }
    g_free(ctx->context_path);
    g_free(ctx->preedit_cache);
    g_free(ctx);
}

gboolean
unim_dbus_process_key(UnimDbusContext *ctx,
                       guint keyval,
                       guint keycode,
                       guint state,
                       UnimDbusKeyResult *result)
{
    GError *error = NULL;
    GVariant *ret;
    gboolean consumed;
    const gchar *preedit_str;
    const gchar *commit_str;

    if (!ctx || !ctx->connection || !ctx->context_path || !result) {
        return FALSE;
    }

    /* 결과 초기화 */
    result->consumed = FALSE;
    result->preedit = NULL;
    result->commit = NULL;

    UNIM_DBUS_DEBUG("ProcessKeyEvent 호출: keyval=%u, keycode=%u, state=%u",
                     keyval, keycode, state);

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "ProcessKeyEvent",
        g_variant_new("(uuu)", keyval, keycode, state),
        G_VARIANT_TYPE("(bss)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("ProcessKeyEvent 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_get(ret, "(b&s&s)", &consumed, &preedit_str, &commit_str);

    result->consumed = consumed;
    result->preedit = g_strdup(preedit_str);
    result->commit = g_strdup(commit_str);

    /* 캐시 업데이트 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup(preedit_str);
    ctx->is_composing = (preedit_str && strlen(preedit_str) > 0);

    UNIM_DBUS_DEBUG("ProcessKeyEvent 결과: consumed=%d, preedit=\"%s\", commit=\"%s\"",
                     consumed, preedit_str, commit_str);

    g_variant_unref(ret);
    return TRUE;
}

void
unim_dbus_focus_in(UnimDbusContext *ctx, const gchar *window_id)
{
    GError *error = NULL;

    if (!ctx || !ctx->connection || !ctx->context_path) return;

    /* window_id가 NULL이면 빈 문자열 사용 */
    const gchar *effective_window_id = window_id ? window_id : "";

    UNIM_DBUS_DEBUG("FocusIn 호출 (window_id: %s)", effective_window_id);

    g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "FocusIn",
        g_variant_new("(s)", effective_window_id),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("FocusIn 실패: %s", error->message);
        g_error_free(error);
    }
}

void
unim_dbus_focus_out(UnimDbusContext *ctx, gchar **commit)
{
    GError *error = NULL;

    if (commit) *commit = NULL;

    if (!ctx || !ctx->connection || !ctx->context_path) return;

    UNIM_DBUS_DEBUG("FocusOut 호출");

    /* 조합 중인 문자가 있으면 커밋 */
    if (ctx->is_composing && ctx->preedit_cache && strlen(ctx->preedit_cache) > 0) {
        if (commit) {
            *commit = g_strdup(ctx->preedit_cache);
            UNIM_DBUS_DEBUG("FocusOut 커밋: \"%s\"", *commit);
        }
    }

    g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "FocusOut",
        NULL,
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("FocusOut 실패: %s", error->message);
        g_error_free(error);
    }

    /* 상태 초기화 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup("");
    ctx->is_composing = FALSE;
}

void
unim_dbus_reset(UnimDbusContext *ctx, gchar **commit)
{
    GError *error = NULL;

    if (commit) *commit = NULL;

    if (!ctx || !ctx->connection || !ctx->context_path) return;

    UNIM_DBUS_DEBUG("Reset 호출");

    /* 조합 중인 문자가 있으면 커밋 */
    if (ctx->is_composing && ctx->preedit_cache && strlen(ctx->preedit_cache) > 0) {
        if (commit) {
            *commit = g_strdup(ctx->preedit_cache);
            UNIM_DBUS_DEBUG("Reset 커밋: \"%s\"", *commit);
        }
    }

    g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "Reset",
        NULL,
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("Reset 실패: %s", error->message);
        g_error_free(error);
    }

    /* 상태 초기화 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup("");
    ctx->is_composing = FALSE;
}

gchar*
unim_dbus_get_preedit(UnimDbusContext *ctx)
{
    if (!ctx || !ctx->preedit_cache) {
        return g_strdup("");
    }
    return g_strdup(ctx->preedit_cache);
}

gboolean
unim_dbus_is_composing(UnimDbusContext *ctx)
{
    if (!ctx) return FALSE;
    return ctx->is_composing;
}
