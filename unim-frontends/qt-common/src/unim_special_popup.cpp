/**
 * UNIM 특수문자 팝업 구현 (Qt)
 *
 * 초성 기반 특수문자 선택을 위한 9x9 그리드 팝업 윈도우입니다.
 * GTK unim_special_popup.c를 Qt로 포팅한 구현입니다.
 */

#include "unim_special_popup.hpp"
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
static bool sp_debug_enabled = false;
static bool sp_debug_checked = false;

static void sp_log(const char *module, const QString &message)
{
    if (!sp_debug_enabled) return;

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

#define SP_DEBUG(...) \
    sp_log("SPECIAL_POPUP", QString(__VA_ARGS__))

static void sp_check_debug()
{
    if (!sp_debug_checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && std::strcmp(env, "1") == 0) {
            sp_debug_enabled = true;
        }
        sp_debug_checked = true;
    }
}

UnimSpecialPopup::UnimSpecialPopup(QWidget *parent)
    : QWidget(parent, Qt::ToolTip | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint)
    , m_totalCount(0)
    , m_currentPage(0)
    , m_totalPages(0)
    , m_rows(0)
    , m_cols(0)
    , m_selRow(0)
    , m_selCol(0)
    , m_mainLayout(nullptr)
    , m_gridLayout(nullptr)
    , m_footerLabel(nullptr)
    , m_flashTimer(nullptr)
    , m_pendingHide(false)
{
    sp_check_debug();

    /* 기본 스타일 설정 — 다크 테마 */
    setStyleSheet(
        "UnimSpecialPopup {"
        "  background-color: #2d2d2d;"
        "  border: 1px solid #555;"
        "  border-radius: 4px;"
        "  padding: 4px;"
        "}"
        "QLabel {"
        "  color: #e0e0e0;"
        "  padding: 2px 4px;"
        "  font-size: 14px;"
        "  min-width: 28px;"
        "  min-height: 24px;"
        "}"
        "QLabel[cellType=\"header\"] {"
        "  color: #aaa;"
        "  font-size: 11px;"
        "  font-weight: bold;"
        "}"
        "QLabel[cellType=\"cell\"] {"
        "  color: #e0e0e0;"
        "  font-size: 14px;"
        "}"
        "QLabel[selected=\"true\"] {"
        "  background-color: #4a90d9;"
        "  color: white;"
        "  border-radius: 2px;"
        "}"
        "QLabel[flash=\"true\"] {"
        "  background-color: #f0a030;"
        "  color: #222;"
        "  border-radius: 2px;"
        "}"
        "QLabel[highlight=\"true\"] {"
        "  color: #6cb5ff;"
        "}"
        "QLabel#footerLabel {"
        "  color: #888;"
        "  font-size: 11px;"
        "  padding: 2px 4px;"
        "}"
    );

    m_mainLayout = new QVBoxLayout(this);
    m_mainLayout->setContentsMargins(4, 4, 4, 4);
    m_mainLayout->setSpacing(1);

    /* 그리드 레이아웃 */
    m_gridLayout = new QGridLayout();
    m_gridLayout->setSpacing(1);
    m_mainLayout->addLayout(m_gridLayout);

    /* 페이지 레이블 */
    m_footerLabel = new QLabel(this);
    m_footerLabel->setObjectName("footerLabel");
    m_footerLabel->setAlignment(Qt::AlignCenter);
    m_footerLabel->setVisible(false);
    m_mainLayout->addWidget(m_footerLabel);

    /* 셀/헤더 초기화 */
    std::memset(m_cells, 0, sizeof(m_cells));
    std::memset(m_colHeaders, 0, sizeof(m_colHeaders));
    std::memset(m_rowHeaders, 0, sizeof(m_rowHeaders));
    std::strncpy(m_topRow, "QWERTYUIO", 10);

    /* 플래시 타이머 */
    m_flashTimer = new QTimer(this);
    m_flashTimer->setSingleShot(true);
    connect(m_flashTimer, &QTimer::timeout, this, [this]() {
        if (m_pendingHide) {
            hidePopup();
        }
    });
}

UnimSpecialPopup::~UnimSpecialPopup()
{
    /* Qt가 자식 위젯을 자동 해제 */
}

