/**
 * UNIM 공통 자동 테스트 라이브러리 — 구현
 */

#include "unim_test.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <glob.h>
#include <glib/gstdio.h>

#define DBUS_NAME    "org.atit.unim.InputMethod"
#define DBUS_IM_PATH "/org/atit/unim/InputMethod"
#define DBUS_IM_IFACE "org.atit.unim.InputMethod"
#define DBUS_IC_IFACE "org.atit.unim.InputContext"
#define DBUS_TIMEOUT  3000

/* 현재 세션·날짜에 해당하는 모든 프로세스 로그 파일을 매칭하는 glob 패턴.
 *   ~/.unim-log/{session-tag}_{YYYY-MM-DD}_*.log
 * 반환값은 g_free 로 해제. 실패 시 NULL.
 */
static gchar *unim_test_log_glob_pattern(void) {
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

    gchar *pattern = g_strdup_printf("%s/%s_%s_*.log", log_dir, raw_tag, date);

    g_free(raw_tag);
    g_free(log_dir);
    return pattern;
}

/* 로그 라인 선두의 "[YYYY/MM/DD HH:MM:SS]" 를 time_t 로 파싱. 실패 시 0 반환. */
static time_t unim_test_log_parse_ts(const char *line) {
    int year, mon, day, hour, min, sec;
    if (sscanf(line, "[%d/%d/%d %d:%d:%d]",
               &year, &mon, &day, &hour, &min, &sec) != 6) {
        return 0;
    }
    struct tm tm;
    memset(&tm, 0, sizeof(tm));
    tm.tm_year = year - 1900;
    tm.tm_mon  = mon - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min  = min;
    tm.tm_sec  = sec;
    tm.tm_isdst = -1;
    return mktime(&tm);
}

/* ─── DBus 헬퍼 구현 ─────────────────────────────────────────── */

static GDBusProxy *create_proxy(const char *path, const char *iface) {
    GError *err = NULL;
    GDBusProxy *proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION,
        G_DBUS_PROXY_FLAGS_NONE,
        NULL,
        DBUS_NAME,
        path,
        iface,
        NULL,
        &err
    );
    if (err) {
        fprintf(stderr, UNIM_RED "DBus 프록시 실패 (%s): %s" UNIM_RESET "\n", iface, err->message);
        g_error_free(err);
    }
    return proxy;
}

char *unim_test_get_layout(GDBusProxy *im_proxy) {
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        im_proxy, "GetConfig",
        g_variant_new("(s)", "korean_layout"),
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) { g_error_free(err); return g_strdup(""); }
    const char *val = NULL;
    g_variant_get(result, "(&s)", &val);
    char *ret = g_strdup(val ? val : "");
    g_variant_unref(result);
    return ret;
}

gboolean unim_test_set_mode(GDBusProxy *im_proxy, gboolean korean) {
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        im_proxy, "SetGlobalMode",
        g_variant_new("(b)", korean),
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) { g_error_free(err); return FALSE; }
    if (result) g_variant_unref(result);
    return TRUE;
}

gboolean unim_test_process_key(GDBusProxy *ic_proxy,
                               uint32_t keyval,
                               uint32_t keycode,
                               uint32_t state,
                               gboolean *consumed,
                               char **preedit,
                               char **commit) {
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        ic_proxy, "ProcessKeyEvent",
        g_variant_new("(uuu)", keyval, keycode, state),
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) {
        fprintf(stderr, "  ProcessKeyEvent 실패: %s\n", err->message);
        g_error_free(err);
        return FALSE;
    }
    gboolean c;
    const char *p = NULL, *cm = NULL;
    g_variant_get(result, "(b&s&s)", &c, &p, &cm);
    *consumed = c;
    *preedit = g_strdup(p ? p : "");
    *commit = g_strdup(cm ? cm : "");
    g_variant_unref(result);
    return TRUE;
}

gboolean unim_test_focus_in(GDBusProxy *ic_proxy, const char *window_id) {
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        ic_proxy, "FocusIn",
        g_variant_new("(s)", window_id ? window_id : ""),
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) { g_error_free(err); return FALSE; }
    if (result) g_variant_unref(result);
    return TRUE;
}

gboolean unim_test_reset(GDBusProxy *ic_proxy) {
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        ic_proxy, "Reset",
        NULL,
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) { g_error_free(err); return FALSE; }
    if (result) g_variant_unref(result);
    return TRUE;
}

