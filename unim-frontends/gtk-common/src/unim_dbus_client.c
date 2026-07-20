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
#include <unistd.h>
#include <glib/gstdio.h>

/* 디버그 로깅 */
static gboolean unim_dbus_debug_enabled = FALSE;
static gboolean unim_dbus_debug_checked = FALSE;

/* 호스트 프로세스 이름을 sanitize 한 형태로 반환. 실패 시 "unknown".
 * 반환값은 g_free 로 해제. */
static gchar *
unim_log_process_name(void)
{
    gchar *contents = NULL;
    gchar *name = NULL;
    if (g_file_get_contents("/proc/self/comm", &contents, NULL, NULL) && contents) {
        name = g_strstrip(contents);
    }
    if (!name || !*name) {
        g_free(contents);
        return g_strdup("unknown");
    }
    gchar *out = g_strdup(name);
    g_free(contents);
    for (gchar *p = out; *p; p++) {
        if (!(g_ascii_isalnum(*p) || *p == '-' || *p == '_')) *p = '-';
    }
    return out;
}

/* 윈도우 세션·앱(프로세스)별 로그 파일 경로 계산.
 *   ~/.unim-log/{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log
 *   session-tag 우선순위: XDG_SESSION_ID > WAYLAND_DISPLAY > DISPLAY.
 *   progname/pid 는 호스트 프로세스 (GTK IM 모듈은 호스트 앱 안에서 동작).
 * 반환값은 g_free 로 해제해야 한다. 실패 시 NULL.
 */
