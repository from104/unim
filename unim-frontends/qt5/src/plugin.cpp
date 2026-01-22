/**
 * UNIM Qt5 Platform Input Context Plugin
 *
 * Qt5 플랫폼 입력 컨텍스트 플러그인 엔트리 포인트입니다.
 */

#include "input_context.hpp"

#include <QtCore/QtPlugin>
#include <qpa/qplatforminputcontextplugin_p.h>
#include <QStringList>

#include <QDebug>

class UnimPlatformInputContextPlugin : public QPlatformInputContextPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QPlatformInputContextFactoryInterface_iid FILE "unim.json")

public:
    QPlatformInputContext *create(const QString &key, const QStringList &paramList) override
    {
        Q_UNUSED(paramList);

        qDebug() << "[UNIM-QT5-PLUGIN] create called with key=" << key;

        if (key.compare(QLatin1String("unim"), Qt::CaseInsensitive) == 0) {
            qDebug() << "[UNIM-QT5-PLUGIN] check passed, creating context";
            return new UnimInputContext();
        }
        return nullptr;
    }
};

#include "plugin.moc"