void UnimSpecialPopup::showPopup(const QString &target,
                                  const QStringList &characters,
                                  const QString &topRow,
                                  int x, int y, int cursorHeight,
                                  UnimSpecialSelectCallback callback)
{
    if (characters.isEmpty()) return;

    m_characters = characters;
    m_totalCount = characters.size();
    m_currentPage = 0;
    m_totalPages = (m_totalCount + SPECIAL_PAGE_SIZE - 1) / SPECIAL_PAGE_SIZE;
    m_callback = callback;
    m_selRow = 0;
    m_selCol = 0;
    m_pendingHide = false;

    /* top_row 저장 */
    if (topRow.length() >= 9) {
        QByteArray ba = topRow.toLatin1();
        std::strncpy(m_topRow, ba.constData(), 9);
        m_topRow[9] = '\0';
    } else {
        std::strncpy(m_topRow, "QWERTYUIO", 10);
    }

    SP_DEBUG(QString::asprintf("특수문자 팝업 표시: target='%s', count=%d, pages=%d",
             qPrintable(target), m_totalCount, m_totalPages));

    updateGrid();
    adjustPosition(x, y, cursorHeight);
    show();
}

void UnimSpecialPopup::hidePopup()
{
    m_pendingHide = false;
    hide();
    SP_DEBUG("특수문자 팝업 숨김");
}

void UnimSpecialPopup::adjustPosition(int x, int y, int cursorHeight)
{
    /* 크기 조정 */
    adjustSize();

    int finalX = x;
    int finalY = y;

    /* 화면 밖으로 나가지 않도록 조정 */
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        QRect screenRect = screen->geometry();
        QSize popupSize = sizeHint();

        if (finalX + popupSize.width() > screenRect.right()) {
            finalX = screenRect.right() - popupSize.width();
            if (finalX < 0) finalX = 0;
        }
        if (finalY + popupSize.height() > screenRect.bottom()) {
            finalY = y - cursorHeight - popupSize.height();
            if (finalY < 0) finalY = 0;
        }
    }

    move(finalX, finalY);
}

void UnimSpecialPopup::updateGrid()
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

    int pageStart = m_currentPage * SPECIAL_PAGE_SIZE;
    int pageChars = m_totalCount - pageStart;
    if (pageChars > SPECIAL_PAGE_SIZE) pageChars = SPECIAL_PAGE_SIZE;

    /* 열 수 계산: ceil(pageChars / MAX_ROWS) */
    m_cols = (pageChars + SPECIAL_MAX_ROWS - 1) / SPECIAL_MAX_ROWS;
    if (m_cols > SPECIAL_MAX_COLS) m_cols = SPECIAL_MAX_COLS;
    if (m_cols < 1) m_cols = 1;

    /* 행 수 */
    m_rows = (pageChars < SPECIAL_MAX_ROWS) ? pageChars : SPECIAL_MAX_ROWS;

    /* 좌상단 빈 셀 */
    QLabel *corner = new QLabel("  ", this);
    corner->setProperty("cellType", "header");
    corner->setAlignment(Qt::AlignCenter);
    m_gridLayout->addWidget(corner, 0, 0);

    /* 열 헤더 (top_row 기반) */
    for (int c = 0; c < m_cols; c++) {
        QLabel *header = new QLabel(QString(m_topRow[c]), this);
        header->setProperty("cellType", "header");
        header->setAlignment(Qt::AlignCenter);
        m_gridLayout->addWidget(header, 0, c + 1);
        m_colHeaders[c] = header;
    }

    /* 셀 채움 (열 우선) */
    for (int c = 0; c < m_cols; c++) {
        for (int r = 0; r < SPECIAL_MAX_ROWS; r++) {
            int idx = pageStart + c * SPECIAL_MAX_ROWS + r;
            if (idx >= m_totalCount) break;

            /* 행 레이블 (첫 열에서만) */
            if (c == 0) {
                QLabel *rowLabel = new QLabel(QString::number(r + 1), this);
                rowLabel->setProperty("cellType", "header");
                rowLabel->setAlignment(Qt::AlignCenter);
                m_gridLayout->addWidget(rowLabel, r + 1, 0);
                m_rowHeaders[r] = rowLabel;
            }

            /* 문자 셀 */
            QLabel *cell = new QLabel(m_characters[idx], this);
            cell->setProperty("cellType", "cell");
            cell->setAlignment(Qt::AlignCenter);
            cell->setCursor(Qt::PointingHandCursor);
            m_gridLayout->addWidget(cell, r + 1, c + 1);
            m_cells[r][c] = cell;
        }
    }

    /* 페이지 표시 */
    if (m_totalPages > 1) {
        m_footerLabel->setText(QString("%1 / %2").arg(m_currentPage + 1).arg(m_totalPages));
        m_footerLabel->setVisible(true);
    } else {
        m_footerLabel->setVisible(false);
    }

    updateSelection();
}