static gchar *
unim_log_resolve_path(void)
{
    const gchar *home = g_get_home_dir();
    if (!home) return NULL;

    gchar *log_dir = g_build_filename(home, ".unim-log", NULL);
    g_mkdir_with_parents(log_dir, 0700);

    const gchar *xdg = g_getenv("XDG_SESSION_ID");
    const gchar *wl = g_getenv("WAYLAND_DISPLAY");
    const gchar *x11 = g_getenv("DISPLAY");
    gchar *raw_tag = NULL;
    if (xdg && *xdg) {
        raw_tag = g_strdup_printf("xdg-%s", xdg);
    } else if (wl && *wl) {
        raw_tag = g_strdup_printf("wl-%s", wl);
    } else if (x11 && *x11) {
        raw_tag = g_strdup_printf("x11-%s", x11);
    } else {
        raw_tag = g_strdup("unknown");
    }
    for (gchar *p = raw_tag; *p; p++) {
        if (!(g_ascii_isalnum(*p) || *p == '-' || *p == '_')) *p = '-';
    }

    time_t now;
    time(&now);
    struct tm *tm_info = localtime(&now);
    char date[16];
    strftime(date, sizeof(date), "%Y-%m-%d", tm_info);

    gchar *progname = unim_log_process_name();
    pid_t pid = getpid();

    gchar *fname = g_strdup_printf("%s_%s_%s-%d.log", raw_tag, date, progname, (int)pid);
    gchar *path = g_build_filename(log_dir, fname, NULL);

    g_free(raw_tag);
    g_free(progname);
    g_free(fname);
    g_free(log_dir);
    return path;
}

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
    gchar *log_path = unim_log_resolve_path();
    if (log_path) {
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
    /* dedupe: reset/focus_out wrapper가 sync로 *commit emit한 직후 daemon이
     * CommitText signal을 또 emit하므로, 마지막 sync commit 값을 기억해서
     * 1회만 skip한다. (외부 click pass-through로 underlying app이 reset 호출
     * 시 이중 commit 발생하던 버그 방지) */
    gchar *pending_skip_commit;
    /* AutoTypeFix 콜백 */
    UnimAutoTypeFixCallback auto_typefix_callback;
    gpointer auto_typefix_user_data;
    guint auto_typefix_signal_id;
    /* CommitText 콜백 (Standalone 팝업 마우스 클릭 커밋용) */
    UnimCommitTextCallback commit_text_callback;
    gpointer commit_text_user_data;
    guint commit_text_signal_id;
    /* HanjaBookmarkChanged 콜백 */
    UnimHanjaBookmarkChangedCallback hanja_bookmark_callback;
    gpointer hanja_bookmark_user_data;
    guint hanja_bookmark_signal_id;
    /* HanjaCandidatesReordered 콜백 */
    UnimHanjaCandidatesReorderedCallback hanja_reordered_callback;
    gpointer hanja_reordered_user_data;
    guint hanja_reordered_signal_id;
    /* ShowEmojiPopupV2 콜백 (PR #4) */
    UnimShowEmojiPopupCallback show_emoji_popup_callback;
    gpointer show_emoji_popup_user_data;
    guint show_emoji_popup_signal_id;
    /* PopupNavigate 콜백 (PR #4 — 한자/특수/이모지 공통) */
    UnimPopupNavigateCallback popup_navigate_callback;
    gpointer popup_navigate_user_data;
    guint popup_navigate_signal_id;
    /* HidePopup 콜백 (PR #4 — 한자/특수/이모지 공통) */
    UnimHidePopupCallback hide_popup_callback;
    gpointer hide_popup_user_data;
    guint hide_popup_signal_id;
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
    ctx->pending_skip_commit = NULL;

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

    /* AutoTypeFix 시그널 구독 해제 */
    if (ctx->auto_typefix_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->auto_typefix_signal_id);
    }
    /* CommitText 시그널 구독 해제 */
    if (ctx->commit_text_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->commit_text_signal_id);
    }
    /* HanjaBookmarkChanged 시그널 구독 해제 */
    if (ctx->hanja_bookmark_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->hanja_bookmark_signal_id);
    }
    /* HanjaCandidatesReordered 시그널 구독 해제 */
    if (ctx->hanja_reordered_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->hanja_reordered_signal_id);
    }
    /* ShowEmojiPopupV2 시그널 구독 해제 (PR #4) */
    if (ctx->show_emoji_popup_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->show_emoji_popup_signal_id);
    }
    /* PopupNavigate 시그널 구독 해제 (PR #4) */
    if (ctx->popup_navigate_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->popup_navigate_signal_id);
    }
    /* HidePopup 시그널 구독 해제 (PR #4) */
    if (ctx->hide_popup_signal_id > 0 && ctx->connection) {
        g_dbus_connection_signal_unsubscribe(ctx->connection, ctx->hide_popup_signal_id);
    }

    if (ctx->connection) {
        g_object_unref(ctx->connection);
    }
    g_free(ctx->context_path);
    g_free(ctx->preedit_cache);
    g_free(ctx->pending_skip_commit);
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
            /* dedupe: daemon이 곧 CommitText signal로 같은 값을 보낼 예정이므로
             * signal handler가 1회 skip하도록 표시 */
            g_free(ctx->pending_skip_commit);
            ctx->pending_skip_commit = g_strdup(*commit);
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

/* keyval 이 수정자 키인가 (Left/Right 의 Control·Shift·Alt·Super).
 * 엔진 KeyCode::is_modifier() 와 동일 범위. gtk-common 은 gdk 에 의존하지
 * 않으므로 keysym 값을 직접 쓴다 (unim_keycode_name_to_gdk_keyval 과 같은 규약). */
static gboolean
unim_keyval_is_modifier(guint keyval)
{
    switch (keyval) {
    case 0xffe1: /* Shift_L */
    case 0xffe2: /* Shift_R */
    case 0xffe3: /* Control_L */
    case 0xffe4: /* Control_R */
    case 0xffe9: /* Alt_L */
    case 0xffea: /* Alt_R */
    case 0xffeb: /* Super_L */
    case 0xffec: /* Super_R */
        return TRUE;
    default:
        return FALSE;
    }
}

