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
#include <QGridLayout>
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <cstdlib>
#include <cstring>
#include <vector>

/* 모드 표시 아이콘 — GTK Standalone·GNOME extension과 동일 */
#define ICON_EXPAND  "⊞"   /* ⊞ */
#define ICON_COMPACT "⊟"   /* ⊟ */

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
    , m_body(nullptr)
    , m_pageLabel(nullptr)
    , m_expandIcon(nullptr)
    , m_currentPage(0)
    , m_selectedIndex(0)
    , m_selRow(0)
    , m_selCol(0)
    , m_cols(1)
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
        /* compact 모드 즐겨찾기 강조 (gridcell 미지정 라벨에만 적용) */
        "QLabel[bookmarked=\"true\"] {"
        "  color: #f9e2af;"
        "  background-color: rgba(249, 226, 175, 20);"
        "  border-radius: 6px;"
        "}"
        "QLabel[bookmarked=\"true\"][selected=\"true\"] {"
        "  background-color: rgba(249, 226, 175, 51);"
        "  color: #f9e2af;"
        "  border-radius: 6px;"
        "}"
        "QLabel#pageLabel {"
        "  color: #6c7086;"
        "  font-size: %6px;"
        "  padding: %2px %7px;"
        "  min-height: 0px;"
        "}"
        "QLabel#expandIcon {"
        "  color: #7f849c;"
        "  font-size: %6px;"
        "  padding: %2px %7px;"
        "  min-height: 0px;"
        "}"
        /* expanded 9×9 grid 셀 — compact 라벨과 별도 룰로 적용 */
        "QLabel[gridcell=\"true\"] {"
        "  color: #cdd6f4;"
        "  font-size: %4px;"
        "  min-height: %5px;"
        "  padding: 2px;"
        "  border-radius: 4px;"
        "  qproperty-alignment: AlignCenter;"
        "}"
        "QLabel[gridcell=\"true\"]:hover {"
        "  background-color: rgba(137, 180, 250, 38);"
        "}"
        "QLabel[gridcell=\"true\"][selected=\"true\"] {"
        "  background-color: rgba(137, 180, 250, 76);"
        "}"
        "QLabel[gridcell=\"true\"][bookmarked=\"true\"] {"
        "  color: #f9e2af;"
        "}"
        "QLabel[gridheader=\"true\"] {"
        "  color: #7f849c;"
        "  font-size: %6px;"
        "  padding: 0 2px;"
        "  qproperty-alignment: AlignCenter;"
        "  min-height: 0px;"
        "}"
        "QLabel[gridrownumber=\"true\"] {"
        "  color: #7f849c;"
        "  font-size: %6px;"
        "  padding: 0 4px;"
        "  qproperty-alignment: AlignCenter;"
        "  min-height: 0px;"
        "}"
        /* 행/열 강조 (special과 동일 글로벌 selector — UX 일관성) */
        "QLabel[highlight=\"true\"] {"
        "  color: #f9e2af;"
        "}"
    ).arg(padding).arg(labelPadV).arg(labelPadH).arg(fontSize)
     .arg(minHeight).arg(pageFontSize).arg(pagePadH));

    m_layout = new QVBoxLayout(this);
    m_layout->setContentsMargins(4, 4, 4, 4);
    m_layout->setSpacing(1);

    /* body 컨테이너 — compact는 QVBoxLayout, expanded는 QGridLayout을 가진다.
     * 모드 전환 시 컨테이너 자체를 갈아끼운다 (자식 라벨도 함께 정리). */
    m_body = new QWidget(this);
    m_layout->addWidget(m_body);

    /* 푸터: 페이지 라벨(좌) + 확장 아이콘(우) */
    QWidget *footer = new QWidget(this);
    QHBoxLayout *footerLayout = new QHBoxLayout(footer);
    footerLayout->setContentsMargins(0, 0, 0, 0);
    footerLayout->setSpacing(2);

    m_pageLabel = new QLabel(footer);
    m_pageLabel->setObjectName("pageLabel");
    m_pageLabel->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_pageLabel->setVisible(false);
    footerLayout->addWidget(m_pageLabel, 1);

    m_expandIcon = new QLabel(footer);
    m_expandIcon->setObjectName("expandIcon");
    m_expandIcon->setText(QString::fromUtf8(ICON_EXPAND));
    m_expandIcon->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    footerLayout->addWidget(m_expandIcon, 0);

    m_layout->addWidget(footer);

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
    m_bookmarks = QList<bool>();
    for (int i = 0; i < candidates.size(); i++) {
        m_bookmarks.append(false);
    }
    m_callback = std::move(callback);
    m_currentPage = 0;
    m_selectedIndex = 0;
    m_selRow = 0;
    m_selCol = 0;
    m_cols = 1;

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

    case UNIM_POPUP_RESULT_UPDATED: {
        /* Period 키 토글로 cols가 1↔9 전환되면 compact↔expanded 위젯 트리도
         * 함께 재구성된다 (updateList가 cols로 분기). */
        int engineCols = unim_popup_get_cols(m_popupState);
        m_cols = engineCols > 0 ? engineCols : 1;
        m_currentPage = unim_popup_get_current_page(m_popupState);
        m_selRow = unim_popup_get_sel_row(m_popupState);
        m_selCol = unim_popup_get_sel_col(m_popupState);
        m_selectedIndex = m_selRow;
        updateList();
        return true;
    }

    case UNIM_POPUP_RESULT_CONSUMED:
        return true;

    case UNIM_POPUP_RESULT_TOGGLE_BOOKMARK:
        if (result.selected_index >= 0 && m_toggleBookmarkCallback) {
            m_toggleBookmarkCallback((quint32)result.selected_index);
        }
        return true;

    case UNIM_POPUP_RESULT_NOT_HANDLED:
    default:
        return false;
    }
}

