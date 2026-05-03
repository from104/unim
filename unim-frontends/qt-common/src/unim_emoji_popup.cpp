/**
 * UNIM 이모지 팝업 구현 (Qt)
 *
 * `ShowEmojiPopupV2` 시그널을 받아 9×9 그리드 + 좌측 9 탭 (Recent + 8 카테고리)
 * 으로 이모지를 표시. 키 처리는 엔진 (popup_state) 이 담당하므로 IM 모듈은
 * 시그널 수신만 하고 본 위젯은 표시·페이지/셀 갱신·셀 클릭만 처리.
 */

#include "unim_emoji_popup.hpp"
#include <QApplication>
#include <QScreen>
#include <QMouseEvent>
#include <QStyle>
#include <QDebug>
#include <QDateTime>
#include <QStandardPaths>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 */
static bool ep_debug_enabled = false;
static bool ep_debug_checked = false;

static void ep_log(const char *module, const QString &message)
{
    if (!ep_debug_enabled) return;

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

#define EP_DEBUG(...) \
    ep_log("EMOJI_POPUP", QString(__VA_ARGS__))

static void ep_check_debug()
{
    if (!ep_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            ep_debug_enabled = true;
        }
        ep_debug_checked = true;
    }
}

UnimEmojiPopup::UnimEmojiPopup(QWidget *parent)
    : QWidget(parent, Qt::ToolTip | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint)
    , m_currentPage(0)
    , m_totalPages(1)
    , m_selRow(0)
    , m_selCol(0)
    , m_mainLayout(nullptr)
    , m_bodyLayout(nullptr)
    , m_tabLayout(nullptr)
    , m_gridLayout(nullptr)
    , m_headerLabel(nullptr)
    , m_footerBox(nullptr)
    , m_footerLabel(nullptr)
    , m_prevPageBtn(nullptr)
    , m_nextPageBtn(nullptr)
    , m_flashTimer(nullptr)
    , m_pendingHide(false)
{
    ep_check_debug();

    /* DPI 스케일 */
    qreal scaleFactor = 1.0;
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        scaleFactor = screen->logicalDotsPerInch() / 96.0;
        if (scaleFactor < 1.0) scaleFactor = 1.0;
    }
    int cellSize = qRound(30 * scaleFactor);
    int fontSize = qRound(18 * scaleFactor);
    int headerFontSize = qRound(11 * scaleFactor);
    int footerFontSize = qRound(12 * scaleFactor);
    int widgetPad = qRound(8 * scaleFactor);

    /* Catppuccin Mocha 스타일 */
    setStyleSheet(QString(
        "UnimEmojiPopup {"
        "  background-color: rgba(30, 30, 46, 247);"
        "  border: 1px solid rgba(255, 255, 255, 38);"
        "  border-radius: 12px;"
        "  padding: %1px;"
        "}"
        "QLabel {"
        "  color: #cdd6f4;"
        "}"
        "QLabel#headerLabel {"
        "  background-color: #313244;"
        "  color: #a6e3a1;"
        "  font-size: %2px;"
        "  font-weight: bold;"
        "  padding: 6px 8px;"
        "  border-radius: 4px;"
        "}"
        "QLabel[cellType=\"header\"] {"
        "  color: #f9e2af;"
        "  font-size: %3px;"
        "  font-weight: bold;"
        "  min-width: 28px; min-height: 18px;"
        "}"
        "QLabel[cellType=\"row-header\"] {"
        "  color: #7f849c;"
        "  font-size: %3px;"
        "  font-weight: bold;"
        "  min-width: 20px; min-height: %4px;"
        "}"
        "QLabel[cellType=\"cell\"] {"
        "  color: #cdd6f4;"
        "  font-size: %5px;"
        "  min-width: %4px;"
        "  min-height: %4px;"
        "  padding: 2px;"
        "  border-radius: 4px;"
        "}"
        "QLabel[cellType=\"cell\"]:hover {"
        "  background-color: rgba(255, 255, 255, 13);"
        "}"
        "QLabel[selected=\"true\"] {"
        "  background-color: rgba(166, 227, 161, 64);"
        "  color: #cdd6f4;"
        "  font-weight: bold;"
        "  border-radius: 6px;"
        "}"
        "QLabel[flash=\"true\"] {"
        "  background-color: #a6e3a1;"
        "  color: #1e1e2e;"
        "  font-weight: bold;"
        "  border-radius: 6px;"
        "}"
        "QLabel[highlight=\"true\"] {"
        "  color: #a6e3a1;"
        "}"
        "QLabel#footerLabel {"
        "  color: #6c7086;"
        "  font-size: %6px;"
        "  padding: 2px 4px;"
        "}"
        "QToolButton {"
        "  color: #cdd6f4;"
        "  background: transparent;"
        "  border-radius: 6px;"
        "  padding: 4px 8px;"
        "  font-size: %3px;"
        "  min-width: 96px;"
        "  text-align: left;"
        "}"
        "QToolButton:hover {"
        "  background-color: rgba(255, 255, 255, 16);"
        "}"
        "QToolButton:checked {"
        "  background-color: rgba(166, 227, 161, 64);"
        "  color: #a6e3a1;"
        "  font-weight: bold;"
        "}"
        /* 마우스 페이지 이동 ◀/▶ (Phase 5) */
        "QPushButton#prevPageBtn, QPushButton#nextPageBtn {"
        "  color: #7f849c;"
        "  background: transparent;"
        "  border: none;"
        "  font-size: %6px;"
        "  min-width: 22px;"
        "  min-height: 22px;"
        "  padding: 2px 6px;"
        "  border-radius: 4px;"
        "}"
        "QPushButton#prevPageBtn:hover, QPushButton#nextPageBtn:hover {"
        "  color: #89b4fa;"
        "  background-color: rgba(137, 180, 250, 51);"
        "}"
        "QPushButton#prevPageBtn:pressed, QPushButton#nextPageBtn:pressed {"
        "  background-color: rgba(137, 180, 250, 89);"
        "}"
    ).arg(widgetPad).arg(headerFontSize+2).arg(headerFontSize)
     .arg(cellSize).arg(fontSize).arg(footerFontSize));

    m_mainLayout = new QVBoxLayout(this);
    m_mainLayout->setContentsMargins(4, 4, 4, 4);
    m_mainLayout->setSpacing(2);

    /* 헤더 라벨 */
    m_headerLabel = new QLabel("", this);
    m_headerLabel->setObjectName("headerLabel");
    m_headerLabel->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_mainLayout->addWidget(m_headerLabel);

    /* 본문 hbox: [좌측 탭 | 우측 grid] */
    m_bodyLayout = new QHBoxLayout();
    m_bodyLayout->setSpacing(6);

    /* 좌측 9 탭 */
    m_tabLayout = new QVBoxLayout();
    m_tabLayout->setSpacing(2);
    m_tabLayout->setAlignment(Qt::AlignTop);
    for (int i = 0; i < EMOJI_TAB_COUNT; i++) {
        QToolButton *btn = new QToolButton(this);
        btn->setCheckable(true);
        btn->setAutoExclusive(true);
        btn->setFocusPolicy(Qt::NoFocus);
        btn->setText("");
        m_tabButtons[i] = btn;
        m_tabLayout->addWidget(btn);
    }
    m_bodyLayout->addLayout(m_tabLayout);

    /* 우측 grid */
    m_gridLayout = new QGridLayout();
    m_gridLayout->setSpacing(1);
    m_bodyLayout->addLayout(m_gridLayout, 1);

    m_mainLayout->addLayout(m_bodyLayout);

    /* 푸터: [◀] [페이지 라벨] [▶] (Phase 5) — 단일 페이지 시 footer_box hide */
    m_footerBox = new QWidget(this);
    m_footerBox->setObjectName("emojiFooterBox");
    QHBoxLayout *footerLayout = new QHBoxLayout(m_footerBox);
    footerLayout->setContentsMargins(0, 0, 0, 0);
    footerLayout->setSpacing(4);

    m_prevPageBtn = new QPushButton(QStringLiteral("\xE2\x97\x80"), m_footerBox); /* ◀ */
    m_prevPageBtn->setObjectName("prevPageBtn");
    m_prevPageBtn->setFlat(true);
    m_prevPageBtn->setFocusPolicy(Qt::NoFocus);
    m_prevPageBtn->setCursor(Qt::PointingHandCursor);
    m_prevPageBtn->setToolTip(QStringLiteral("이전 페이지"));
    QObject::connect(m_prevPageBtn, &QPushButton::clicked, this, [this]() {
        if (m_pageChangeCallback) m_pageChangeCallback(0);
    });
    footerLayout->addWidget(m_prevPageBtn, 0);

    m_footerLabel = new QLabel(m_footerBox);
    m_footerLabel->setObjectName("footerLabel");
    m_footerLabel->setAlignment(Qt::AlignCenter);
    footerLayout->addWidget(m_footerLabel, 1);

    m_nextPageBtn = new QPushButton(QStringLiteral("\xE2\x96\xB6"), m_footerBox); /* ▶ */
    m_nextPageBtn->setObjectName("nextPageBtn");
    m_nextPageBtn->setFlat(true);
    m_nextPageBtn->setFocusPolicy(Qt::NoFocus);
    m_nextPageBtn->setCursor(Qt::PointingHandCursor);
    m_nextPageBtn->setToolTip(QStringLiteral("다음 페이지"));
    QObject::connect(m_nextPageBtn, &QPushButton::clicked, this, [this]() {
        if (m_pageChangeCallback) m_pageChangeCallback(1);
    });
    footerLayout->addWidget(m_nextPageBtn, 0);

    m_footerBox->setVisible(false);
    m_mainLayout->addWidget(m_footerBox);

    std::memset(m_cells, 0, sizeof(m_cells));
    std::memset(m_colHeaders, 0, sizeof(m_colHeaders));
    std::memset(m_rowHeaders, 0, sizeof(m_rowHeaders));
    std::strncpy(m_topRow, "QWERTYUIO", 10);

    m_flashTimer = new QTimer(this);
    m_flashTimer->setSingleShot(true);
    connect(m_flashTimer, &QTimer::timeout, this, [this]() {
        if (m_pendingHide) {
            hidePopup();
        }
    });
}

