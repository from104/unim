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
    result->typefix_delete = 0;
    result->typefix_replacement = NULL;

    UNIM_DBUS_DEBUG("ProcessKeyEvent 호출: keyval=%u, keycode=%u, state=%u",
                     keyval, keycode, state);

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "ProcessKeyEvent",
        g_variant_new("(uuu)", keyval, keycode, state),
        G_VARIANT_TYPE("(bssis)"),
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

    gint32 typefix_del = 0;
    const gchar *typefix_repl = NULL;
    g_variant_get(ret, "(b&s&si&s)", &consumed, &preedit_str, &commit_str,
                  &typefix_del, &typefix_repl);

    result->consumed = consumed;
    result->preedit = g_strdup(preedit_str);
    result->commit = g_strdup(commit_str);
    result->typefix_delete = typefix_del;
    result->typefix_replacement = (typefix_repl && strlen(typefix_repl) > 0)
                                  ? g_strdup(typefix_repl) : NULL;

    /* 캐시 업데이트 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup(preedit_str);
    ctx->is_composing = (preedit_str && strlen(preedit_str) > 0);

    UNIM_DBUS_DEBUG("ProcessKeyEvent 결과: consumed=%d, preedit=\"%s\", commit=\"%s\", typefix=(%d,'%s')",
                     consumed, preedit_str, commit_str, typefix_del,
                     typefix_repl ? typefix_repl : "");

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

    /* DBus FocusOut 호출 — 서버가 반환하는 커밋 텍스트를 우선 사용 */
    GVariant *result = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "FocusOut",
        NULL,
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (result) {
        const gchar *server_commit = NULL;
        g_variant_get(result, "(&s)", &server_commit);
        if (commit && server_commit && strlen(server_commit) > 0) {
            *commit = g_strdup(server_commit);
            UNIM_DBUS_DEBUG("FocusOut 커밋 (서버): \"%s\"", *commit);
        }
        g_variant_unref(result);
    } else {
        UNIM_DBUS_DEBUG("FocusOut 실패: %s", error->message);
        g_error_free(error);
        /* DBus 실패 시 로컬 캐시 폴백 */
        if (ctx->is_composing && ctx->preedit_cache && strlen(ctx->preedit_cache) > 0) {
            if (commit && !*commit) {
                *commit = g_strdup(ctx->preedit_cache);
                UNIM_DBUS_DEBUG("FocusOut 커밋 (로컬 폴백): \"%s\"", *commit);
            }
        }
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

/* =========================================
 * 커서 위치 보고
 * ========================================= */

void
unim_dbus_report_cursor_rect(UnimDbusContext *ctx,
                              gint x, gint y,
                              gint width, gint height)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    /* fire-and-forget (비동기, 응답 불필요) */
    g_dbus_connection_call(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "ReportCursorRect",
        g_variant_new("(iiii)", x, y, width, height),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        NULL,
        NULL
    );
}

void
unim_dbus_set_content_type(UnimDbusContext *ctx, guint purpose)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    /* fire-and-forget */
    g_dbus_connection_call(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "SetContentType",
        g_variant_new("(u)", purpose),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        NULL,
        NULL
    );
}

void
unim_dbus_set_surrounding_text(UnimDbusContext *ctx,
                                const gchar *text,
                                guint cursor_pos,
                                guint anchor_pos)
{
    if (!ctx || !ctx->connection || !ctx->context_path || !text) return;

    /* fire-and-forget */
    g_dbus_connection_call(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "SetSurroundingText",
        g_variant_new("(suu)", text, cursor_pos, anchor_pos),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        NULL,
        NULL
    );
}

/* =========================================
 * 한자 변환 관련 함수 구현
 * ========================================= */

