/**
 * UNIM Qt6 Input Context
 *
 * Qt6 애플리케이션에서 한글 입력을 제공하는 Input Context 구현입니다.
 * Qt5와 거의 동일하지만, Qt6 API 변경 사항을 반영합니다.
 */

#ifndef UNIM_INPUT_CONTEXT_HPP
#define UNIM_INPUT_CONTEXT_HPP

#include <qpa/qplatforminputcontext.h>
#include <QObject>
#include <QString>

extern "C" {
#include <unim.h>
}

class UnimInputContext : public QPlatformInputContext
{
    Q_OBJECT

public:
    UnimInputContext();
    ~UnimInputContext() override;

    bool isValid() const override;
    void reset() override;
    void commit() override;
    void update(Qt::InputMethodQueries queries) override;
    void invokeAction(QInputMethod::Action action, int cursorPosition) override;
    bool filterEvent(const QEvent *event) override;
    QRectF keyboardRect() const override;
    bool isAnimating() const override;
    void showInputPanel() override;
    void hideInputPanel() override;
    bool isInputPanelVisible() const override;
    QLocale locale() const override;
    Qt::LayoutDirection inputDirection() const override;
    void setFocusObject(QObject *object) override;

private:
    void updatePreedit();
    void commitString(const QString &str);

    UnimEngine *m_engine;
    UnimConfig *m_config;
    QObject *m_focusObject;
    bool m_composing;
};

#endif // UNIM_INPUT_CONTEXT_HPP