/* 수정자 토큰 1개를 UNIM_HOTKEY_MOD_* 비트로. 미지 토큰이면 0 */
static guint
unim_hotkey_mod_token_to_bit(const gchar *token)
{
    static const struct { const char *name; guint bit; } map[] = {
        { "ctrl",    UNIM_HOTKEY_MOD_CTRL },
        { "control", UNIM_HOTKEY_MOD_CTRL },
        { "alt",     UNIM_HOTKEY_MOD_ALT },
        { "super",   UNIM_HOTKEY_MOD_SUPER },
        { "win",     UNIM_HOTKEY_MOD_SUPER },
        { "meta",    UNIM_HOTKEY_MOD_SUPER },
        { "shift",   UNIM_HOTKEY_MOD_SHIFT },
        { NULL, 0 }
    };

    gchar *lower = g_ascii_strdown(token, -1);
    guint bit = 0;
    for (int i = 0; map[i].name != NULL; i++) {
        if (g_strcmp0(lower, map[i].name) == 0) {
            bit = map[i].bit;
            break;
        }
    }
    g_free(lower);
    return bit;
}

gboolean
unim_parse_hotkey_spec(const gchar *spec, guint *out_keyval, guint *out_mods)
{
    if (!spec || !out_keyval || !out_mods) return FALSE;

    gchar **tokens = g_strsplit(spec, "+", -1);
    gsize n = g_strv_length(tokens);
    if (n == 0) {
        g_strfreev(tokens);
        return FALSE;
    }

    /* 마지막 토큰이 KeyName, 그 앞은 전부 수정자 */
    guint mods = 0;
    gboolean ok = TRUE;
    for (gsize i = 0; i + 1 < n; i++) {
        g_strstrip(tokens[i]);
        guint bit = unim_hotkey_mod_token_to_bit(tokens[i]);
        if (bit == 0) {          /* 미지 수정자 → 전체 실패 (침묵 무시) */
            ok = FALSE;
            break;
        }
        mods |= bit;             /* 중복 지정은 멱등 */
    }

    guint keyval = 0;
    if (ok) {
        g_strstrip(tokens[n - 1]);
        /* KeyName 은 대소문자 구분 — 엔진 KeyCode::from_name 과 동일 */
        keyval = unim_keycode_name_to_gdk_keyval(tokens[n - 1]);
        if (keyval == 0) ok = FALSE;   /* 빈 base("Ctrl+") 도 여기서 걸린다 */

        /* base 가 수정자 키면 거부 — 엔진 parse_atf_hotkey 의 KeyCode::is_modifier()
         * 배제와 동일하게 맞춘다 (Left/Right 의 Control·Shift·Alt·Super). */
        if (ok && unim_keyval_is_modifier(keyval)) ok = FALSE;
    }

    g_strfreev(tokens);
    if (!ok) return FALSE;

    *out_keyval = keyval;
    *out_mods = mods;
    return TRUE;
}

/* =========================================
 * AutoTypeFix 시그널 구독
 * ========================================= */

static void
on_auto_typefix_signal(GDBusConnection *connection G_GNUC_UNUSED,
                        const gchar *sender_name G_GNUC_UNUSED,
                        const gchar *object_path G_GNUC_UNUSED,
                        const gchar *interface_name G_GNUC_UNUSED,
                        const gchar *signal_name G_GNUC_UNUSED,
                        GVariant *parameters,
                        gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->auto_typefix_callback) return;

    guint delete_chars = 0;
    const gchar *commit_text = NULL;
    const gchar *preedit_text = NULL;

    g_variant_get(parameters, "(u&s&s)", &delete_chars, &commit_text, &preedit_text);

    if (delete_chars > 0 && commit_text) {
        UNIM_DBUS_DEBUG("AutoTypeFix 시그널 수신: delete=%u, commit='%s', preedit='%s'",
                         delete_chars, commit_text, preedit_text ? preedit_text : "");
        ctx->auto_typefix_callback(delete_chars, commit_text,
                                    preedit_text ? preedit_text : "",
                                    ctx->auto_typefix_user_data);
    }
}

