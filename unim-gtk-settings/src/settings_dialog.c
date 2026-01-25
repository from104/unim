/**
 * UNIM GTK4 Settings Dialog - Implementation
 *
 * 입력기 설정을 위한 GTK4 기반 다이얼로그 구현
 * unim-capi를 사용하여 설정 관리
 */

#include "settings_dialog.h"
#include <stdio.h>
#include <libintl.h>

/* UNIM C API */
#include "unim.h"

#define _(String) gettext (String)

struct _UnimSettingsDialog {
    GtkWindow parent_instance;

    /* UI 위젯 */
    GtkWidget *korean_layout_combo;
    GtkWidget *english_layout_combo;
    GtkWidget *auto_switch_check;
    GtkWidget *threshold_scale;
    GtkWidget *threshold_label;
    GtkWidget *save_button;
    GtkWidget *cancel_button;

    /* UNIM 설정 객체 */
    UnimConfig *config;

    /* 설정 상태 (UI 상태) */
    int korean_layout;  /* 0: 두벌식, 1: 세벌식390, 2: 세벌식391 */
    int english_layout; /* 0: QWERTY, 1: Dvorak */
    gboolean auto_switch_enabled;
    double auto_switch_threshold;
};

G_DEFINE_TYPE(UnimSettingsDialog, unim_settings_dialog, GTK_TYPE_WINDOW)

/* C API를 통해 설정 로드 */
static void load_config(UnimSettingsDialog *self) {
    /* 기본값 설정 */
    self->korean_layout = 0;
    self->english_layout = 0;
    self->auto_switch_enabled = FALSE;
    self->auto_switch_threshold = 0.7;

    /* C API로 설정 로드 */
    self->config = unim_config_load();
    if (!self->config) {
        g_warning("Failed to load config, using defaults");
        self->config = unim_config_default();
        return;
    }

    /* C API로 값 읽기 */
    self->korean_layout = (int)unim_config_get_korean_layout(self->config);
    self->english_layout = (int)unim_config_get_english_layout(self->config);
    self->auto_switch_enabled = unim_config_get_auto_switch_enabled(self->config);
    self->auto_switch_threshold = (double)unim_config_get_auto_switch_threshold(self->config);
}

/* C API를 통해 설정 저장 */
static gboolean save_config(UnimSettingsDialog *self) {
    if (!self->config) {
        g_warning("No config object");
        return FALSE;
    }

    /* C API로 값 설정 */
    unim_config_set_korean_layout(self->config, (UnimKoreanLayout)self->korean_layout);
    unim_config_set_english_layout(self->config, (UnimEnglishLayout)self->english_layout);
    unim_config_set_auto_switch_enabled(self->config, self->auto_switch_enabled);
    unim_config_set_auto_switch_threshold(self->config, (float)self->auto_switch_threshold);

    /* C API로 저장 */
    return unim_config_save(self->config);
}

/* UI 이벤트 핸들러 */
static void on_korean_layout_changed(GtkComboBox *combo, UnimSettingsDialog *self) {
    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    self->korean_layout = gtk_combo_box_get_active(combo);
    G_GNUC_END_IGNORE_DEPRECATIONS
}

static void on_english_layout_changed(GtkComboBox *combo, UnimSettingsDialog *self) {
    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    self->english_layout = gtk_combo_box_get_active(combo);
    G_GNUC_END_IGNORE_DEPRECATIONS
}

static void on_auto_switch_toggled(GtkCheckButton *check, UnimSettingsDialog *self) {
    self->auto_switch_enabled = gtk_check_button_get_active(check);
    gtk_widget_set_sensitive(self->threshold_scale, self->auto_switch_enabled);
}

static void update_threshold_label(UnimSettingsDialog *self) {
    char label[32];
    snprintf(label, sizeof(label), "%.0f%%", self->auto_switch_threshold * 100);
    gtk_label_set_text(GTK_LABEL(self->threshold_label), label);
}