UnimEmojiPopup::~UnimEmojiPopup() = default;

QString UnimEmojiPopup::tabLabelFor(const QString &catId) const
{
    if (catId == "Recent") return QStringLiteral("최근");
    if (catId == "SmileysPeople") return QStringLiteral("표정·인물");
    if (catId == "Animals") return QStringLiteral("동물·자연");
    if (catId == "Food") return QStringLiteral("음식·음료");
    if (catId == "Activities") return QStringLiteral("활동");
    if (catId == "Travel") return QStringLiteral("여행·장소");
    if (catId == "Objects") return QStringLiteral("사물");
    if (catId == "Symbols") return QStringLiteral("기호");
    if (catId == "Flags") return QStringLiteral("국기");
    return catId;
}

void UnimEmojiPopup::showPopup(const QString &targetCatId,
                                const QStringList &items,
                                const QString &topRow,
                                const QStringList &recent,
                                const QList<UnimEmojiCategoryInfo> &categories,
                                int x, int y, int cursorHeight,
                                UnimEmojiCommitCallback callback)
{
    m_currentCatId = targetCatId;
    m_items = items;
    m_recent = recent;
    m_categories = categories;
    m_callback = callback;
    m_currentPage = 0;
    m_selRow = 0;
    m_selCol = 0;
    m_pendingHide = false;

    if (topRow.length() >= 9) {
        QByteArray ba = topRow.toLatin1();
        std::strncpy(m_topRow, ba.constData(), 9);
        m_topRow[9] = '\0';
    } else {
        std::strncpy(m_topRow, "QWERTYUIO", 10);
    }

    if (m_items.isEmpty()) {
        m_totalPages = 1;
    } else {
        m_totalPages = (m_items.size() + EMOJI_PAGE_SIZE - 1) / EMOJI_PAGE_SIZE;
    }

    /* 좌측 탭 라벨/active 동기화 */
    int activeIdx = 0;
    for (int i = 0; i < EMOJI_TAB_COUNT; i++) {
        QToolButton *btn = m_tabButtons[i];
        if (!btn) continue;
        if (i < m_categories.size()) {
            const auto &meta = m_categories[i];
            QString label = meta.nameKo.isEmpty() ? tabLabelFor(meta.id) : meta.nameKo;
            btn->setText(label);
            btn->setVisible(true);
            if (meta.id == m_currentCatId) {
                activeIdx = i;
            }
        } else {
            btn->setText("");
            btn->setVisible(false);
        }
    }
    for (int i = 0; i < EMOJI_TAB_COUNT; i++) {
        if (m_tabButtons[i]) {
            m_tabButtons[i]->setChecked(i == activeIdx);
        }
    }

    /* 헤더 텍스트 */
    QString catLabel = (activeIdx < m_categories.size())
        ? (m_categories[activeIdx].nameKo.isEmpty()
            ? tabLabelFor(m_categories[activeIdx].id)
            : m_categories[activeIdx].nameKo)
        : tabLabelFor(m_currentCatId);
    m_headerLabel->setText(QString::fromUtf8("「%1」 → 이모지").arg(catLabel));

    EP_DEBUG(QString("이모지 팝업 표시: cat='%1', items=%2, recent=%3, cats=%4")
             .arg(m_currentCatId).arg(m_items.size())
             .arg(m_recent.size()).arg(m_categories.size()));

    rebuildGrid();
    adjustPosition(x, y, cursorHeight);
    show();
}

