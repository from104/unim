/**
 * UNIM Qt5 Input Context 구현
 *
 * DBus를 통해 unim-daemon과 통신합니다.
 */

#include "input_context.hpp"
#include "unim_dbus_client.hpp"
#include "unim_hanja_popup.hpp"
#include "unim_special_popup.hpp"
#include "unim_emoji_popup.hpp"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QTextCharFormat>
#include <QDBusConnection>
#include <QDBusMessage>
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
    unim_log_message("QT5_IM", QString(__VA_ARGS__))

static void unim_check_debug_env()
{
    if (!unim_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            unim_log_message("QT5_IM", "디버그 모드 활성화 (UNIM_DEVELOP=1)");
        }
        unim_debug_checked = true;
    }
}


UnimInputContext::UnimInputContext()
    : m_dbus(nullptr)
    , m_hanjaPopup(nullptr)
    , m_specialPopup(nullptr)
    , m_emojiPopup(nullptr)
    , m_focusObject(nullptr)
    , m_composing(false)
{
    unim_check_debug_env();
    UNIM_DEBUG("UnimInputContext 생성 시작");
    
    // 창 식별자 생성 (컨텍스트 포인터 기반)
    m_windowId = QString::asprintf("qt5-ctx-%p", static_cast<void*>(this));
    
    m_dbus = new UnimDbusClient(QStringLiteral("qt5-unim"), m_windowId);

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
        // HanjaBookmarkChanged 시그널 → 팝업 별 갱신
        m_dbus->setHanjaBookmarkChangedCallback([this](quint32 index, bool bookmarked) {
            if (m_hanjaPopup) {
                m_hanjaPopup->setBookmark(index, bookmarked);
            }
        });
        // HanjaCandidatesReordered 시그널 → 후보·즐겨찾기·커서 일괄 교체
        // (item-tracking: 토글된 한자가 정렬 후 새 위치로 이동, cursor도 따라감)
        // Phase 7: was=true → now=false (★ 해제) 시 cursor 셀 yellow flash.
        m_dbus->setHanjaCandidatesReorderedCallback(
            [this](const QString &target,
                   const QList<UnimHanjaCandidate> &candidates,
                   const QList<bool> &bookmarks,
                   quint32 newCursor, qint32 page, qint32 selRow, qint32 selCol,
                   bool bookmarked, bool wasBookmarked) {
                Q_UNUSED(target);
                Q_UNUSED(newCursor);
                if (m_hanjaPopup) {
                    m_hanjaPopup->replaceCandidates(candidates, bookmarks,
                                                     page, selRow, selCol);
                    if (wasBookmarked && !bookmarked) {
                        m_hanjaPopup->flashCursorCell();
                    }
                }
            });

        /* PR #4: 이모지 팝업 시그널 콜백 — ShowEmojiPopupV2/PopupNavigate/HidePopup */
        m_dbus->setShowEmojiPopupCallback(
            [this](const QString &targetCatId,
                   const QStringList &items,
                   const QString &topRow,
                   const QStringList &recent,
                   const QList<UnimDbusClient::EmojiCategoryInfo> &categories,
                   qint32 cx, qint32 cy, qint32 cw, qint32 ch) {
                Q_UNUSED(cw);
                if (isStandalonePopup()) return;  /* GUI 팝업이 전담 */
                ensurePopups();
                if (!m_emojiPopup) return;

                /* 다른 팝업 숨김 */
                if (m_hanjaPopup) m_hanjaPopup->hidePopup();
                if (m_specialPopup) m_specialPopup->hidePopup();

                /* DBusClient 의 카테고리 메타 → 위젯 메타 (필드 동일, 타입만 분리) */
                QList<UnimEmojiCategoryInfo> cats;
                cats.reserve(categories.size());
                for (const auto &c : categories) {
                    UnimEmojiCategoryInfo info;
                    info.id = c.id;
                    info.nameKo = c.nameKo;
                    info.nameEn = c.nameEn;
                    info.count = c.count;
                    cats.append(info);
                }

                m_emojiPopup->showPopup(
                    targetCatId, items, topRow, recent, cats,
                    cx, cy, ch,
                    [this](const QString &emoji) {
                        if (m_dbus) m_dbus->commitEmoji(emoji);
                        if (m_emojiPopup) m_emojiPopup->hidePopup();
                    });
            });
        m_dbus->setPopupNavigateCallback(
            [this](qint32 page, qint32 totalPages, qint32 selected,
                   qint32 rows, qint32 cols, qint32 selRow, qint32 selCol) {
                if (m_emojiPopup && m_emojiPopup->isVisible()) {
                    m_emojiPopup->navigatePopup(page, totalPages, selected,
                                                 rows, cols, selRow, selCol);
                }
            });
        m_dbus->setHidePopupCallback([this]() {
            if (m_emojiPopup && m_emojiPopup->isVisible()) {
                m_emojiPopup->hidePopup();
            }
        });
    } else {
        UNIM_DEBUG("UnimInputContext 생성 (DBus 연결 실패)");
    }

    /* 팝업은 lazy 초기화 — QApplication 초기화 전에 QWidget 생성 불가 */
}