/* ─── 러너 생성/해제 ─────────────────────────────────────────── */

UnimTestRunner *unim_test_runner_new(gboolean verbose) {
    UnimTestRunner *runner = g_new0(UnimTestRunner, 1);
    runner->verbose = verbose;

    /* InputMethod 프록시 */
    runner->im_proxy = create_proxy(DBUS_IM_PATH, DBUS_IM_IFACE);
    if (!runner->im_proxy) {
        fprintf(stderr, UNIM_RED "❌ 데몬 연결 실패" UNIM_RESET "\n");
        fprintf(stderr, "데몬이 실행 중인지 확인: UNIM_DEVELOP=1 unim-daemon -n --replace &\n");
        g_free(runner);
        return NULL;
    }

    /* 레이아웃 감지 */
    char *layout = unim_test_get_layout(runner->im_proxy);
    g_strlcpy(runner->layout, layout, sizeof(runner->layout));
    g_free(layout);

    /* 컨텍스트 생성 */
    GError *err = NULL;
    GVariant *result = g_dbus_proxy_call_sync(
        runner->im_proxy, "CreateInputContext",
        g_variant_new("(ss)", "test-auto", ""),
        G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, &err
    );
    if (err) {
        fprintf(stderr, UNIM_RED "❌ 컨텍스트 생성 실패: %s" UNIM_RESET "\n", err->message);
        g_error_free(err);
        g_object_unref(runner->im_proxy);
        g_free(runner);
        return NULL;
    }
    const char *path = NULL;
    g_variant_get(result, "(&s)", &path);
    runner->context_path = g_strdup(path);
    g_variant_unref(result);

    /* InputContext 프록시 */
    runner->ic_proxy = create_proxy(runner->context_path, DBUS_IC_IFACE);
    if (!runner->ic_proxy) {
        g_object_unref(runner->im_proxy);
        g_free(runner->context_path);
        g_free(runner);
        return NULL;
    }

    return runner;
}

void unim_test_runner_free(UnimTestRunner *runner) {
    if (!runner) return;

    /* 컨텍스트 파괴 */
    if (runner->ic_proxy) {
        g_dbus_proxy_call_sync(
            runner->ic_proxy, "Destroy", NULL,
            G_DBUS_CALL_FLAGS_NONE, DBUS_TIMEOUT, NULL, NULL
        );
        g_object_unref(runner->ic_proxy);
    }
    if (runner->im_proxy) g_object_unref(runner->im_proxy);
    g_free(runner->context_path);
    g_free(runner);
}

/* ─── 테스트 실행 ─────────────────────────────────────────────── */

gboolean unim_test_run_case(UnimTestRunner *runner, const UnimTestCase *tc) {
    /* 모드 설정 + FocusIn */
    unim_test_set_mode(runner->im_proxy, tc->korean_mode);
    unim_test_focus_in(runner->ic_proxy, "");

    char total_commit[1024] = "";
    char last_preedit[256] = "";
    gboolean ok = TRUE;

    /* 키 시퀀스 실행 */
    for (int i = 0; tc->keys[i].keycode != 0; i++) {
        gboolean consumed;
        char *preedit = NULL, *commit = NULL;

        if (!unim_test_process_key(runner->ic_proxy,
                                   0, tc->keys[i].keycode, tc->keys[i].state,
                                   &consumed, &preedit, &commit)) {
            ok = FALSE;
            fprintf(stderr, "  " UNIM_RED "ProcessKeyEvent 실패 (step %d)" UNIM_RESET "\n", i);
            break;
        }

        if (runner->verbose) {
            fprintf(stderr, "    " UNIM_DIM "key=%u state=%u → consumed=%d preedit=\"%s\" commit=\"%s\"" UNIM_RESET "\n",
                    tc->keys[i].keycode, tc->keys[i].state, consumed, preedit, commit);
        }

        g_strlcpy(last_preedit, preedit, sizeof(last_preedit));
        g_strlcat(total_commit, commit, sizeof(total_commit));
        g_free(preedit);
        g_free(commit);
    }

    /* 검증 */
    char detail[512] = "";

    if (ok && tc->expected_preedit[0] != '\0') {
        if (strcmp(last_preedit, tc->expected_preedit) != 0) {
            ok = FALSE;
            snprintf(detail, sizeof(detail), "preedit: 기대=\"%s\" 실제=\"%s\"",
                     tc->expected_preedit, last_preedit);
        }
    }

    if (ok && tc->expected_commit[0] != '\0') {
        if (strcmp(total_commit, tc->expected_commit) != 0) {
            ok = FALSE;
            snprintf(detail, sizeof(detail), "commit: 기대=\"%s\" 실제=\"%s\"",
                     tc->expected_commit, total_commit);
        }
    }

    /* Reset */
    unim_test_reset(runner->ic_proxy);

    /* 결과 출력 */
    if (ok) {
        runner->passed++;
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " %s\n", tc->name);
    } else {
        runner->failed++;
        printf("  " UNIM_RED "FAIL" UNIM_RESET " %s\n", tc->name);
        if (detail[0] != '\0') {
            printf("       " UNIM_DIM "%s" UNIM_RESET "\n", detail);
        }
    }

    return ok;
}

