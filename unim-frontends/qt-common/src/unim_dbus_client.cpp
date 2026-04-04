/**
 * UNIM DBus Client Implementation for Qt
 *
 * QtDBus를 사용하여 unim-daemon과 통신하는 클라이언트 구현입니다.
 */

#include "unim_dbus_client.hpp"
#include <QDebug>
#include <QDBusMessage>
#include <QDBusPendingReply>
#include <QStandardPaths>
#include <QDateTime>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 */
static bool unim_dbus_debug_enabled = false;
static bool unim_dbus_debug_checked = false;

/* 중앙 로깅 함수 - 콘솔과 파일에 동시 출력 */
static void unim_log_message(const char *module, const QString &message)
{
    if (!unim_dbus_debug_enabled) return;

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

#define UNIM_DBUS_DEBUG(...) \
    unim_log_message("QT_DBUS", QString(__VA_ARGS__))

static void unim_dbus_check_debug_env()
{
    if (!unim_dbus_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            unim_dbus_debug_enabled = true;
        }
        unim_dbus_debug_checked = true;
    }
}


UnimDbusClient::UnimDbusClient(const QString &clientName, const QString &windowId)
    : m_bus(QDBusConnection::sessionBus())
    , m_isComposing(false)
    , m_connected(false)
{
    unim_dbus_check_debug_env();
    
    if (!m_bus.isConnected()) {
        UNIM_DBUS_DEBUG("디버그 세션 버스 연결 실패");
        return;
    }
    
    UNIM_DBUS_DEBUG("디버그 세션 버스 연결 성공");
    
    // window_id가 비어있으면 빈 문자열 사용
    QString effectiveWindowId = windowId.isEmpty() ? QString() : windowId;
    
    // InputContext 생성 요청 (window_id 포함)
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_PATH,
        UNIM_DBUS_INTERFACE,
        QStringLiteral("CreateInputContext")
    );
    msg << clientName << effectiveWindowId;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("CreateInputContext 실패: %s", qPrintable(reply.errorMessage())));
        return;
    }
    
    if (reply.arguments().size() > 0) {
        m_contextPath = reply.arguments().at(0).toString();
        m_connected = true;
        UNIM_DBUS_DEBUG(QString::asprintf("InputContext 생성: %s (window_id: %s)", qPrintable(m_contextPath), qPrintable(effectiveWindowId)));
    }
}

UnimDbusClient::~UnimDbusClient()
{
    if (!m_connected || m_contextPath.isEmpty()) return;
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("Destroy")
    );
    
    m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    UNIM_DBUS_DEBUG(QString::asprintf("InputContext 파괴: %s", qPrintable(m_contextPath)));
}

bool UnimDbusClient::isValid() const
{
    return m_connected && !m_contextPath.isEmpty();
}

UnimDbusKeyResult UnimDbusClient::processKey(quint32 keyval, quint32 keycode, quint32 state)
{
    UnimDbusKeyResult result;
    result.consumed = false;
    
    if (!isValid()) return result;
    
    UNIM_DBUS_DEBUG(QString::asprintf("ProcessKeyEvent: keyval=%u, keycode=%u, state=%u", keyval, keycode, state));
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("ProcessKeyEvent")
    );
    msg << keyval << keycode << state;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("ProcessKeyEvent 실패: %s", qPrintable(reply.errorMessage())));
        return result;
    }
    
    QList<QVariant> args = reply.arguments();
    if (args.size() >= 3) {
        result.consumed = args.at(0).toBool();
        result.preedit = args.at(1).toString();
        result.commit = args.at(2).toString();

        // 캐시 업데이트
        m_preeditCache = result.preedit;
        m_isComposing = !result.preedit.isEmpty();

        UNIM_DBUS_DEBUG(QString::asprintf("ProcessKeyEvent 결과: consumed=%d, preedit=%s, commit=%s",
                        result.consumed, qPrintable(result.preedit), qPrintable(result.commit)));
    }
    
    return result;
}