void UnimSpecialPopup::updateSelection()
{
    /* 전체 셀의 selected/highlight 속성 리셋 */
    for (int r = 0; r < SPECIAL_MAX_ROWS; r++) {
        for (int c = 0; c < SPECIAL_MAX_COLS; c++) {
            if (m_cells[r][c]) {
                m_cells[r][c]->setProperty("selected", false);
                m_cells[r][c]->style()->unpolish(m_cells[r][c]);
                m_cells[r][c]->style()->polish(m_cells[r][c]);
            }
        }
    }

    /* 헤더 highlight 리셋 */
    for (int c = 0; c < SPECIAL_MAX_COLS; c++) {
        if (m_colHeaders[c]) {
            m_colHeaders[c]->setProperty("highlight", false);
            m_colHeaders[c]->style()->unpolish(m_colHeaders[c]);
            m_colHeaders[c]->style()->polish(m_colHeaders[c]);
        }
    }
    for (int r = 0; r < SPECIAL_MAX_ROWS; r++) {
        if (m_rowHeaders[r]) {
            m_rowHeaders[r]->setProperty("highlight", false);
            m_rowHeaders[r]->style()->unpolish(m_rowHeaders[r]);
            m_rowHeaders[r]->style()->polish(m_rowHeaders[r]);
        }
    }

    /* 현재 선택 셀 강조 */
    if (m_selRow >= 0 && m_selRow < SPECIAL_MAX_ROWS &&
        m_selCol >= 0 && m_selCol < SPECIAL_MAX_COLS &&
        m_cells[m_selRow][m_selCol]) {
        m_cells[m_selRow][m_selCol]->setProperty("selected", true);
        m_cells[m_selRow][m_selCol]->style()->unpolish(m_cells[m_selRow][m_selCol]);
        m_cells[m_selRow][m_selCol]->style()->polish(m_cells[m_selRow][m_selCol]);
    }

    /* 선택 열/행 헤더 강조 */
    if (m_selCol >= 0 && m_selCol < SPECIAL_MAX_COLS && m_colHeaders[m_selCol]) {
        m_colHeaders[m_selCol]->setProperty("highlight", true);
        m_colHeaders[m_selCol]->style()->unpolish(m_colHeaders[m_selCol]);
        m_colHeaders[m_selCol]->style()->polish(m_colHeaders[m_selCol]);
    }
    if (m_selRow >= 0 && m_selRow < SPECIAL_MAX_ROWS && m_rowHeaders[m_selRow]) {
        m_rowHeaders[m_selRow]->setProperty("highlight", true);
        m_rowHeaders[m_selRow]->style()->unpolish(m_rowHeaders[m_selRow]);
        m_rowHeaders[m_selRow]->style()->polish(m_rowHeaders[m_selRow]);
    }
}

bool UnimSpecialPopup::cellHasChar(int row, int col) const
{
    int idx = getCharIndex(row, col);
    return idx >= 0 && idx < m_totalCount;
}

int UnimSpecialPopup::getCharIndex(int row, int col) const
{
    return m_currentPage * SPECIAL_PAGE_SIZE + col * SPECIAL_MAX_ROWS + row;
}

void UnimSpecialPopup::selectCurrent()
{
    if (m_pendingHide) return;

    int idx = getCharIndex(m_selRow, m_selCol);
    if (idx < 0 || idx >= m_totalCount) return;

    QString selected = m_characters[idx];
    SP_DEBUG(QString::asprintf("특수문자 선택: '%s' (idx=%d)", qPrintable(selected), idx));

    /* 플래시 효과 */
    if (m_cells[m_selRow][m_selCol]) {
        m_cells[m_selRow][m_selCol]->setProperty("flash", true);
        m_cells[m_selRow][m_selCol]->style()->unpolish(m_cells[m_selRow][m_selCol]);
        m_cells[m_selRow][m_selCol]->style()->polish(m_cells[m_selRow][m_selCol]);
    }

    m_pendingHide = true;
    m_flashTimer->start(FLASH_DURATION_MS);

    if (m_callback) {
        m_callback(selected);
    }
}

void UnimSpecialPopup::mousePressEvent(QMouseEvent *event)
{
    if (m_pendingHide) return;

    QPoint pos = event->pos();

    /* 그리드 내 셀 클릭 감지 */
    for (int r = 0; r < SPECIAL_MAX_ROWS; r++) {
        for (int c = 0; c < m_cols; c++) {
            if (m_cells[r][c] && m_cells[r][c]->geometry().contains(pos)) {
                if (cellHasChar(r, c)) {
                    m_selRow = r;
                    m_selCol = c;
                    updateSelection();
                    selectCurrent();
                    return;
                }
            }
        }
    }
}

