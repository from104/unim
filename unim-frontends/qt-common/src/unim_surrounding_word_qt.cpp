/**
 * UNIM Qt 공통 — 한자 단어 surrounding 보고·교체·능력 선언 구현
 *
 * qt5/qt6 input_context.cpp 에서 옮겨 온 코드다. 동작은 옮기기 전과 같다.
 */

#include "unim_surrounding_word_qt.hpp"
#include "unim_dbus_client.hpp"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QInputMethodQueryEvent>
#include <QVariant>
#include <cstring>
#include <utility>

/* Qt::ImCursorPosition/ImAnchorPosition 은 QString 단위(UTF-16 코드유닛) 오프셋을
 * 반환하는데, 엔진 쪽 검증(§2.2.3)·대상② 판정은 문자(유니코드 스칼라) 개수를
 * 기대한다. text.left(pos) 로 접두를 자른 뒤 toUcs4() 로 실제 문자 수를 센다 —
 * pos 가 text 길이를 넘어도 QString::left() 가 안전하게 클램프한다
 * (HANJA_WORD_SPEC.md §4.5). */
static quint32 toCharOffset(const QString &text, int pos)
{
    if (pos <= 0) {
        return 0;
    }
    return static_cast<quint32>(text.left(pos).toUcs4().size());
}

UnimSurroundingWordQt::UnimSurroundingWordQt(UnimDbusClient *dbus,
                                             const quint32 *contentPurpose,
                                             LogFn log)
    : m_dbus(dbus)
    , m_contentPurpose(contentPurpose)
    , m_log(std::move(log))
{
}

/* surrounding 질의·전송. update() 는 값이 바뀐 때만(force=false), filterEvent 는 매 키마다
 * (force=true — 한자키 시점 송신을 포함하는 상위 집합, HANJA_WORD_SPEC v1.6 §4.5)
 * 부른다. 비번/PIN 필드는 재질의 결과를 버리고 ("",0,0) 만 보낸다(§2.8 프런트 이중 게이트).
 * ImAnchorPosition 이 invalid 면 anchor 를 cursor 로 두고(가짜 선택 방지) false 를 돌려준다. */
bool UnimSurroundingWordQt::push(QObject *focusObject, bool force)
{
    if (!focusObject || !m_dbus) return true;

    QString surrText;
    quint32 cursorChars = 0;
    quint32 anchorChars = 0;
    bool anchorValid = true;

    if (!unim_is_sensitive(*m_contentPurpose)) {
        QInputMethodQueryEvent surrQuery(
            Qt::ImSurroundingText | Qt::ImCursorPosition | Qt::ImAnchorPosition);
        QCoreApplication::sendEvent(focusObject, &surrQuery);
        surrText = surrQuery.value(Qt::ImSurroundingText).toString();
        cursorChars = toCharOffset(surrText, surrQuery.value(Qt::ImCursorPosition).toInt());
        const QVariant anchorVar = surrQuery.value(Qt::ImAnchorPosition);
        anchorValid = anchorVar.isValid();
        anchorChars = anchorValid ? toCharOffset(surrText, anchorVar.toInt()) : cursorChars;
    }

    /* 빈 텍스트도 전송(§4.5) — stale 선택 제거 목적. 값이 안 바뀌면 스팸 방지(force 제외). */
    if (force ||
        surrText != m_textCache ||
        cursorChars != m_cursorCache ||
        anchorChars != m_anchorCache) {
        m_textCache = surrText;
        m_cursorCache = cursorChars;
        m_anchorCache = anchorChars;
        m_dbus->setSurroundingText(surrText, cursorChars, anchorChars);
    }
    return anchorValid;
}

void UnimSurroundingWordQt::pushOnKey(QObject *focusObject)
{
    if (!push(focusObject, true)) {
        m_dbus->surroundingHint(5 /* SELECTION_UNKNOWN */, 0, 0);
    }
}

void UnimSurroundingWordQt::declareCapabilities()
{
    if (m_dbus) {
        m_dbus->setClientCapabilities(0x3);
    }
}