void unim_test_run_cases(UnimTestRunner *runner,
                         const char *suite_name,
                         const UnimTestCase *cases,
                         int n_cases) {
    printf("\n" UNIM_BOLD "── %s ──" UNIM_RESET "\n", suite_name);
    for (int i = 0; i < n_cases; i++) {
        unim_test_run_case(runner, &cases[i]);
    }
}

/* ─── 빌트인 테스트 스위트 ────────────────────────────────────── */

/* 키 헬퍼 매크로 */
#define K(code) { code, 0 }
#define KS(code) { code, 1 }
#define KEND { 0, 0 }

static const UnimTestCase dubeolsik_cases[] = {
    { "[2벌] 초성: ㅎ", TRUE,
      { K(34), KEND }, "ㅎ", "" },
    { "[2벌] 초성+중성: 하", TRUE,
      { K(34), K(37), KEND }, "하", "" },
    { "[2벌] 완성형: 한", TRUE,
      { K(34), K(37), K(31), KEND }, "한", "" },
    { "[2벌] 두 글자: 한글", TRUE,
      { K(34), K(37), K(31), K(19), K(50), K(33), KEND }, "글", "한" },
    { "[2벌] 쌍자음: ㄲ", TRUE,
      { KS(19), KEND }, "ㄲ", "" },
    { "[2벌] Backspace", TRUE,
      { K(34), K(37), K(14), KEND }, "ㅎ", "" },
    { "[2벌] 스페이스 확정", TRUE,
      { K(34), K(37), K(57), KEND }, "", "하 " },
};

static const UnimTestCase sebeolsik390_cases[] = {
    { "[3벌390] 초성: ㅎ", TRUE,
      { K(50), KEND }, "ㅎ", "" },
    { "[3벌390] 초성+중성: 하", TRUE,
      { K(50), K(33), KEND }, "하", "" },
    { "[3벌390] 완성형: 한", TRUE,
      { K(50), K(33), K(31), KEND }, "한", "" },
    { "[3벌390] 두 글자: 한글", TRUE,
      { K(50), K(33), K(31), K(37), K(34), K(17), KEND }, "글", "한" },
    { "[3벌390] 중성 단독: ㅏ", TRUE,
      { K(33), KEND }, "ㅏ", "" },
    { "[3벌390] Backspace", TRUE,
      { K(50), K(33), K(14), KEND }, "ㅎ", "" },
    { "[3벌390] 스페이스 확정", TRUE,
      { K(50), K(33), K(57), KEND }, "", "하 " },
};

static const UnimTestCase common_cases[] = {
    { "영문 모드: 패스스루", FALSE,
      { K(34), KEND }, "", "" },
    { "한/영 전환키", FALSE,
      { K(122), KEND }, "", "" },
};

#undef K
#undef KS
#undef KEND

const UnimTestCase *unim_test_dubeolsik_cases(int *n) {
    *n = G_N_ELEMENTS(dubeolsik_cases);
    return dubeolsik_cases;
}

const UnimTestCase *unim_test_sebeolsik390_cases(int *n) {
    *n = G_N_ELEMENTS(sebeolsik390_cases);
    return sebeolsik390_cases;
}

const UnimTestCase *unim_test_common_cases(int *n) {
    *n = G_N_ELEMENTS(common_cases);
    return common_cases;
}

