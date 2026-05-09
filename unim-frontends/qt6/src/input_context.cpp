/**
 * UNIM Qt6 Input Context 구현
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
#include <QWidget>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDebug>
#include <QStandardPaths>
#include <QDir>
#include <QDateTime>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>
#include <algorithm>
#include <unistd.h>

/* 디버그 로깅 시스템 */
static bool unim_debug_enabled = false;
static bool unim_debug_checked = false;

static QString unimLogSanitize(const QString &raw)
{
    QString out;
    out.reserve(raw.size());
    for (int i = 0; i < raw.size(); ++i) {
        QChar c = raw.at(i);
        ushort u = c.unicode();
        const bool ok = (u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z')
                     || (u >= '0' && u <= '9') || u == '-' || u == '_';
        out.append(ok ? c : QChar('-'));
    }
    return out;
}

static QString unimLogProcessName()
{
    QFile f(QStringLiteral("/proc/self/comm"));
    if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QByteArray data = f.readAll().trimmed();
        f.close();
        if (!data.isEmpty()) {
            QString s = unimLogSanitize(QString::fromLocal8Bit(data));
            if (!s.isEmpty()) return s;
        }
    }
    return QStringLiteral("unknown");
}

/* 윈도우 세션·앱(프로세스)별 로그 파일 경로:
 *   ~/.unim-log/{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log
 *   session-tag 우선순위: XDG_SESSION_ID > WAYLAND_DISPLAY > DISPLAY.
 *   progname/pid 는 호스트 프로세스 (Qt IM 모듈은 호스트 앱 안에서 동작).
 */
static QString unimLogPath()
{
    QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    QString dir = home + "/.unim-log";
    QDir().mkpath(dir);

    QByteArray xdg = qgetenv("XDG_SESSION_ID");
    QByteArray wl = qgetenv("WAYLAND_DISPLAY");
    QByteArray x11 = qgetenv("DISPLAY");
    QString rawTag;
    if (!xdg.isEmpty()) rawTag = QStringLiteral("xdg-") + QString::fromLocal8Bit(xdg);
    else if (!wl.isEmpty()) rawTag = QStringLiteral("wl-") + QString::fromLocal8Bit(wl);
    else if (!x11.isEmpty()) rawTag = QStringLiteral("x11-") + QString::fromLocal8Bit(x11);
    else rawTag = QStringLiteral("unknown");

    const QString tag = unimLogSanitize(rawTag);
    const QString date = QDateTime::currentDateTime().toString("yyyy-MM-dd");
    const QString progname = unimLogProcessName();
    const long long pid = static_cast<long long>(getpid());

    return dir + "/" + tag + "_" + date + "_" + progname + "-" + QString::number(pid) + ".log";
}

