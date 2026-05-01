/**
 * UNIM DBus Client for Qt IM Modules
 *
 * QtDBus를 사용하여 unim-daemon과 통신하는 클라이언트 헬퍼입니다.
 * Qt5/Qt6 IM 모듈에서 공통으로 사용됩니다.
 */

#ifndef UNIM_DBUS_CLIENT_HPP
#define UNIM_DBUS_CLIENT_HPP

#include <QString>
#include <QStringList>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusMessage>
#include <QDBusReply>
#include <QObject>
#include <memory>
#include <functional>

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
    bool consumed;              /* 키가 소비되었는지 */
    QString preedit;            /* preedit 문자열 */
    QString commit;             /* commit 문자열 */
};

/**
 * 한자 후보 항목
 */
struct UnimHanjaCandidate {
    QString hanja;    /* 한자 문자열 */
    QString meaning;  /* 뜻풀이 */
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

    /**
     * 커서 위치 보고 (팝업 포지셔닝용)
     * @param x 커서 X 좌표
     * @param y 커서 Y 좌표
     * @param width 커서 너비
     * @param height 커서 높이
     */
    void reportCursorRect(int x, int y, int width, int height);
    
    /**
     * 한자 후보 목록 조회
     * @param target 변환 대상 문자열 (출력)
     * @param candidates 후보 목록 (출력)
     * @return 성공 시 true
     */
    bool getHanjaCandidates(QString &target, QList<UnimHanjaCandidate> &candidates);
    
    /**
     * 한자 선택
     * @param index 선택할 인덱스 (0부터)
     * @param selectedHanja 선택된 한자 (출력)
     * @return 성공 시 true
     */
    bool selectHanja(quint32 index, QString &selectedHanja);
    
    /**
     * 한자 모드 취소
     */
    QString cancelHanja();
    
    /**
     * 특수문자 후보 목록 조회
     * @param target 변환 대상 초성 (출력)
     * @param characters 특수문자 배열 (출력)
     * @param topRow 영문 키맵 상단 행 레이블 (출력, 예: "QWERTYUIO")
     * @return 성공 시 true
     */
    bool getSpecialCharCandidates(QString &target, QStringList &characters, QString &topRow);
    
    /**
     * 특수문자 선택
     * @param index 선택할 인덱스 (0부터)
     * @param selectedChar 선택된 특수문자 (출력)
     * @return 성공 시 true
     */
    bool selectSpecialChar(quint32 index, QString &selectedChar);
    
    /**
     * 특수문자 모드 취소
     */
    QString cancelSpecialChar();

    /**
     * 입력 필드 목적 설정 (비밀번호/PIN 감지용)
     * @param purpose 목적 (0=Normal, 1=Password, 2=Pin, ...)
     */
    void setContentType(quint32 purpose);

    /**
     * Surrounding text 설정
     * @param text 텍스트
     * @param cursorPos 커서 위치 (문자 단위)
     * @param anchorPos 앵커 위치 (문자 단위)
     */
    void setSurroundingText(const QString &text, quint32 cursorPos, quint32 anchorPos);

    /**
     * AutoTypeFix 콜백 설정
     * @param callback (delete_chars, commit_text, preedit_text)를 받는 콜백
     */
    using AutoTypeFixCallback = std::function<void(quint32 deleteChars, const QString &commitText, const QString &preeditText)>;
    void setAutoTypeFixCallback(AutoTypeFixCallback callback);

    /**
     * CommitText 콜백 설정 (Standalone 팝업 마우스 클릭 커밋용)
     * @param callback 커밋할 텍스트를 받는 콜백
     */
    using CommitTextCallback = std::function<void(const QString &text)>;
    void setCommitTextCallback(CommitTextCallback callback);

    /**
     * 현재 한자 후보 목록의 즐겨찾기 상태 조회
     * @param states 상태 배열 (출력, candidates와 동일 순서)
     * @return 성공 시 true
     */
    bool getHanjaBookmarkStates(QList<bool> &states);

    /**
     * 한자 후보 즐겨찾기 토글 (엔진이 persist + 시그널 발행)
     * @param index 토글할 후보 인덱스
     * @return 성공 시 true
     */
    bool toggleHanjaBookmark(quint32 index);

