/**
 * UNIM 테스트 앱 — 코어 필드 엔진 (툴킷 무관)
 *
 * 코어 필드의 텍스트 상태·편집·레이아웃·화면 문자열 계산을 순수 C 로 둔다.
 * GTK3 · GTK4 · Qt5 · Qt6 · XIM 이 이 코드를 공유하므로 **앱마다 동작이
 * 어긋날 수 없다.** 툴킷 코드가 하는 일은 (a) IM 시그널을 이 API 로 옮기고
 * (b) `unim_field_rendered()` 가 준 문자열을 폰트로 그리는 것뿐이다.
 *
 * 모든 변경 함수는 끝에 `field.render` 로그를 발행한다 — 상태가 바뀌었는데
 * 로그가 없는 경우는 없다.
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §7
 */

#ifndef UNIM_TEST_FIELD_H
#define UNIM_TEST_FIELD_H

#include <stddef.h>
#include "unim_test_spec.h"

#ifdef __cplusplus
extern "C" {
#endif

#define UNIM_FIELD_TEXT_MAX    4096
#define UNIM_FIELD_PREEDIT_MAX 512

typedef struct {
    /* ─ 정체 ─ */
    const char   *id;        /* "core.plain" — UNIM_SPEC_CORE_FIELDS 에서 */
    const char   *label;
    UnimFieldHint hint;

    /* ─ 텍스트 상태 ─ */
    char committed[UNIM_FIELD_TEXT_MAX];  /* 확정된 텍스트 */
    int  caret;                           /* committed 내 바이트 오프셋 */
    char preedit[UNIM_FIELD_PREEDIT_MAX]; /* 조합 중 문자열 */
    int  preedit_caret;                   /* preedit 내 바이트 오프셋 */
    int  composing;                       /* preedit-start ~ end 사이인가 */

    /* ─ 기하 (논리 픽셀, 스케일 적용 후) ─ */
    int x, y, w, h;

    /* ─ 상태 ─ */
    int focused;
} UnimTestField;

/** 텍스트 폭 측정 — 툴킷이 제공한다. 바이트 길이 nbytes 만큼의 폭(px). */
typedef int (*UnimTextWidthFn)(const char *utf8, size_t nbytes, void *user);

/* ─── 수명·레이아웃 ───────────────────────────────────────────────────── */

/** 스펙 한 줄로 필드를 초기화한다. */
void unim_field_init(UnimTestField *f, const UnimSpecField *spec);

/**
 * 코어 필드 6개의 기하를 스펙대로 한꺼번에 계산한다.
 * 캔버스에 직접 그리는 앱(XIM, 그리고 GTK/Qt 의 DrawingArea)이 쓴다.
 *
 * @param fields  UNIM_SPEC_N_CORE_FIELDS 개 배열
 * @param top     첫 필드 상단 y
 * @param width   사용 가능한 전체 폭
 * @param scale   HiDPI 배율 (1.0 = 96dpi)
 * @return        마지막 필드 아래 y
 */
int unim_field_layout(UnimTestField *fields, int n,
                      int top, int width, double scale);

/** (x, y) 를 품는 필드의 인덱스. 없으면 -1. */
int unim_field_hit(const UnimTestField *fields, int n, int x, int y);

/* ─── IM 이 부르는 것 ─────────────────────────────────────────────────── */

/**
 * 확정 문자열을 캐럿 위치에 끼워 넣는다.
 *
 * ⚠️ **preedit 을 함께 지우지 않는다.** ON-THE-SPOT 에서 preedit 의 소유자는
 * IM 이다. 같은 키에서 commit 과 새 preedit 이 함께 오는 경우(예: ㄹ 연타)
 * 앱이 임의로 비우면 방금 그린 새 조합까지 사라진다 — 2026-08-07 회귀.
 * 조합 종료는 IM 이 보내는 preedit 갱신/종료로만 처리한다.
 */
void unim_field_commit(UnimTestField *f, const char *text);

/** 조합 시작 (`preedit-start`). */
void unim_field_preedit_start(UnimTestField *f);

/** 조합 갱신 (`preedit-changed`). cursor < 0 이면 문자열 끝으로 본다. */
void unim_field_set_preedit(UnimTestField *f, const char *text, int cursor);

/** 조합 종료 (`preedit-end`) — 내용을 비우고 composing 을 내린다. */
void unim_field_preedit_end(UnimTestField *f);

/* ─── 앱이 부르는 것 ──────────────────────────────────────────────────── */

void unim_field_backspace(UnimTestField *f);
void unim_field_delete(UnimTestField *f);
void unim_field_insert(UnimTestField *f, const char *text);  /* IM 미필터 문자 */
void unim_field_move_caret(UnimTestField *f, int dir);       /* -1 왼쪽, +1 오른쪽 */
void unim_field_caret_home(UnimTestField *f);
void unim_field_caret_end(UnimTestField *f);
void unim_field_clear(UnimTestField *f);
void unim_field_set_focus(UnimTestField *f, int focused, const char *prev_id);

/* ─── 조회 ────────────────────────────────────────────────────────────── */

/**
 * **화면에 실제로 나타나는 문자열.** 확정 텍스트의 캐럿 위치에 preedit 을
 * 끼워 넣은 결과. 하네스의 판정 기준이다.
 */
const char *unim_field_rendered(const UnimTestField *f, char *out, size_t n);

/** 화면 표시용 문자열 — 비밀번호 필드면 •로 마스킹한 것. */
const char *unim_field_display(const UnimTestField *f, char *out, size_t n);

/** 캐럿 앞부분(= 캐럿 x 좌표 계산용) 문자열. */
const char *unim_field_before_caret(const UnimTestField *f, char *out, size_t n);

/** 클릭 x 좌표 → 확정 텍스트 내 바이트 오프셋. */
int unim_field_caret_from_x(const UnimTestField *f, int x,
                            UnimTextWidthFn measure, void *user);

/** 지금 상태를 `field.render` 로 한 번 더 찍는다 (다시 그린 직후 등). */
void unim_field_log_render(const UnimTestField *f);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_FIELD_H */
