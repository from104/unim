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
#include <QDir>
#include <QDateTime>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>
#include <unistd.h>

/* 디버그 로깅 */
static bool unim_dbus_debug_enabled = false;
static bool unim_dbus_debug_checked = false;

/* ASCII 알파벳/숫자/대시/언더스코어만 통과시키고 나머지는 '-' 로 치환 */
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

/* 호스트 프로세스 이름. 실패 시 "unknown". */
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
    if (!unim_dbus_debug_enabled) return;

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

void UnimDbusClient::setAutoTypeFixCallback(AutoTypeFixCallback callback) {
    m_autoTypeFixCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimAutoTypeFixReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("AutoTypefixApply"),
        receiver,
        SLOT(onAutoTypefixApply(quint32,QString,QString))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("AutoTypeFix signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimAutoTypeFixReceiver::onAutoTypefixApply(quint32 deleteChars, const QString &commitText, const QString &preeditText) {
    if (m_client && m_client->m_autoTypeFixCallback) {
        UNIM_DBUS_DEBUG(QString::asprintf("AutoTypeFix received: delete=%u, commit='%s', preedit='%s'",
                         deleteChars, qPrintable(commitText), qPrintable(preeditText)));
        m_client->m_autoTypeFixCallback(deleteChars, commitText, preeditText);
    }
}

void UnimDbusClient::setCommitTextCallback(CommitTextCallback callback) {
    m_commitTextCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimCommitTextReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("CommitText"),
        receiver,
        SLOT(onCommitText(QString))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("CommitText signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimCommitTextReceiver::onCommitText(const QString &text) {
    if (m_client && m_client->m_commitTextCallback && !text.isEmpty()) {
        UNIM_DBUS_DEBUG(QString::asprintf("CommitText received: '%s'", qPrintable(text)));
        m_client->m_commitTextCallback(text);
    }
}

/* =========================================
 * HanjaBookmark 관련 구현
 * ========================================= */

bool UnimDbusClient::getHanjaBookmarkStates(QList<bool> &states)
{
    if (!isValid()) return false;
    states.clear();

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("GetHanjaBookmarkStates")
    );

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("GetHanjaBookmarkStates 실패: %s",
                         qPrintable(reply.errorMessage())));
        return false;
    }

    const QList<QVariant> args = reply.arguments();
    if (args.isEmpty()) return false;

    // zbus 'ab' → QDBusArgument or QVariantList
    const QVariant &v = args.at(0);
    if (v.canConvert<QDBusArgument>()) {
        const QDBusArgument arg = v.value<QDBusArgument>();
        arg.beginArray();
        while (!arg.atEnd()) {
            bool b = false;
            arg >> b;
            states.append(b);
        }
        arg.endArray();
    } else {
        const QVariantList list = v.toList();
        for (const QVariant &e : list) {
            states.append(e.toBool());
        }
    }
    return true;
}

bool UnimDbusClient::toggleHanjaBookmark(quint32 index)
{
    if (!isValid()) return false;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("ToggleHanjaBookmark")
    );
    msg << index;

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("ToggleHanjaBookmark 실패 index=%u: %s",
                         index, qPrintable(reply.errorMessage())));
        return false;
    }
    return true;
}

bool UnimDbusClient::popupChangePage(qint32 direction)
{
    if (!isValid()) return false;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("PopupChangePage")
    );
    msg << direction;

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("PopupChangePage 실패 dir=%d: %s",
                         direction, qPrintable(reply.errorMessage())));
        return false;
    }
    return true;
}