gboolean
unim_dbus_get_hanja_candidates(UnimDbusContext *ctx,
                                gchar **target,
                                UnimHanjaCandidate **candidates,
                                gsize *count)
{
    GError *error = NULL;
    GVariant *ret;
    const gchar *target_str;
    GVariantIter *iter;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    if (target) *target = NULL;
    if (candidates) *candidates = NULL;
    if (count) *count = 0;

    UNIM_DBUS_DEBUG("GetHanjaCandidates 호출");

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "GetHanjaCandidates",
        NULL,
        G_VARIANT_TYPE("(sa(ss))"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("GetHanjaCandidates 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_get(ret, "(&sa(ss))", &target_str, &iter);

    if (target) {
        *target = g_strdup(target_str);
    }

    /* 후보 개수 파악 */
    gsize n_candidates = g_variant_iter_n_children(iter);
    
    if (candidates && n_candidates > 0) {
        *candidates = g_new0(UnimHanjaCandidate, n_candidates);
        
        const gchar *hanja_str, *meaning_str;
        gsize i = 0;
        while (g_variant_iter_loop(iter, "(&s&s)", &hanja_str, &meaning_str)) {
            (*candidates)[i].hanja = g_strdup(hanja_str);
            (*candidates)[i].meaning = g_strdup(meaning_str);
            i++;
        }
    }

    g_variant_iter_free(iter);
    g_variant_unref(ret);

    if (count) {
        *count = n_candidates;
    }

    UNIM_DBUS_DEBUG("GetHanjaCandidates 결과: target='%s', count=%zu",
                     target_str, n_candidates);

    return TRUE;
}

gboolean
unim_dbus_select_hanja(UnimDbusContext *ctx,
                        guint index,
                        gchar **selected_hanja)
{
    GError *error = NULL;
    GVariant *ret;
    const gchar *hanja_str;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    if (selected_hanja) *selected_hanja = NULL;

    UNIM_DBUS_DEBUG("SelectHanja 호출: index=%u", index);

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "SelectHanja",
        g_variant_new("(u)", index),
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("SelectHanja 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_get(ret, "(&s)", &hanja_str);

    if (selected_hanja) {
        *selected_hanja = g_strdup(hanja_str);
    }

    UNIM_DBUS_DEBUG("SelectHanja 결과: '%s'", hanja_str);

    g_variant_unref(ret);
    return TRUE;
}

gchar *
unim_dbus_cancel_hanja(UnimDbusContext *ctx)
{
    GError *error = NULL;
    GVariant *ret;
    gchar *commit = NULL;

    if (!ctx || !ctx->connection || !ctx->context_path) return NULL;

    UNIM_DBUS_DEBUG("CancelHanja 호출");

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "CancelHanja",
        NULL,
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("CancelHanja 실패: %s", error->message);
        g_error_free(error);
    } else if (ret) {
        const gchar *text = NULL;
        g_variant_get(ret, "(&s)", &text);
        if (text && strlen(text) > 0) {
            commit = g_strdup(text);
            UNIM_DBUS_DEBUG("CancelHanja 커밋: '%s'", commit);
        }
        g_variant_unref(ret);
    }

    /* 엔진의 preedit이 클리어되었으므로 로컬 캐시도 동기화 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup("");
    ctx->is_composing = FALSE;

    return commit;
}

void
unim_hanja_candidates_free(UnimHanjaCandidate *candidates, gsize count)
{
    if (!candidates) return;

    for (gsize i = 0; i < count; i++) {
        g_free(candidates[i].hanja);
        g_free(candidates[i].meaning);
    }
    g_free(candidates);
}

/* =========================================
 * 특수문자 변환 관련 함수 구현
 * ========================================= */

gboolean
unim_dbus_get_special_char_candidates(UnimDbusContext *ctx,
                                       gchar **target,
                                       gchar ***characters,
                                       gsize *count,
                                       gchar **top_row)
{
    GError *error = NULL;
    GVariant *ret;
    const gchar *target_str;
    const gchar *top_row_str;
    GVariantIter *iter;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    if (target) *target = NULL;
    if (characters) *characters = NULL;
    if (count) *count = 0;
    if (top_row) *top_row = NULL;

    UNIM_DBUS_DEBUG("GetSpecialCharCandidates 호출");

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "GetSpecialCharCandidates",
        NULL,
        G_VARIANT_TYPE("(sass)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("GetSpecialCharCandidates 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_get(ret, "(&sas&s)", &target_str, &iter, &top_row_str);

    if (target) {
        *target = g_strdup(target_str);
    }

    if (top_row) {
        *top_row = g_strdup(top_row_str);
    }

    gsize n_chars = g_variant_iter_n_children(iter);

    if (characters && n_chars > 0) {
        *characters = g_new0(gchar*, n_chars);

        const gchar *ch_str;
        gsize i = 0;
        while (g_variant_iter_loop(iter, "&s", &ch_str)) {
            (*characters)[i] = g_strdup(ch_str);
            i++;
        }
    }

    g_variant_iter_free(iter);
    g_variant_unref(ret);

    if (count) {
        *count = n_chars;
    }

    UNIM_DBUS_DEBUG("GetSpecialCharCandidates 결과: target='%s', count=%zu, top_row='%s'",
                     target_str, n_chars, top_row_str);

    return TRUE;
}

gboolean
unim_dbus_select_special_char(UnimDbusContext *ctx,
                               guint index,
                               gchar **selected_char)
{
    GError *error = NULL;
    GVariant *ret;
    const gchar *ch_str;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    if (selected_char) *selected_char = NULL;

    UNIM_DBUS_DEBUG("SelectSpecialChar 호출: index=%u", index);

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "SelectSpecialChar",
        g_variant_new("(u)", index),
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("SelectSpecialChar 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_get(ret, "(&s)", &ch_str);

    if (selected_char) {
        *selected_char = g_strdup(ch_str);
    }

    UNIM_DBUS_DEBUG("SelectSpecialChar 결과: '%s'", ch_str);

    g_variant_unref(ret);
    return TRUE;
}

gchar *
unim_dbus_cancel_special_char(UnimDbusContext *ctx)
{
    GError *error = NULL;
    GVariant *ret;
    gchar *commit = NULL;

    if (!ctx || !ctx->connection || !ctx->context_path) return NULL;

    UNIM_DBUS_DEBUG("CancelSpecialChar 호출");

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "CancelSpecialChar",
        NULL,
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("CancelSpecialChar 실패: %s", error->message);
        g_error_free(error);
    } else if (ret) {
        const gchar *text = NULL;
        g_variant_get(ret, "(&s)", &text);
        if (text && strlen(text) > 0) {
            commit = g_strdup(text);
            UNIM_DBUS_DEBUG("CancelSpecialChar 커밋: '%s'", commit);
        }
        g_variant_unref(ret);
    }

    /* 엔진의 preedit이 클리어되었으므로 로컬 캐시도 동기화 */
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup("");
    ctx->is_composing = FALSE;

    return commit;
}

void
unim_special_chars_free(gchar **characters, gsize count)
{
    if (!characters) return;

    for (gsize i = 0; i < count; i++) {
        g_free(characters[i]);
    }
    g_free(characters);
}

/* =========================================
 * 설정 조회 관련 함수 구현
 * ========================================= */

gchar*
unim_dbus_get_config(UnimDbusContext *ctx, const gchar *key)
{
    if (!ctx || !ctx->connection || !key) return NULL;

    GError *error = NULL;
    GVariant *result = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_PATH,
        UNIM_DBUS_INTERFACE,
        "GetConfig",
        g_variant_new("(s)", key),
        G_VARIANT_TYPE("(s)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("GetConfig(%s) 실패: %s", key, error->message);
        g_error_free(error);
        return NULL;
    }

    gchar *value = NULL;
    g_variant_get(result, "(s)", &value);
    g_variant_unref(result);

    UNIM_DBUS_DEBUG("GetConfig(%s) = %s", key, value);
    return value;
}

guint
unim_keycode_name_to_gdk_keyval(const gchar *name)
{
    if (!name) return 0;

    /* GDK keyval 매핑 테이블 */
    static const struct { const char *name; guint keyval; } map[] = {
        { "Hanja",        0xff34 },   /* GDK_KEY_Hangul_Hanja */
        { "Korean",       0xff31 },   /* GDK_KEY_Hangul */
        { "F1",           0xffbe },   { "F2",  0xffbf },  { "F3",  0xffc0 },
        { "F4",           0xffc1 },   { "F5",  0xffc2 },  { "F6",  0xffc3 },
        { "F7",           0xffc4 },   { "F8",  0xffc5 },  { "F9",  0xffc6 },
        { "F10",          0xffc7 },   { "F11", 0xffc8 },  { "F12", 0xffc9 },
        { "RightAlt",     0xffea },   /* GDK_KEY_Alt_R */
        { "LeftAlt",      0xffe9 },   /* GDK_KEY_Alt_L */
        { "RightControl", 0xffe4 },   /* GDK_KEY_Control_R */
        { "LeftControl",  0xffe3 },   /* GDK_KEY_Control_L */
        { "RightShift",   0xffe2 },   /* GDK_KEY_Shift_R */
        { "LeftShift",    0xffe1 },   /* GDK_KEY_Shift_L */
        { "Space",        0x0020 },
        { "Escape",       0xff1b },
        { "CapsLock",     0xffe5 },
        { NULL, 0 }
    };

    for (int i = 0; map[i].name != NULL; i++) {
        if (g_strcmp0(name, map[i].name) == 0) {
            return map[i].keyval;
        }
    }
    return 0;
}