void UnimDbusClient::focusIn(const QString &windowId)
{
    if (!isValid()) return;
    
    QString effectiveWindowId = windowId.isEmpty() ? QString() : windowId;
    UNIM_DBUS_DEBUG(QString::asprintf("FocusIn (window_id: %s)", qPrintable(effectiveWindowId)));
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("FocusIn")
    );
    msg << effectiveWindowId;
    
    m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
}

QString UnimDbusClient::focusOut()
{
    QString commitStr;
    
    if (!isValid()) return commitStr;
    
    UNIM_DBUS_DEBUG("FocusOut");
    
    // DBus FocusOut 호출 — 서버 반환값을 우선 사용
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("FocusOut")
    );

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
        QString serverCommit = reply.arguments().first().toString();
        if (!serverCommit.isEmpty()) {
            commitStr = serverCommit;
            UNIM_DBUS_DEBUG(QString::asprintf("FocusOut 커밋 (서버): %s", qPrintable(commitStr)));
        }
    } else if (m_isComposing && !m_preeditCache.isEmpty()) {
        // DBus 실패 시 로컬 캐시 폴백
        commitStr = m_preeditCache;
        UNIM_DBUS_DEBUG(QString::asprintf("FocusOut 커밋 (로컬 폴백): %s", qPrintable(commitStr)));
    }
    
    // 상태 초기화
    m_preeditCache.clear();
    m_isComposing = false;
    
    return commitStr;
}

QString UnimDbusClient::reset()
{
    QString commitStr;
    
    if (!isValid()) return commitStr;
    
    UNIM_DBUS_DEBUG("Reset");
    
    // 조합 중인 문자가 있으면 반환
    if (m_isComposing && !m_preeditCache.isEmpty()) {
        commitStr = m_preeditCache;
        UNIM_DBUS_DEBUG(QString::asprintf("Reset 커밋: %s", qPrintable(commitStr)));
    }
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("Reset")
    );
    
    m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    // 상태 초기화
    m_preeditCache.clear();
    m_isComposing = false;
    
    return commitStr;
}

QString UnimDbusClient::getPreedit() const
{
    return m_preeditCache;
}

bool UnimDbusClient::isComposing() const
{
    return m_isComposing;
}

void UnimDbusClient::reportCursorRect(int x, int y, int width, int height)
{
    if (!isValid()) return;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("ReportCursorRect")
    );
    msg << x << y << width << height;

    // fire-and-forget (비동기)
    m_bus.call(msg, QDBus::NoBlock);
}

bool UnimDbusClient::getHanjaCandidates(QString &target, QList<UnimHanjaCandidate> &candidates)
{
    if (!isValid()) return false;
    
    UNIM_DBUS_DEBUG("GetHanjaCandidates 호출");
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("GetHanjaCandidates")
    );
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("GetHanjaCandidates 실패: %s", qPrintable(reply.errorMessage())));
        return false;
    }
    
    QList<QVariant> args = reply.arguments();
    if (args.size() >= 2) {
        target = args.at(0).toString();
        
        // a(ss) 형태의 배열 파싱 — const ref로 접근하여 write 모드 전환 방지
        const QDBusArgument &dbusArg = args.at(1).value<QDBusArgument>();
        candidates.clear();
        
        dbusArg.beginArray();
        while (!dbusArg.atEnd()) {
            QString hanja, meaning;
            dbusArg.beginStructure();
            dbusArg >> hanja >> meaning;
            dbusArg.endStructure();
            UnimHanjaCandidate cand;
            cand.hanja = hanja;
            cand.meaning = meaning;
            candidates.append(cand);
        }
        dbusArg.endArray();
        
        UNIM_DBUS_DEBUG(QString::asprintf("GetHanjaCandidates 결과: target=%s, count=%d",
                        qPrintable(target), static_cast<int>(candidates.size())));
    }
    
    return true;
}

