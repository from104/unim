/**
 * UNIM Qt6 Platform Input Context Plugin
 */

#include "input_context.hpp"

#include <QtCore/QtPlugin>
#include <qpa/qplatforminputcontextplugin_p.h>
#include <QStringList>

class UnimPlatformInputContextPlugin : public QPlatformInputContextPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QPlatformInputContextFactoryInterface" FILE "unim.json")

public:
    QPlatformInputContext *create(const QString &key, const QStringList &paramList) override
    {
        Q_UNUSED(paramList);

        if (key.compare(QLatin1String("unim"), Qt::CaseInsensitive) == 0) {
            return new UnimInputContext();
        }
        return nullptr;
    }
};

#include "plugin.moc"