void unim_test_run_suite(UnimTestRunner *runner, UnimTestSuiteId suite_id) {
    printf(UNIM_BOLD "═══ UNIM 자동 테스트 ═══" UNIM_RESET "\n");
    printf("레이아웃: %s\n", runner->layout[0] ? runner->layout : "기본값");

    int n;
    const UnimTestCase *cases;

    /* 레이아웃별 조합 테스트 */
    if (suite_id == UNIM_TEST_SUITE_AUTO || suite_id == UNIM_TEST_SUITE_2BUL ||
        suite_id == UNIM_TEST_SUITE_3BUL390) {
        gboolean is_3bul = (suite_id == UNIM_TEST_SUITE_3BUL390) ||
                           (suite_id == UNIM_TEST_SUITE_AUTO &&
                            strstr(runner->layout, "3bul") != NULL);
        if (is_3bul) {
            cases = unim_test_sebeolsik390_cases(&n);
            unim_test_run_cases(runner, "3벌식 390 조합", cases, n);
        } else {
            cases = unim_test_dubeolsik_cases(&n);
            unim_test_run_cases(runner, "2벌식 조합", cases, n);
        }
    }

    /* 공통 테스트 */
    if (suite_id == UNIM_TEST_SUITE_AUTO || suite_id == UNIM_TEST_SUITE_COMMON) {
        cases = unim_test_common_cases(&n);
        unim_test_run_cases(runner, "공통 테스트", cases, n);
    }

    unim_test_print_summary(runner);
}

void unim_test_print_summary(UnimTestRunner *runner) {
    int total = runner->passed + runner->failed;
    printf("\n");
    if (runner->failed == 0) {
        printf(UNIM_GREEN UNIM_BOLD "✅ ALL PASS" UNIM_RESET " (%d/%d)\n",
               runner->passed, total);
    } else {
        printf(UNIM_RED UNIM_BOLD "❌ %d FAILED" UNIM_RESET " / %d passed / %d total\n",
               runner->failed, runner->passed, total);
    }
}

/* ─── 로그 검사 ───────────────────────────────────────────────── */

/* 마크는 wall-clock epoch 초. check 단계에서 모든 세션 로그 파일을 스캔하면서
 * 라인 타임스탬프가 mark 이상인 항목만 검사한다. (per-PID 로그가 다중 파일에
 * 분산되므로 byte offset 방식은 사용 불가.) */
long unim_test_log_mark(void) {
    return (long)time(NULL);
}

void unim_test_log_check(UnimTestRunner *runner, long mark) {
    gchar *pattern = unim_test_log_glob_pattern();
    glob_t gres;
    int rc = pattern ? glob(pattern, 0, NULL, &gres) : -1;

    int panics = 0, errors = 0, lines = 0;
    int files_scanned = 0;
    char first_error[512] = "";

    if (rc == 0) {
        for (size_t i = 0; i < gres.gl_pathc; i++) {
            FILE *f = fopen(gres.gl_pathv[i], "r");
            if (!f) continue;
            files_scanned++;
            char line[4096];
            while (fgets(line, sizeof(line), f)) {
                time_t ts = unim_test_log_parse_ts(line);
                if (ts != 0 && (long)ts < mark) continue;
                lines++;
                if (strstr(line, "panic") || strstr(line, "PANIC")) {
                    panics++;
                }
                if (strstr(line, "ERROR") || strstr(line, "error:")) {
                    if (first_error[0] == '\0') {
                        g_strlcpy(first_error, line, sizeof(first_error));
                        char *nl = strchr(first_error, '\n');
                        if (nl) *nl = '\0';
                    }
                    errors++;
                }
            }
            fclose(f);
        }
        globfree(&gres);
    }
    g_free(pattern);

    printf("\n" UNIM_BOLD "── 로그 검증 ──" UNIM_RESET "\n");

    if (files_scanned == 0) {
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " 로그 파일 없음 (정상)\n");
        runner->passed++;
        runner->passed++;  /* panic + error 두 항목 모두 PASS 처리 */
        return;
    }

    if (panics == 0) {
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " panic 없음 (%d줄 검사 / %d파일)\n",
               lines, files_scanned);
        runner->passed++;
    } else {
        printf("  " UNIM_RED "FAIL" UNIM_RESET " panic %d건 발견\n", panics);
        runner->failed++;
    }

    if (errors == 0) {
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " 에러 없음\n");
        runner->passed++;
    } else {
        printf("  " UNIM_RED "FAIL" UNIM_RESET " 에러 %d건\n", errors);
        if (first_error[0]) {
            printf("       " UNIM_DIM "%s" UNIM_RESET "\n", first_error);
        }
        runner->failed++;
    }
}
