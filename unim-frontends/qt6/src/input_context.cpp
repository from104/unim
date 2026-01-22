/**
 * UNIM Qt6 Input Context 구현
 */

#include "input_context.hpp"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QTextCharFormat>
#include <QDebug>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 시스템 (GTK4 모듈과 동일한 패턴) */
static bool unim_debug_enabled = false;

#define UNIM_DEBUG(...) \
    do { \
        if (unim_debug_enabled) { \
            qDebug() << "[UNIM-QT6-IM]" << __VA_ARGS__; \
        } \
    } while (0)

static void unim_check_debug_env()
{
    static bool checked = false;
    if (!checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            qDebug() << "[UNIM-QT6-IM] 디버그 모드 활성화 (UNIM_DEVELOP=1)";
        }
        checked = true;
    }
}

UnimInputContext::UnimInputContext()
    : QPlatformInputContext()
    , m_engine(nullptr)
    , m_config(nullptr)
    , m_focusObject(nullptr)
    , m_composing(false)
{
    unim_check_debug_env();
    UNIM_DEBUG("UnimInputContext 생성 시작");
    m_config = unim_config_load();
    m_engine = unim_engine_new(m_config);
    UNIM_DEBUG("UnimInputContext 생성 완료, engine=" << m_engine << ", config=" << m_config);
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
                static_cast<qsizetype>(commitStr.len)
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
        UNIM_DEBUG("filterEvent: 엔진/설정/포커스 없음, 키 무시");
        return false;
    }

    if (event->type() != QEvent::KeyPress) {
        return false;
    }

    const QKeyEvent *keyEvent = static_cast<const QKeyEvent *>(event);

    // 수정자 상태 변환
    UnimModifierState state = {
        .shift = (keyEvent->modifiers() & Qt::ShiftModifier) != 0,
        .control = (keyEvent->modifiers() & Qt::ControlModifier) != 0,
        .alt = (keyEvent->modifiers() & Qt::AltModifier) != 0,
        .super_key = (keyEvent->modifiers() & Qt::MetaModifier) != 0,
        .caps_lock = false,
        .num_lock = false
    };

    // 설정 파일 변경 체크 (mtime 기반, 매우 가벼움)
    if (m_config && unim_config_reload(m_config)) {
        // 설정이 변경되었으면 엔진에 새 레이아웃 적용
        if (m_engine) {
            unim_engine_set_hangul_layout(m_engine,
                unim_config_get_hangul_layout(m_config));
            unim_engine_set_latin_layout(m_engine,
                unim_config_get_latin_layout(m_config));
        }
    }

    // 키 입력 처리
    // X11에서 nativeScanCode() = X11 keycode = evdev + 8
    quint32 scanCode = keyEvent->nativeScanCode();
    uint16_t evdev_code = (scanCode > 8) ? static_cast<uint16_t>(scanCode - 8) : 0;
    
    UNIM_DEBUG("키 입력: key=" << keyEvent->key() << ", scanCode=" << scanCode << ", evdev=" << evdev_code
               << ", shift=" << state.shift << ", ctrl=" << state.control << ", alt=" << state.alt);
    
    UnimInputResult result = unim_engine_press_key(
        m_engine,
        m_config,
        evdev_code,
        state
    );
    
    UNIM_DEBUG("엔진 결과: consumed=" << result.consumed 
               << ", preedit_changed=" << result.preedit_changed 
               << ", commit_changed=" << result.commit_changed);

    if (result.consumed) {
        // 커밋 처리
        if (result.commit_changed) {
            UnimStr commitStr = unim_engine_commit_str(m_engine);
            if (commitStr.len > 0) {
                QString str = QString::fromUtf8(
                    reinterpret_cast<const char *>(commitStr.ptr),
                    static_cast<qsizetype>(commitStr.len)
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
    } else {
        // 엔진이 소비하지 않은 키: 조합 중이었다면 강제 커밋
        if (m_composing) {
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
    if (m_focusObject && m_composing) {
        UNIM_DEBUG("setFocusObject: 조합 중, 커밋 수행");
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
                static_cast<qsizetype>(preedit.len)
            );
        }
    }

    QList<QInputMethodEvent::Attribute> attrs;
    if (!preeditStr.isEmpty()) {
        QTextCharFormat format;
        format.setUnderlineStyle(QTextCharFormat::SingleUnderline);
        attrs << QInputMethodEvent::Attribute(
            QInputMethodEvent::TextFormat,
            0,
            preeditStr.length(),
            format
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