void
unim_dbus_set_preedit_cache(UnimDbusContext *ctx, const gchar *preedit)
{
    if (!ctx) return;
    g_free(ctx->preedit_cache);
    ctx->preedit_cache = g_strdup(preedit ? preedit : "");
    ctx->is_composing = (preedit && preedit[0] != '\0');
}

void
unim_dbus_set_auto_typefix_callback(UnimDbusContext *ctx,
                                     UnimAutoTypeFixCallback callback,
                                     gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->auto_typefix_callback = callback;
    ctx->auto_typefix_user_data = user_data;

    /* 자기 context의 AutoTypefixApply 시그널 구독 */
    ctx->auto_typefix_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "AutoTypefixApply",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_auto_typefix_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("AutoTypeFix 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->auto_typefix_signal_id);
}

/* CommitText 시그널 핸들러 (Standalone 팝업 마우스 클릭 커밋) */
static void
on_commit_text_signal(GDBusConnection *connection,
                      const gchar *sender_name,
                      const gchar *object_path,
                      const gchar *interface_name,
                      const gchar *signal_name,
                      GVariant *parameters,
                      gpointer user_data)
{
    (void)connection; (void)sender_name; (void)object_path;
    (void)interface_name; (void)signal_name;

    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->commit_text_callback) return;

    const gchar *text = NULL;
    g_variant_get(parameters, "(&s)", &text);

    if (text && text[0] != '\0') {
        /* dedupe: wrapper(reset/focus_out)가 sync로 *commit emit한 직후 daemon
         * 이 같은 값을 CommitText signal로 또 emit. 1회만 skip해서 이중 commit
         * 을 방지한다. */
        if (ctx->pending_skip_commit && strcmp(text, ctx->pending_skip_commit) == 0) {
            UNIM_DBUS_DEBUG("CommitText 시그널 dedupe skip: text='%s'", text);
            g_free(ctx->pending_skip_commit);
            ctx->pending_skip_commit = NULL;
            return;
        }
        UNIM_DBUS_DEBUG("CommitText 시그널 수신: text='%s'", text);
        ctx->commit_text_callback(text, ctx->commit_text_user_data);
    }
}

void
unim_dbus_set_commit_text_callback(UnimDbusContext *ctx,
                                    UnimCommitTextCallback callback,
                                    gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->commit_text_callback = callback;
    ctx->commit_text_user_data = user_data;

    /* 자기 context의 CommitText 시그널 구독 */
    ctx->commit_text_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "CommitText",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_commit_text_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("CommitText 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->commit_text_signal_id);
}

/* =========================================
 * HanjaBookmark 관련 함수
 * ========================================= */

gboolean
unim_dbus_get_hanja_bookmark_states(UnimDbusContext *ctx,
                                     gboolean **states,
                                     gsize *count)
{
    GError *error = NULL;
    GVariant *ret;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;
    if (!states || !count) return FALSE;

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "GetHanjaBookmarkStates",
        NULL,
        G_VARIANT_TYPE("(ab)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("GetHanjaBookmarkStates 실패: %s", error->message);
        g_error_free(error);
        return FALSE;
    }

    GVariant *arr = NULL;
    g_variant_get(ret, "(@ab)", &arr);

    gsize n = g_variant_n_children(arr);
    gboolean *out = g_new0(gboolean, n > 0 ? n : 1);
    for (gsize i = 0; i < n; i++) {
        gboolean b = FALSE;
        g_variant_get_child(arr, i, "b", &b);
        out[i] = b;
    }

    g_variant_unref(arr);
    g_variant_unref(ret);

    *states = out;
    *count = n;
    return TRUE;
}

gboolean
unim_dbus_toggle_hanja_bookmark(UnimDbusContext *ctx, guint index)
{
    GError *error = NULL;
    GVariant *ret;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "ToggleHanjaBookmark",
        g_variant_new("(u)", index),
        G_VARIANT_TYPE("(ub)"),
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("ToggleHanjaBookmark 실패 index=%u: %s", index, error->message);
        g_error_free(error);
        return FALSE;
    }

    g_variant_unref(ret);
    return TRUE;
}

