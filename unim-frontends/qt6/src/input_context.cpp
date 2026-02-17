/**
 * UNIM Qt6 Input Context 구현
 *
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#include "input_context.hpp"
#include "unim_dbus_client.hpp"
#include "unim_hanja_popup.hpp"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QTextCharFormat>
#include <QDebug>
#include <QStandardPaths>
#include <QDateTime>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>
#include <algorithm>

/* 디버그 로깅 시스템 */
static bool unim_debug_enabled = false;
static bool unim_debug_checked = false;

/* 중앙 로깅 함수 - 콘솔과 파일에 동시 출력 */
static void unim_log_message(const char *module, const QString &message)
{
    if (!unim_debug_enabled) return;

    QString timestamp = QDateTime::currentDateTime().toString("yyyy/MM/dd hh:mm:ss");
    QString logLine = QString("[%1] - [%2] - %3").arg(timestamp, module, message);

    /* 콘솔 출력 */
    qDebug().noquote() << logLine;

    /* 파일 출력 */
    QString logPath = QStandardPaths::writableLocation(QStandardPaths::HomeLocation) + "/.unim-errors.log";
    QFile file(logPath);
    if (file.open(QIODevice::Append | QIODevice::Text)) {
        QTextStream out(&file);
        out << logLine << "\n";
        file.close();
    }
}

#define UNIM_DEBUG(...) \
    unim_log_message("QT6_IM", QString(__VA_ARGS__))

static void unim_check_debug_env()
{
    if (!unim_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            unim_log_message("QT6_IM", "디버그 모드 활성화 (UNIM_DEVELOP=1)");
        }
        unim_debug_checked = true;
    }
}


UnimInputContext::UnimInputContext()
    : QPlatformInputContext()
    , m_dbus(nullptr)
    , m_hanjaPopup(nullptr)
    , m_focusObject(nullptr)
    , m_composing(false)
{
    unim_check_debug_env();
    UNIM_DEBUG("UnimInputContext 생성 시작");
    
    // 창 식별자 생성 (컨텍스트 포인터 기반)
    m_windowId = QString::asprintf("qt6-ctx-%p", static_cast<void*>(this));
    
    m_dbus = new UnimDbusClient(QStringLiteral("qt6-unim"), m_windowId);
    
    if (m_dbus && m_dbus->isValid()) {
        UNIM_DEBUG(QString::asprintf("UnimInputContext 생성 완료 (window_id: %s)", qPrintable(m_windowId)));
    } else {
        UNIM_DEBUG("UnimInputContext 생성 (DBus 연결 실패)");
    }
    
    m_hanjaPopup = new UnimHanjaPopup();
}

UnimInputContext::~UnimInputContext()
{
    delete m_hanjaPopup;
    m_hanjaPopup = nullptr;
    delete m_dbus;
    m_dbus = nullptr;
}

bool UnimInputContext::isValid() const
{
    return m_dbus != nullptr && m_dbus->isValid();
}

void UnimInputContext::reset()
{
    /* 조합 중인 글자를 먼저 커밋 */
    if (m_dbus) {
        QString commit = m_dbus->reset();
        if (!commit.isEmpty()) {
            commitString(commit);
        }
        m_composing = false;
        updatePreedit();
    }

    /* 한자 팝업이 표시 중이면 닫기 */
    if (m_hanjaPopup && m_hanjaPopup->isVisible()) {
        m_hanjaPopup->hidePopup();
        if (m_dbus) {
            m_dbus->cancelHanja();
        }
    }
}

void UnimInputContext::commit()
{
    /* 조합 중인 글자를 먼저 커밋 */
    if (m_dbus && m_composing) {
        QString commit = m_dbus->reset();
        if (!commit.isEmpty()) {
            commitString(commit);
        }
        m_composing = false;
        updatePreedit();
    }

    /* 한자 팝업이 표시 중이면 닫기 */
    if (m_hanjaPopup && m_hanjaPopup->isVisible()) {
        m_hanjaPopup->hidePopup();
        if (m_dbus) {
            m_dbus->cancelHanja();
        }
    }
}

