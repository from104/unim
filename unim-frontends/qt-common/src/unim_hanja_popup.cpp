/**
 * UNIM 한자 후보 팝업 구현 (Qt)
 *
 * 한자 변환 시 후보 목록을 표시하는 팝업 윈도우입니다.
 */

#include "unim_hanja_popup.hpp"
#include <QApplication>
#include <QStyle>
#include <QPainter>
#include <QDebug>
#include <QDateTime>
#include <QStandardPaths>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 */
static bool popup_debug_enabled = false;
static bool popup_debug_checked = false;

static void popup_log(const char *module, const QString &message)
{
    if (!popup_debug_enabled) return;

    QString timestamp = QDateTime::currentDateTime().toString("yyyy/MM/dd hh:mm:ss");
    QString logLine = QString("[%1] - [%2] - %3").arg(timestamp, module, message);
    qDebug().noquote() << logLine;

    QString logPath = QStandardPaths::writableLocation(QStandardPaths::HomeLocation) + "/.unim-errors.log";
    QFile file(logPath);
    if (file.open(QIODevice::Append | QIODevice::Text)) {
        QTextStream out(&file);
        out << logLine << "\n";
        file.close();
    }
}

#define POPUP_DEBUG(...) \
    popup_log("HANJA_POPUP", QString(__VA_ARGS__))

static void popup_check_debug()
{
    if (!popup_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            popup_debug_enabled = true;
        }
        popup_debug_checked = true;
    }
}

UnimHanjaPopup::UnimHanjaPopup(QWidget *parent)
    : QWidget(parent, Qt::ToolTip | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint)
    , m_layout(nullptr)
    , m_pageLabel(nullptr)
    , m_currentPage(0)
    , m_selectedIndex(0)
{
    popup_check_debug();

    /* 기본 스타일 설정 */
    setStyleSheet(
        "UnimHanjaPopup {"
        "  background-color: #2d2d2d;"
        "  border: 1px solid #555;"
        "  border-radius: 4px;"
        "  padding: 4px;"
        "}"
        "QLabel {"
        "  color: #e0e0e0;"
        "  padding: 2px 8px;"
        "  font-size: 14px;"
        "}"
        "QLabel[selected=\"true\"] {"
        "  background-color: #4a90d9;"
        "  color: white;"
        "  border-radius: 2px;"
        "}"
        "QLabel#pageLabel {"
        "  color: #888;"
        "  font-size: 11px;"
        "  padding: 2px 4px;"
        "}"
    );

    m_layout = new QVBoxLayout(this);
    m_layout->setContentsMargins(4, 4, 4, 4);
    m_layout->setSpacing(1);

    /* 후보 레이블 생성 */
    for (int i = 0; i < MAX_VISIBLE_CANDIDATES; i++) {
        m_labels[i] = new QLabel(this);
        m_labels[i]->setVisible(false);
        m_labels[i]->setCursor(Qt::PointingHandCursor);
        m_layout->addWidget(m_labels[i]);
    }

    /* 페이지 레이블 */
    m_pageLabel = new QLabel(this);
    m_pageLabel->setObjectName("pageLabel");
    m_pageLabel->setAlignment(Qt::AlignCenter);
    m_pageLabel->setVisible(false);
    m_layout->addWidget(m_pageLabel);

    setLayout(m_layout);
    hide();

    POPUP_DEBUG("한자 팝업 생성 완료");
}

UnimHanjaPopup::~UnimHanjaPopup()
{
    POPUP_DEBUG("한자 팝업 소멸");
}

void UnimHanjaPopup::showPopup(const QString &target,
                                const QList<UnimHanjaCandidate> &candidates,
                                int x, int y, int cursorHeight,
                                UnimHanjaSelectCallback callback)
{
    if (candidates.isEmpty()) return;

    m_candidates = candidates;
    m_callback = std::move(callback);
    m_currentPage = 0;
    m_selectedIndex = 0;

    updateList();
    adjustPosition(x, y, cursorHeight);
    show();

    POPUP_DEBUG(QString::asprintf("한자 팝업 표시: target='%s', count=%d",
                qPrintable(target), candidates.size()));
}

void UnimHanjaPopup::hidePopup()
{
    hide();
    m_candidates.clear();
    m_callback = nullptr;
    POPUP_DEBUG("한자 팝업 숨김");
}

