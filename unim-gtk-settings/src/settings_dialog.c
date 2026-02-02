/**
 * UNIM GTK4 Settings Dialog - Qt6 디자인 완벽 일치 버전
 */

#include "settings_dialog.h"
#include <unim.h>
#include <glib/gi18n.h>
#include <gio/gio.h>
#include <stdio.h>

#define _(String) gettext (String)

struct _UnimSettingsDialog {
    GtkWindow parent_instance;

    /* UI 위젯 */
    GtkWidget *korean_layout_dropdown;
    GtkWidget *english_layout_dropdown;
    GtkWidget *auto_switch_switch;
    GtkWidget *threshold_scale;
    GtkWidget *threshold_label;

    /* 레이아웃 모델 */
    GListModel *korean_layout_model;
    GListModel *english_layout_model;

    /* UNIM 설정 객체 */
    UnimConfig *config;

    /* 현재 설정값 캐시 */
    int korean_layout;
    int english_layout;
    int initial_mode;  /* 0 = Korean, 1 = English */
    int mode_sharing;  /* 0 = Global, 1 = PerApp */
    gboolean auto_switch_enabled;
    double auto_switch_threshold;

    /* DBus 연결 */
    GDBusConnection *dbus_conn;
    guint signal_subscription_id;
    gboolean updating_from_signal;  /* 시그널 처리 중 플래그 */
};

G_DEFINE_TYPE(UnimSettingsDialog, unim_settings_dialog, GTK_TYPE_WINDOW)

/* 설정 저장 함수 */
static gboolean save_config(UnimSettingsDialog *self) {
    if (!self->config) return FALSE;

    unim_config_set_korean_layout(self->config, (UnimKoreanLayout)self->korean_layout);
    unim_config_set_english_layout(self->config, (UnimEnglishLayout)self->english_layout);
    unim_config_set_default_category(self->config, (UnimInputCategory)self->initial_mode);
    unim_config_set_mode_sharing(self->config, (UnimModeSharingMode)self->mode_sharing);
    unim_config_set_auto_switch_enabled(self->config, self->auto_switch_enabled);
    unim_config_set_auto_switch_threshold(self->config, (float)self->auto_switch_threshold);

    return unim_config_save(self->config);
}

/* UI 이벤트 핸들러 */
static void on_korean_layout_changed(GtkDropDown *dropdown, GParamSpec *pspec G_GNUC_UNUSED, UnimSettingsDialog *self) {
    self->korean_layout = (int)gtk_drop_down_get_selected(dropdown);
    save_config(self);
}

static void on_english_layout_changed(GtkDropDown *dropdown, GParamSpec *pspec G_GNUC_UNUSED, UnimSettingsDialog *self) {
    self->english_layout = (int)gtk_drop_down_get_selected(dropdown);
    save_config(self);
}

static void on_initial_mode_changed(GtkDropDown *dropdown, GParamSpec *pspec G_GNUC_UNUSED, UnimSettingsDialog *self) {
    self->initial_mode = (int)gtk_drop_down_get_selected(dropdown);
    save_config(self);
}

static void on_mode_sharing_changed(GtkDropDown *dropdown, GParamSpec *pspec G_GNUC_UNUSED, UnimSettingsDialog *self) {
    self->mode_sharing = (int)gtk_drop_down_get_selected(dropdown);
    save_config(self);
}

static void on_auto_switch_toggled(GtkSwitch *sw, GParamSpec *pspec G_GNUC_UNUSED, UnimSettingsDialog *self) {
    self->auto_switch_enabled = gtk_switch_get_active(sw);
    gtk_widget_set_sensitive(self->threshold_scale, self->auto_switch_enabled);
    gtk_widget_set_sensitive(self->threshold_label, self->auto_switch_enabled);
    save_config(self);
}

static void update_threshold_label(UnimSettingsDialog *self) {
    char label[32];
    snprintf(label, sizeof(label), "%.0f%%", self->auto_switch_threshold * 100);
    gtk_label_set_text(GTK_LABEL(self->threshold_label), label);
}

static void on_threshold_changed(GtkRange *range, UnimSettingsDialog *self) {
    self->auto_switch_threshold = gtk_range_get_value(range);
    update_threshold_label(self);
    save_config(self);
}

/* DBus를 통해 설정 변경 알림 */
static void notify_config_via_dbus(UnimSettingsDialog *self, const char *key, const char *value) {
    if (!self->dbus_conn) return;

    GError *error = NULL;
    GVariant *result = g_dbus_connection_call_sync(
        self->dbus_conn,
        "org.atit.unim",
        "/org/atit/unim/InputMethod",
        "org.atit.unim.InputMethod",
        "SetConfig",
        g_variant_new("(ss)", key, value),
        NULL,
        G_DBUS_CALL_FLAGS_NONE,
        1000,
        NULL,
        &error
    );

    if (error) {
        g_warning("DBus SetConfig failed: %s", error->message);
        g_error_free(error);
    } else if (result) {
        g_variant_unref(result);
    }
}

