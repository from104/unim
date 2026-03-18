/**
 * UNIM 공통 자동 테스트 라이브러리
 *
 * 모든 프론트엔드 테스트 앱(GTK3/4, Qt5/6, XIM, GNOME)에서
 * --auto 모드로 DBus 기반 자동 회귀 테스트를 실행합니다.
 *
 * 사용법:
 *   #include "unim_test.h"
 *
 *   if (auto_mode) {
 *       UnimTestRunner *runner = unim_test_runner_new(verbose);
 *       unim_test_run_suite(runner, UNIM_TEST_SUITE_AUTO);
 *       int ret = runner->failed > 0 ? 1 : 0;
 *       unim_test_runner_free(runner);
 *       return ret;
 *   }
 */

#ifndef UNIM_TEST_H
#define UNIM_TEST_H

#include <gio/gio.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ─── ANSI 색상 ───────────────────────────────────────────────── */

#define UNIM_RED     "\x1b[0;31m"
#define UNIM_GREEN   "\x1b[0;32m"
#define UNIM_YELLOW  "\x1b[0;33m"
#define UNIM_BOLD    "\x1b[1m"
#define UNIM_DIM     "\x1b[2m"
#define UNIM_RESET   "\x1b[0m"

/* ─── 테스트 케이스 정의 ──────────────────────────────────────── */

/** 키 하나 (evdev keycode + modifier state) */
typedef struct {
    uint32_t keycode;
    uint32_t state;   /* 0=일반, 1=Shift */
} UnimTestKey;

/** 단일 테스트 케이스 */
typedef struct {
    const char *name;
    gboolean    korean_mode;       /* TRUE=한글, FALSE=영문 */
    UnimTestKey keys[32];          /* 0-terminated (keycode==0이면 끝) */
    const char *expected_preedit;  /* 최종 preedit (""이면 검사 안 함) */
    const char *expected_commit;   /* 누적 commit (""이면 검사 안 함) */
} UnimTestCase;

/** 빌트인 테스트 스위트 ID */
typedef enum {
    UNIM_TEST_SUITE_AUTO,     /* 현재 레이아웃 자동 감지 */
    UNIM_TEST_SUITE_2BUL,     /* 2벌식 */
    UNIM_TEST_SUITE_3BUL390,  /* 3벌식 390 */
    UNIM_TEST_SUITE_COMMON,   /* 공통 (영문 모드 등) */
    UNIM_TEST_SUITE_HANJA,    /* 한자 팝업 */
    UNIM_TEST_SUITE_SPECIAL,  /* 특수문자 팝업 */
} UnimTestSuiteId;

/* ─── 테스트 러너 ─────────────────────────────────────────────── */

typedef struct {
    GDBusProxy *im_proxy;       /* InputMethod 프록시 */
    GDBusProxy *ic_proxy;       /* InputContext 프록시 */
    char       *context_path;   /* 생성된 컨텍스트 경로 */
    int         passed;
    int         failed;
    gboolean    verbose;
    char        layout[64];     /* 감지된 레이아웃 */
} UnimTestRunner;

/* ─── API ─────────────────────────────────────────────────────── */

/** 러너 생성 (데몬 연결 + 컨텍스트 생성) */
UnimTestRunner *unim_test_runner_new(gboolean verbose);

/** 러너 해제 */
void unim_test_runner_free(UnimTestRunner *runner);

/** 단일 테스트 케이스 실행 → TRUE=pass */
gboolean unim_test_run_case(UnimTestRunner *runner, const UnimTestCase *tc);

/** 빌트인 스위트 실행 */
void unim_test_run_suite(UnimTestRunner *runner, UnimTestSuiteId suite_id);

/** 커스텀 테스트 배열 실행 */
void unim_test_run_cases(UnimTestRunner *runner,
                         const char *suite_name,
                         const UnimTestCase *cases,
                         int n_cases);

/** 결과 요약 출력 */
void unim_test_print_summary(UnimTestRunner *runner);

/* ─── 빌트인 테스트 스위트 조회 ───────────────────────────────── */

const UnimTestCase *unim_test_dubeolsik_cases(int *n);
const UnimTestCase *unim_test_sebeolsik390_cases(int *n);
const UnimTestCase *unim_test_common_cases(int *n);

/* ─── DBus 헬퍼 ───────────────────────────────────────────────── */

/** 현재 레이아웃 이름 조회 (caller frees) */
char *unim_test_get_layout(GDBusProxy *im_proxy);

/** 한/영 모드 설정 */
gboolean unim_test_set_mode(GDBusProxy *im_proxy, gboolean korean);

/** ProcessKeyEvent 호출 */
gboolean unim_test_process_key(GDBusProxy *ic_proxy,
                               uint32_t keyval,
                               uint32_t keycode,
                               uint32_t state,
                               gboolean *consumed,
                               char **preedit,
                               char **commit);

/** FocusIn 호출 */
gboolean unim_test_focus_in(GDBusProxy *ic_proxy, const char *window_id);

/** Reset 호출 */
gboolean unim_test_reset(GDBusProxy *ic_proxy);

/* ─── 로그 검사 ───────────────────────────────────────────────── */

/** 로그 파일 현재 위치 기록 (테스트 시작 전 호출) */
long unim_test_log_mark(void);

/** mark 이후 로그에서 panic/에러 검출, 결과 출력 */
void unim_test_log_check(UnimTestRunner *runner, long mark);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_H */