gboolean
unim_dbus_popup_change_page(UnimDbusContext *ctx, gint direction)
{
    GError *error = NULL;
    GVariant *ret;

    if (!ctx || !ctx->connection || !ctx->context_path) return FALSE;

    ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "PopupChangePage",
        g_variant_new("(i)", direction),
        NULL, /* 반환 시그니처 없음 */
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );

    if (error) {
        UNIM_DBUS_DEBUG("PopupChangePage 실패 dir=%d: %s", direction, error->message);
        g_error_free(error);
        return FALSE;
    }

    if (ret) g_variant_unref(ret);
    return TRUE;
}

/* HanjaBookmarkChanged 시그널 핸들러 */
static void
on_hanja_bookmark_changed_signal(GDBusConnection *connection G_GNUC_UNUSED,
                                  const gchar *sender_name G_GNUC_UNUSED,
                                  const gchar *object_path G_GNUC_UNUSED,
                                  const gchar *interface_name G_GNUC_UNUSED,
                                  const gchar *signal_name G_GNUC_UNUSED,
                                  GVariant *parameters,
                                  gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->hanja_bookmark_callback) return;

    guint index = 0;
    gboolean bookmarked = FALSE;
    g_variant_get(parameters, "(ub)", &index, &bookmarked);

    UNIM_DBUS_DEBUG("HanjaBookmarkChanged 시그널 수신: index=%u, bookmarked=%d",
                     index, bookmarked);
    ctx->hanja_bookmark_callback(index, bookmarked, ctx->hanja_bookmark_user_data);
}