/* DBus ConfigChanged 시그널 핸들러 */
static void on_dbus_config_changed(GDBusConnection *connection G_GNUC_UNUSED,
                                    const gchar *sender_name G_GNUC_UNUSED,
                                    const gchar *object_path G_GNUC_UNUSED,
                                    const gchar *interface_name G_GNUC_UNUSED,
                                    const gchar *signal_name G_GNUC_UNUSED,
                                    GVariant *parameters,
                                    gpointer user_data) {
    UnimSettingsDialog *self = UNIM_SETTINGS_DIALOG(user_data);
    if (self->updating_from_signal) return;

    const gchar *key = NULL;
    const gchar *value = NULL;
    g_variant_get(parameters, "(&s&s)", &key, &value);

    self->updating_from_signal = TRUE;

    /* 설정값 업데이트 및 UI 반영 */
    if (g_strcmp0(key, "korean_layout") == 0) {
        int idx = 0;
        if (g_strcmp0(value, "Sebeolsik390") == 0) idx = 1;
        else if (g_strcmp0(value, "Sebeolsik391") == 0) idx = 2;
        else if (g_strcmp0(value, "SebeolsikNoShift") == 0) idx = 3;
        self->korean_layout = idx;
        gtk_drop_down_set_selected(GTK_DROP_DOWN(self->korean_layout_dropdown), idx);
    } else if (g_strcmp0(key, "english_layout") == 0) {
        int idx = 0;
        if (g_strcmp0(value, "Dvorak") == 0) idx = 1;
        else if (g_strcmp0(value, "Colemak") == 0) idx = 2;
        else if (g_strcmp0(value, "ColemakDh") == 0) idx = 3;
        else if (g_strcmp0(value, "Workman") == 0) idx = 4;
        self->english_layout = idx;
        gtk_drop_down_set_selected(GTK_DROP_DOWN(self->english_layout_dropdown), idx);
    }
    /* 필요시 다른 설정 키도 처리 가능 */

    self->updating_from_signal = FALSE;
}

/* DBus 연결 초기화 */
static void setup_dbus_connection(UnimSettingsDialog *self) {
    GError *error = NULL;
    self->dbus_conn = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (error) {
        g_warning("Failed to connect to session bus: %s", error->message);
        g_error_free(error);
        return;
    }

    self->signal_subscription_id = g_dbus_connection_signal_subscribe(
        self->dbus_conn,
        "org.atit.unim",
        "org.atit.unim.InputMethod",
        "ConfigChanged",
        "/org/atit/unim/InputMethod",
        NULL,
        G_DBUS_SIGNAL_FLAGS_NONE,
        on_dbus_config_changed,
        self,
        NULL
    );
}

/* CSS 인라인 로드 - Qt6와 동일한 스타일 */
static void load_css(void) {
    GtkCssProvider *provider = gtk_css_provider_new();
    
    const char *css = 
        "/* 섹션 타이틀 - 파란색 */\n"
        ".section-title {\n"
        "    color: #3584e4;\n"
        "    font-weight: bold;\n"
        "    font-size: 18px;\n"
        "    padding: 20px 4px 8px 4px;\n"
        "}\n"
        "\n"
        "/* 카드 스타일 - 둥근 배경 박스 */\n"
        ".settings-card {\n"
        "    background-color: #3a3a3a;\n"
        "    border-radius: 12px;\n"
        "    padding: 2px 0;\n"
        "}\n"
        "\n"
        "/* 설정 행 스타일 */\n"
        ".settings-row {\n"
        "    padding: 8px 16px;\n"
        "    min-height: 40px;\n"
        "}\n"
        "\n"
        "/* 행 구분선 */\n"
        ".row-separator {\n"
        "    background-color: #3d3d3d;\n"
        "    min-height: 1px;\n"
        "    margin: 0 16px;\n"
        "}\n"
        "\n"
        "/* 드롭다운 스타일 */\n"
        "dropdown button {\n"
        "    padding: 6px 12px;\n"
        "    background-color: #333;\n"
        "    border-radius: 6px;\n"
        "    min-width: 140px;\n"
        "}\n"
        "\n"
        "dropdown button:hover {\n"
        "    background-color: #3d3d3d;\n"
        "}\n";

    gtk_css_provider_load_from_string(provider, css);
    gtk_style_context_add_provider_for_display(
        gdk_display_get_default(),
        GTK_STYLE_PROVIDER(provider),
        GTK_STYLE_PROVIDER_PRIORITY_APPLICATION
    );
    g_object_unref(provider);
}