static void on_threshold_changed(GtkRange *range, UnimSettingsDialog *self) {
    self->auto_switch_threshold = gtk_range_get_value(range);
    update_threshold_label(self);
}

static void on_save_clicked(GtkButton *button G_GNUC_UNUSED, UnimSettingsDialog *self) {
    if (save_config(self)) {
        g_message("%s", _("Settings saved successfully."));
        gtk_window_close(GTK_WINDOW(self));
    } else {
        g_warning("Failed to save config");
    }
}

static void on_cancel_clicked(GtkButton *button G_GNUC_UNUSED, UnimSettingsDialog *self) {
    gtk_window_close(GTK_WINDOW(self));
}

/* 다이얼로그 파괴 시 정리 */
static void unim_settings_dialog_dispose(GObject *object) {
    UnimSettingsDialog *self = UNIM_SETTINGS_DIALOG(object);

    if (self->config) {
        unim_config_delete(self->config);
        self->config = NULL;
    }

    G_OBJECT_CLASS(unim_settings_dialog_parent_class)->dispose(object);
}

/* UI 구성 */
static void unim_settings_dialog_init(UnimSettingsDialog *self) {
    /* API 버전 확인 */
    if (unim_api_version() != UNIM_API_VERSION) {
        g_warning("UNIM API version mismatch: expected %d, got %zu",
                  UNIM_API_VERSION, unim_api_version());
    }

    /* 설정 로드 */
    load_config(self);

    /* 윈도우 설정 */
    gtk_window_set_title(GTK_WINDOW(self), _("UNIM Settings"));
    gtk_window_set_default_size(GTK_WINDOW(self), 400, 300);
    gtk_window_set_resizable(GTK_WINDOW(self), FALSE);

    /* 메인 박스 */
    GtkWidget *main_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 12);
    gtk_widget_set_margin_start(main_box, 20);
    gtk_widget_set_margin_end(main_box, 20);
    gtk_widget_set_margin_top(main_box, 20);
    gtk_widget_set_margin_bottom(main_box, 20);
    gtk_window_set_child(GTK_WINDOW(self), main_box);

    /* 헤더 */
    GtkWidget *header = gtk_label_new(NULL);
    char *header_markup = g_strdup_printf("<span size='large' weight='bold'>%s</span>", _("UNIM Settings"));
    gtk_label_set_markup(GTK_LABEL(header), header_markup);
    g_free(header_markup);
    gtk_box_append(GTK_BOX(main_box), header);

    /* 구분선 */
    gtk_box_append(GTK_BOX(main_box), gtk_separator_new(GTK_ORIENTATION_HORIZONTAL));

    /* 설정 그리드 */
    GtkWidget *grid = gtk_grid_new();
    gtk_grid_set_row_spacing(GTK_GRID(grid), 12);
    gtk_grid_set_column_spacing(GTK_GRID(grid), 12);
    gtk_box_append(GTK_BOX(main_box), grid);

    int row = 0;

    /* 한국어 레이아웃 */
    GtkWidget *korean_label = gtk_label_new(_("Korean Layout:"));
    gtk_widget_set_halign(korean_label, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), korean_label, 0, row, 1, 1);

    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    self->korean_layout_combo = gtk_combo_box_text_new();
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->korean_layout_combo), _("2-bul Standard"));
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->korean_layout_combo), _("3-bul 390"));
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->korean_layout_combo), _("3-bul Final"));
    gtk_combo_box_set_active(GTK_COMBO_BOX(self->korean_layout_combo), self->korean_layout);
    G_GNUC_END_IGNORE_DEPRECATIONS
    gtk_widget_set_hexpand(self->korean_layout_combo, TRUE);
    g_signal_connect(self->korean_layout_combo, "changed", G_CALLBACK(on_korean_layout_changed), self);
    gtk_grid_attach(GTK_GRID(grid), self->korean_layout_combo, 1, row, 1, 1);
    row++;

    /* 영문 레이아웃 */
    GtkWidget *english_label = gtk_label_new(_("English Layout:"));
    gtk_widget_set_halign(english_label, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), english_label, 0, row, 1, 1);

    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    self->english_layout_combo = gtk_combo_box_text_new();
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->english_layout_combo), "QWERTY");
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->english_layout_combo), "Dvorak");
    gtk_combo_box_set_active(GTK_COMBO_BOX(self->english_layout_combo), self->english_layout);
    G_GNUC_END_IGNORE_DEPRECATIONS
    gtk_widget_set_hexpand(self->english_layout_combo, TRUE);
    g_signal_connect(self->english_layout_combo, "changed", G_CALLBACK(on_english_layout_changed), self);
    gtk_grid_attach(GTK_GRID(grid), self->english_layout_combo, 1, row, 1, 1);
    row++;

    /* 자동 전환 */
    GtkWidget *auto_label = gtk_label_new(_("Auto Switch:"));
    gtk_widget_set_halign(auto_label, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), auto_label, 0, row, 1, 1);

    self->auto_switch_check = gtk_check_button_new_with_label(_("Enabled"));
    gtk_check_button_set_active(GTK_CHECK_BUTTON(self->auto_switch_check), self->auto_switch_enabled);
    g_signal_connect(self->auto_switch_check, "toggled", G_CALLBACK(on_auto_switch_toggled), self);
    gtk_grid_attach(GTK_GRID(grid), self->auto_switch_check, 1, row, 1, 1);
    row++;

    /* 임계값 */
    GtkWidget *threshold_label_title = gtk_label_new(_("Threshold:"));
    gtk_widget_set_halign(threshold_label_title, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), threshold_label_title, 0, row, 1, 1);

    GtkWidget *threshold_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    self->threshold_scale = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0.0, 1.0, 0.05);
    gtk_range_set_value(GTK_RANGE(self->threshold_scale), self->auto_switch_threshold);
    gtk_widget_set_hexpand(self->threshold_scale, TRUE);
    gtk_widget_set_sensitive(self->threshold_scale, self->auto_switch_enabled);
    g_signal_connect(self->threshold_scale, "value-changed", G_CALLBACK(on_threshold_changed), self);
    gtk_box_append(GTK_BOX(threshold_box), self->threshold_scale);

    self->threshold_label = gtk_label_new("");
    update_threshold_label(self);
    gtk_box_append(GTK_BOX(threshold_box), self->threshold_label);

    gtk_grid_attach(GTK_GRID(grid), threshold_box, 1, row, 1, 1);
    row++;

    /* 스페이서 */
    GtkWidget *spacer = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_vexpand(spacer, TRUE);
    gtk_box_append(GTK_BOX(main_box), spacer);

    /* 버튼 영역 */
    GtkWidget *button_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    gtk_widget_set_halign(button_box, GTK_ALIGN_END);
    gtk_box_append(GTK_BOX(main_box), button_box);

    self->cancel_button = gtk_button_new_with_label(_("Cancel"));
    g_signal_connect(self->cancel_button, "clicked", G_CALLBACK(on_cancel_clicked), self);
    gtk_box_append(GTK_BOX(button_box), self->cancel_button);

    self->save_button = gtk_button_new_with_label(_("Save"));
    gtk_widget_add_css_class(self->save_button, "suggested-action");
    g_signal_connect(self->save_button, "clicked", G_CALLBACK(on_save_clicked), self);
    gtk_box_append(GTK_BOX(button_box), self->save_button);
}

static void unim_settings_dialog_class_init(UnimSettingsDialogClass *klass) {
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    object_class->dispose = unim_settings_dialog_dispose;
}

UnimSettingsDialog *unim_settings_dialog_new(void) {
    return g_object_new(UNIM_TYPE_SETTINGS_DIALOG, NULL);
}

void unim_settings_dialog_present(UnimSettingsDialog *dialog) {
    gtk_window_present(GTK_WINDOW(dialog));
}