void UnimEmojiPopup::hidePopup()
{
    m_pendingHide = false;
    hide();
    EP_DEBUG("이모지 팝업 숨김");
}

void UnimEmojiPopup::adjustPosition(int x, int y, int cursorHeight)
{
    adjustSize();

    int finalX = x;
    int finalY = y + 4;

    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        QRect screenRect = screen->availableGeometry();
        QSize popupSize = sizeHint();

        if (finalX + popupSize.width() > screenRect.right()) {
            finalX = screenRect.right() - popupSize.width() - 4;
            if (finalX < 0) finalX = 0;
        }
        if (finalY + popupSize.height() > screenRect.bottom()) {
            finalY = y - cursorHeight - popupSize.height() - 4;
            if (finalY < 0) finalY = 0;
        }
    }
    move(finalX, finalY);
}

void UnimEmojiPopup::rebuildGrid()
{
    /* 기존 셀 제거 */
    while (m_gridLayout->count() > 0) {
        QLayoutItem *item = m_gridLayout->takeAt(0);
        if (item->widget()) {
            delete item->widget();
        }
        delete item;
    }
    std::memset(m_cells, 0, sizeof(m_cells));
    std::memset(m_colHeaders, 0, sizeof(m_colHeaders));
    std::memset(m_rowHeaders, 0, sizeof(m_rowHeaders));

    int pageStart = m_currentPage * EMOJI_PAGE_SIZE;
    int pageChars = m_items.size() - pageStart;
    if (pageChars < 0) pageChars = 0;
    if (pageChars > EMOJI_PAGE_SIZE) pageChars = EMOJI_PAGE_SIZE;

    int activeCols = (pageChars + EMOJI_MAX_ROWS - 1) / EMOJI_MAX_ROWS;
    if (activeCols > EMOJI_MAX_COLS) activeCols = EMOJI_MAX_COLS;
    if (activeCols < 1 && pageChars > 0) activeCols = 1;

    /* 좌상단 코너 */
    QLabel *corner = new QLabel("  ", this);
    corner->setProperty("cellType", "row-header");
    corner->setAlignment(Qt::AlignCenter);
    m_gridLayout->addWidget(corner, 0, 0);

    /* 컬럼 헤더 */
    for (int c = 0; c < activeCols; c++) {
        QLabel *header = new QLabel(QString(QChar(m_topRow[c])), this);
        header->setProperty("cellType", "header");
        header->setAlignment(Qt::AlignCenter);
        m_gridLayout->addWidget(header, 0, c + 1);
        m_colHeaders[c] = header;
    }

    /* 셀 (column-major) */
    for (int c = 0; c < activeCols; c++) {
        for (int r = 0; r < EMOJI_MAX_ROWS; r++) {
            int idx = pageStart + c * EMOJI_MAX_ROWS + r;
            if (idx >= m_items.size()) break;

            if (c == 0) {
                QLabel *rowLabel = new QLabel(QString::number(r + 1), this);
                rowLabel->setProperty("cellType", "row-header");
                rowLabel->setAlignment(Qt::AlignCenter);
                m_gridLayout->addWidget(rowLabel, r + 1, 0);
                m_rowHeaders[r] = rowLabel;
            }

            QLabel *cell = new QLabel(m_items[idx], this);
            cell->setProperty("cellType", "cell");
            cell->setAlignment(Qt::AlignCenter);
            cell->setCursor(Qt::PointingHandCursor);
            m_gridLayout->addWidget(cell, r + 1, c + 1);
            m_cells[c][r] = cell;
        }
    }

    /* 페이지 인디케이터: 단일 페이지면 footer_box hide */
    if (m_totalPages > 1) {
        m_footerLabel->setText(QString("[%1]  %2/%3")
                               .arg(tabLabelFor(m_currentCatId))
                               .arg(m_currentPage + 1)
                               .arg(m_totalPages));
        m_footerBox->setVisible(true);
    } else {
        m_footerBox->setVisible(false);
    }

    updateSelection();
}