void UnimInputContext::ensurePopups()
{
    if (!m_hanjaPopup) {
        m_hanjaPopup = new UnimHanjaPopup();
        m_hanjaPopup->setToggleBookmarkCallback([this](quint32 globalIndex) {
            if (m_dbus) {
                m_dbus->toggleHanjaBookmark(globalIndex);
            }
        });
        /* ◀/▶ 풋터 → DBus PopupChangePage (Phase 5) */
        m_hanjaPopup->setPageChangeCallback([this](int direction) {
            if (m_dbus) m_dbus->popupChangePage(direction);
        });
    }
    if (!m_specialPopup) {
        m_specialPopup = new UnimSpecialPopup();
        m_specialPopup->setPageChangeCallback([this](int direction) {
            if (m_dbus) m_dbus->popupChangePage(direction);
        });
    }
    if (!m_emojiPopup) {
        m_emojiPopup = new UnimEmojiPopup();
        m_emojiPopup->setPageChangeCallback([this](int direction) {
            if (m_dbus) m_dbus->popupChangePage(direction);
        });
    }
}

bool UnimInputContext::isStandalonePopup() const
{
    QDBusMessage msg = QDBusMessage::createMethodCall(
        QStringLiteral("org.atit.unim.InputMethod"),
        QStringLiteral("/org/atit/unim/InputMethod"),
        QStringLiteral("org.atit.unim.InputMethod"),
        QStringLiteral("GetConfig")
    );
    msg << QStringLiteral("popup_mode");
    QDBusMessage reply = QDBusConnection::sessionBus().call(msg, QDBus::Block, 100);
    if (reply.type() == QDBusMessage::ReplyMessage &&
        reply.arguments().value(0).toString() == QStringLiteral("Standalone")) {
        return true;
    }
    return false;
}