void UnimHanjaPopup::setToggleBookmarkCallback(ToggleBookmarkCallback callback)
{
    m_toggleBookmarkCallback = std::move(callback);
}

void UnimHanjaPopup::setBookmarkStates(const QList<bool> &states)
{
    m_bookmarks.clear();
    int n = states.size() < m_candidates.size() ? states.size() : m_candidates.size();
    for (int i = 0; i < n; i++) {
        m_bookmarks.append(states.at(i));
    }
    while (m_bookmarks.size() < m_candidates.size()) {
        m_bookmarks.append(false);
    }
    if (isVisible()) {
        updateList();
    }
}

void UnimHanjaPopup::setBookmark(quint32 globalIndex, bool bookmarked)
{
    int idx = (int)globalIndex;
    if (idx < 0 || idx >= m_candidates.size()) return;
    while (m_bookmarks.size() < m_candidates.size()) {
        m_bookmarks.append(false);
    }
    m_bookmarks[idx] = bookmarked;

    // 현재 페이지에 포함될 때만 재렌더 (페이지 크기는 모드 의존)
    int ps = pageSize();
    int pageStart = m_currentPage * ps;
    int pageEnd = pageStart + ps;
    if (idx >= pageStart && idx < pageEnd && isVisible()) {
        updateList();
    }
}

void UnimHanjaPopup::replaceCandidates(const QList<UnimHanjaCandidate> &candidates,
                                        const QList<bool> &bookmarks,
                                        int page,
                                        int selRow,
                                        int selCol)
{
    m_candidates = candidates;
    m_bookmarks = bookmarks;
    while (m_bookmarks.size() < m_candidates.size()) {
        m_bookmarks.append(false);
    }
    if (m_bookmarks.size() > m_candidates.size()) {
        m_bookmarks = m_bookmarks.mid(0, m_candidates.size());
    }
    m_currentPage = page < 0 ? 0 : page;
    m_selRow = selRow < 0 ? 0 : selRow;
    m_selCol = selCol < 0 ? 0 : selCol;
    m_selectedIndex = m_selRow; // compact용
    if (isVisible()) {
        updateList();
    }
}

int UnimHanjaPopup::totalPages() const
{
    int ps = (m_cols > 1) ? EXPANDED_PAGE_SIZE : COMPACT_PAGE_SIZE;
    if (m_candidates.isEmpty()) return 1;
    return (m_candidates.size() + ps - 1) / ps;
}