void UnimEmojiPopup::updateSelection()
{
    /* selected/highlight 리셋 */
    for (int c = 0; c < EMOJI_MAX_COLS; c++) {
        for (int r = 0; r < EMOJI_MAX_ROWS; r++) {
            if (m_cells[c][r]) {
                m_cells[c][r]->setProperty("selected", false);
                m_cells[c][r]->style()->unpolish(m_cells[c][r]);
                m_cells[c][r]->style()->polish(m_cells[c][r]);
            }
        }
    }
    for (int c = 0; c < EMOJI_MAX_COLS; c++) {
        if (m_colHeaders[c]) {
            m_colHeaders[c]->setProperty("highlight", false);
            m_colHeaders[c]->style()->unpolish(m_colHeaders[c]);
            m_colHeaders[c]->style()->polish(m_colHeaders[c]);
        }
    }
    for (int r = 0; r < EMOJI_MAX_ROWS; r++) {
        if (m_rowHeaders[r]) {
            m_rowHeaders[r]->setProperty("highlight", false);
            m_rowHeaders[r]->style()->unpolish(m_rowHeaders[r]);
            m_rowHeaders[r]->style()->polish(m_rowHeaders[r]);
        }
    }

    if (m_selCol >= 0 && m_selCol < EMOJI_MAX_COLS &&
        m_selRow >= 0 && m_selRow < EMOJI_MAX_ROWS &&
        m_cells[m_selCol][m_selRow]) {
        QLabel *sel = m_cells[m_selCol][m_selRow];
        sel->setProperty("selected", true);
        sel->style()->unpolish(sel);
        sel->style()->polish(sel);
    }
    if (m_selCol >= 0 && m_selCol < EMOJI_MAX_COLS && m_colHeaders[m_selCol]) {
        m_colHeaders[m_selCol]->setProperty("highlight", true);
        m_colHeaders[m_selCol]->style()->unpolish(m_colHeaders[m_selCol]);
        m_colHeaders[m_selCol]->style()->polish(m_colHeaders[m_selCol]);
    }
    if (m_selRow >= 0 && m_selRow < EMOJI_MAX_ROWS && m_rowHeaders[m_selRow]) {
        m_rowHeaders[m_selRow]->setProperty("highlight", true);
        m_rowHeaders[m_selRow]->style()->unpolish(m_rowHeaders[m_selRow]);
        m_rowHeaders[m_selRow]->style()->polish(m_rowHeaders[m_selRow]);
    }
}

