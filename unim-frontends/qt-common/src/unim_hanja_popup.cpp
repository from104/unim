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
#include <QMouseEvent>
#include <cstdlib>
#include <cstring>
#include <vector>

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

    /* DPI 스케일 팩터 계산 (96 DPI 기준) */
    qreal scaleFactor = 1.0;
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        scaleFactor = screen->logicalDotsPerInch() / 96.0;
        if (scaleFactor < 1.0) scaleFactor = 1.0;
    }
    int fontSize = qRound(14 * scaleFactor);
    int pageFontSize = qRound(12 * scaleFactor);
    int minHeight = qRound(28 * scaleFactor);
    int padding = qRound(12 * scaleFactor);
    int labelPadV = qRound(2 * scaleFactor);
    int labelPadH = qRound(8 * scaleFactor);
    int pagePadH = qRound(4 * scaleFactor);

    /* Catppuccin Mocha 스타일 (DPI 스케일 적용) */
    setStyleSheet(QString(
        "UnimHanjaPopup {"
        "  background-color: rgba(30, 30, 46, 242);"
        "  border: 1px solid rgba(255, 255, 255, 38);"
        "  border-radius: 12px;"
        "  padding: %1px;"
        "}"
        "QLabel {"
        "  color: #cdd6f4;"
        "  padding: %2px %3px;"
        "  font-size: %4px;"
        "  min-height: %5px;"
        "  border-radius: 6px;"
        "}"
        "QLabel:hover {"
        "  background-color: rgba(255, 255, 255, 13);"
        "  border-radius: 6px;"
        "}"
        "QLabel[selected=\"true\"] {"
        "  background-color: rgba(137, 180, 250, 51);"
        "  color: #cdd6f4;"
        "  border-radius: 6px;"
        "}"
        "QLabel[selected=\"true\"]:hover {"
        "  background-color: rgba(137, 180, 250, 64);"
        "  color: #cdd6f4;"
        "  border-radius: 6px;"
        "}"
        "QLabel#pageLabel {"
        "  color: #6c7086;"
        "  font-size: %6px;"
        "  padding: %2px %7px;"
        "  min-height: 0px;"
        "}"
    ).arg(padding).arg(labelPadV).arg(labelPadH).arg(fontSize)
     .arg(minHeight).arg(pageFontSize).arg(pagePadH));

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
    if (m_popupState) {
        unim_popup_free(m_popupState);
        m_popupState = nullptr;
    }
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

    /* PopupState 생성 */
    int count = m_candidates.size();
    std::vector<QByteArray> hanjaBytes(count), meaningBytes(count);
    std::vector<const uint8_t*> hanjaPtrs(count), meaningPtrs(count);
    std::vector<size_t> hanjaLens(count), meaningLens(count);
    for (int i = 0; i < count; i++) {
        hanjaBytes[i] = m_candidates[i].hanja.toUtf8();
        meaningBytes[i] = m_candidates[i].meaning.toUtf8();
        hanjaPtrs[i] = (const uint8_t*)hanjaBytes[i].constData();
        hanjaLens[i] = hanjaBytes[i].size();
        meaningPtrs[i] = (const uint8_t*)meaningBytes[i].constData();
        meaningLens[i] = meaningBytes[i].size();
    }
    QByteArray targetBytes = target.toUtf8();
    if (m_popupState) unim_popup_free(m_popupState);
    m_popupState = unim_popup_new_hanja(
        (const uint8_t*)targetBytes.constData(), targetBytes.size(),
        hanjaPtrs.data(), hanjaLens.data(),
        meaningPtrs.data(), meaningLens.data(), count
    );

    updateList();
    adjustPosition(x, y, cursorHeight);
    show();

    POPUP_DEBUG(QString::asprintf("한자 팝업 표시: target='%s', count=%d",
                qPrintable(target), static_cast<int>(candidates.size())));
}