UnimInputContext::~UnimInputContext()
{
    delete m_emojiPopup;
    m_emojiPopup = nullptr;
    delete m_specialPopup;
    m_specialPopup = nullptr;
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
            QString trigger = m_dbus->cancelHanja();
            if (!trigger.isEmpty()) commitString(trigger);
        }
    }

    /* 특수문자 팝업이 표시 중이면 닫기 */
    if (m_specialPopup && m_specialPopup->isVisible()) {
        m_specialPopup->hidePopup();
        if (m_dbus) {
            QString trigger = m_dbus->cancelSpecialChar();
            if (!trigger.isEmpty()) commitString(trigger);
        }
    }

    /* 이모지 팝업 — PR #4 (엔진이 reset 시 HidePopup 시그널 발행하지만 즉시 닫음) */
    if (m_emojiPopup && m_emojiPopup->isVisible()) {
        m_emojiPopup->hidePopup();
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
            QString trigger = m_dbus->cancelHanja();
            if (!trigger.isEmpty()) commitString(trigger);
        }
    }

    /* 특수문자 팝업이 표시 중이면 닫기 */
    if (m_specialPopup && m_specialPopup->isVisible()) {
        m_specialPopup->hidePopup();
        if (m_dbus) {
            QString trigger = m_dbus->cancelSpecialChar();
            if (!trigger.isEmpty()) commitString(trigger);
        }
    }

    /* 이모지 팝업 — PR #4 */
    if (m_emojiPopup && m_emojiPopup->isVisible()) {
        m_emojiPopup->hidePopup();
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

    /* 팝업 lazy 초기화 (QApplication 초기화 완료 후) */
    ensurePopups();

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
            QString trigger = m_dbus->cancelHanja();
            if (!trigger.isEmpty()) commitString(trigger);
        }
        m_hanjaPopup->hidePopup();

        /* 3. FocusIn으로 컨텍스트 복원 (FocusOut 후 필요) */
        if (m_dbus) {
            m_dbus->focusIn(m_windowId);
        }

        /* fall-through → 아래 processKey 경로에서 엔진이 새 키 처리 */
    }

    /* 특수문자 팝업이 표시 중이면 먼저 팝업에서 키 처리 */
    if (m_specialPopup && m_specialPopup->isVisible()) {
        /* Escape → 팝업 닫기 + 조합 복원 */
        if (key == Qt::Key_Escape) {
            UNIM_DEBUG("특수문자 팝업 Escape -> 조합 복원 + 팝업 닫기");

            if (m_dbus) {
                UnimDbusKeyResult resetResult = m_dbus->processKey(0, 0, 0);
                if (!resetResult.commit.isEmpty()) {
                    commitString(resetResult.commit);
                }
            }

            if (m_dbus) {
                m_dbus->cancelSpecialChar();
            }

            m_composing = m_dbus && m_dbus->isComposing();
            updatePreedit();
            m_specialPopup->hidePopup();
            return true;
        }

        /* 팝업 내부 처리 */
        if (m_specialPopup->handleKey(key)) {
            return true;
        }

        /* 미지원 키 → 조합 커밋 + 팝업 닫기 + fall-through */
        UNIM_DEBUG("특수문자 팝업 미지원 키 -> 조합 커밋 + 팝업 닫고 엔진에 키 전달");

        if (m_dbus) {
            QString commitText = m_dbus->focusOut();
            if (!commitText.isEmpty()) {
                commitString(commitText);
            }
        }

        m_composing = false;
        updatePreedit();

        if (m_dbus) {
            QString trigger = m_dbus->cancelSpecialChar();
            if (!trigger.isEmpty()) commitString(trigger);
        }
        m_specialPopup->hidePopup();

        if (m_dbus) {
            m_dbus->focusIn(m_windowId);
        }
    }

    /* 한자 키 처리 (F9 또는 Hangul_Hanja) */
    if (key == Qt::Key_F9 || key == Qt::Key_Hangul_Hanja) {
        if (m_dbus) {
            /* 먼저 한자 후보 조회 (start_hanja_conversion 트리거) */
            QString target;
            QList<UnimHanjaCandidate> candidates;
            if (m_hanjaPopup && m_dbus->getHanjaCandidates(target, candidates) && !candidates.isEmpty()) {
                int popupX = m_cursorRect.x();
                int popupY = m_cursorRect.y() + m_cursorRect.height() + 4;

                UNIM_DEBUG(QString::asprintf("한자 후보 표시: target='%s', count=%d, pos=(%d,%d)",
                           qPrintable(target), candidates.size(), popupX, popupY));

                if (!isStandalonePopup()) {
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

                    // 초기 즐겨찾기 상태 fetch
                    QList<bool> states;
                    if (m_dbus->getHanjaBookmarkStates(states)) {
                        m_hanjaPopup->setBookmarkStates(states);
                    }
                } else {
                    UNIM_DEBUG("popup_mode=Standalone, 한자 팝업 표시 생략");
                }
            } else {
                /* 한자 후보 없음 → 특수문자 후보 확인 */
                QString spTarget;
                QStringList spChars;
                QString spTopRow;
                if (m_specialPopup && m_dbus->getSpecialCharCandidates(spTarget, spChars, spTopRow) && !spChars.isEmpty()) {
                    int popupX = m_cursorRect.x();
                    int popupY = m_cursorRect.y() + m_cursorRect.height() + 4;

                    UNIM_DEBUG(QString::asprintf("특수문자 후보 표시: target='%s', count=%d",
                               qPrintable(spTarget), spChars.size()));

                    if (!isStandalonePopup()) {
                        m_specialPopup->showPopup(spTarget, spChars, spTopRow, popupX, popupY, m_cursorRect.height(),
                            [this](const QString &character) {
                                UNIM_DEBUG(QString::asprintf("특수문자 선택: '%s'", qPrintable(character)));
                                if (m_dbus) {
                                    m_dbus->cancelSpecialChar();
                                }
                                m_composing = false;
                                updatePreedit();
                                commitString(character);
                            });
                    } else {
                        UNIM_DEBUG("popup_mode=Standalone, 특수문자 팝업 표시 생략");
                    }
                } else {
                    UNIM_DEBUG("한자/특수문자 후보 없음 → idle Hanja: emoji 트리거 위임");
                    /* idle (preedit/조합 비어있음) → 엔진의 dual-purpose
                     * Hanja 분기가 emoji popup 트리거. ShowEmojiPopupV2
                     * signal handler 가 popup 을 띄운다. */
                    quint32 mod_state_emoji = 0;
                    if (keyEvent->modifiers() & Qt::ShiftModifier)   mod_state_emoji |= (1 << 0);
                    if (keyEvent->modifiers() & Qt::ControlModifier) mod_state_emoji |= (1 << 2);
                    if (keyEvent->modifiers() & Qt::AltModifier)     mod_state_emoji |= (1 << 3);
                    if (keyEvent->modifiers() & Qt::MetaModifier)    mod_state_emoji |= (1 << 26);
                    quint32 scanCode_emoji = keyEvent->nativeScanCode();
                    quint32 evdev_emoji = (scanCode_emoji > 8) ? (scanCode_emoji - 8) : 0;
                    /* idle Hanja 키는 InputResult::consumed() 반환 — commit/preedit
                     * 변동 없음. 응답값은 무시하고 RPC 호출만으로 엔진의 emoji
                     * popup state 진입을 유도. */
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

    /* 한자 팝업이 표시 중이면 닫기 */
    if (m_hanjaPopup && m_hanjaPopup->isVisible()) {
        UNIM_DEBUG("setFocusObject: 한자 팝업 닫기");
        m_hanjaPopup->hidePopup();
        if (m_dbus) {
            QString trigger = m_dbus->cancelHanja();
            if (!trigger.isEmpty()) commitString(trigger);
        }
    }

    /* 특수문자 팝업이 표시 중이면 닫기 */
    if (m_specialPopup && m_specialPopup->isVisible()) {
        UNIM_DEBUG("setFocusObject: 특수문자 팝업 닫기");
        m_specialPopup->hidePopup();
        if (m_dbus) {
            QString trigger = m_dbus->cancelSpecialChar();
            if (!trigger.isEmpty()) commitString(trigger);
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
        QTextCharFormat charFormat;
        charFormat.setUnderlineStyle(QTextCharFormat::SingleUnderline);
        attrs << QInputMethodEvent::Attribute(
            QInputMethodEvent::TextFormat,
            0,
            preeditStr.length(),
            charFormat
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