void UnimEmojiPopup::emitSelect(int col, int row)
{
    int pageStart = m_currentPage * EMOJI_PAGE_SIZE;
    int idx = pageStart + col * EMOJI_MAX_ROWS + row;
    if (idx < 0 || idx >= m_items.size()) return;

    const QString &emoji = m_items[idx];
    EP_DEBUG(QString("이모지 선택: '%1' (idx=%2)").arg(emoji).arg(idx));

    /* 플래시 */
    if (m_cells[col][row]) {
        m_cells[col][row]->setProperty("flash", true);
        m_cells[col][row]->style()->unpolish(m_cells[col][row]);
        m_cells[col][row]->style()->polish(m_cells[col][row]);
    }

    m_pendingHide = true;
    m_flashTimer->start(EMOJI_FLASH_DURATION_MS);

    if (m_callback) {
        m_callback(emoji);
    }
}

void UnimEmojiPopup::mousePressEvent(QMouseEvent *event)
{
    if (m_pendingHide) return;
    QPoint pos = event->pos();

    for (int c = 0; c < EMOJI_MAX_COLS; c++) {
        for (int r = 0; r < EMOJI_MAX_ROWS; r++) {
            if (m_cells[c][r] && m_cells[c][r]->geometry().contains(pos)) {
                m_selCol = c;
                m_selRow = r;
                updateSelection();
                emitSelect(c, r);
                return;
            }
        }
    }
}

void UnimEmojiPopup::navigatePopup(int page, int totalPages, int selected,
                                    int rows, int cols, int selRow, int selCol)
{
    Q_UNUSED(selected);
    Q_UNUSED(rows);
    Q_UNUSED(cols);

    if (totalPages > 0) {
        m_totalPages = totalPages;
    }

    int newPage = (page < 0) ? 0 : page;
    bool pageChanged = (newPage != m_currentPage);
    m_currentPage = newPage;
    m_selRow = (selRow < 0) ? 0 : selRow;
    m_selCol = (selCol < 0) ? 0 : selCol;

    if (pageChanged) {
        rebuildGrid();
    } else {
        updateSelection();
    }
}

void UnimEmojiPopup::setRecent(const QStringList &emojis)
{
    m_recent = emojis;
    EP_DEBUG(QString("Recent 캐시 갱신: count=%1").arg(m_recent.size()));
}

void UnimEmojiPopup::setPageChangeCallback(PageChangeCallback callback)
{
    m_pageChangeCallback = std::move(callback);
}