void
unim_dbus_set_hanja_bookmark_callback(UnimDbusContext *ctx,
                                       UnimHanjaBookmarkChangedCallback callback,
                                       gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->hanja_bookmark_callback = callback;
    ctx->hanja_bookmark_user_data = user_data;

    /* 자기 context의 HanjaBookmarkChanged 시그널 구독 */
    ctx->hanja_bookmark_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "HanjaBookmarkChanged",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_hanja_bookmark_changed_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("HanjaBookmarkChanged 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->hanja_bookmark_signal_id);
}

/* HanjaCandidatesReordered 시그널 핸들러 */
static void
on_hanja_candidates_reordered_signal(GDBusConnection *connection G_GNUC_UNUSED,
                                      const gchar *sender_name G_GNUC_UNUSED,
                                      const gchar *object_path G_GNUC_UNUSED,
                                      const gchar *interface_name G_GNUC_UNUSED,
                                      const gchar *signal_name G_GNUC_UNUSED,
                                      GVariant *parameters,
                                      gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->hanja_reordered_callback) return;

    /* 시그니처: (s, as, as, ab, u, i, i, i, b, b)
     *  - 마지막 b: was_bookmarked (Phase 1, mouse-paginate UX). 본 콜백은 아직
     *    사용하지 않지만 g_variant_get 가 정확한 시그니처를 요구하므로 받아만 둔다 —
     *    Phase 7 (visual flash) 에서 콜백 시그니처 확장 시 넘기게 된다. */
    const gchar *target = NULL;
    GVariantIter *hanjas_iter = NULL;
    GVariantIter *meanings_iter = NULL;
    GVariantIter *bookmarks_iter = NULL;
    guint new_cursor = 0;
    gint page = 0;
    gint sel_row = 0;
    gint sel_col = 0;
    gboolean bookmarked = FALSE;
    gboolean was_bookmarked = FALSE;

    g_variant_get(parameters, "(&sasasabuiiibb)",
                  &target, &hanjas_iter, &meanings_iter, &bookmarks_iter,
                  &new_cursor, &page, &sel_row, &sel_col, &bookmarked, &was_bookmarked);

    /* 한자/뜻 배열 추출 */
    GPtrArray *hanjas = g_ptr_array_new_with_free_func(g_free);
    GPtrArray *meanings = g_ptr_array_new_with_free_func(g_free);
    const gchar *s;
    while (g_variant_iter_next(hanjas_iter, "&s", &s)) {
        g_ptr_array_add(hanjas, g_strdup(s));
    }
    while (g_variant_iter_next(meanings_iter, "&s", &s)) {
        g_ptr_array_add(meanings, g_strdup(s));
    }
    g_variant_iter_free(hanjas_iter);
    g_variant_iter_free(meanings_iter);

    /* 즐겨찾기 배열 추출 */
    GArray *bookmarks = g_array_new(FALSE, FALSE, sizeof(gboolean));
    gboolean b;
    while (g_variant_iter_next(bookmarks_iter, "b", &b)) {
        g_array_append_val(bookmarks, b);
    }
    g_variant_iter_free(bookmarks_iter);

    /* UnimHanjaCandidate 배열 신규 할당 (콜백이 소유권 이관) */
    gsize count = hanjas->len;
    UnimHanjaCandidate *cands = g_new0(UnimHanjaCandidate, count > 0 ? count : 1);
    for (gsize i = 0; i < count; i++) {
        cands[i].hanja = g_strdup((const gchar *)g_ptr_array_index(hanjas, i));
        cands[i].meaning = i < meanings->len
                              ? g_strdup((const gchar *)g_ptr_array_index(meanings, i))
                              : g_strdup("");
    }
    g_ptr_array_free(hanjas, TRUE);
    g_ptr_array_free(meanings, TRUE);

    UNIM_DBUS_DEBUG("HanjaCandidatesReordered 수신: target='%s', count=%zu, new_cursor=%u, page=%d, was=%d now=%d",
                     target, count, new_cursor, page, was_bookmarked, bookmarked);

    ctx->hanja_reordered_callback(
        target, cands, count,
        (const gboolean *)bookmarks->data, bookmarks->len,
        new_cursor, page, sel_row, sel_col, bookmarked, was_bookmarked,
        ctx->hanja_reordered_user_data
    );

    g_array_free(bookmarks, TRUE);
}

