/**
 * UNIM GTK4 Settings - Main Entry Point
 * 
 * GTK4 기반 입력기 설정 애플리케이션
 */

#include "settings_dialog.h"
#include <locale.h>
#include <libintl.h>

int main(int argc, char *argv[]) {
    /* 로케일 설정 */
    setlocale(LC_ALL, "");
    bindtextdomain("unim-gtk-settings", "/usr/share/locale");
    textdomain("unim-gtk-settings");
    
    /* GtkApplication 생성 */
    GtkApplication *app = gtk_application_new(
        "io.github.from104.unim.settings",
        G_APPLICATION_DEFAULT_FLAGS
    );
    
    g_signal_connect(app, "activate", G_CALLBACK((void (*)(void))unim_settings_dialog_new), NULL);
    
    /* 애플리케이션 실행 */
    int status = g_application_run(G_APPLICATION(app), argc, argv);
    
    g_object_unref(app);
    return status;
}
