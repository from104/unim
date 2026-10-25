/**
 * UNIM Qt 공통 — 한자 단어(HANJA_WORD_SPEC) surrounding 보고·교체·능력 선언
 *
 * Qt5·Qt6 input_context.cpp 가 이름만 다른 복제본으로 들고 있던 블록을 한 곳으로
 * 모았다. 두 플러그인은 이 클래스에 위임만 한다. Qt5/Qt6 API 차이는 쓰는 범위에서
 * 없다(QInputMethodQueryEvent·QInputMethodEvent·QString::toUcs4 는 양쪽 동일).
 *
 * - push()            : surrounding 질의·전송(update 는 force=false, 키 시점은 true)
 * - pushOnKey()       : 키 시점 송신 + anchor 불명이면 SELECTION_UNKNOWN 힌트
 * - declareCapabilities() / pushFocusInSnapshot() : FocusIn 시 능력 선언·초기 스냅샷
 * - installReplaceSurroundingCallback() : ReplaceSurroundingText(대상③) 대조/실행
 *
 * 로그는 호출한 플러그인의 로거(태그 QT5_IM / QT6_IM)로 위임한다.
 */

#ifndef UNIM_SURROUNDING_WORD_QT_HPP
#define UNIM_SURROUNDING_WORD_QT_HPP

#include <QObject>
#include <QString>
#include <QtGlobal>
#include <functional>

class UnimDbusClient;

/* 민감 필드(비밀번호/PIN) 여부 — purpose 1=Password, 2=Pin(ContentPurpose). */
inline bool unim_is_sensitive(quint32 purpose)
{
    return purpose == 1 || purpose == 2;
}

/* dev 로그용 내용 표기 — 사용자 입력 평문은 남기지 않고 길이("(N자)")만 찍는다
 * (HANJA_WORD_SPEC §2.8). 민감 필드(비밀번호/PIN)는 "***".
 * QString 을 돌려줘 호출부의 qPrintable() 임시 수명 문제를 피한다. */
inline QString unim_mask(quint32 purpose, const QString &text)
{
    return unim_is_sensitive(purpose)
               ? QStringLiteral("***")
               : QStringLiteral("(%1자)").arg(text.toUcs4().size());
}

class UnimSurroundingWordQt
{
public:
    using LogFn = std::function<void(const QString &)>;

    /* dbus·contentPurpose 는 소유하지 않는다 — 호출한 input context 가 이 객체보다
     * 오래 산다. contentPurpose 는 input context 의 m_contentPurpose 를 가리킨다
     * (포커스 진입·mid-focus 에서 갱신되며 여기서는 읽기만 한다). */
    UnimSurroundingWordQt(UnimDbusClient *dbus, const quint32 *contentPurpose, LogFn log);

    /* surrounding 질의·전송. force=false 면 dedupe 캐시와 같을 때 보내지 않는다.
     * 비번/PIN 필드는 질의하지 않고 ("",0,0) 만 보낸다(§2.8 프런트 이중 게이트).
     * 반환: ImAnchorPosition 이 유효했는지(false 면 선택 여부 불명). */
    bool push(QObject *focusObject, bool force);

    /* 키 시점 송신(HANJA_WORD_SPEC v1.6 §2.4.6, I6). anchor 를 알 수 없으면
     * SELECTION_UNKNOWN(5) 힌트로 다음 한자키 한 번의 대상③ 을 막는다. */
    void pushOnKey(QObject *focusObject);

    /* 대상③ 능력 선언 — 교체 가능(0x1) + 선택 신뢰(0x2). FocusIn 마다 보낸다(멱등). */
    void declareCapabilities();

    /* 포커스 진입 직후 초기 스냅샷. dedupe 캐시는 건드리지 않는다 — 다음 update() 가
     * 문자 단위로 변환한 값을 한 번 더 보내 오프셋 단위까지 바로잡게 둔다(§7.1). */
    void pushFocusInSnapshot(QObject *object, quint32 purpose);

    /* ReplaceSurroundingText 시그널 콜백 등록(대상③ 커서 주변 단어 교체, v1.6). */
    void installReplaceSurroundingCallback();

private:
    void log(const QString &msg) const { if (m_log) m_log(msg); }

    UnimDbusClient *m_dbus;
    const quint32 *m_contentPurpose;
    LogFn m_log;

    /* update(Qt::ImSurroundingText|...) 재질의 dedupe 캐시(한자 단어 대상②,
     * HANJA_WORD_SPEC.md §4.5) — 값이 실제로 바뀐 경우에만 DBus 로 보낸다. */
    QString m_textCache;
    quint32 m_cursorCache = 0;
    quint32 m_anchorCache = 0;
};

#endif // UNIM_SURROUNDING_WORD_QT_HPP