void UnimDbusClient::setHanjaBookmarkChangedCallback(HanjaBookmarkChangedCallback callback) {
    m_hanjaBookmarkCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimHanjaBookmarkReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("HanjaBookmarkChanged"),
        receiver,
        SLOT(onHanjaBookmarkChanged(quint32,bool))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("HanjaBookmarkChanged signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimHanjaBookmarkReceiver::onHanjaBookmarkChanged(quint32 index, bool bookmarked) {
    if (m_client && m_client->m_hanjaBookmarkCallback) {
        UNIM_DBUS_DEBUG(QString::asprintf("HanjaBookmarkChanged received: index=%u, bookmarked=%d",
                         index, int(bookmarked)));
        m_client->m_hanjaBookmarkCallback(index, bookmarked);
    }
}

/* HanjaCandidatesReordered 시그널 등록 + 파서.
 * 시그니처: (s, as, as, ab, u, i, i, i, b, b)
 *  - 마지막 b: was_bookmarked (Phase 1 mouse-paginate UX, args[9]).
 *    본 콜백은 아직 사용하지 않지만 Phase 7 visual flash 에서 활용 예정.
 *
 * QtDBus의 자동 슬롯 디마샬링은 ab(QList<bool>)를 직접 매핑하지 못해
 * QDBusMessage 통째로 받아 인덱스별로 파싱한다. GTK 측 client (gtk-common)와
 * 동일한 시그니처·필드 순서. */
void UnimDbusClient::setHanjaCandidatesReorderedCallback(HanjaCandidatesReorderedCallback callback) {
    m_hanjaReorderedCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimHanjaCandidatesReorderedReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("HanjaCandidatesReordered"),
        receiver,
        SLOT(onReordered(QDBusMessage))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("HanjaCandidatesReordered signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimHanjaCandidatesReorderedReceiver::onReordered(const QDBusMessage &msg) {
    if (!m_client || !m_client->m_hanjaReorderedCallback) return;

    const QList<QVariant> args = msg.arguments();
    if (args.size() < 9) {
        UNIM_DBUS_DEBUG(QString::asprintf("HanjaCandidatesReordered: 인자 부족 (%d)",
                         static_cast<int>(args.size())));
        return;
    }

    QString target = args.at(0).toString();
    QStringList hanjas = args.at(1).toStringList();
    QStringList meanings = args.at(2).toStringList();

    QList<bool> bookmarks;
    {
        const QVariant &v = args.at(3);
        if (v.canConvert<QDBusArgument>()) {
            const QDBusArgument arg = v.value<QDBusArgument>();
            arg.beginArray();
            while (!arg.atEnd()) {
                bool b = false;
                arg >> b;
                bookmarks.append(b);
            }
            arg.endArray();
        } else {
            const QVariantList list = v.toList();
            for (const QVariant &e : list) bookmarks.append(e.toBool());
        }
    }

    quint32 newCursor   = args.at(4).toUInt();
    qint32 page         = args.at(5).toInt();
    qint32 selRow       = args.at(6).toInt();
    qint32 selCol       = args.at(7).toInt();
    bool bookmarked     = args.at(8).toBool();
    bool wasBookmarked  = args.size() >= 10 ? args.at(9).toBool() : false;

    QList<UnimHanjaCandidate> candidates;
    int n = hanjas.size();
    candidates.reserve(n);
    for (int i = 0; i < n; i++) {
        UnimHanjaCandidate c;
        c.hanja = hanjas.at(i);
        c.meaning = i < meanings.size() ? meanings.at(i) : QString();
        candidates.append(c);
    }

    UNIM_DBUS_DEBUG(QString::asprintf(
        "HanjaCandidatesReordered received: target='%s', count=%d, new_cursor=%u, page=%d, sel=(%d,%d), was=%d now=%d",
        qPrintable(target), n, newCursor, page, selRow, selCol, int(wasBookmarked), int(bookmarked)));

    m_client->m_hanjaReorderedCallback(target, candidates, bookmarks,
                                        newCursor, page, selRow, selCol,
                                        bookmarked, wasBookmarked);
}

/* =========================================
 * 이모지 팝업 시그널 (PR #4 emoji overhaul)
 * ========================================= */

void UnimDbusClient::setShowEmojiPopupCallback(ShowEmojiPopupCallback callback) {
    m_showEmojiPopupCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimShowEmojiPopupReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("ShowEmojiPopupV2"),
        receiver,
        SLOT(onShowEmoji(QDBusMessage))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("ShowEmojiPopupV2 signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimShowEmojiPopupReceiver::onShowEmoji(const QDBusMessage &msg) {
    if (!m_client || !m_client->m_showEmojiPopupCallback) return;

    const QList<QVariant> args = msg.arguments();
    if (args.size() < 9) {
        UNIM_DBUS_DEBUG(QString::asprintf("ShowEmojiPopupV2: 인자 부족 (%d)",
                         static_cast<int>(args.size())));
        return;
    }

    QString targetCatId = args.at(0).toString();
    QStringList items = args.at(1).toStringList();
    QString topRow = args.at(2).toString();
    QStringList recent = args.at(3).toStringList();

    /* a(sssu) — QDBusArgument 로 직접 파싱 */
    QList<UnimDbusClient::EmojiCategoryInfo> categories;
    {
        const QVariant &v = args.at(4);
        if (v.canConvert<QDBusArgument>()) {
            const QDBusArgument arg = v.value<QDBusArgument>();
            arg.beginArray();
            while (!arg.atEnd()) {
                UnimDbusClient::EmojiCategoryInfo info{};
                arg.beginStructure();
                arg >> info.id >> info.nameKo >> info.nameEn >> info.count;
                arg.endStructure();
                categories.append(info);
            }
            arg.endArray();
        }
    }

    qint32 cx = args.at(5).toInt();
    qint32 cy = args.at(6).toInt();
    qint32 cw = args.at(7).toInt();
    qint32 ch = args.at(8).toInt();

    UNIM_DBUS_DEBUG(QString::asprintf(
        "ShowEmojiPopupV2 received: cat='%s', items=%d, recent=%d, cats=%d, cursor=(%d,%d,%d,%d)",
        qPrintable(targetCatId),
        static_cast<int>(items.size()),
        static_cast<int>(recent.size()),
        static_cast<int>(categories.size()),
        cx, cy, cw, ch));

    m_client->m_showEmojiPopupCallback(targetCatId, items, topRow, recent,
                                        categories, cx, cy, cw, ch);
}

void UnimDbusClient::setPopupNavigateCallback(PopupNavigateCallback callback) {
    m_popupNavigateCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimPopupNavigateReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("PopupNavigate"),
        receiver,
        SLOT(onPopupNavigate(qint32,qint32,qint32,qint32,qint32,qint32,qint32))
    );
    UNIM_DBUS_DEBUG(QString::asprintf("PopupNavigate signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimPopupNavigateReceiver::onPopupNavigate(qint32 page, qint32 totalPages, qint32 selected,
                                                 qint32 rows, qint32 cols, qint32 selRow, qint32 selCol) {
    if (m_client && m_client->m_popupNavigateCallback) {
        UNIM_DBUS_DEBUG(QString::asprintf(
            "PopupNavigate received: page=%d/%d, sel=(%d,%d)",
            page, totalPages, selRow, selCol));
        m_client->m_popupNavigateCallback(page, totalPages, selected, rows, cols, selRow, selCol);
    }
}

void UnimDbusClient::setHidePopupCallback(HidePopupCallback callback) {
    m_hidePopupCallback = std::move(callback);
    if (!m_connected || m_contextPath.isEmpty()) return;

    auto *receiver = new UnimHidePopupReceiver(this);
    m_bus.connect(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("HidePopup"),
        receiver,
        SLOT(onHidePopup())
    );
    UNIM_DBUS_DEBUG(QString::asprintf("HidePopup signal subscribed: path=%s",
                     qPrintable(m_contextPath)));
}

void UnimHidePopupReceiver::onHidePopup() {
    if (m_client && m_client->m_hidePopupCallback) {
        UNIM_DBUS_DEBUG("HidePopup received");
        m_client->m_hidePopupCallback();
    }
}

bool UnimDbusClient::commitEmoji(const QString &emoji) {
    if (!m_connected || m_contextPath.isEmpty()) return false;

    QDBusMessage msg = QDBusMessage::createMethodCall(
        QString::fromUtf8(UNIM_DBUS_SERVICE),
        m_contextPath,
        QString::fromUtf8(UNIM_DBUS_IC_INTERFACE),
        QStringLiteral("CommitEmoji")
    );
    msg << emoji;

    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG(QString::asprintf("CommitEmoji 실패: emoji='%s', err=%s",
                         qPrintable(emoji), qPrintable(reply.errorMessage())));
        return false;
    }
    return true;
}
