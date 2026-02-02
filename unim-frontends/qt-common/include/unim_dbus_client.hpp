/**
 * UNIM DBus Client for Qt IM Modules
 *
 * QtDBus를 사용하여 unim-daemon과 통신하는 클라이언트 헬퍼입니다.
 * Qt5/Qt6 IM 모듈에서 공통으로 사용됩니다.
 */

#ifndef UNIM_DBUS_CLIENT_HPP
#define UNIM_DBUS_CLIENT_HPP

#include <QString>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <memory>

/* DBus 서비스 정보 */
#define UNIM_DBUS_SERVICE       "org.atit.unim.InputMethod"
#define UNIM_DBUS_PATH          "/org/atit/unim/InputMethod"
#define UNIM_DBUS_INTERFACE     "org.atit.unim.InputMethod"
#define UNIM_DBUS_IC_INTERFACE  "org.atit.unim.InputContext"

/* 타임아웃 (밀리초) */
#define UNIM_DBUS_TIMEOUT_MS    500

/**
 * 키 처리 결과
 */
struct UnimDbusKeyResult {
    bool consumed;       /* 키가 소비되었는지 */
    QString preedit;     /* preedit 문자열 */
    QString commit;      /* commit 문자열 */
};

/**
 * DBus 클라이언트 클래스
 */
class UnimDbusClient {
public:
    /**
     * 생성자
     * @param clientName 클라이언트 이름 (예: "qt6-unim")
     * @param windowId 창 식별자 (예: "qt6-ctx-0x12345")
     */
    explicit UnimDbusClient(const QString &clientName, const QString &windowId = QString());
    
    /**
     * 소멸자
     */
    ~UnimDbusClient();
    
    /**
     * 유효한 연결인지 확인
     */
    bool isValid() const;
    
    /**
     * 키 이벤트 처리
     * @param keyval GDK keyval (사용되지 않음, 향후 호환성용)
     * @param keycode 키코드 (evdev 형식)
     * @param state 수정자 상태 비트필드
     * @return 처리 결과
     */
    UnimDbusKeyResult processKey(quint32 keyval, quint32 keycode, quint32 state);
    
    /**
     * 포커스 획득 알림
     * @param windowId 창 식별자 (비어있으면 빈 문자열 전달)
     */
    void focusIn(const QString &windowId = QString());
    
    /**
     * 포커스 상실 알림
     * @return 커밋해야 할 문자열 (조합 중이었던 문자)
     */
    QString focusOut();
    
    /**
     * 입력 상태 초기화
     * @return 커밋해야 할 문자열 (조합 중이었던 문자)
     */
    QString reset();
    
    /**
     * 현재 preedit 문자열 가져오기
     */
    QString getPreedit() const;
    
    /**
     * 조합 중인지 확인
     */
    bool isComposing() const;
    
private:
    QDBusConnection m_bus;
    QString m_contextPath;
    QString m_preeditCache;
    bool m_isComposing;
    bool m_connected;
};

#endif /* UNIM_DBUS_CLIENT_HPP */
