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
#include <QDir>
#include <QFile>
#include <QTextStream>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <unistd.h>

/* 디버그 로깅 */
static bool sp_debug_enabled = false;
static bool sp_debug_checked = false;

static QString unimLogSanitize(const QString &raw)
{
    QString out;
    out.reserve(raw.size());
    for (int i = 0; i < raw.size(); ++i) {
        QChar c = raw.at(i);
        ushort u = c.unicode();
        const bool ok = (u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z')
                     || (u >= '0' && u <= '9') || u == '-' || u == '_';
        out.append(ok ? c : QChar('-'));
    }
    return out;
}

static QString unimLogProcessName()
{
    QFile f(QStringLiteral("/proc/self/comm"));
    if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QByteArray data = f.readAll().trimmed();
        f.close();
        if (!data.isEmpty()) {
            QString s = unimLogSanitize(QString::fromLocal8Bit(data));
            if (!s.isEmpty()) return s;
        }
    }
    return QStringLiteral("unknown");
}

/* 윈도우 세션·앱(프로세스)별 로그 파일 경로:
 *   ~/.unim-log/{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log
 *   session-tag 우선순위: XDG_SESSION_ID > WAYLAND_DISPLAY > DISPLAY.
 *   progname/pid 는 호스트 프로세스 (Qt 팝업은 호스트 앱 안에서 동작).
 */
static QString unimLogPath()
{
    QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    QString dir = home + "/.unim-log";
    QDir().mkpath(dir);

    QByteArray xdg = qgetenv("XDG_SESSION_ID");
    QByteArray wl = qgetenv("WAYLAND_DISPLAY");
    QByteArray x11 = qgetenv("DISPLAY");
    QString rawTag;
    if (!xdg.isEmpty()) rawTag = QStringLiteral("xdg-") + QString::fromLocal8Bit(xdg);
    else if (!wl.isEmpty()) rawTag = QStringLiteral("wl-") + QString::fromLocal8Bit(wl);
    else if (!x11.isEmpty()) rawTag = QStringLiteral("x11-") + QString::fromLocal8Bit(x11);
    else rawTag = QStringLiteral("unknown");

    const QString tag = unimLogSanitize(rawTag);
    const QString date = QDateTime::currentDateTime().toString("yyyy-MM-dd");
    const QString progname = unimLogProcessName();
    const long long pid = static_cast<long long>(getpid());

    return dir + "/" + tag + "_" + date + "_" + progname + "-" + QString::number(pid) + ".log";
}

