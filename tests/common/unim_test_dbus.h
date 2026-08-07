/**
 * UNIM 테스트 앱 — 데몬 연결·상태 패널 (전 앱 공용)
 *
 * 6개 앱이 같은 방식으로 데몬에 붙고 **같은 문자열**로 상태를 표시한다.
 * 앱이 직접 GDBus 를 만지면 표현이 어긋나므로 여기를 통해서만 쓴다.
 *
 * 모든 DBus 활동은 `dbus.*` 사건으로 로그에 남는다.
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §3
 */

#ifndef UNIM_TEST_DBUS_H
#define UNIM_TEST_DBUS_H

#include <glib.h>
#include "unim_test_spec.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UnimTestDaemon UnimTestDaemon;

/** 데몬 상태가 바뀌었을 때(모드 변경 등) 불린다 — 앱은 화면만 갱신하면 된다. */
typedef void (*UnimDaemonChangedFn)(void *user_data);

/**
 * 세션 버스의 `org.atit.unim.InputMethod` 에 붙는다.
 * 데몬이 없어도 NULL 이 아닌 객체를 돌려준다(연결 실패 상태로).
 */
UnimTestDaemon *unim_daemon_connect(UnimDaemonChangedFn cb, void *user_data);
void unim_daemon_free(UnimTestDaemon *d);

gboolean    unim_daemon_connected(const UnimTestDaemon *d);
gboolean    unim_daemon_korean(const UnimTestDaemon *d);
const char *unim_daemon_layout(const UnimTestDaemon *d);
const char *unim_daemon_error(const UnimTestDaemon *d);

/** 한/영 토글 (테스트 앱의 토글 버튼용). */
void unim_daemon_toggle(UnimTestDaemon *d);

/** 모드를 다시 읽는다. */
void unim_daemon_refresh(UnimTestDaemon *d);

/* ─── 상태 패널 ───────────────────────────────────────────────────────── */

#define UNIM_STATUS_VALUE_MAX 256

typedef struct {
    const char *frontend;      /* "gtk3" */
    const char *im_path;       /* "GTK_IM_MODULE=unim" 등 결정된 경로 */
    const char *focus_field;   /* "core.plain" */
    const char *preedit;
    int         preedit_caret;
    const char *last_commit;
} UnimStatusInput;

/**
 * 상태 패널 6줄의 **값** 문자열을 스펙 순서대로 채운다.
 * 라벨은 `UNIM_SPEC_STATUS_LABELS` 를 쓴다.
 */
void unim_status_render(const UnimTestDaemon *d, const UnimStatusInput *in,
                        char out[UNIM_STATUS_N][UNIM_STATUS_VALUE_MAX]);

/** 이 프로세스에서 실제로 결정된 IM 경로 요약 ("GTK_IM_MODULE=unim" 등). */
const char *unim_status_im_path(const char *frontend);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_DBUS_H */