bool UnimDbusClient::selectHanja(quint32 index, QString &selectedHanja)
{
    if (!isValid()) return false;
    
    UNIM_DBUS_DEBUG(QString::asprintf("SelectHanja 호출: index=%u", index));
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("SelectHanja")
    );
    msg << index;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("SelectHanja 실패: %s", qPrintable(reply.errorMessage())));
        return false;
    }
    
    QList<QVariant> args = reply.arguments();
    if (args.size() >= 1) {
        selectedHanja = args.at(0).toString();
        UNIM_DBUS_DEBUG(QString::asprintf("SelectHanja 결과: '%s'", qPrintable(selectedHanja)));
    }
    
    return true;
}

QString UnimDbusClient::cancelHanja()
{
    if (!isValid()) return QString();

    UNIM_DBUS_DEBUG("CancelHanja 호출");

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("CancelHanja")
    );

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);

    QString commit;
    if (reply.type() == QDBusMessage::ReplyMessage && reply.arguments().size() >= 1) {
        commit = reply.arguments().at(0).toString();
        if (!commit.isEmpty()) {
            UNIM_DBUS_DEBUG(QString::asprintf("CancelHanja 커밋: '%s'", qPrintable(commit)));
        }
    }

    // 엔진의 preedit이 클리어되었으므로 로컬 캐시도 동기화
    m_preeditCache.clear();
    m_isComposing = false;

    return commit;
}

bool UnimDbusClient::getSpecialCharCandidates(QString &target, QStringList &characters, QString &topRow)
{
    if (!isValid()) return false;
    
    UNIM_DBUS_DEBUG("GetSpecialCharCandidates 호출");
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("GetSpecialCharCandidates")
    );
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("GetSpecialCharCandidates 실패: %s", qPrintable(reply.errorMessage())));
        return false;
    }
    
    // 반환 형식: (sass) → target, characters[], top_row
    QList<QVariant> args = reply.arguments();
    if (args.size() >= 3) {
        target = args.at(0).toString();
        characters = args.at(1).toStringList();
        topRow = args.at(2).toString();
        
        UNIM_DBUS_DEBUG(QString::asprintf("GetSpecialCharCandidates 결과: target='%s', count=%d, topRow='%s'",
                        qPrintable(target), static_cast<int>(characters.size()), qPrintable(topRow)));
    }
    
    return true;
}

bool UnimDbusClient::selectSpecialChar(quint32 index, QString &selectedChar)
{
    if (!isValid()) return false;
    
    UNIM_DBUS_DEBUG(QString::asprintf("SelectSpecialChar 호출: index=%u", index));
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("SelectSpecialChar")
    );
    msg << index;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("SelectSpecialChar 실패: %s", qPrintable(reply.errorMessage())));
        return false;
    }
    
    QList<QVariant> args = reply.arguments();
    if (args.size() >= 1) {
        selectedChar = args.at(0).toString();
        UNIM_DBUS_DEBUG(QString::asprintf("SelectSpecialChar 결과: '%s'", qPrintable(selectedChar)));
    }
    
    return true;
}

QString UnimDbusClient::cancelSpecialChar()
{
    if (!isValid()) return QString();

    UNIM_DBUS_DEBUG("CancelSpecialChar 호출");

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("CancelSpecialChar")
    );

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);

    QString commit;
    if (reply.type() == QDBusMessage::ReplyMessage && reply.arguments().size() >= 1) {
        commit = reply.arguments().at(0).toString();
        if (!commit.isEmpty()) {
            UNIM_DBUS_DEBUG(QString::asprintf("CancelSpecialChar 커밋: '%s'", qPrintable(commit)));
        }
    }

    // 엔진의 preedit이 클리어되었으므로 로컬 캐시도 동기화
    m_preeditCache.clear();
    m_isComposing = false;

    return commit;
}

void UnimDbusClient::setContentType(quint32 purpose)
{
    if (!isValid()) return;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("SetContentType")
    );
    msg << purpose;
    m_bus.call(msg, QDBus::NoBlock);
}

void UnimDbusClient::setSurroundingText(const QString &text, quint32 cursorPos, quint32 anchorPos)
{
    if (!isValid()) return;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("SetSurroundingText")
    );
    msg << text << cursorPos << anchorPos;
    m_bus.call(msg, QDBus::NoBlock);
}