/* body 자식 위젯 모두 정리 — 모드 전환 또는 재렌더 진입점에서 호출. */
void UnimHanjaPopup::clearBodyLabels()
{
    if (!m_body) return;
    QLayout *old = m_body->layout();
    if (old) {
        QLayoutItem *item;
        while ((item = old->takeAt(0)) != nullptr) {
            if (QWidget *w = item->widget()) {
                w->setParent(nullptr);
                w->deleteLater();
            }
            delete item;
        }
        delete old;
    }
    m_cells.clear();
}

void UnimHanjaPopup::updateList()
{
    int ps = pageSize();
    int pageStart = m_currentPage * ps;
    int pageEnd = qMin(pageStart + ps, m_candidates.size());

    clearBodyLabels();

    if (m_cols > 1) {
        renderExpandedGrid(pageStart, pageEnd);
    } else {
        renderCompactList(pageStart, pageEnd);
    }

    /* 푸터 — 페이지 라벨 + 확장 아이콘 갱신 */
    int total = totalPages();
    if (total > 1) {
        m_pageLabel->setText(QString("%1/%2").arg(m_currentPage + 1).arg(total));
        m_pageLabel->setVisible(true);
    } else {
        m_pageLabel->setText(QString());
        m_pageLabel->setVisible(false);
    }
    if (m_expandIcon) {
        /* QStringLiteral은 매크로이므로 ternary로 풀 수 없다 — QString::fromUtf8 사용. */
        m_expandIcon->setText(QString::fromUtf8(m_cols > 1 ? ICON_COMPACT : ICON_EXPAND));
    }

    adjustSize();
}

/* compact 모드 — 1×9 QLabel 세로 나열, 각 라벨 클릭 시 selectCandidate 호출. */
void UnimHanjaPopup::renderCompactList(int pageStart, int pageEnd)
{
    QVBoxLayout *vbox = new QVBoxLayout(m_body);
    vbox->setContentsMargins(0, 0, 0, 0);
    vbox->setSpacing(1);

    int n = pageEnd - pageStart;
    for (int i = 0; i < n; i++) {
        int globalIdx = pageStart + i;
        const UnimHanjaCandidate &cand = m_candidates.at(globalIdx);
        bool bookmarked = (globalIdx < m_bookmarks.size())
                          ? m_bookmarks.at(globalIdx)
                          : false;
        QString star = bookmarked ? QStringLiteral("  ★") : QStringLiteral("  ☆");
        QString text = QString("%1. %2  %3%4")
                           .arg(i + 1)
                           .arg(cand.hanja, cand.meaning, star);
        QLabel *lbl = new QLabel(text, m_body);
        lbl->setCursor(Qt::PointingHandCursor);
        lbl->setProperty("selected", i == m_selectedIndex);
        lbl->setProperty("bookmarked", bookmarked);
        lbl->style()->unpolish(lbl);
        lbl->style()->polish(lbl);
        vbox->addWidget(lbl);
        m_cells.push_back(lbl);
    }
}

/* expanded 모드 — 9×9 QGridLayout. 헤더 행에 1-9, 그 아래 col 우선 인덱싱
 * (idx = col*9 + row)으로 셀 배치. 기현 클릭은 셀의 unim-global-idx 프로퍼티로
 * 식별한다. */
