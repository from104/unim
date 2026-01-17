/**
 * UNIM Qt6 Settings - Main Entry Point
 * 
 * Qt6 기반 입력기 설정 애플리케이션
 */

#include "SettingsDialog.h"

#include <QApplication>
#include <QLocale>
#include <QTranslator>

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    
    /* 번역 로드 */
    QTranslator translator;
    const QStringList uiLanguages = QLocale::system().uiLanguages();
    for (const QString &locale : uiLanguages) {
        const QString baseName = "unim-qt-settings_" + QLocale(locale).name();
        if (translator.load(":/i18n/" + baseName)) {
            app.installTranslator(&translator);
            break;
        }
    }
    
    app.setApplicationName("UNIM Settings");
    app.setApplicationVersion("0.1.0");
    app.setOrganizationName("from104");
    app.setOrganizationDomain("github.io");
    
    SettingsDialog dialog;
    dialog.show();
    
    return app.exec();
}
