/**
 * UNIM DBus Client Implementation for Qt
 *
 * QtDBus를 사용하여 unim-daemon과 통신하는 클라이언트 구현입니다.
 */

#include "unim_dbus_client.hpp"
#include <QDebug>
#include <QDBusMessage>
#include <QDBusPendingReply>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 */
static bool unim_dbus_debug_enabled = false;
static bool unim_dbus_debug_checked = false;

#define UNIM_DBUS_DEBUG(...) \
    do { \
        if (unim_dbus_debug_enabled) { \
            qDebug() << "[UNIM-DBUS]" << __VA_ARGS__; \
        } \
    } while (0)

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

UnimDbusClient::UnimDbusClient(const QString &clientName)
    : m_bus(QDBusConnection::sessionBus())
    , m_isComposing(false)
    , m_connected(false)
{
    unim_dbus_check_debug_env();
    
    if (!m_bus.isConnected()) {
        UNIM_DBUS_DEBUG("DBus 세션 버스 연결 실패");
        return;
    }
    
    UNIM_DBUS_DEBUG("DBus 세션 버스 연결 성공");
    
    // InputContext 생성 요청
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        UNIM_DBUS_PATH,
        UNIM_DBUS_INTERFACE,
        QStringLiteral("CreateInputContext")
    );
    msg << clientName;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG("CreateInputContext 실패:" << reply.errorMessage());
        return;
    }
    
    if (reply.arguments().size() > 0) {
        m_contextPath = reply.arguments().at(0).toString();
        m_connected = true;
        UNIM_DBUS_DEBUG("InputContext 생성:" << m_contextPath);
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
    UNIM_DBUS_DEBUG("InputContext 파괴:" << m_contextPath);
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
    
    UNIM_DBUS_DEBUG("ProcessKeyEvent: keyval=" << keyval << "keycode=" << keycode << "state=" << state);
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("ProcessKeyEvent")
    );
    msg << keyval << keycode << state;
    
    QDBusMessage reply = m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
    if (reply.type() == QDBusMessage::ErrorMessage) {
        UNIM_DBUS_DEBUG("ProcessKeyEvent 실패:" << reply.errorMessage());
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
        
        UNIM_DBUS_DEBUG("ProcessKeyEvent 결과: consumed=" << result.consumed 
                        << "preedit=" << result.preedit << "commit=" << result.commit);
    }
    
    return result;
}

void UnimDbusClient::focusIn()
{
    if (!isValid()) return;
    
    UNIM_DBUS_DEBUG("FocusIn");
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("FocusIn")
    );
    
    m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
}

QString UnimDbusClient::focusOut()
{
    QString commitStr;
    
    if (!isValid()) return commitStr;
    
    UNIM_DBUS_DEBUG("FocusOut");
    
    // 조합 중인 문자가 있으면 반환
    if (m_isComposing && !m_preeditCache.isEmpty()) {
        commitStr = m_preeditCache;
        UNIM_DBUS_DEBUG("FocusOut 커밋:" << commitStr);
    }
    
    QDBusMessage msg = QDBusMessage::createMethodCall(
        UNIM_DBUS_SERVICE,
        m_contextPath,
        UNIM_DBUS_IC_INTERFACE,
        QStringLiteral("FocusOut")
    );
    
    m_bus.call(msg, QDBus::Block, UNIM_DBUS_TIMEOUT_MS);
    
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
        UNIM_DBUS_DEBUG("Reset 커밋:" << commitStr);
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