void UnimSurroundingWordQt::pushFocusInSnapshot(QObject *object, quint32 purpose)
{
    if (!m_dbus) return;

    QString surroundingText;
    int cursorPos = 0;
    int anchorPos = 0;
    /* 비번/PIN 필드는 질의하지 않고 빈 값만 보낸다(§2.8 프런트 이중 게이트) —
     * push() 와 같은 규칙. 여기가 빠지면 포커스 진입 순간 평문이 한 번 나간다. */
    if (!unim_is_sensitive(purpose)) {
        QInputMethodQueryEvent stQuery(
            Qt::ImSurroundingText | Qt::ImCursorPosition | Qt::ImAnchorPosition);
        QCoreApplication::sendEvent(object, &stQuery);
        surroundingText = stQuery.value(Qt::ImSurroundingText).toString();
        cursorPos = stQuery.value(Qt::ImCursorPosition).toInt();
        anchorPos = stQuery.value(Qt::ImAnchorPosition).toInt();
    }
    /* 빈 텍스트도 전송한다(HANJA_WORD_SPEC.md §4.5). DBus 컨텍스트는 창 단위
     * (focusIn(m_windowId))라, 같은 창 안에서 선택이 있던 위젯에서 빈 위젯으로
     * 포커스가 옮겨갈 때 미전송이면 이전 위젯의 선택 스냅샷이 stale 로 남아
     * 대상②(선택 영역)를 오판한다. 여기서는 dedupe 캐시를 건드리지 않는다 —
     * 다음 update() 가 문자 단위로 변환한 값을 한 번 더 보내 오프셋 단위까지
     * 바로잡게 둔다(§7.1). */
    m_dbus->setSurroundingText(surroundingText,
                               static_cast<quint32>(cursorPos),
                               static_cast<quint32>(anchorPos));
}

/* 데몬 캐시가 낡았을 수 있으므로 surrounding 을 다시 묻고, 선택이 없으며 구간이
 * expect 와 완전 일치할 때만 교체한다. 로그에는 a·b·길이·이유만 남긴다(평문 금지). */
void UnimSurroundingWordQt::installReplaceSurroundingCallback()
{
    m_dbus->setReplaceSurroundingCallback([this](quint32 a, quint32 b,
                                                 const QString &commitText,
                                                 const QString &expectText) {
        auto drop = [&](const char *reason) {
            log(QString::asprintf("대상③ 교체 버림: a=%u, b=%u, commit=%d자, reason=%s",
                a, b, static_cast<int>(commitText.toUcs4().size()), reason));
        };
        QObject *focusObj = QGuiApplication::focusObject();
        if (!focusObj) { drop("no_focus"); return; }
        const char *className = focusObj->metaObject()->className();
        if (className && std::strstr(className, "TerminalDisplay") != nullptr) {
            drop("terminal"); return;
        }
        if (unim_is_sensitive(*m_contentPurpose)) { drop("sensitive"); return; }
        if (commitText.isEmpty() || expectText.isEmpty()) { drop("verify_mismatch"); return; }

        // surrounding 재조회 — anchor 가 유효하고 cursor 와 다르면 선택이 있다
        QInputMethodQueryEvent q(
            Qt::ImSurroundingText | Qt::ImCursorPosition | Qt::ImAnchorPosition);
        QCoreApplication::sendEvent(focusObj, &q);
        const QString text = q.value(Qt::ImSurroundingText).toString();
        const QVariant cursorVar = q.value(Qt::ImCursorPosition);
        const QVariant anchorVar = q.value(Qt::ImAnchorPosition);
        if (!cursorVar.isValid()) { drop("verify_mismatch"); return; }
        if (anchorVar.isValid() && anchorVar.toInt() != cursorVar.toInt()) {
            drop("selection"); return;
        }

        // 코드포인트(문자) 단위 대조 — text[c-a, c+b) == expect
        const auto textUcs = text.toUcs4();
        const auto expectUcs = expectText.toUcs4();
        const quint32 c = toCharOffset(text, cursorVar.toInt());
        const quint32 total = static_cast<quint32>(textUcs.size());
        if (c < a || c > total || b > total - c
            || static_cast<quint32>(expectUcs.size()) != a + b) {
            drop("verify_mismatch"); return;
        }
        for (quint32 i = 0; i < a + b; ++i) {
            if (textUcs[static_cast<int>(c - a + i)] != expectUcs[static_cast<int>(i)]) {
                drop("verify_mismatch"); return;
            }
        }

        // 앞 a자·뒤 b자의 UTF-16 길이(BMP 밖 문자는 2단위)
        int aU16 = 0, bU16 = 0;
        for (quint32 i = 0; i < a + b; ++i) {
            const int units = (expectUcs[static_cast<int>(i)] > 0xFFFFu) ? 2 : 1;
            (i < a ? aU16 : bU16) += units;
        }

        QInputMethodEvent ev;
        ev.setCommitString(commitText, -aU16, aU16 + bU16);
        QCoreApplication::sendEvent(focusObj, &ev);
        log(QString::asprintf("대상③ 교체 적용: a=%u, b=%u, commit=%d자",
            a, b, static_cast<int>(commitText.toUcs4().size())));
    });
}
