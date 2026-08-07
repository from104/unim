/**
 * UNIM 테스트 앱 — 화면 스펙 FFI 창구
 *
 * `unim_test_spec.h` 의 매크로·static 배열은 다른 언어에서 보이지 않는다.
 * Rust 앱이 스펙을 **복사하지 않고 그대로** 쓰도록 접근자를 노출한다.
 * 미러를 두면 언젠가 어긋나지만, 이 길은 어긋날 수가 없다.
 */

#include "unim_test_spec.h"

static const UnimSpecMetrics M = {
    .win_width        = UNIM_SPEC_WIN_WIDTH,
    .win_height       = UNIM_SPEC_WIN_HEIGHT,
    .margin           = UNIM_SPEC_MARGIN,
    .section_gap      = UNIM_SPEC_SECTION_GAP,
    .row_gap          = UNIM_SPEC_ROW_GAP,
    .label_col_w      = UNIM_SPEC_LABEL_COL_W,
    .field_h          = UNIM_SPEC_FIELD_H,
    .field_h_multi    = UNIM_SPEC_FIELD_H_MULTI,
    .field_pad_x      = UNIM_SPEC_FIELD_PAD_X,
    .log_h            = UNIM_SPEC_LOG_H,
    .log_lines        = UNIM_SPEC_LOG_LINES,
    .font_size_ui     = UNIM_SPEC_FONT_SIZE_UI,
    .font_size_field  = UNIM_SPEC_FONT_SIZE_FIELD,
    .font_size_log    = UNIM_SPEC_FONT_SIZE_LOG,
    .font_size_title  = UNIM_SPEC_FONT_SIZE_TITLE,
    .col_bg           = UNIM_SPEC_COL_BG,
    .col_panel        = UNIM_SPEC_COL_PANEL,
    .col_field_bg     = UNIM_SPEC_COL_FIELD_BG,
    .col_field_focus  = UNIM_SPEC_COL_FIELD_FOCUS,
    .col_border       = UNIM_SPEC_COL_BORDER,
    .col_border_focus = UNIM_SPEC_COL_BORDER_FOCUS,
    .col_text         = UNIM_SPEC_COL_TEXT,
    .col_label        = UNIM_SPEC_COL_LABEL,
    .col_preedit      = UNIM_SPEC_COL_PREEDIT,
    .col_caret        = UNIM_SPEC_COL_CARET,
    .col_ok           = UNIM_SPEC_COL_OK,
    .col_warn         = UNIM_SPEC_COL_WARN,
    .col_err          = UNIM_SPEC_COL_ERR,
};

const UnimSpecMetrics *unim_spec_metrics(void) { return &M; }

const UnimSpecField *unim_spec_core_field(int i) {
    if (i < 0 || i >= UNIM_SPEC_N_CORE_FIELDS) return 0;
    return &UNIM_SPEC_CORE_FIELDS[i];
}

int unim_spec_n_core_fields(void) { return UNIM_SPEC_N_CORE_FIELDS; }

const char *unim_spec_status_label(int i) {
    if (i < 0 || i >= UNIM_STATUS_N) return 0;
    return UNIM_SPEC_STATUS_LABELS[i];
}

int unim_spec_n_status(void) { return UNIM_STATUS_N; }

const char *unim_spec_font_ui(void)       { return UNIM_SPEC_FONT_UI; }
const char *unim_spec_font_mono(void)     { return UNIM_SPEC_FONT_MONO; }
const char *unim_spec_win_title_fmt(void) { return UNIM_SPEC_WIN_TITLE_FMT; }
