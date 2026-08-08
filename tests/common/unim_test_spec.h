/**
 * UNIM 테스트 앱 — 화면 스펙 단일 정의
 *
 * GTK3 · GTK4 · Qt5 · Qt6 · XIM · GNOME 이 모두 이 헤더를 읽어 같은 화면을
 * 만든다. 여기를 고치면 6개 앱이 함께 바뀐다. 앱 쪽에 수치를 적지 말 것.
 *
 * Rust 앱(unim-test-wayland)은 tests/common-rs/src/spec.rs 가 이 파일의
 * 미러다. 둘의 동기는 `make check-test-spec` 이 검사한다.
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §3
 */

#ifndef UNIM_TEST_SPEC_H
#define UNIM_TEST_SPEC_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ─── 창 ──────────────────────────────────────────────────────────────── */

#define UNIM_SPEC_WIN_TITLE_FMT   "UNIM %s 테스트"   /* %s = 프런트엔드 이름 */
#define UNIM_SPEC_WIN_WIDTH       760
#define UNIM_SPEC_WIN_HEIGHT      960
#define UNIM_SPEC_MARGIN          20
#define UNIM_SPEC_SECTION_GAP     18
#define UNIM_SPEC_ROW_GAP         10

/* ─── 타이포 ──────────────────────────────────────────────────────────── */

#define UNIM_SPEC_FONT_UI         "Noto Sans CJK KR"
#define UNIM_SPEC_FONT_MONO       "Noto Sans Mono CJK KR"
#define UNIM_SPEC_FONT_SIZE_UI    13
#define UNIM_SPEC_FONT_SIZE_FIELD 15
#define UNIM_SPEC_FONT_SIZE_LOG   11
#define UNIM_SPEC_FONT_SIZE_TITLE 14

/* ─── 색 (0xRRGGBB) ───────────────────────────────────────────────────── */

#define UNIM_SPEC_COL_BG          0x1e1e26
#define UNIM_SPEC_COL_PANEL       0x282833
#define UNIM_SPEC_COL_FIELD_BG    0x32323f
#define UNIM_SPEC_COL_FIELD_FOCUS 0x3c3c50
#define UNIM_SPEC_COL_BORDER      0x4a4a5c
#define UNIM_SPEC_COL_BORDER_FOCUS 0x7aa2f7
#define UNIM_SPEC_COL_TEXT        0xe4e4ec
#define UNIM_SPEC_COL_LABEL       0xa0a0b4
#define UNIM_SPEC_COL_PREEDIT     0x9ece6a  /* 조합 중 글자 */
#define UNIM_SPEC_COL_PREEDIT_UL  0x9ece6a  /* 조합 밑줄 */
#define UNIM_SPEC_COL_CARET       0x7aa2f7
#define UNIM_SPEC_COL_OK          0x9ece6a
#define UNIM_SPEC_COL_WARN        0xe0af68
#define UNIM_SPEC_COL_ERR         0xf7768e

/* ─── 치수 ────────────────────────────────────────────────────────────── */

#define UNIM_SPEC_LABEL_COL_W     110   /* 필드 라벨 열 너비 */
/* 상태 패널 라벨 열 너비. "최근 commit" 이 가장 길어서 이보다 좁으면
 * 값과 겹친다 — 앱마다 다른 수를 쓰면 정렬이 어긋난다. */
#define UNIM_SPEC_STATUS_LABEL_W  125
#define UNIM_SPEC_FIELD_H         38    /* 한 줄 필드 높이 */
#define UNIM_SPEC_FIELD_H_MULTI   84    /* 여러 줄 필드 높이 */
#define UNIM_SPEC_FIELD_PAD_X     10    /* 필드 안쪽 좌우 여백 */
#define UNIM_SPEC_LOG_H           220   /* 로그 패널 높이 */
#define UNIM_SPEC_LOG_LINES       500   /* 로그 패널 보관 줄 수 */

/* ─── 입력 힌트 ───────────────────────────────────────────────────────── */

typedef enum {
    UNIM_HINT_NONE = 0,
    UNIM_HINT_NUMBER,     /* 숫자 입력 */
    UNIM_HINT_PASSWORD,   /* 비밀번호 — AutoTypeFix·팝업 억제 대상 */
    UNIM_HINT_SEARCH,     /* 검색 */
    UNIM_HINT_MULTILINE,  /* 여러 줄 */
} UnimFieldHint;

/* ─── 코어 필드 정의 ──────────────────────────────────────────────────── */

/**
 * 하네스 시나리오가 `id` 로 필드를 지목한다. **id 는 계약이다** — 바꾸면
 * tests/harness/scenarios 아래 JSON 을 함께 고쳐야 한다.
 */
typedef struct {
    const char   *id;      /* "core.plain" — 시나리오·로그의 키 */
    const char   *label;   /* 화면 라벨 */
    UnimFieldHint hint;
    const char   *purpose; /* 이 필드가 무엇을 검증하는가 (툴팁·문서용) */
} UnimSpecField;

