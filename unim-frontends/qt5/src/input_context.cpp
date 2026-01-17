/**
 * UNIM Qt5 Input Context 구현
 */

#include <type_traits>
#include <QInputMethodEvent>
#include <QFlags>
#include "input_context.hpp"

#include <QCoreApplication>
#include <QEvent>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QTextCharFormat>
#include <QTextFormat>

Q_DECLARE_METATYPE(QTextCharFormat)


UnimInputContext::UnimInputContext()
    : m_engine(nullptr)
    , m_config(nullptr)
    , m_focusObject(nullptr)
    , m_composing(false)
{
    m_config = unim_config_load();
    m_engine = unim_engine_new(m_config);
}

UnimInputContext::~UnimInputContext()
{
    if (m_engine) {
        unim_engine_delete(m_engine);
        m_engine = nullptr;
    }
    if (m_config) {
        unim_config_delete(m_config);
        m_config = nullptr;
    }
}

bool UnimInputContext::isValid() const
{
    return m_engine != nullptr && m_config != nullptr;
}

void UnimInputContext::reset()
{
    if (m_engine) {
        unim_engine_reset(m_engine);
        m_composing = false;
        updatePreedit();
    }
}

void UnimInputContext::commit()
{
    if (m_engine && m_composing) {
        unim_engine_clear_preedit(m_engine);
        UnimStr commitStr = unim_engine_commit_str(m_engine);
        if (commitStr.len > 0) {
            QString str = QString::fromUtf8(
                reinterpret_cast<const char *>(commitStr.ptr),
                static_cast<int>(commitStr.len)
            );
            commitString(str);
        }
        unim_engine_clear_commit(m_engine);
        m_composing = false;
        updatePreedit();
    }
}

void UnimInputContext::update(Qt::InputMethodQueries queries)
{
    Q_UNUSED(queries);
}

void UnimInputContext::invokeAction(QInputMethod::Action action, int cursorPosition)
{
    Q_UNUSED(action);
    Q_UNUSED(cursorPosition);
}

bool UnimInputContext::filterEvent(const QEvent *event)
{
    if (!m_engine || !m_config || !m_focusObject) {
        return false;
    }

    if (event->type() != QEvent::KeyPress) {
        return false;
    }

    const QKeyEvent *keyEvent = dynamic_cast<const QKeyEvent *>(event);
    if (!keyEvent) {
        return false;
    }

    // 수정자 상태 변환
    Qt::KeyboardModifiers modifiers = keyEvent->modifiers();
    int modInt = *reinterpret_cast<int*>(&modifiers);

    UnimModifierState state = {
        .shift = bool(modInt & Qt::ShiftModifier),
        .control = bool(modInt & Qt::ControlModifier),
        .alt = bool(modInt & Qt::AltModifier),
        .super_key = bool(modInt & Qt::MetaModifier),
        .caps_lock = false,
        .num_lock = false
    };

    // 키 입력 처리
    UnimInputResult result = unim_engine_press_key(
        m_engine,
        m_config,
        static_cast<uint16_t>(keyEvent->nativeScanCode()),
        state
    );

    if (result.consumed) {
        // 커밋 처리
        if (result.commit_changed) {
            UnimStr commitStr = unim_engine_commit_str(m_engine);
            if (commitStr.len > 0) {
                QString str = QString::fromUtf8(
                    reinterpret_cast<const char *>(commitStr.ptr),
                    static_cast<int>(commitStr.len)
                );
                commitString(str);
            }
            unim_engine_clear_commit(m_engine);
        }

        // preedit 변경
        if (result.preedit_changed) {
            m_composing = unim_engine_is_composing(m_engine);
            updatePreedit();
        }

        return true;
    }

    return false;
}

QRectF UnimInputContext::keyboardRect() const
{
    return QRectF();
}

bool UnimInputContext::isAnimating() const
{
    return false;
}

void UnimInputContext::showInputPanel()
{
}

void UnimInputContext::hideInputPanel()
{
}

bool UnimInputContext::isInputPanelVisible() const
{
    return false;
}

QLocale UnimInputContext::locale() const
{
    return QLocale::Korean;
}

Qt::LayoutDirection UnimInputContext::inputDirection() const
{
    return Qt::LeftToRight;
}

void UnimInputContext::setFocusObject(QObject *object)
{
    if (m_focusObject && m_composing) {
        // 이전 객체에서 포커스 아웃 시 커밋
        commit();
    }
    m_focusObject = object;
}

void UnimInputContext::updatePreedit()
{
    if (!m_focusObject) {
        return;
    }

    QString preeditStr;
    if (m_engine) {
        UnimStr preedit = unim_engine_preedit_str(m_engine);
        if (preedit.len > 0) {
            preeditStr = QString::fromUtf8(
                reinterpret_cast<const char *>(preedit.ptr),
                static_cast<int>(preedit.len)
            );
        }
    }

    QList<QInputMethodEvent::Attribute> attrs;
    if (!preeditStr.isEmpty()) {
        QTextCharFormat charFormat;
        charFormat.setUnderlineStyle(QTextCharFormat::SingleUnderline);
        attrs << QInputMethodEvent::Attribute(
            QInputMethodEvent::TextFormat,
            0,
            preeditStr.length(),
            QVariant::fromValue(charFormat)
        );
    }

    QInputMethodEvent imEvent(preeditStr, attrs);
    QCoreApplication::sendEvent(m_focusObject, (QEvent*)&imEvent);
}

void UnimInputContext::commitString(const QString &str)
{
    if (!m_focusObject || str.isEmpty()) {
        return;
    }

    QInputMethodEvent imEvent;
    imEvent.setCommitString(str);
    QCoreApplication::sendEvent(m_focusObject, (QEvent*)&imEvent);
}
