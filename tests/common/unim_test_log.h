/**
 * UNIM 테스트 앱 — 구조화 로거
 *
 * 한 줄 = 한 사건. JSON Lines 로 파일에, 사람이 읽는 형식으로 stdout 에,
 * 그리고 앱의 로그 패널로 동시에 나간다.
 *
 * 원칙: **침묵은 버그다.** 관측할 수 있는 것은 전부 남긴다. 로그가 없어서
 * 못 잡는 버그가 로그가 많아서 생기는 불편보다 훨씬 비싸다.
 *
 * 의존: C 표준 라이브러리만. glib/Qt 없이 어디서나 링크된다.
 *
 * 환경변수:
 *   UNIM_TEST_LOG          JSONL 파일 경로 (미지정 시 파일 출력 없음)
 *   UNIM_TEST_LOG_FORMAT   json | human | both   (기본 both)
 *   UNIM_TEST_LOG_LEVEL    all | no-key          (기본 all)
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §4
 */

#ifndef UNIM_TEST_LOG_H
#define UNIM_TEST_LOG_H

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ─── 수명 ────────────────────────────────────────────────────────────── */

/**
 * 로거를 켠다. main() 첫 줄에서 호출할 것 — 그래야 시작 실패도 기록된다.
 * stdout 을 줄 버퍼링으로 바꾸므로 `stdbuf -oL` 로 감쌀 필요가 없다.
 *
 * @param app_name  "gtk3" "gtk4" "qt5" "qt6" "xim" "gnome" — 로그의 `app` 키
 */
void unim_log_init(const char *app_name, int argc, char **argv);

/** UI 가 다 뜬 뒤 호출 — 하네스가 이 사건을 보고 키 주입을 시작한다. */
void unim_log_ready(void);

/** `app.exit` 을 남기고 파일을 닫는다. atexit 에 걸어도 된다. */
void unim_log_shutdown(void);

/** 지금까지 기록한 사건 수 (seq 의 마지막 값). */
long unim_log_seq(void);

/* ─── 앱 로그 패널 연결 ───────────────────────────────────────────────── */

/**
 * 사건이 생길 때마다 사람이 읽는 한 줄을 앱으로 넘긴다. 앱은 이걸 로그
 * 패널에 붙이면 된다 — 패널 내용이 stdout 과 자동으로 같아진다.
 */
typedef void (*UnimLogSink)(const char *line, void *user_data);
void unim_log_set_sink(UnimLogSink sink, void *user_data);

/* ─── 사건 ────────────────────────────────────────────────────────────── */

/** 환경변수 일습 + 툴킷 버전. `unim_log_init` 직후 한 번. */
void unim_log_env(const char *toolkit_version);

/**
 * 키 사건.
 * @param phase     "press" | "release"
 * @param keyval    툴킷 keyval (GDK/Qt) — 모르면 0
 * @param keysym    X11 keysym — 모르면 0
 * @param hw        하드웨어 keycode (X11 keycode = evdev + 8)
 * @param state     modifier 비트마스크
 * @param str       이 키가 만들어낸 문자열 (없으면 NULL)
 * @param filtered  IM 이 삼켰으면 1, 앱으로 내려왔으면 0, 모르면 -1
 */
void unim_log_key(const char *phase, unsigned keyval, unsigned keysym,
                  unsigned hw, unsigned state, const char *str, int filtered);

/**
 * IM 필터 진입·이탈. elapsed 로 데몬 왕복 시간을 잰다.
 * @param phase  "enter" | "leave"
 */
void unim_log_im(const char *phase, const char *field,
                 const char *result, double elapsed_ms);

/**
 * 조합 사건.
 * @param phase   "start" | "changed" | "end"
 * @param text    조합 문자열 (end 면 NULL 또는 "")
 * @param cursor  preedit 내 커서 바이트 오프셋 (모르면 -1)
 * @param attrs   attribute 요약 ("underline" 등, 없으면 NULL)
 */
void unim_log_preedit(const char *phase, const char *field,
                      const char *text, int cursor, const char *attrs);

/** 확정. */
void unim_log_commit(const char *field, const char *text);

/**
 * **화면의 진실.** 필드를 다시 그릴 때마다 호출한다. 하네스는 이 사건의
 * `rendered` 로 합격/불합격을 정한다 — 다른 어떤 사건보다 우선한다.
 *
 * @param committed  확정 텍스트
 * @param preedit    조합 중 문자열
 * @param caret      확정 텍스트 내 캐럿 바이트 오프셋
 * @param rendered   화면에 실제로 나타나는 최종 문자열 (확정+preedit 삽입)
 */
void unim_log_field_render(const char *field, const char *committed,
                           const char *preedit, int caret,
                           const char *rendered);

/** 포커스. @param phase "in" | "out" */
void unim_log_focus(const char *phase, const char *field, const char *prev);

/** 마우스 클릭 → 캐럿 이동. */
void unim_log_click(int x, int y, const char *field,
                    int caret_before, int caret_after);

/** IC 리셋. @param reason "focus-out" | "click" | "mode-toggle" | … */
void unim_log_reset(const char *field, const char *reason);

/**
 * DBus.
 * @param kind  "connect" | "call" | "signal" | "error"
 */
void unim_log_dbus(const char *kind, const char *iface, const char *member,
                   const char *detail, double elapsed_ms);

/** 주변 문맥. @param kind "retrieve" | "delete" */
void unim_log_surrounding(const char *kind, const char *text, int cursor,
                          int offset, int n_chars);

/** 자유 진단. */
void unim_log_note(const char *fmt, ...);
void unim_log_warn(const char *fmt, ...);
void unim_log_error(const char *fmt, ...);

/**
 * 위 셋의 비-가변인자 판. 가변인자 함수는 FFI 로 부르기 까다로워서
 * Rust 앱(`tests/common-rs`)은 이쪽을 쓴다. 포맷은 호출자가 미리 한다.
 */
void unim_log_note_str(const char *msg);
void unim_log_warn_str(const char *msg);
void unim_log_error_str(const char *msg);

/**
 * 위 함수로 표현 못 하는 사건. `json_kv` 는 JSON 오브젝트 본문 조각이며
 * **이스케이프는 호출자 책임**이다. 문자열 값은 `unim_log_json_escape` 로.
 * 예: unim_log_raw("popup.open", "\"kind\":\"hanja\",\"n\":12");
 */
void unim_log_raw(const char *ev, const char *json_kv);

/* ─── 유틸 ────────────────────────────────────────────────────────────── */

/** JSON 문자열 이스케이프. NUL 종료 보장. 반환값 = out. */
char *unim_log_json_escape(const char *in, char *out, size_t out_size);

/** UTF-8 코드포인트 수. */
size_t unim_log_utf8_len(const char *s);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_LOG_H */