void UnimHanjaPopup::renderExpandedGrid(int pageStart, int pageEnd)
{
    QGridLayout *grid = new QGridLayout(m_body);
    grid->setContentsMargins(0, 0, 0, 0);
    grid->setHorizontalSpacing(2);
    grid->setVerticalSpacing(2);

    /* (0, 0) corner — 빈 라벨 (special과 정합) */
    QLabel *corner = new QLabel(QString(), m_body);
    corner->setProperty("gridrownumber", true);
    grid->addWidget(corner, 0, 0);

    /* 행 번호 (col 0, row 1..=9). sel_row면 highlight=true — UX 일관성 (special과 정합). */
    for (int row = 0; row < EXPANDED_ROWS; row++) {
        QLabel *numLabel = new QLabel(QString::number(row + 1), m_body);
        numLabel->setProperty("gridrownumber", true);
        numLabel->setProperty("highlight", (row == m_selRow));
        numLabel->setAlignment(Qt::AlignCenter);
        numLabel->style()->unpolish(numLabel);
        numLabel->style()->polish(numLabel);
        grid->addWidget(numLabel, row + 1, 0);
    }

    /* 가로 레이블 키 시퀀스(special과 동일). 엔진 SSOT는 PopupState.top_row()이지만
     * 멤버 추가/시그널 변경을 피하기 위해 동일 상수를 직접 사용. */
    static const char TOP_ROW[] = "QWERTYUIO";
    for (int col = 0; col < EXPANDED_COLS; col++) {
        QChar headerChar = (col < (int)(sizeof(TOP_ROW) - 1)) ? QChar(TOP_ROW[col]) : QChar();
        QLabel *header = new QLabel(QString(headerChar), m_body);
        header->setProperty("gridheader", true);
        header->setProperty("highlight", (col == m_selCol));
        header->setAlignment(Qt::AlignCenter);
        header->style()->unpolish(header);
        header->style()->polish(header);
        grid->addWidget(header, 0, col + 1);

        for (int row = 0; row < EXPANDED_ROWS; row++) {
            int offset = col * EXPANDED_ROWS + row;
            int global = pageStart + offset;
            if (global >= pageEnd) continue;
            const UnimHanjaCandidate &cand = m_candidates.at(global);
            bool bookmarked = (global < m_bookmarks.size())
                              ? m_bookmarks.at(global)
                              : false;
            QLabel *cell = new QLabel(cand.hanja, m_body);
            cell->setCursor(Qt::PointingHandCursor);
            cell->setProperty("gridcell", true);
            cell->setProperty("selected", (col == m_selCol && row == m_selRow));
            cell->setProperty("bookmarked", bookmarked);
            cell->setProperty("unim-global-idx", global);
            cell->setAlignment(Qt::AlignCenter);
            cell->style()->unpolish(cell);
            cell->style()->polish(cell);
            grid->addWidget(cell, row + 1, col + 1);
            m_cells.push_back(cell);
        }
    }
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
    int total = totalPages();
    if (total > 1) {
        if (m_currentPage < total - 1) {
            m_currentPage++;
        } else {
            m_currentPage = 0;
        }
        m_selectedIndex = 0;
        m_selRow = 0;
        m_selCol = 0;
        updateList();
    }
}

void UnimHanjaPopup::prevPage()
{
    int total = totalPages();
    if (total > 1) {
        if (m_currentPage > 0) {
            m_currentPage--;
        } else {
            m_currentPage = total - 1;
        }
        m_selectedIndex = 0;
        m_selRow = 0;
        m_selCol = 0;
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
                    m_currentPage + 1, totalPages()));
        event->accept();
        return;
    }

    if (event->button() == Qt::LeftButton) {
        /* 좌클릭 → 동적 셀 풀(m_cells)에서 hit-test.
         * 각 셀은 compact는 i번째(=row local), expanded는 unim-global-idx 프로퍼티에
         * 글로벌 인덱스를 가지고 있어 페이지·모드와 무관하게 정확히 해석된다. */
        QPoint bodyPos = m_body ? m_body->mapFrom(this, event->pos()) : event->pos();
        for (size_t i = 0; i < m_cells.size(); i++) {
            QLabel *cell = m_cells[i];
            if (!cell || !cell->isVisible()) continue;
            if (cell->geometry().contains(bodyPos)) {
                QVariant gv = cell->property("unim-global-idx");
                int actualIndex;
                if (gv.isValid()) {
                    actualIndex = gv.toInt();
                } else {
                    /* compact 모드: i가 곧 페이지 내 행. */
                    actualIndex = m_currentPage * COMPACT_PAGE_SIZE + static_cast<int>(i);
                }
                if (actualIndex >= 0 && actualIndex < m_candidates.size()) {
                    selectCandidate(actualIndex);
                }
                event->accept();
                return;
            }
        }
    }

    QWidget::mousePressEvent(event);
}