/* 설정 행 생성 - 라벨 + 컨트롤 */
static GtkWidget *create_settings_row(const char *label_text, GtkWidget *control) {
    GtkWidget *row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    gtk_widget_add_css_class(row, "settings-row");

    GtkWidget *label = gtk_label_new(label_text);
    gtk_widget_set_halign(label, GTK_ALIGN_START);
    gtk_widget_set_hexpand(label, TRUE);
    gtk_box_append(GTK_BOX(row), label);

    gtk_widget_set_halign(control, GTK_ALIGN_END);
    gtk_widget_set_valign(control, GTK_ALIGN_CENTER);
    gtk_box_append(GTK_BOX(row), control);

    return row;
}

/* 구분선 생성 */
static GtkWidget *create_separator(void) {
    GtkWidget *sep = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
    gtk_widget_add_css_class(sep, "row-separator");
    gtk_widget_set_size_request(sep, -1, 1);
    return sep;
}

static void unim_settings_dialog_init(UnimSettingsDialog *self) {
    load_css();

    /* 설정 로드 */
    self->config = unim_config_load();
    if (!self->config) {
        self->config = unim_config_default();
    }
    self->korean_layout = (int)unim_config_get_korean_layout(self->config);
    self->english_layout = (int)unim_config_get_english_layout(self->config);
    self->initial_mode = (int)unim_config_get_default_category(self->config);
    self->mode_sharing = (int)unim_config_get_mode_sharing(self->config);
    self->auto_switch_enabled = unim_config_get_auto_switch_enabled(self->config);
    self->auto_switch_threshold = (double)unim_config_get_auto_switch_threshold(self->config);

    /* DBus 연결 초기화 */
    self->updating_from_signal = FALSE;
    setup_dbus_connection(self);

    gtk_window_set_title(GTK_WINDOW(self), _("UNIM Settings"));
    gtk_window_set_default_size(GTK_WINDOW(self), 480, -1);
    gtk_window_set_resizable(GTK_WINDOW(self), FALSE);

    GtkWidget *main_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_margin_start(main_box, 24);
    gtk_widget_set_margin_end(main_box, 24);
    gtk_widget_set_margin_top(main_box, 8);
    gtk_widget_set_margin_bottom(main_box, 32);
    gtk_window_set_child(GTK_WINDOW(self), main_box);

    /* === Keyboard Layout Section === */
    GtkWidget *layout_title = gtk_label_new(_("Keyboard Layout"));
    gtk_widget_add_css_class(layout_title, "section-title");
    gtk_widget_set_halign(layout_title, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(main_box), layout_title);

    GtkWidget *layout_card = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_add_css_class(layout_card, "settings-card");

    /* Korean Layout Row - 동적 생성 */
    size_t korean_count = unim_korean_layout_count();
    GtkStringList *korean_list = gtk_string_list_new(NULL);
    for (size_t i = 0; i < korean_count; i++) {
        UnimStr name = unim_korean_layout_display_name(unim_korean_layout_at(i));
        char *name_str = g_strndup((const char *)name.ptr, name.len);
        gtk_string_list_append(korean_list, name_str);
        g_free(name_str);
    }
    self->korean_layout_model = G_LIST_MODEL(korean_list);
    self->korean_layout_dropdown = gtk_drop_down_new(self->korean_layout_model, NULL);
    gtk_drop_down_set_selected(GTK_DROP_DOWN(self->korean_layout_dropdown), (guint)self->korean_layout);
    g_signal_connect(self->korean_layout_dropdown, "notify::selected", G_CALLBACK(on_korean_layout_changed), self);
    gtk_box_append(GTK_BOX(layout_card), create_settings_row(_("Korean Layout"), self->korean_layout_dropdown));

    /* Separator */
    gtk_box_append(GTK_BOX(layout_card), create_separator());

    /* English Layout Row - 동적 생성 */
    size_t english_count = unim_english_layout_count();
    GtkStringList *english_list = gtk_string_list_new(NULL);
    for (size_t i = 0; i < english_count; i++) {
        UnimStr name = unim_english_layout_display_name(unim_english_layout_at(i));
        char *name_str = g_strndup((const char *)name.ptr, name.len);
        gtk_string_list_append(english_list, name_str);
        g_free(name_str);
    }
    self->english_layout_model = G_LIST_MODEL(english_list);
    self->english_layout_dropdown = gtk_drop_down_new(self->english_layout_model, NULL);
    gtk_drop_down_set_selected(GTK_DROP_DOWN(self->english_layout_dropdown), (guint)self->english_layout);
    g_signal_connect(self->english_layout_dropdown, "notify::selected", G_CALLBACK(on_english_layout_changed), self);
    gtk_box_append(GTK_BOX(layout_card), create_settings_row(_("English Layout"), self->english_layout_dropdown));

    /* Separator */
    gtk_box_append(GTK_BOX(layout_card), create_separator());

    /* Initial Mode Row */
    const char *initial_modes[] = { _("Korean"), _("English"), NULL };
    GtkStringList *initial_mode_list = gtk_string_list_new(initial_modes);
    GtkWidget *initial_mode_dropdown = gtk_drop_down_new(G_LIST_MODEL(initial_mode_list), NULL);
    gtk_drop_down_set_selected(GTK_DROP_DOWN(initial_mode_dropdown), (guint)self->initial_mode);
    g_signal_connect(initial_mode_dropdown, "notify::selected", G_CALLBACK(on_initial_mode_changed), self);
    gtk_box_append(GTK_BOX(layout_card), create_settings_row(_("Initial Mode"), initial_mode_dropdown));

    /* Separator */
    gtk_box_append(GTK_BOX(layout_card), create_separator());

    /* Mode Sharing Row - 동적 생성 */
    size_t mode_sharing_count = unim_mode_sharing_count();
    GtkStringList *mode_sharing_list = gtk_string_list_new(NULL);
    for (size_t i = 0; i < mode_sharing_count; i++) {
        UnimStr name = unim_mode_sharing_display_name(unim_mode_sharing_at(i));
        char *name_str = g_strndup((const char *)name.ptr, name.len);
        gtk_string_list_append(mode_sharing_list, name_str);
        g_free(name_str);
    }
    GtkWidget *mode_sharing_dropdown = gtk_drop_down_new(G_LIST_MODEL(mode_sharing_list), NULL);
    gtk_drop_down_set_selected(GTK_DROP_DOWN(mode_sharing_dropdown), (guint)self->mode_sharing);
    g_signal_connect(mode_sharing_dropdown, "notify::selected", G_CALLBACK(on_mode_sharing_changed), self);
    gtk_box_append(GTK_BOX(layout_card), create_settings_row(_("Mode Sharing"), mode_sharing_dropdown));

    gtk_box_append(GTK_BOX(main_box), layout_card);

    /* === Auto Switch Section === */
    GtkWidget *switch_title = gtk_label_new(_("Auto Switch"));
    gtk_widget_add_css_class(switch_title, "section-title");
    gtk_widget_set_halign(switch_title, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(main_box), switch_title);

    GtkWidget *switch_card = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_add_css_class(switch_card, "settings-card");

    /* Enable Auto Switch Row */
    self->auto_switch_switch = gtk_switch_new();
    gtk_switch_set_active(GTK_SWITCH(self->auto_switch_switch), self->auto_switch_enabled);
    g_signal_connect(self->auto_switch_switch, "notify::active", G_CALLBACK(on_auto_switch_toggled), self);
    gtk_box_append(GTK_BOX(switch_card), create_settings_row(_("Enable Auto Switch"), self->auto_switch_switch));

    /* Separator */
    gtk_box_append(GTK_BOX(switch_card), create_separator());

    /* Threshold Row */
    GtkWidget *threshold_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    
    self->threshold_scale = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 1, 0.05);
    gtk_scale_set_draw_value(GTK_SCALE(self->threshold_scale), FALSE);
    gtk_range_set_value(GTK_RANGE(self->threshold_scale), self->auto_switch_threshold);
    gtk_widget_set_size_request(self->threshold_scale, 140, -1);
    gtk_widget_set_sensitive(self->threshold_scale, self->auto_switch_enabled);
    g_signal_connect(self->threshold_scale, "value-changed", G_CALLBACK(on_threshold_changed), self);
    gtk_box_append(GTK_BOX(threshold_box), self->threshold_scale);

    self->threshold_label = gtk_label_new(NULL);
    gtk_widget_set_sensitive(self->threshold_label, self->auto_switch_enabled);
    update_threshold_label(self);
    gtk_box_append(GTK_BOX(threshold_box), self->threshold_label);

    gtk_box_append(GTK_BOX(switch_card), create_settings_row(_("Threshold"), threshold_box));

    gtk_box_append(GTK_BOX(main_box), switch_card);
}

static void unim_settings_dialog_dispose(GObject *object) {
    UnimSettingsDialog *self = UNIM_SETTINGS_DIALOG(object);
    if (self->config) {
        unim_config_delete(self->config);
        self->config = NULL;
    }
    G_OBJECT_CLASS(unim_settings_dialog_parent_class)->dispose(object);
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
