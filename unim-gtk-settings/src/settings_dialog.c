/**
 * UNIM GTK4 Settings Dialog - Implementation
 * 
 * 입력기 설정을 위한 GTK4 기반 다이얼로그 구현
 */

#include "settings_dialog.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <libintl.h>

#define _(String) gettext (String)

struct _UnimSettingsDialog {
    GtkWindow parent_instance;
    
    /* UI 위젯 */
    GtkWidget *hangul_layout_combo;
    GtkWidget *latin_layout_combo;
    GtkWidget *auto_switch_check;
    GtkWidget *threshold_scale;
    GtkWidget *threshold_label;
    GtkWidget *save_button;
    GtkWidget *cancel_button;
    
    /* 설정 상태 */
    int hangul_layout;  /* 0: 두벌식, 1: 세벌식390, 2: 세벌식391 */
    int latin_layout;   /* 0: QWERTY, 1: Dvorak */
    gboolean auto_switch_enabled;
    double auto_switch_threshold;
};

G_DEFINE_TYPE(UnimSettingsDialog, unim_settings_dialog, GTK_TYPE_WINDOW)

/* 설정 파일 경로 */
static char *get_config_path(void) {
    const char *home = g_get_home_dir();
    return g_build_filename(home, ".config", "unim", "config.yaml", NULL);
}

/* 간단한 YAML 파싱 */
static void load_config(UnimSettingsDialog *self) {
    char *path = get_config_path();
    char *contents = NULL;
    gsize length;
    GError *error = NULL;
    
    /* 기본값 설정 */
    self->hangul_layout = 0;
    self->latin_layout = 0;
    self->auto_switch_enabled = FALSE;
    self->auto_switch_threshold = 0.7;
    
    if (!g_file_get_contents(path, &contents, &length, &error)) {
        g_clear_error(&error);
        g_free(path);
        return;
    }
    
    gchar **lines = g_strsplit(contents, "\n", -1);
    for (int i = 0; lines[i] != NULL; i++) {
        gchar *line = g_strstrip(lines[i]);
        
        if (g_str_has_prefix(line, "layout:")) {
            gchar *value = g_strstrip(line + 7);
            if (g_strcmp0(value, "Dubeolsik") == 0) self->hangul_layout = 0;
            else if (g_strcmp0(value, "Sebeolsik390") == 0) self->hangul_layout = 1;
            else if (g_strcmp0(value, "Sebeolsik391") == 0) self->hangul_layout = 2;
        }
        else if (g_str_has_prefix(line, "enabled:")) {
            gchar *value = g_strstrip(line + 8);
            self->auto_switch_enabled = (g_strcmp0(value, "true") == 0);
        }
        else if (g_str_has_prefix(line, "threshold:")) {
            gchar *value = g_strstrip(line + 10);
            self->auto_switch_threshold = g_ascii_strtod(value, NULL);
        }
    }
    
    g_strfreev(lines);
    g_free(contents);
    g_free(path);
}

/* 설정 저장 */
static gboolean save_config(UnimSettingsDialog *self) {
    char *path = get_config_path();
    char *dir = g_path_get_dirname(path);
    
    if (g_mkdir_with_parents(dir, 0755) != 0) {
        g_free(dir);
        g_free(path);
        return FALSE;
    }
    g_free(dir);
    
    const char *hangul_name;
    switch (self->hangul_layout) {
        case 1: hangul_name = "Sebeolsik390"; break;
        case 2: hangul_name = "Sebeolsik391"; break;
        default: hangul_name = "Dubeolsik"; break;
    }
    
    const char *latin_name;
    switch (self->latin_layout) {
        case 1: latin_name = "Dvorak"; break;
        default: latin_name = "Qwerty"; break;
    }
    
    char *contents = g_strdup_printf(
        "engine:\n"
        "  default_category: Hangul\n"
        "  hangul:\n"
        "    layout: %s\n"
        "    preedit_johab: false\n"
        "    word_commit: false\n"
        "  latin:\n"
        "    layout: %s\n"
        "    preferred_direct: true\n"
        "  auto_switch:\n"
        "    enabled: %s\n"
        "    threshold: %.2f\n"
        "    show_notification: false\n",
        hangul_name,
        latin_name,
        self->auto_switch_enabled ? "true" : "false",
        self->auto_switch_threshold
    );
    
    GError *error = NULL;
    gboolean result = g_file_set_contents(path, contents, -1, &error);
    
    if (!result) {
        g_clear_error(&error);
    }
    
    g_free(contents);
    g_free(path);
    return result;
}

/* UI 이벤트 핸들러 */
static void on_hangul_layout_changed(GtkComboBox *combo, UnimSettingsDialog *self) {
    self->hangul_layout = gtk_combo_box_get_active(combo);
}

static void on_latin_layout_changed(GtkComboBox *combo, UnimSettingsDialog *self) {
    self->latin_layout = gtk_combo_box_get_active(combo);
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
    }
}

static void on_cancel_clicked(GtkButton *button G_GNUC_UNUSED, UnimSettingsDialog *self) {
    gtk_window_close(GTK_WINDOW(self));
}

/* UI 구성 */
static void unim_settings_dialog_init(UnimSettingsDialog *self) {
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
    
    /* 한글 레이아웃 */
    GtkWidget *hangul_label = gtk_label_new(_("Korean Layout:"));
    gtk_widget_set_halign(hangul_label, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), hangul_label, 0, row, 1, 1);
    
    self->hangul_layout_combo = gtk_combo_box_text_new();
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->hangul_layout_combo), _("2-bul Standard"));
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->hangul_layout_combo), _("3-bul 390"));
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->hangul_layout_combo), _("3-bul Final"));
    gtk_combo_box_set_active(GTK_COMBO_BOX(self->hangul_layout_combo), self->hangul_layout);
    gtk_widget_set_hexpand(self->hangul_layout_combo, TRUE);
    g_signal_connect(self->hangul_layout_combo, "changed", G_CALLBACK(on_hangul_layout_changed), self);
    gtk_grid_attach(GTK_GRID(grid), self->hangul_layout_combo, 1, row, 1, 1);
    row++;
    
    /* 영문 레이아웃 */
    GtkWidget *latin_label = gtk_label_new(_("English Layout:"));
    gtk_widget_set_halign(latin_label, GTK_ALIGN_END);
    gtk_grid_attach(GTK_GRID(grid), latin_label, 0, row, 1, 1);
    
    self->latin_layout_combo = gtk_combo_box_text_new();
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->latin_layout_combo), "QWERTY");
    gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(self->latin_layout_combo), "Dvorak");
    gtk_combo_box_set_active(GTK_COMBO_BOX(self->latin_layout_combo), self->latin_layout);
    gtk_widget_set_hexpand(self->latin_layout_combo, TRUE);
    g_signal_connect(self->latin_layout_combo, "changed", G_CALLBACK(on_latin_layout_changed), self);
    gtk_grid_attach(GTK_GRID(grid), self->latin_layout_combo, 1, row, 1, 1);
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

static void unim_settings_dialog_class_init(UnimSettingsDialogClass *klass G_GNUC_UNUSED) {
}

UnimSettingsDialog *unim_settings_dialog_new(void) {
    return g_object_new(UNIM_TYPE_SETTINGS_DIALOG, NULL);
}

void unim_settings_dialog_present(UnimSettingsDialog *dialog) {
    gtk_window_present(GTK_WINDOW(dialog));
}