static void sp_log(const char *module, const QString &message)
{
    if (!sp_debug_enabled) return;

    QString timestamp = QDateTime::currentDateTime().toString("yyyy/MM/dd hh:mm:ss");
    QString logLine = QString("[%1] - [%2] - %3").arg(timestamp, module, message);
    qDebug().noquote() << logLine;

    QString logPath = unimLogPath();
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
    , m_footerBox(nullptr)
    , m_footerLabel(nullptr)
    , m_prevPageBtn(nullptr)
    , m_nextPageBtn(nullptr)
    , m_flashTimer(nullptr)
    , m_pendingHide(false)
{
    sp_check_debug();

    /* DPI 스케일 팩터 계산 (96 DPI 기준) */
    qreal scaleFactor = 1.0;
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        scaleFactor = screen->logicalDotsPerInch() / 96.0;
        if (scaleFactor < 1.0) scaleFactor = 1.0;
    }
    int cellSize = qRound(30 * scaleFactor);
    int fontSize = qRound(16 * scaleFactor);
    int headerFontSize = qRound(11 * scaleFactor);
    int footerFontSize = qRound(12 * scaleFactor);
    int widgetPad = qRound(4 * scaleFactor);
    int cellPadV = qRound(2 * scaleFactor);
    int cellPadH = qRound(4 * scaleFactor);

    /* Catppuccin Mocha 스타일 (DPI 스케일 적용) */
    setStyleSheet(QString(
        "UnimSpecialPopup {"
        "  background-color: rgba(30, 30, 46, 242);"
        "  border: 1px solid rgba(255, 255, 255, 38);"
        "  border-radius: 12px;"
        "  padding: %1px;"
        "}"
        "QLabel {"
        "  color: #cdd6f4;"
        "  padding: %2px %3px;"
        "  font-size: %4px;"
        "  min-width: %5px;"
        "  min-height: %5px;"
        "}"
        "QLabel[cellType=\"header\"] {"
        "  color: #f9e2af;"
        "  font-size: %6px;"
        "  font-weight: bold;"
        "}"
        "QLabel[cellType=\"cell\"] {"
        "  color: #cdd6f4;"
        "  font-size: %4px;"
        "}"
        "QLabel[cellType=\"cell\"]:hover {"
        "  background-color: rgba(255, 255, 255, 13);"
        "  border-radius: 6px;"
        "}"
        "QLabel[selected=\"true\"] {"
        "  background-color: rgba(166, 227, 161, 64);"
        "  color: #cdd6f4;"
        "  font-weight: bold;"
        "  border-radius: 6px;"
        "}"
        "QLabel[selected=\"true\"]:hover {"
        "  background-color: rgba(166, 227, 161, 77);"
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
        "  font-size: %7px;"
        "  padding: %2px %3px;"
        "}"
        /* 마우스 페이지 이동 ◀/▶ (Phase 5)
         * hit-target 28×28px — WCAG 2.5.5 (24×24) 초과, 셀과 시각 비례 */
        "QPushButton#prevPageBtn, QPushButton#nextPageBtn {"
        "  color: #7f849c;"
        "  background: transparent;"
        "  border: none;"
        "  font-size: %7px;"
        "  min-width: 28px;"
        "  min-height: 28px;"
        "  padding: 4px 10px;"
        "  border-radius: 4px;"
        "}"
        "QPushButton#prevPageBtn:hover, QPushButton#nextPageBtn:hover {"
        "  color: #89b4fa;"
        "  background-color: rgba(137, 180, 250, 51);"
        "}"
        "QPushButton#prevPageBtn:pressed, QPushButton#nextPageBtn:pressed {"
        "  background-color: rgba(137, 180, 250, 89);"
        "}"
    ).arg(widgetPad).arg(cellPadV).arg(cellPadH).arg(fontSize)
     .arg(cellSize).arg(headerFontSize).arg(footerFontSize));

    m_mainLayout = new QVBoxLayout(this);
    m_mainLayout->setContentsMargins(4, 4, 4, 4);
    m_mainLayout->setSpacing(1);

    /* 그리드 레이아웃 */
    m_gridLayout = new QGridLayout();
    m_gridLayout->setSpacing(1);
    m_mainLayout->addLayout(m_gridLayout);

    /* 푸터: [◀] [페이지 라벨] [▶] (Phase 5) — 단일 페이지면 footer_box hide */
    m_footerBox = new QWidget(this);
    m_footerBox->setObjectName("specialFooterBox");
    QHBoxLayout *footerLayout = new QHBoxLayout(m_footerBox);
    footerLayout->setContentsMargins(0, 0, 0, 0);
    footerLayout->setSpacing(4);

    m_prevPageBtn = new QPushButton(QStringLiteral("\xE2\x97\x80"), m_footerBox); /* ◀ */
    m_prevPageBtn->setObjectName("prevPageBtn");
    m_prevPageBtn->setFlat(true);
    m_prevPageBtn->setFocusPolicy(Qt::NoFocus);
    m_prevPageBtn->setCursor(Qt::PointingHandCursor);
    /* tr() — Qt 번역 시스템. .ts 미배포 시 영문 fallback. msgid 표준 일관성. */
    m_prevPageBtn->setToolTip(QObject::tr("Previous page"));
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
    m_nextPageBtn->setToolTip(QObject::tr("Next page"));
    QObject::connect(m_nextPageBtn, &QPushButton::clicked, this, [this]() {
        if (m_pageChangeCallback) m_pageChangeCallback(1);
    });
    footerLayout->addWidget(m_nextPageBtn, 0);

    m_footerBox->setVisible(false);
    m_mainLayout->addWidget(m_footerBox);

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
    if (m_popupState) {
        unim_popup_free(m_popupState);
        m_popupState = nullptr;
    }
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

    /* Build arrays for C-API PopupState */
    {
        int count = m_totalCount;
        std::vector<QByteArray> charBytes(count);
        std::vector<const uint8_t*> charPtrs(count);
        std::vector<size_t> charLens(count);
        for (int i = 0; i < count; i++) {
            charBytes[i] = m_characters[i].toUtf8();
            charPtrs[i] = (const uint8_t*)charBytes[i].constData();
            charLens[i] = charBytes[i].size();
        }
        QByteArray targetBytes = target.toUtf8();
        QByteArray topRowBytes = QString(topRow).toUtf8();
        if (m_popupState) unim_popup_free(m_popupState);
        m_popupState = unim_popup_new_special(
            (const uint8_t*)targetBytes.constData(), targetBytes.size(),
            charPtrs.data(), charLens.data(), count,
            (const uint8_t*)topRowBytes.constData(), topRowBytes.size()
        );
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

    /* 페이지 표시: 단일 페이지면 footer_box 통째 hide */
    if (m_totalPages > 1) {
        m_footerLabel->setText(QString("%1 / %2").arg(m_currentPage + 1).arg(m_totalPages));
        m_footerBox->setVisible(true);
    } else {
        m_footerBox->setVisible(false);
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
    if (!m_popupState) return false;

    /* Convert Qt key to C-API PopupKey */
    uint32_t popupKey = unim_popup_key_from_qt(key);

    /* Delegate to C-API */
    UnimPopupKeyResult result = unim_popup_handle_key(m_popupState, popupKey);

    switch (result.kind) {
    case UNIM_POPUP_RESULT_SELECT:
    {
        /* Sync state from C-API before selecting */
        m_selRow = unim_popup_get_sel_row(m_popupState);
        m_selCol = unim_popup_get_sel_col(m_popupState);
        m_currentPage = unim_popup_get_current_page(m_popupState);
        updateSelection();
        selectCurrent();
        return true;
    }

    case UNIM_POPUP_RESULT_CANCEL:
        hidePopup();
        return false;

    case UNIM_POPUP_RESULT_UPDATED:
    {
        /* Sync all state from C-API */
        int oldPage = m_currentPage;
        m_selRow = unim_popup_get_sel_row(m_popupState);
        m_selCol = unim_popup_get_sel_col(m_popupState);
        m_currentPage = unim_popup_get_current_page(m_popupState);
        m_rows = unim_popup_get_rows(m_popupState);
        m_cols = unim_popup_get_cols(m_popupState);

        if (m_currentPage != oldPage) {
            updateGrid();
        } else {
            updateSelection();
        }
        return true;
    }

    case UNIM_POPUP_RESULT_CONSUMED:
        return true;

    case UNIM_POPUP_RESULT_NOT_HANDLED:
    default:
        return false;
    }
}

void UnimSpecialPopup::setPageChangeCallback(PageChangeCallback callback)
{
    m_pageChangeCallback = std::move(callback);
}