/* 중앙 로깅 함수 - 콘솔과 파일에 동시 출력 */
static void unim_log_message(const char *module, const QString &message)
{
    if (!unim_debug_enabled) return;

    QString timestamp = QDateTime::currentDateTime().toString("yyyy/MM/dd hh:mm:ss");
    QString logLine = QString("[%1] - [%2] - %3").arg(timestamp, module, message);

    /* 콘솔 출력 */
    qDebug().noquote() << logLine;

    /* 파일 출력 */
    QString logPath = unimLogPath();
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
    , m_focusObject(nullptr)
    , m_composing(false)
    , m_popupActive(false)
{
    unim_check_debug_env();
    UNIM_DEBUG("UnimInputContext 생성 시작");
    
    // 창 식별자 생성 (컨텍스트 포인터 기반)
    m_windowId = QString::asprintf("qt6-ctx-%p", static_cast<void*>(this));
    
    m_dbus = new UnimDbusClient(QStringLiteral("qt6-unim"), m_windowId);

    if (m_dbus && m_dbus->isValid()) {
        UNIM_DEBUG(QString::asprintf("UnimInputContext 생성 완료 (window_id: %s)", qPrintable(m_windowId)));
        // AutoTypeFix 시그널 콜백 등록
        m_dbus->setAutoTypeFixCallback([this](quint32 deleteChars, const QString &commitText, const QString &preeditText) {
            UNIM_DEBUG(QString::asprintf("AutoTypeFix: delete=%u, commit='%s', preedit='%s'",
                       deleteChars, qPrintable(commitText), qPrintable(preeditText)));
            QObject *focusObj = QGuiApplication::focusObject();
            if (!focusObj) return;

            // 1. 기존 텍스트 삭제 + commit 텍스트 적용
            {
                QInputMethodEvent ev;
                ev.setCommitString(commitText, -(int)deleteChars, (int)deleteChars);
                QCoreApplication::sendEvent(focusObj, &ev);
            }

            // 2. preedit 설정 (순방향: 마지막 음절을 조합 상태로)
            if (!preeditText.isEmpty()) {
                QList<QInputMethodEvent::Attribute> attrs;
                QTextCharFormat fmt;
                fmt.setUnderlineStyle(QTextCharFormat::SingleUnderline);
                attrs << QInputMethodEvent::Attribute(
                    QInputMethodEvent::TextFormat, 0, preeditText.length(), fmt);
                QInputMethodEvent ev(preeditText, attrs);
                QCoreApplication::sendEvent(focusObj, &ev);
                m_composing = true;
            } else {
                m_composing = false;
            }
        });
        // CommitText 시그널 콜백 등록 (Standalone 팝업 마우스 클릭 커밋)
        m_dbus->setCommitTextCallback([this](const QString &text) {
            UNIM_DEBUG(QString::asprintf("CommitText: '%s'", qPrintable(text)));
            QObject *focusObj = QGuiApplication::focusObject();
            if (!focusObj) return;
            QInputMethodEvent ev;
            ev.setCommitString(text);
            QCoreApplication::sendEvent(focusObj, &ev);
        });
        /* Standalone popup 시그널 핸들러 — m_popupActive 플래그 관리용 */
        m_dbus->setShowEmojiPopupCallback(
            [this](const QString &,
                   const QStringList &,
                   const QString &,
                   const QStringList &,
                   const QList<UnimDbusClient::EmojiCategoryInfo> &,
                   qint32, qint32, qint32, qint32) {
                /* Standalone: unim-gui-gtk 가 팝업 전담. 플래그만 세운다. */
                m_popupActive = true;
            });
        m_dbus->setHidePopupCallback([this]() {
            m_popupActive = false;
        });
    } else {
        UNIM_DEBUG("UnimInputContext 생성 (DBus 연결 실패)");
    }

}

UnimInputContext::~UnimInputContext()
{
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

                /* 커서 위치를 데몬에 보고 (팝업 포지셔닝용) */
                if (m_dbus && m_cursorRect.isValid()) {
                    m_dbus->reportCursorRect(
                        m_cursorRect.x(), m_cursorRect.y(),
                        m_cursorRect.width(), m_cursorRect.height());
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
        key == Qt::Key_Hyper_L || key == Qt::Key_Hyper_R ||
        key == Qt::Key_CapsLock || key == Qt::Key_NumLock ||
        key == Qt::Key_ScrollLock ||
        key == Qt::Key_AltGr) {
        return false;
    }

    /* 한자 키 처리 (F9 또는 Hangul_Hanja) — Standalone: unim-gui-gtk 팝업 전담 */
    if (key == Qt::Key_F9 || key == Qt::Key_Hangul_Hanja) {
        if (m_dbus) {
            QString target;
            QList<UnimHanjaCandidate> candidates;
            if (m_dbus->getHanjaCandidates(target, candidates) && !candidates.isEmpty()) {
                UNIM_DEBUG(QString::asprintf("한자 후보 %d개 — Standalone popup 위임",
                           static_cast<int>(candidates.size())));
            } else {
                /* 한자 후보 없음 → 특수문자 후보 확인 */
                QString spTarget;
                QStringList spChars;
                QString spTopRow;
                if (m_dbus->getSpecialCharCandidates(spTarget, spChars, spTopRow) && !spChars.isEmpty()) {
                    UNIM_DEBUG(QString::asprintf("특수문자 후보 %d개 — Standalone popup 위임",
                               static_cast<int>(spChars.size())));
                } else {
                    UNIM_DEBUG("한자/특수문자 후보 없음 → idle Hanja: emoji 트리거 위임");
                    quint32 mod_state_emoji = 0;
                    if (keyEvent->modifiers() & Qt::ShiftModifier)   mod_state_emoji |= (1 << 0);
                    if (keyEvent->modifiers() & Qt::ControlModifier) mod_state_emoji |= (1 << 2);
                    if (keyEvent->modifiers() & Qt::AltModifier)     mod_state_emoji |= (1 << 3);
                    if (keyEvent->modifiers() & Qt::MetaModifier)    mod_state_emoji |= (1 << 26);
                    quint32 scanCode_emoji = keyEvent->nativeScanCode();
                    quint32 evdev_emoji = (scanCode_emoji > 8) ? (scanCode_emoji - 8) : 0;
                    (void)m_dbus->processKey(keyEvent->key(), evdev_emoji, mod_state_emoji);
                }
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

        /* 입력 필드 목적 감지: Qt::ImhHiddenText → Password */
        QInputMethodQueryEvent query(Qt::ImHints);
        QCoreApplication::sendEvent(object, &query);
        Qt::InputMethodHints hints = static_cast<Qt::InputMethodHints>(
            query.value(Qt::ImHints).toInt());
        quint32 purpose = 0; /* Normal */
        if (hints & Qt::ImhHiddenText) {
            purpose = 1; /* Password */
        } else if (hints & Qt::ImhDigitsOnly) {
            purpose = 4; /* Number */
        } else if (hints & Qt::ImhUrlCharactersOnly) {
            purpose = 5; /* Url */
        } else if (hints & Qt::ImhEmailCharactersOnly) {
            purpose = 3; /* Email */
        }
        m_dbus->setContentType(purpose);
        UNIM_DEBUG(QString::asprintf("content_type 전달: hints=0x%x, purpose=%u",
                   static_cast<int>(hints), purpose));

        /* Surrounding text 전달 */
        QInputMethodQueryEvent stQuery(Qt::ImSurroundingText | Qt::ImCursorPosition | Qt::ImAnchorPosition);
        QCoreApplication::sendEvent(object, &stQuery);
        QString surroundingText = stQuery.value(Qt::ImSurroundingText).toString();
        int cursorPos = stQuery.value(Qt::ImCursorPosition).toInt();
        int anchorPos = stQuery.value(Qt::ImAnchorPosition).toInt();
        if (!surroundingText.isEmpty()) {
            m_dbus->setSurroundingText(surroundingText,
                                        static_cast<quint32>(cursorPos),
                                        static_cast<quint32>(anchorPos));
        }
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
