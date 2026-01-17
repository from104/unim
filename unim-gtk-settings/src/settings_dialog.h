/**
 * UNIM GTK4 Settings Dialog
 * 
 * 입력기 설정을 위한 GTK4 기반 다이얼로그
 */

#ifndef UNIM_SETTINGS_DIALOG_H
#define UNIM_SETTINGS_DIALOG_H

#include <gtk/gtk.h>

G_BEGIN_DECLS

#define UNIM_TYPE_SETTINGS_DIALOG (unim_settings_dialog_get_type())
G_DECLARE_FINAL_TYPE(UnimSettingsDialog, unim_settings_dialog, UNIM, SETTINGS_DIALOG, GtkWindow)

/**
 * unim_settings_dialog_new:
 *
 * 새로운 설정 다이얼로그를 생성합니다.
 *
 * Returns: 새로운 UnimSettingsDialog 인스턴스
 */
UnimSettingsDialog *unim_settings_dialog_new(void);

/**
 * unim_settings_dialog_present:
 * @dialog: 설정 다이얼로그
 *
 * 다이얼로그를 화면에 표시합니다.
 */
void unim_settings_dialog_present(UnimSettingsDialog *dialog);

G_END_DECLS

#endif /* UNIM_SETTINGS_DIALOG_H */
