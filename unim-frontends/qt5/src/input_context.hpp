/**
 * UNIM Qt5 Input Context
 *
 * Qt5 애플리케이션에서 한글 입력을 제공하는 Input Context 구현입니다.
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#ifndef UNIM_INPUT_CONTEXT_HPP
#define UNIM_INPUT_CONTEXT_HPP

#include <qpa/qplatforminputcontext.h>
#include <QObject>
#include <QString>
#include <QRect>

/* 전방 선언 */
class UnimDbusClient;
class UnimHanjaPopup;
class UnimSpecialPopup;

class UnimInputContext : public QPlatformInputContext
{
    Q_OBJECT

public:
    UnimInputContext();
    ~UnimInputContext() override;

    // QPlatformInputContext 인터페이스
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
    void ensurePopups();
    bool isStandalonePopup() const;
    void updatePreedit();
    void commitString(const QString &str);

    UnimDbusClient *m_dbus;
    UnimHanjaPopup *m_hanjaPopup;
    UnimSpecialPopup *m_specialPopup;
    QObject *m_focusObject;
    QString m_windowId;
    bool m_composing;
    QRect m_cursorRect;
};

#endif // UNIM_INPUT_CONTEXT_HPP