    /**
     * HanjaBookmarkChanged 콜백 설정
     * @param callback (index, bookmarked)를 받는 콜백
     */
    using HanjaBookmarkChangedCallback = std::function<void(quint32 index, bool bookmarked)>;
    void setHanjaBookmarkChangedCallback(HanjaBookmarkChangedCallback callback);

    /**
     * HanjaCandidatesReordered 콜백 설정.
     *
     * 시그널 시그니처: (s, as, as, ab, u, i, i, i, b)
     *   target, hanjas[], meanings[], bookmarks[], new_cursor, page, sel_row, sel_col, bookmarked
     *
     * 콜백은 후보·즐겨찾기·커서를 한 호출로 일괄 교체하기 위한 payload를 받는다.
     */
    using HanjaCandidatesReorderedCallback = std::function<void(
        const QString &target,
        const QList<UnimHanjaCandidate> &candidates,
        const QList<bool> &bookmarks,
        quint32 newCursor,
        qint32 page,
        qint32 selRow,
        qint32 selCol,
        bool bookmarked)>;
    void setHanjaCandidatesReorderedCallback(HanjaCandidatesReorderedCallback callback);

private:
    QDBusConnection m_bus;
    QString m_contextPath;
    QString m_preeditCache;
    bool m_isComposing;
    bool m_connected;
    AutoTypeFixCallback m_autoTypeFixCallback;
    CommitTextCallback m_commitTextCallback;
    HanjaBookmarkChangedCallback m_hanjaBookmarkCallback;
    HanjaCandidatesReorderedCallback m_hanjaReorderedCallback;

    friend class UnimAutoTypeFixReceiver;
    friend class UnimCommitTextReceiver;
    friend class UnimHanjaBookmarkReceiver;
    friend class UnimHanjaCandidatesReorderedReceiver;
};

/**
 * AutoTypeFix DBus 시그널 수신 헬퍼 (QObject 필요)
 */
class UnimAutoTypeFixReceiver : public QObject {
    Q_OBJECT
public:
    explicit UnimAutoTypeFixReceiver(UnimDbusClient *client, QObject *parent = nullptr)
        : QObject(parent), m_client(client) {}
public slots:
    void onAutoTypefixApply(quint32 deleteChars, const QString &commitText, const QString &preeditText);
private:
    UnimDbusClient *m_client;
};

/**
 * CommitText DBus 시그널 수신 헬퍼 (QObject 필요)
 */
class UnimCommitTextReceiver : public QObject {
    Q_OBJECT
public:
    explicit UnimCommitTextReceiver(UnimDbusClient *client, QObject *parent = nullptr)
        : QObject(parent), m_client(client) {}
public slots:
    void onCommitText(const QString &text);
private:
    UnimDbusClient *m_client;
};

/**
 * HanjaBookmarkChanged DBus 시그널 수신 헬퍼 (QObject 필요)
 */
class UnimHanjaBookmarkReceiver : public QObject {
    Q_OBJECT
public:
    explicit UnimHanjaBookmarkReceiver(UnimDbusClient *client, QObject *parent = nullptr)
        : QObject(parent), m_client(client) {}
public slots:
    void onHanjaBookmarkChanged(quint32 index, bool bookmarked);
private:
    UnimDbusClient *m_client;
};

/**
 * HanjaCandidatesReordered DBus 시그널 수신 헬퍼 (QObject 필요)
 *
 * 시그니처: (s, as, as, ab, u, i, i, i, b). QtDBus는 ab(bool 배열)을
 * QDBusArgument로 디마샬링하므로 본 receiver는 generic QDBusMessage 슬롯으로
 * 받아 직접 파싱한다 (GTK 측 패턴과 동일).
 */
class UnimHanjaCandidatesReorderedReceiver : public QObject {
    Q_OBJECT
public:
    explicit UnimHanjaCandidatesReorderedReceiver(UnimDbusClient *client, QObject *parent = nullptr)
        : QObject(parent), m_client(client) {}
public slots:
    void onReordered(const QDBusMessage &msg);
private:
    UnimDbusClient *m_client;
};

#endif /* UNIM_DBUS_CLIENT_HPP */