bool UnimSpecialPopup::handleKey(int key)
{
    if (m_pendingHide) return false;

    /* top_row 키: 열 점프 (물리 키 위치 기준 — QWERTY "qwertyuio") */
    {
        static const char physicalKeys[] = "qwertyuio";
        int qtKey = key;
        /* Qt::Key_Q ~ Qt::Key_O 매칭 */
        for (int i = 0; i < 9; i++) {
            if (i >= m_cols) break;
            int expectedKey = Qt::Key_Q + (physicalKeys[i] - 'q');
            /* Qt 키 코드는 대문자 기반 */
            if (physicalKeys[i] == 'q') expectedKey = Qt::Key_Q;
            else if (physicalKeys[i] == 'w') expectedKey = Qt::Key_W;
            else if (physicalKeys[i] == 'e') expectedKey = Qt::Key_E;
            else if (physicalKeys[i] == 'r') expectedKey = Qt::Key_R;
            else if (physicalKeys[i] == 't') expectedKey = Qt::Key_T;
            else if (physicalKeys[i] == 'y') expectedKey = Qt::Key_Y;
            else if (physicalKeys[i] == 'u') expectedKey = Qt::Key_U;
            else if (physicalKeys[i] == 'i') expectedKey = Qt::Key_I;
            else if (physicalKeys[i] == 'o') expectedKey = Qt::Key_O;

            if (qtKey == expectedKey) {
                if (cellHasChar(0, i)) {
                    m_selCol = i;
                    if (!cellHasChar(m_selRow, m_selCol)) {
                        m_selRow = 0;
                    }
                    updateSelection();
                    SP_DEBUG(QString::asprintf("열 점프: %c → 열 %d", m_topRow[i], i));
                }
                return true;
            }
        }
    }

    switch (key) {
    /* 숫자 1-9: 현재 열의 N번째 행 선택 */
    case Qt::Key_1: case Qt::Key_2: case Qt::Key_3:
    case Qt::Key_4: case Qt::Key_5: case Qt::Key_6:
    case Qt::Key_7: case Qt::Key_8: case Qt::Key_9:
    {
        int row = key - Qt::Key_1;
        if (cellHasChar(row, m_selCol)) {
            m_selRow = row;
            updateSelection();
            selectCurrent();
        }
        return true;
    }

    /* 화살표: 네비게이션 */
    case Qt::Key_Up:
        if (m_selRow > 0) {
            m_selRow--;
        } else {
            int lastRow = SPECIAL_MAX_ROWS - 1;
            while (lastRow > 0 && !cellHasChar(lastRow, m_selCol)) {
                lastRow--;
            }
            m_selRow = lastRow;
        }
        updateSelection();
        return true;

    case Qt::Key_Down:
        if (m_selRow < SPECIAL_MAX_ROWS - 1 && cellHasChar(m_selRow + 1, m_selCol)) {
            m_selRow++;
        } else {
            m_selRow = 0;
        }
        updateSelection();
        return true;

    case Qt::Key_Left:
        if (m_selCol > 0) {
            m_selCol--;
        } else {
            m_selCol = m_cols - 1;
        }
        if (!cellHasChar(m_selRow, m_selCol)) {
            m_selRow = 0;
        }
        updateSelection();
        return true;

    case Qt::Key_Right:
        if (m_selCol < m_cols - 1) {
            m_selCol++;
        } else {
            m_selCol = 0;
        }
        if (!cellHasChar(m_selRow, m_selCol)) {
            m_selRow = 0;
        }
        updateSelection();
        return true;

    /* 페이지 이동 */
    case Qt::Key_PageUp:
        if (m_currentPage > 0) {
            m_currentPage--;
            m_selRow = 0;
            m_selCol = 0;
            updateGrid();
        }
        return true;

    case Qt::Key_PageDown:
    case Qt::Key_Space:
        if (m_currentPage < m_totalPages - 1) {
            m_currentPage++;
            m_selRow = 0;
            m_selCol = 0;
            updateGrid();
        }
        return true;

    /* Enter: 선택 확정 */
    case Qt::Key_Return:
    case Qt::Key_Enter:
        selectCurrent();
        return true;

    /* Tab / Shift+Tab: 페이지 순환 */
    case Qt::Key_Tab:
        if (m_currentPage < m_totalPages - 1) {
            m_currentPage++;
        } else {
            m_currentPage = 0;
        }
        m_selCol = 0;
        m_selRow = 0;
        updateGrid();
        return true;

    case Qt::Key_Backtab:  /* Shift+Tab */
        if (m_currentPage > 0) {
            m_currentPage--;
        } else {
            m_currentPage = m_totalPages - 1;
        }
        m_selCol = 0;
        m_selRow = 0;
        updateGrid();
        return true;

    /* 수정자 키 무시 */
    case Qt::Key_Shift:
    case Qt::Key_Control:
    case Qt::Key_Alt:
    case Qt::Key_Meta:
    case Qt::Key_Super_L:
    case Qt::Key_Super_R:
        return true;

    default:
        return false;
    }
}
