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

    /* ─ 선택 (2026-10 추가 — 맨 끝에 붙여 기존 배치를 건드리지 않는다) ─
     * 선택 = [min(anchor,caret), max(anchor,caret)) (committed 기준 바이트).
     * anchor == caret 이면 선택 없음. 직접 읽지 말고 unim_field_anchor() 를 쓴다.
     * sel_caret 은 내부용 — anchor 를 마지막으로 확정했을 때의 caret 이다.
     * 앱이 `f->caret = …` 으로 직접 옮기면 둘이 어긋나고, 그때 anchor 는
     * 무효(= 선택 해제)로 본다. 그래서 caret 만 직접 대입하는 기존 앱 코드도
     * 유령 선택을 만들지 않는다. */
    int anchor;
    int sel_caret;
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

/* ─── 선택 ────────────────────────────────────────────────────────────────
 *
 * 규칙 (실제 GTK 위젯 관례):
 *  - Shift 없는 이동은 선택을 해제한다. 선택 중 Left/Right 는 선택의 앞/뒤 끝으로
 *    접고(그 방향으로 한 글자 더 가지 않는다), Home/End 는 그 끝으로 간다.
 *  - 선택 상태의 insert/commit 은 선택을 먼저 지우고 넣는다. backspace/delete 는
 *    선택만 지운다. clear 는 선택도 푼다.
 *  - preedit_start / set_preedit 은 선택을 건드리지 않는다 — GNOME 한자 변환은 선택
 *    텍스트를 delete-surrounding 으로 직접 지운다. preedit 은 caret 위치에 그려진다.
 *  - 키보드 계열(확장 이동·select_all)은 조합 중(composing)이면 기존 이동과 같이 무시한다.
 *    마우스 계열(set_anchor·extend_to·select_word_at)은 막지 않는다 — 앱이 먼저 IM 을 리셋한다.
 *  - 비밀번호 필드도 선택은 되지만 unim_field_primary_text() 가 NULL 을 준다.
 *  - 모든 오프셋은 UTF-8 글자 경계로 맞춘다 (입력이 경계 중간이면 앞쪽으로 내림).
 *  - 선택이 바뀌면 `field.select` 1줄, 그리고 평소처럼 `field.render`.
 */

/** Shift 확장 판 이동. extend=0 이면 unim_field_move_caret 과 같다(+선택 접기). */
void unim_field_move_caret_ex(UnimTestField *f, int dir, int extend);
void unim_field_caret_home_ex(UnimTestField *f, int extend);
void unim_field_caret_end_ex(UnimTestField *f, int extend);

/** Ctrl+A / 트리플클릭 — 전체 선택 (anchor=0, caret=끝). */
void unim_field_select_all(UnimTestField *f);

/**
 * 더블클릭 — off 가 놓인 단어 선택. 단어 = 공백·구두점이 아닌 글자의 연속
 * (ASCII 공백·구두점과 U+00A0~BF·U+2000~206F·U+3000~3003·전각 구두점을 경계로 본다. 한글·한자·영숫자·_ 는 단어 글자).
 * off 가 경계 글자 위면 같은 종류(경계 글자)의 연속을 고른다. 빈 필드면 무동작.
 */
void unim_field_select_word_at(UnimTestField *f, int off);

/** 마우스 누름 — caret 과 anchor 를 off 로 둔다(선택 해제). 드래그 시작점. */
void unim_field_set_anchor(UnimTestField *f, int off);
/** 마우스 드래그 / Shift+클릭 — anchor 는 두고 caret 만 off 로. */
void unim_field_extend_to(UnimTestField *f, int off);
/** 클릭으로 캐럿만 옮기고 선택을 푼다 (`f->caret = …` 직접 대입 대신 쓸 것). */
void unim_field_set_caret(UnimTestField *f, int off);

/** 선택 텍스트를 지운다. 지웠으면 1, 선택이 없었으면 0. 조합 가드 없음. */
int  unim_field_delete_selection(UnimTestField *f);

/**
 * IM 의 delete-surrounding (GTK/Qt 방식, 단위=글자). **캐럿 기준**이며 선택과 무관하다 —
 * 선택이 있어도 선택이 아니라 [caret+offset, +n_chars) 를 지운다. 끝나면 anchor=caret.
 * 기존 앱 코드(move_caret 반복 + delete 반복)와 같은 가드를 따른다
 * (조합 중이면 이동 안 함, 조합 글자가 있으면 삭제 안 함).
 * ⚠️ 선택이 있을 때 move_caret/delete 반복으로 대신하면 선택이 접히거나 선택이 지워진다.
 */
void unim_field_delete_surrounding(UnimTestField *f, int offset, int n_chars);
/** 같은 일, 단위=바이트 (text-input-v3). 글자 경계 쪽으로 올림해 지운다. */
void unim_field_delete_surrounding_bytes(UnimTestField *f,
                                         unsigned before, unsigned after);

/* ─── 선택 조회 ───────────────────────────────────────────────────────── */

/** 유효한 anchor (선택 없으면 caret 과 같다). surrounding 보고용. */
int unim_field_anchor(const UnimTestField *f);
int unim_field_has_selection(const UnimTestField *f);
/** 선택 [start,end) 를 committed 바이트로. 선택이 있으면 1, 없으면 0 (start=end=caret). */
int unim_field_selection_range(const UnimTestField *f, int *start, int *end);
/** 선택 텍스트 (없으면 ""). 비밀번호여도 실물 — 내보내기 여부는 primary_text 로. */
const char *unim_field_selected_text(const UnimTestField *f, char *out, size_t n);
/** PRIMARY 로 내보낼 텍스트. 선택이 없거나 비밀번호 필드면 NULL, 아니면 out. */
const char *unim_field_primary_text(const UnimTestField *f, char *out, size_t n);
/**
 * **그리기용.** 선택 구간을 `unim_field_display()` 가 준 문자열 안의 바이트 오프셋으로.
 * preedit 이 선택 앞(caret==선택 시작)에 끼면 그만큼 밀고, 비밀번호는 • 기준으로 환산한다.
 * 선택이 있으면 1, 없으면 0. 폭은 display 문자열의 [0,start)·[0,end) 접두를 재서 구한다.
 */
int unim_field_selection_display_range(const UnimTestField *f, int *start, int *end);

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

/** 클릭 x 좌표 → 확정 텍스트 내 바이트 오프셋.
 *  비밀번호 필드는 •로 마스킹한 접두의 폭으로 잰다(그려진 폭과 일치). */
int unim_field_caret_from_x(const UnimTestField *f, int x,
                            UnimTextWidthFn measure, void *user);

/** 지금 상태를 `field.render` 로 한 번 더 찍는다 (다시 그린 직후 등). */
void unim_field_log_render(const UnimTestField *f);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_FIELD_H */