bool UnimHanjaPopup::handleKey(int key)
{
    if (!isVisible()) return false;

    switch (key) {
    case Qt::Key_1: case Qt::Key_2: case Qt::Key_3:
    case Qt::Key_4: case Qt::Key_5: case Qt::Key_6:
    case Qt::Key_7: case Qt::Key_8: case Qt::Key_9:
    {
        int idx = key - Qt::Key_1;
        int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
        int pageCount = qMin(static_cast<int>(MAX_VISIBLE_CANDIDATES),
                             m_candidates.size() - pageStart);
        if (idx < pageCount) {
            selectCandidate(pageStart + idx);
        }
        return true;
    }

    case Qt::Key_Down:
    {
        int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
        int pageCount = qMin(static_cast<int>(MAX_VISIBLE_CANDIDATES),
                             m_candidates.size() - pageStart);
        m_selectedIndex = (m_selectedIndex + 1) % pageCount;
        updateList();
        return true;
    }

    case Qt::Key_Up:
    {
        int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
        int pageCount = qMin(static_cast<int>(MAX_VISIBLE_CANDIDATES),
                             m_candidates.size() - pageStart);
        m_selectedIndex = (m_selectedIndex - 1 + pageCount) % pageCount;
        updateList();
        return true;
    }

    case Qt::Key_Return:
    case Qt::Key_Enter:
    {
        int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
        selectCandidate(pageStart + m_selectedIndex);
        return true;
    }

    /* 이전 페이지 */
    case Qt::Key_Left:
    case Qt::Key_PageUp:
    case Qt::Key_Backspace:
        prevPage();
        return true;

    /* 다음 페이지 */
    case Qt::Key_Right:
    case Qt::Key_PageDown:
    case Qt::Key_Space:
        nextPage();
        return true;

    /* 모디파이어 키 → 소비 (팝업 유지, 앱에 전달하지 않음) */
    case Qt::Key_Shift:
    case Qt::Key_Control:
    case Qt::Key_Alt:
    case Qt::Key_Meta:
    case Qt::Key_Super_L:
    case Qt::Key_Super_R:
    case Qt::Key_AltGr:
    case Qt::Key_CapsLock:
    case Qt::Key_NumLock:
    case Qt::Key_ScrollLock:
        return true;

    /* Escape, 기타 미지원 키 → false (input_context에서 처리) */
    default:
        return false;
    }
}

void UnimHanjaPopup::updateList()
{
    int totalPages = (m_candidates.size() + MAX_VISIBLE_CANDIDATES - 1) / MAX_VISIBLE_CANDIDATES;
    int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
    int pageCount = qMin(static_cast<int>(MAX_VISIBLE_CANDIDATES),
                         m_candidates.size() - pageStart);

    for (int i = 0; i < MAX_VISIBLE_CANDIDATES; i++) {
        if (i < pageCount) {
            const UnimHanjaCandidate &cand = m_candidates.at(pageStart + i);
            QString text = QString("%1. %2  %3").arg(i + 1).arg(cand.hanja, cand.meaning);
            m_labels[i]->setText(text);
            m_labels[i]->setProperty("selected", i == m_selectedIndex);
            m_labels[i]->setVisible(true);
            /* 스타일 새로고침 */
            m_labels[i]->style()->unpolish(m_labels[i]);
            m_labels[i]->style()->polish(m_labels[i]);
        } else {
            m_labels[i]->setVisible(false);
        }
    }

    /* 페이지 표시 */
    if (totalPages > 1) {
        m_pageLabel->setText(QString("%1/%2").arg(m_currentPage + 1).arg(totalPages));
        m_pageLabel->setVisible(true);
    } else {
        m_pageLabel->setVisible(false);
    }

    adjustSize();
}

void UnimHanjaPopup::selectCandidate(int index)
{
    if (index < 0 || index >= m_candidates.size()) return;

    QString hanja = m_candidates.at(index).hanja;
    POPUP_DEBUG(QString::asprintf("한자 선택: index=%d, hanja='%s'", index, qPrintable(hanja)));

    if (m_callback) {
        m_callback(hanja);
    }

    hidePopup();
}

void UnimHanjaPopup::nextPage()
{
    int totalPages = (m_candidates.size() + MAX_VISIBLE_CANDIDATES - 1) / MAX_VISIBLE_CANDIDATES;
    if (m_currentPage < totalPages - 1) {
        m_currentPage++;
        m_selectedIndex = 0;
        updateList();
    }
}

void UnimHanjaPopup::prevPage()
{
    if (m_currentPage > 0) {
        m_currentPage--;
        m_selectedIndex = 0;
        updateList();
    }
}

void UnimHanjaPopup::adjustPosition(int x, int y, int cursorHeight)
{
    /* 팝업 크기 측정 */
    adjustSize();
    QSize popupSize = size();

    int finalX = x;
    int finalY = y;

    /* 화면 크기 가져오기 */
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        QRect screenRect = screen->availableGeometry();

        /* 오른쪽 넘침 보정 */
        if (finalX + popupSize.width() > screenRect.right()) {
            finalX = screenRect.right() - popupSize.width();
            if (finalX < screenRect.left()) finalX = screenRect.left();
        }

        /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 */
        if (finalY + popupSize.height() > screenRect.bottom()) {
            finalY = y - cursorHeight - popupSize.height();
            if (finalY < screenRect.top()) finalY = screenRect.top();
        }

        POPUP_DEBUG(QString::asprintf("화면 경계 보정: screen=(%d,%d,%d,%d), popup=(%d,%d), req=(%d,%d) -> final=(%d,%d)",
                    screenRect.left(), screenRect.top(), screenRect.width(), screenRect.height(),
                    x, y, popupSize.width(), popupSize.height(), finalX, finalY));
    }

    move(finalX, finalY);
}
