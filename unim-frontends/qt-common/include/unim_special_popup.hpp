/**
 * UNIM 특수문자 팝업 (Qt 공통)
 *
 * 초성 기반 특수문자 선택을 위한 9x9 그리드 팝업 윈도우입니다.
 * Qt5/Qt6 모두에서 사용할 수 있도록 설계되었습니다.
 */

#ifndef UNIM_SPECIAL_POPUP_HPP
#define UNIM_SPECIAL_POPUP_HPP

#include <QWidget>
#include <QLabel>
#include <QGridLayout>
#include <QVBoxLayout>
#include <QKeyEvent>
#include <QStringList>
#include <QTimer>
#include <functional>

#include "unim.h"

/* 그리드 상수 */
#define SPECIAL_MAX_ROWS  9   /* 최대 행 수 (숫자 1-9) */
#define SPECIAL_MAX_COLS  9   /* 최대 열 수 (QWERTYUIO) */
#define SPECIAL_PAGE_SIZE (SPECIAL_MAX_ROWS * SPECIAL_MAX_COLS)  /* 한 페이지 81자 */
#define FLASH_DURATION_MS 120 /* 선택 플래시 지속 시간 */

/**
 * 특수문자 선택 콜백 타입
 */
using UnimSpecialSelectCallback = std::function<void(const QString &character)>;

/**
 * 특수문자 팝업 위젯
 */
class UnimSpecialPopup : public QWidget
{
    Q_OBJECT

public:
    explicit UnimSpecialPopup(QWidget *parent = nullptr);
    ~UnimSpecialPopup() override;

    /**
     * 팝업 표시
     */
    void showPopup(const QString &target,
                   const QStringList &characters,
                   const QString &topRow,
                   int x, int y, int cursorHeight,
                   UnimSpecialSelectCallback callback);

    /**
     * 팝업 숨기기
     */
    void hidePopup();

    /**
     * 키 처리 (팝업이 보이는 동안)
     * @return true이면 키가 처리됨
     */
    bool handleKey(int key);

protected:
    void mousePressEvent(QMouseEvent *event) override;

private:
    void updateGrid();
    void updateSelection();
    void selectCurrent();
    bool cellHasChar(int row, int col) const;
    int getCharIndex(int row, int col) const;
    void adjustPosition(int x, int y, int cursorHeight);

    /* 데이터 */
    QStringList m_characters;
    int m_totalCount;
    int m_currentPage;
    int m_totalPages;
    int m_rows;
    int m_cols;

    /* 선택 커서 */
    int m_selRow;
    int m_selCol;

    /* 열 헤더 레이블 (영문 키보드 상단 행) */
    char m_topRow[10];

    /* 위젯 */
    QVBoxLayout *m_mainLayout;
    QGridLayout *m_gridLayout;
    QLabel *m_cells[SPECIAL_MAX_ROWS][SPECIAL_MAX_COLS];
    QLabel *m_colHeaders[SPECIAL_MAX_COLS];
    QLabel *m_rowHeaders[SPECIAL_MAX_ROWS];
    QLabel *m_footerLabel;

    /* 콜백 */
    UnimSpecialSelectCallback m_callback;

    /* 플래시 타이머 */
    QTimer *m_flashTimer;
    bool m_pendingHide;

    /* C-API popup state */
    UnimPopupState *m_popupState = nullptr;
};

#endif // UNIM_SPECIAL_POPUP_HPP