void UnimHanjaPopup::hidePopup()
{
    hide();
    m_candidates.clear();
    m_callback = nullptr;
    if (m_popupState) {
        unim_popup_free(m_popupState);
        m_popupState = nullptr;
    }
    POPUP_DEBUG("한자 팝업 숨김");
}

bool UnimHanjaPopup::handleKey(int key)
{
    if (!isVisible() || !m_popupState) return false;

    uint32_t popupKey = unim_popup_key_from_qt(key);
    UnimPopupKeyResult result = unim_popup_handle_key(m_popupState, popupKey);

    switch (result.kind) {
    case UNIM_POPUP_RESULT_SELECT:
        selectCandidate(result.selected_index);
        return true;

    case UNIM_POPUP_RESULT_CANCEL:
        hidePopup();
        return false;

    case UNIM_POPUP_RESULT_UPDATED:
        m_currentPage = unim_popup_get_current_page(m_popupState);
        m_selectedIndex = unim_popup_get_sel_row(m_popupState);
        updateList();
        return true;

    case UNIM_POPUP_RESULT_CONSUMED:
        return true;

    case UNIM_POPUP_RESULT_NOT_HANDLED:
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
    if (totalPages > 1) {
        if (m_currentPage < totalPages - 1) {
            m_currentPage++;
        } else {
            m_currentPage = 0;
        }
        m_selectedIndex = 0;
        updateList();
    }
}

void UnimHanjaPopup::prevPage()
{
    int totalPages = (m_candidates.size() + MAX_VISIBLE_CANDIDATES - 1) / MAX_VISIBLE_CANDIDATES;
    if (totalPages > 1) {
        if (m_currentPage > 0) {
            m_currentPage--;
        } else {
            m_currentPage = totalPages - 1;
        }
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
            finalX = screenRect.right() - popupSize.width() - 4;
            if (finalX < screenRect.left()) finalX = screenRect.left();
        }

        /* 아래쪽 넘침 보정: 커서(preedit) 위로 올림 */
        if (finalY + popupSize.height() > screenRect.bottom()) {
            finalY = y - cursorHeight - popupSize.height() - 4;
            if (finalY < screenRect.top()) finalY = screenRect.top();
        }

        POPUP_DEBUG(QString::asprintf("화면 경계 보정: screen=(%d,%d,%d,%d), popup=(%d,%d), req=(%d,%d) -> final=(%d,%d)",
                    screenRect.left(), screenRect.top(), screenRect.width(), screenRect.height(),
                    x, y, popupSize.width(), popupSize.height(), finalX, finalY));
    }

    move(finalX, finalY);
}

void UnimHanjaPopup::mousePressEvent(QMouseEvent *event)
{
    if (!isVisible() || m_candidates.isEmpty()) {
        QWidget::mousePressEvent(event);
        return;
    }

    if (event->button() == Qt::RightButton) {
        /* 우클릭 → 다음 페이지 (순환) */
        nextPage();
        POPUP_DEBUG(QString::asprintf("우클릭 → 다음 페이지: %d/%d",
                    m_currentPage + 1,
                    static_cast<int>((m_candidates.size() + MAX_VISIBLE_CANDIDATES - 1) / MAX_VISIBLE_CANDIDATES)));
        event->accept();
        return;
    }

    if (event->button() == Qt::LeftButton) {
        /* 좌클릭 → 클릭 위치의 행을 찾아 선택 */
        int clickY = event->pos().y();
        for (int i = 0; i < MAX_VISIBLE_CANDIDATES; i++) {
            if (m_labels[i]->isVisible()) {
                QRect labelRect = m_labels[i]->geometry();
                if (labelRect.contains(event->pos().x(), clickY)) {
                    int pageStart = m_currentPage * MAX_VISIBLE_CANDIDATES;
                    int actualIndex = pageStart + i;
                    if (actualIndex < m_candidates.size()) {
                        selectCandidate(actualIndex);
                    }
                    event->accept();
                    return;
                }
            }
        }
    }

    QWidget::mousePressEvent(event);
}
