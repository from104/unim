/**
 * UNIM 공통 자동 테스트 라이브러리 — 구현
 */

#include "unim_test.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define DBUS_NAME    "org.atit.unim.InputMethod"
#define DBUS_IM_PATH "/org/atit/unim/InputMethod"
#define DBUS_IM_IFACE "org.atit.unim.InputMethod"
#define DBUS_IC_IFACE "org.atit.unim.InputContext"
#define DBUS_TIMEOUT  3000

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

long unim_test_log_mark(void) {
    const char *home = g_get_home_dir();
    char *path = g_build_filename(home, ".unim-errors.log", NULL);
    FILE *f = fopen(path, "r");
    g_free(path);
    if (!f) return 0;
    fseek(f, 0, SEEK_END);
    long pos = ftell(f);
    fclose(f);
    return pos;
}

void unim_test_log_check(UnimTestRunner *runner, long mark) {
    const char *home = g_get_home_dir();
    char *path = g_build_filename(home, ".unim-errors.log", NULL);
    FILE *f = fopen(path, "r");
    g_free(path);
    if (!f) {
        printf("\n" UNIM_BOLD "── 로그 검증 ──" UNIM_RESET "\n");
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " 로그 파일 없음 (정상)\n");
        runner->passed++;
        return;
    }

    fseek(f, mark, SEEK_SET);

    int panics = 0, errors = 0, lines = 0;
    char line[4096];
    char first_error[512] = "";

    while (fgets(line, sizeof(line), f)) {
        lines++;
        if (strstr(line, "panic") || strstr(line, "PANIC")) {
            panics++;
        }
        if (strstr(line, "ERROR") || strstr(line, "error:")) {
            if (first_error[0] == '\0') {
                g_strlcpy(first_error, line, sizeof(first_error));
                /* 줄바꿈 제거 */
                char *nl = strchr(first_error, '\n');
                if (nl) *nl = '\0';
            }
            errors++;
        }
    }
    fclose(f);

    printf("\n" UNIM_BOLD "── 로그 검증 ──" UNIM_RESET "\n");

    if (panics == 0) {
        printf("  " UNIM_GREEN "PASS" UNIM_RESET " panic 없음 (%d줄 검사)\n", lines);
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