void UnimInputContext::update(Qt::InputMethodQueries queries)
{
    if (queries & Qt::ImCursorRectangle) {
        if (m_focusObject) {
            QInputMethodQueryEvent query(Qt::ImCursorRectangle);
            QCoreApplication::sendEvent(m_focusObject, &query);
            QRect rect = query.value(Qt::ImCursorRectangle).toRect();
            if (rect.isValid()) {
                /* 위젯의 글로벌 좌표로 변환 */
                QObject *window = m_focusObject;
                while (window) {
                    if (auto *w = qobject_cast<QWidget*>(window)) {
                        QPoint globalPos = w->mapToGlobal(rect.topLeft());
                        m_cursorRect = QRect(globalPos, rect.size());
                        break;
                    }
                    window = window->parent();
                }
            }
        }
    }
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

    /* 한자 팝업이 표시 중이면 먼저 팝업에서 키 처리 */
    if (m_hanjaPopup && m_hanjaPopup->isVisible()) {
        /* Escape → 조합 복원 + 팝업 닫기 */
        if (key == Qt::Key_Escape) {
            UNIM_DEBUG("한자 팝업 Escape -> 조합 복원 + 팝업 닫기");

            /* ProcessKey(0,0,0)로 엔진 리셋 → preedit/commit 응답 받기 */
            if (m_dbus) {
                UnimDbusKeyResult resetResult = m_dbus->processKey(0, 0, 0);
                if (!resetResult.commit.isEmpty()) {
                    commitString(resetResult.commit);
                }
            }

            /* CancelHanja → 한자 모드 해제 */
            if (m_dbus) {
                m_dbus->cancelHanja();
            }

            /* preedit 복원 */
            m_composing = m_dbus && m_dbus->isComposing();
            updatePreedit();

            /* 팝업 닫기 */
            m_hanjaPopup->hidePopup();
            return true;
        }

        /* 팝업 내부 처리 (숫자 선택, 네비게이션, 모디파이어 등) */
        if (m_hanjaPopup->handleKey(key)) {
            return true;
        }

        /* 미지원 키 → 조합 커밋 + 팝업 닫기 + fall-through (엔진에 키 전달) */
        UNIM_DEBUG("한자 팝업 미지원 키 -> 조합 커밋 + 팝업 닫고 엔진에 키 전달");

        /* 1. FocusOut으로 조합 중 한글 커밋 */
        if (m_dbus) {
            QString commitText = m_dbus->focusOut();
            if (!commitText.isEmpty()) {
                UNIM_DEBUG(QString::asprintf("조합 커밋: \"%s\"", qPrintable(commitText)));
                commitString(commitText);
            }
        }

        /* preedit 클리어 */
        m_composing = false;
        updatePreedit();

        /* 2. CancelHanja + 팝업 닫기 */
        if (m_dbus) {
            m_dbus->cancelHanja();
        }
        m_hanjaPopup->hidePopup();

        /* 3. FocusIn으로 컨텍스트 복원 (FocusOut 후 필요) */
        if (m_dbus) {
            m_dbus->focusIn(m_windowId);
        }

        /* fall-through → 아래 processKey 경로에서 엔진이 새 키 처리 */
    }

    /* 한자 키 처리 (F9 또는 Hangul_Hanja) */
    if (key == Qt::Key_F9 || key == Qt::Key_Hangul_Hanja) {
        if (m_dbus && m_hanjaPopup) {
            QString target;
            QList<UnimHanjaCandidate> candidates;
            if (m_dbus->getHanjaCandidates(target, candidates) && !candidates.isEmpty()) {
                int popupX = m_cursorRect.x();
                int popupY = m_cursorRect.y() + m_cursorRect.height();
                
                UNIM_DEBUG(QString::asprintf("한자 후보 표시: target='%s', count=%d, pos=(%d,%d)",
                           qPrintable(target), candidates.size(), popupX, popupY));
                
                m_hanjaPopup->showPopup(target, candidates, popupX, popupY, m_cursorRect.height(),
                    [this](const QString &hanja) {
                        UNIM_DEBUG(QString::asprintf("한자 선택: '%s'", qPrintable(hanja)));
                        if (m_dbus) {
                            m_dbus->cancelHanja();
                        }
                        m_composing = false;
                        updatePreedit();
                        commitString(hanja);
                    });
            } else {
                UNIM_DEBUG("한자 후보 없음");
            }
        }
        return true;
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
    
    UNIM_DEBUG(QString::asprintf("키 입력: key=%d, scanCode=%u, evdev=%u, state=%u",
               keyEvent->key(), scanCode, evdev_code, mod_state));

    /* DBus를 통해 키 처리 */
    UnimDbusKeyResult result = m_dbus->processKey(keyEvent->key(), evdev_code, mod_state);
    
    UNIM_DEBUG(QString::asprintf("엔진 결과: consumed=%d, preedit=%s, commit=%s",
               result.consumed, qPrintable(result.preedit), qPrintable(result.commit)));

    if (result.consumed) {
        /* 선택 영역 삭제 처리 */
        if (m_focusObject) {
            QInputMethodQueryEvent query(Qt::ImAnchorPosition | Qt::ImCursorPosition);
            QCoreApplication::sendEvent(m_focusObject, &query);
            int anchorPos = query.value(Qt::ImAnchorPosition).toInt();
            int cursorPos = query.value(Qt::ImCursorPosition).toInt();

            if (anchorPos != cursorPos) {
                int start = std::min(anchorPos, cursorPos);
                int end = std::max(anchorPos, cursorPos);
                UNIM_DEBUG(QString::asprintf("Qt 선택 영역 삭제: start=%d, end=%d", start, end));
                
                QInputMethodEvent deleteEvent;
                deleteEvent.setCommitString("", start - cursorPos, end - start);
                QCoreApplication::sendEvent(m_focusObject, &deleteEvent);
            }
        }

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
    UNIM_DEBUG(QString::asprintf("setFocusObject: object=%p", static_cast<void*>(object)));

    /* 한자 팝업이 표시 중이면 닫기 */
    if (m_hanjaPopup && m_hanjaPopup->isVisible()) {
        UNIM_DEBUG("setFocusObject: 한자 팝업 닫기");
        m_hanjaPopup->hidePopup();
        if (m_dbus) {
            m_dbus->cancelHanja();
        }
    }

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
        m_dbus->focusIn(m_windowId);
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
