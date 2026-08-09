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
    void updatePreedit();
    void commitString(const QString &str);

    UnimDbusClient *m_dbus;
    QObject *m_focusObject;
    QString m_windowId;
    bool m_composing;
    bool m_popupActive;  /* Standalone popup 활성 여부 (nav 키 우회 차단용) */
    QRect m_cursorRect;
    /* 현재 입력 필드 목적 (focus 시 갱신). 1=Password, 2=Pin 이면 dev 로그의
     * 내용 필드(key·commit·preedit)를 "***"로 마스킹해 평문 잔류 방지. */
    quint32 m_contentPurpose = 0;
};

#endif // UNIM_INPUT_CONTEXT_HPP