void
unim_dbus_set_hanja_candidates_reordered_callback(UnimDbusContext *ctx,
                                                   UnimHanjaCandidatesReorderedCallback callback,
                                                   gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->hanja_reordered_callback = callback;
    ctx->hanja_reordered_user_data = user_data;

    ctx->hanja_reordered_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "HanjaCandidatesReordered",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_hanja_candidates_reordered_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("HanjaCandidatesReordered 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->hanja_reordered_signal_id);
}

/* =========================================
 * 이모지 팝업 시그널 (PR #4 emoji overhaul)
 * ========================================= */

/* ShowEmojiPopupV2 시그널 핸들러
 * 시그니처: (s, as, s, as, a(sssu), i, i, i, i)
 *   target_cat_id, items[], top_row, recent[], categories[], cx, cy, cw, ch
 */
static void
on_show_emoji_popup_signal(GDBusConnection *connection G_GNUC_UNUSED,
                            const gchar *sender_name G_GNUC_UNUSED,
                            const gchar *object_path G_GNUC_UNUSED,
                            const gchar *interface_name G_GNUC_UNUSED,
                            const gchar *signal_name G_GNUC_UNUSED,
                            GVariant *parameters,
                            gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->show_emoji_popup_callback) return;

    const gchar *target_cat_id = NULL;
    GVariantIter *items_iter = NULL;
    const gchar *top_row = NULL;
    GVariantIter *recent_iter = NULL;
    GVariantIter *categories_iter = NULL;
    gint cx = 0, cy = 0, cw = 0, ch = 0;

    g_variant_get(parameters, "(&sas&sasa(sssu)iiii)",
                  &target_cat_id,
                  &items_iter,
                  &top_row,
                  &recent_iter,
                  &categories_iter,
                  &cx, &cy, &cw, &ch);

    /* items 배열 추출 */
    GPtrArray *items_arr = g_ptr_array_new_with_free_func(g_free);
    const gchar *s = NULL;
    while (g_variant_iter_next(items_iter, "&s", &s)) {
        g_ptr_array_add(items_arr, g_strdup(s));
    }
    g_variant_iter_free(items_iter);

    /* recent 배열 추출 */
    GPtrArray *recent_arr = g_ptr_array_new_with_free_func(g_free);
    while (g_variant_iter_next(recent_iter, "&s", &s)) {
        g_ptr_array_add(recent_arr, g_strdup(s));
    }
    g_variant_iter_free(recent_iter);

    /* categories 배열 추출 */
    gsize cat_count = g_variant_iter_n_children(categories_iter);
    UnimEmojiCategoryMeta *cats = g_new0(UnimEmojiCategoryMeta, cat_count > 0 ? cat_count : 1);
    const gchar *cid = NULL;
    const gchar *cko = NULL;
    const gchar *cen = NULL;
    guint ccount = 0;
    gsize ci = 0;
    while (g_variant_iter_next(categories_iter, "(&s&s&su)", &cid, &cko, &cen, &ccount)) {
        cats[ci].id = g_strdup(cid ? cid : "");
        cats[ci].name_ko = g_strdup(cko ? cko : "");
        cats[ci].name_en = g_strdup(cen ? cen : "");
        cats[ci].count = ccount;
        ci++;
    }
    g_variant_iter_free(categories_iter);

    UNIM_DBUS_DEBUG("ShowEmojiPopupV2 시그널 수신: cat='%s', items=%u, recent=%u, cats=%zu, cursor=(%d,%d,%d,%d)",
                     target_cat_id ? target_cat_id : "",
                     items_arr->len, recent_arr->len, cat_count,
                     cx, cy, cw, ch);

    /* 콜백을 호출하기 위해 const char* const* 배열로 변환 */
    const gchar **items_ptrs = g_new0(const gchar*, items_arr->len + 1);
    for (gsize i = 0; i < items_arr->len; i++) {
        items_ptrs[i] = (const gchar*)g_ptr_array_index(items_arr, i);
    }
    const gchar **recent_ptrs = g_new0(const gchar*, recent_arr->len + 1);
    for (gsize i = 0; i < recent_arr->len; i++) {
        recent_ptrs[i] = (const gchar*)g_ptr_array_index(recent_arr, i);
    }

    ctx->show_emoji_popup_callback(
        target_cat_id ? target_cat_id : "",
        items_ptrs, items_arr->len,
        top_row ? top_row : "",
        recent_ptrs, recent_arr->len,
        cats, cat_count,
        cx, cy, cw, ch,
        ctx->show_emoji_popup_user_data
    );

    /* 정리 */
    g_free(items_ptrs);
    g_free(recent_ptrs);
    g_ptr_array_free(items_arr, TRUE);
    g_ptr_array_free(recent_arr, TRUE);
    for (gsize i = 0; i < cat_count; i++) {
        g_free(cats[i].id);
        g_free(cats[i].name_ko);
        g_free(cats[i].name_en);
    }
    g_free(cats);
}