#define UNIM_SPEC_N_CORE_FIELDS 6

static const UnimSpecField UNIM_SPEC_CORE_FIELDS[UNIM_SPEC_N_CORE_FIELDS] = {
    { "core.plain",     "일반",      UNIM_HINT_NONE,
      "기본 조합·확정" },
    { "core.plain2",    "일반 2",    UNIM_HINT_NONE,
      "포커스 전환·필드 간 클릭 커밋" },
    { "core.numeric",   "숫자",      UNIM_HINT_NUMBER,
      "숫자 힌트에서의 IM 동작" },
    { "core.password",  "비밀번호",  UNIM_HINT_PASSWORD,
      "AutoTypeFix·한자 팝업 억제" },
    { "core.search",    "검색",      UNIM_HINT_SEARCH,
      "검색 힌트 경로" },
    { "core.multiline", "여러 줄",   UNIM_HINT_MULTILINE,
      "줄바꿈·멀티라인 캐럿" },
};

/* ─── 네이티브 위젯 섹션 ──────────────────────────────────────────────── */

/**
 * 툴킷 기본 위젯. 관측은 얕지만(내부 IM 컨텍스트가 숨겨져 있다) 실제 앱과
 * 같은 경로라 회귀 감시 가치가 있다. 코어 필드 아래에 병행 배치한다.
 */
typedef enum {
    UNIM_NATIVE_ENTRY = 0,
    UNIM_NATIVE_PASSWORD,
    UNIM_NATIVE_MULTILINE,
} UnimSpecNativeKind;

typedef struct {
    const char        *id;     /* "native.entry" */
    const char        *label;
    UnimSpecNativeKind kind;
} UnimSpecNative;

#define UNIM_SPEC_N_NATIVE 3

static const UnimSpecNative UNIM_SPEC_NATIVE[UNIM_SPEC_N_NATIVE] = {
    { "native.entry",     "Entry",     UNIM_NATIVE_ENTRY     },
    { "native.password",  "Password",  UNIM_NATIVE_PASSWORD  },
    { "native.multiline", "Multiline", UNIM_NATIVE_MULTILINE },
};

/* ─── 상태 패널 행 ────────────────────────────────────────────────────── */

/** 상태 패널은 아래 순서를 지킨다. 앱마다 순서가 달라지면 대조가 어렵다. */
typedef enum {
    UNIM_STATUS_DBUS = 0,     /* DBus 연결 */
    UNIM_STATUS_FRONTEND,     /* 프런트엔드 이름 + 결정된 IM 경로 */
    UNIM_STATUS_MODE,         /* 한/영 + 레이아웃 */
    UNIM_STATUS_FOCUS,        /* 포커스 필드 id */
    UNIM_STATUS_PREEDIT,      /* 현재 preedit + 길이 + 커서 */
    UNIM_STATUS_COMMIT,       /* 최근 commit */
    UNIM_STATUS_N
} UnimSpecStatusRow;

static const char *const UNIM_SPEC_STATUS_LABELS[UNIM_STATUS_N] = {
    "DBus", "프런트엔드", "엔진 모드", "포커스", "preedit", "최근 commit",
};

/* ─── 다른 언어에서 읽기 (FFI) ────────────────────────────────────────── */

/**
 * Rust 앱(`unim-test-wayland`)이 이 스펙을 **복사하지 않고 그대로** 쓰기
 * 위한 창구다. 매크로와 static 배열은 FFI 로 안 보이므로 접근자를 둔다.
 * 미러를 만들면 언젠가 어긋나지만, 이 길은 어긋날 수가 없다.
 */
typedef struct {
    int win_width, win_height, margin, section_gap, row_gap;
    int label_col_w, status_label_w, field_h, field_h_multi, field_pad_x;
    int log_h, log_lines;
    int font_size_ui, font_size_field, font_size_log, font_size_title;
    unsigned col_bg, col_panel, col_field_bg, col_field_focus;
    unsigned col_border, col_border_focus, col_text, col_label;
    unsigned col_preedit, col_caret, col_ok, col_warn, col_err;
} UnimSpecMetrics;

const UnimSpecMetrics *unim_spec_metrics(void);
const UnimSpecField   *unim_spec_core_field(int i);
int                    unim_spec_n_core_fields(void);
const char            *unim_spec_status_label(int i);
int                    unim_spec_n_status(void);
const char            *unim_spec_font_ui(void);
const char            *unim_spec_font_mono(void);
const char            *unim_spec_win_title_fmt(void);

/* ─── HiDPI ───────────────────────────────────────────────────────────── */

/**
 * 스케일은 앱이 시작 시 한 번 정한다(GDK/Qt 는 툴킷이 알려주고, XIM 은
 * Xft.dpi 를 읽는다). 스펙 수치는 전부 96 dpi 기준 논리 픽셀이다.
 */
#define UNIM_SPEC_BASE_DPI 96.0

#ifdef __cplusplus
}
#endif

#endif /* UNIM_TEST_SPEC_H */
