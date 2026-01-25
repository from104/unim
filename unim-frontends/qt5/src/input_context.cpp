/**
 * UNIM Qt5 Input Context 구현
 *
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#include "input_context.hpp"
#include "unim_dbus_client.hpp"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QTextCharFormat>
#include <QDebug>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 시스템 */
static bool unim_debug_enabled = false;

#define UNIM_DEBUG(...) \
    do { \
        if (unim_debug_enabled) { \
            qDebug() << "[UNIM-QT5-IM]" << __VA_ARGS__; \
        } \
    } while (0)

static void unim_check_debug_env()
{
    static bool checked = false;
    if (!checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            qDebug() << "[UNIM-QT5-IM] 디버그 모드 활성화 (UNIM_DEVELOP=1)";
        }
        checked = true;
    }
}

UnimInputContext::UnimInputContext()
    : m_dbus(nullptr)
    , m_focusObject(nullptr)
    , m_composing(false)
{
    unim_check_debug_env();
    UNIM_DEBUG("UnimInputContext 생성 시작");
    
    m_dbus = new UnimDbusClient(QStringLiteral("qt5-unim"));
    
    if (m_dbus && m_dbus->isValid()) {
        UNIM_DEBUG("UnimInputContext 생성 완료 (DBus 연결됨)");
    } else {
        UNIM_DEBUG("UnimInputContext 생성 (DBus 연결 실패)");
    }
}

UnimInputContext::~UnimInputContext()
{
    if (m_dbus) {
        delete m_dbus;
        m_dbus = nullptr;
    }
}

bool UnimInputContext::isValid() const
{
    return m_dbus != nullptr && m_dbus->isValid();
}

void UnimInputContext::reset()
{
    if (m_dbus) {
        QString commit = m_dbus->reset();
        if (!commit.isEmpty()) {
            commitString(commit);
        }
        m_composing = false;
        updatePreedit();
    }
}

void UnimInputContext::commit()
{
    if (m_dbus && m_composing) {
        // reset()을 사용하여 조합 중인 문자를 커밋 (focusOut은 포커스 상실용)
        QString commit = m_dbus->reset();
        if (!commit.isEmpty()) {
            commitString(commit);
        }
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
    if (!m_dbus || !m_dbus->isValid() || !m_focusObject) {
        UNIM_DEBUG("filterEvent: DBus/포커스 없음, 키 무시");
        return false;
    }

    if (event->type() != QEvent::KeyPress) {
        return false;
    }

    const QKeyEvent *keyEvent = static_cast<const QKeyEvent *>(event);

    /* 수정자 키만 눌린 경우 바이패스 (preedit에 영향 없이 앱으로 전달) */
    int key = keyEvent->key();
    if (key == Qt::Key_Shift || key == Qt::Key_Control ||
        key == Qt::Key_Alt || key == Qt::Key_Meta ||
        key == Qt::Key_Super_L || key == Qt::Key_Super_R ||
        key == Qt::Key_AltGr) {
        return false;
    }

    /* 수정자 상태 변환 - DBus 호출용 비트필드 */
    quint32 mod_state = 0;
    if (keyEvent->modifiers() & Qt::ShiftModifier) mod_state |= (1 << 0);
    if (keyEvent->modifiers() & Qt::ControlModifier) mod_state |= (1 << 2);
    if (keyEvent->modifiers() & Qt::AltModifier) mod_state |= (1 << 3);
    if (keyEvent->modifiers() & Qt::MetaModifier) mod_state |= (1 << 26);

    /* X11에서 nativeScanCode() = X11 keycode = evdev + 8 */
    quint32 scanCode = keyEvent->nativeScanCode();
    quint32 evdev_code = (scanCode > 8) ? (scanCode - 8) : 0;
    
    UNIM_DEBUG("키 입력: key=" << keyEvent->key() << ", scanCode=" << scanCode 
               << ", evdev=" << evdev_code << ", state=" << mod_state);

    /* DBus를 통해 키 처리 */
    UnimDbusKeyResult result = m_dbus->processKey(keyEvent->key(), evdev_code, mod_state);
    
    UNIM_DEBUG("엔진 결과: consumed=" << result.consumed 
               << ", preedit=" << result.preedit 
               << ", commit=" << result.commit);

    if (result.consumed) {
        /* 커밋 처리 */
        if (!result.commit.isEmpty()) {
            commitString(result.commit);
        }

        /* preedit 업데이트 */
        m_composing = !result.preedit.isEmpty();
        updatePreedit();

        return true;
    } else {
        /* 엔진이 소비하지 않은 키: 커밋이 있으면 처리 (Enter, Tab 등) */
        if (!result.commit.isEmpty()) {
            commitString(result.commit);
            m_composing = false;
            updatePreedit();
        } else if (m_composing) {
            /* 조합 중이었다면 로컬 캐시의 preedit을 커밋 */
            UNIM_DEBUG("Bypassed non-text key while composing -> Committing current preedit");
            commit();
        }
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
    UNIM_DEBUG("setFocusObject: object=" << object);
    if (m_focusObject && m_composing && m_dbus) {
        UNIM_DEBUG("setFocusObject: 조합 중, 커밋 수행");
        QString commitStr = m_dbus->focusOut();
        if (!commitStr.isEmpty()) {
            commitString(commitStr);
        }
        m_composing = false;
        updatePreedit();
    }
    m_focusObject = object;
    
    if (m_dbus && object) {
        m_dbus->focusIn();
    }
}

void UnimInputContext::updatePreedit()
{
    if (!m_focusObject) {
        return;
    }

    QString preeditStr;
    if (m_dbus) {
        preeditStr = m_dbus->getPreedit();
    }

    QList<QInputMethodEvent::Attribute> attrs;
    if (!preeditStr.isEmpty()) {
        QTextCharFormat charFormat;
        charFormat.setUnderlineStyle(QTextCharFormat::SingleUnderline);
        attrs << QInputMethodEvent::Attribute(
            QInputMethodEvent::TextFormat,
            0,
            preeditStr.length(),
            charFormat
        );
    }

    QInputMethodEvent imEvent(preeditStr, attrs);
    QCoreApplication::sendEvent(m_focusObject, &imEvent);
}

void UnimInputContext::commitString(const QString &str)
{
    if (!m_focusObject || str.isEmpty()) {
        return;
    }

    QInputMethodEvent imEvent;
    imEvent.setCommitString(str);
    QCoreApplication::sendEvent(m_focusObject, &imEvent);
}