void
unim_dbus_set_show_emoji_popup_callback(UnimDbusContext *ctx,
                                         UnimShowEmojiPopupCallback callback,
                                         gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->show_emoji_popup_callback = callback;
    ctx->show_emoji_popup_user_data = user_data;

    ctx->show_emoji_popup_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "ShowEmojiPopupV2",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_show_emoji_popup_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("ShowEmojiPopupV2 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->show_emoji_popup_signal_id);
}

/* PopupNavigate 시그널 핸들러 (page, total_pages, selected, rows, cols, sel_row, sel_col) */
static void
on_popup_navigate_signal(GDBusConnection *connection G_GNUC_UNUSED,
                          const gchar *sender_name G_GNUC_UNUSED,
                          const gchar *object_path G_GNUC_UNUSED,
                          const gchar *interface_name G_GNUC_UNUSED,
                          const gchar *signal_name G_GNUC_UNUSED,
                          GVariant *parameters,
                          gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->popup_navigate_callback) return;

    gint page = 0, total = 0, selected = 0;
    gint rows = 0, cols = 0, sr = 0, sc = 0;
    g_variant_get(parameters, "(iiiiiii)",
                  &page, &total, &selected, &rows, &cols, &sr, &sc);

    UNIM_DBUS_DEBUG("PopupNavigate 시그널 수신: page=%d/%d, sel=(%d,%d)",
                     page, total, sr, sc);

    ctx->popup_navigate_callback(page, total, selected, rows, cols, sr, sc,
                                  ctx->popup_navigate_user_data);
}

void
unim_dbus_set_popup_navigate_callback(UnimDbusContext *ctx,
                                       UnimPopupNavigateCallback callback,
                                       gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->popup_navigate_callback = callback;
    ctx->popup_navigate_user_data = user_data;

    ctx->popup_navigate_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "PopupNavigate",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_popup_navigate_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("PopupNavigate 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->popup_navigate_signal_id);
}

/* HidePopup 시그널 핸들러 (인자 없음) */
static void
on_hide_popup_signal(GDBusConnection *connection G_GNUC_UNUSED,
                      const gchar *sender_name G_GNUC_UNUSED,
                      const gchar *object_path G_GNUC_UNUSED,
                      const gchar *interface_name G_GNUC_UNUSED,
                      const gchar *signal_name G_GNUC_UNUSED,
                      GVariant *parameters G_GNUC_UNUSED,
                      gpointer user_data)
{
    UnimDbusContext *ctx = (UnimDbusContext *)user_data;
    if (!ctx || !ctx->hide_popup_callback) return;

    UNIM_DBUS_DEBUG("HidePopup 시그널 수신");
    ctx->hide_popup_callback(ctx->hide_popup_user_data);
}

void
unim_dbus_set_hide_popup_callback(UnimDbusContext *ctx,
                                   UnimHidePopupCallback callback,
                                   gpointer user_data)
{
    if (!ctx || !ctx->connection || !ctx->context_path) return;

    ctx->hide_popup_callback = callback;
    ctx->hide_popup_user_data = user_data;

    ctx->hide_popup_signal_id = g_dbus_connection_signal_subscribe(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_IC_INTERFACE,
        "HidePopup",
        ctx->context_path,
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_hide_popup_signal,
        ctx,
        NULL
    );

    UNIM_DBUS_DEBUG("HidePopup 시그널 구독: path=%s, id=%u",
                     ctx->context_path, ctx->hide_popup_signal_id);
}

gboolean
unim_dbus_commit_emoji(UnimDbusContext *ctx, const gchar *emoji)
{
    if (!ctx || !ctx->connection || !ctx->context_path || !emoji) return FALSE;

    GError *error = NULL;
    GVariant *ret = g_dbus_connection_call_sync(
        ctx->connection,
        UNIM_DBUS_SERVICE,
        ctx->context_path,
        UNIM_DBUS_IC_INTERFACE,
        "CommitEmoji",
        g_variant_new("(s)", emoji),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        UNIM_DBUS_TIMEOUT_MS,
        NULL,
        &error
    );
    if (error) {
        UNIM_DBUS_DEBUG("CommitEmoji 실패: emoji='%s', err=%s", emoji, error->message);
        g_error_free(error);
        return FALSE;
    }
    if (ret) g_variant_unref(ret);
    return TRUE;
}
