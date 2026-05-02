/**
 * UNIM 이모지 팝업 (Qt 공통)
 *
 * `ShowEmojiPopupV2` DBus 시그널을 받아 9×9 그리드 + 좌측 9 탭 (Recent + 8 카테고리)
 * 으로 이모지를 표시하는 팝업 위젯입니다. Qt5/Qt6 IM 모듈에서 공통 사용.
 *
 * 한자/특수문자 팝업과 다른 점 (PR #4 emoji overhaul, OPTION X engine-driven):
 * - 키 처리는 엔진이 담당 → IM 모듈은 시그널 수신만 (`handleKey` 없음)
 * - 페이지/셀 갱신은 `PopupNavigate` 시그널 → `navigatePopup()`
 * - 카테고리 전환은 `ShowEmojiPopupV2` 재발행 → `showPopup()` 재호출
 * - 즐겨찾기 대신 'Recent' (MRU) 캐시
 */

#ifndef UNIM_EMOJI_POPUP_HPP
#define UNIM_EMOJI_POPUP_HPP

#include <QWidget>
#include <QLabel>
#include <QToolButton>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QTimer>
#include <QStringList>
#include <functional>

/* 그리드 상수 — 한자/특수와 통일 */
#define EMOJI_MAX_ROWS  9
#define EMOJI_MAX_COLS  9
#define EMOJI_PAGE_SIZE (EMOJI_MAX_ROWS * EMOJI_MAX_COLS)  /* 81 */
#define EMOJI_TAB_COUNT 9                                   /* Recent + 8 카테고리 */
#define EMOJI_FLASH_DURATION_MS 120

/**
 * 좌측 9 탭 메타 (id, name_ko, name_en, count) — `ShowEmojiPopupV2` payload 의
 * `categories: a(sssu)` 와 1:1.
 */
struct UnimEmojiCategoryInfo {
    QString id;
    QString nameKo;
    QString nameEn;
    quint32 count;
};

/**
 * 이모지 커밋 콜백 타입
 */
using UnimEmojiCommitCallback = std::function<void(const QString &emoji)>;

/**
 * 이모지 팝업 위젯
 */
class UnimEmojiPopup : public QWidget
{
    Q_OBJECT

public:
    explicit UnimEmojiPopup(QWidget *parent = nullptr);
    ~UnimEmojiPopup() override;

    /**
     * 팝업 표시 (`ShowEmojiPopupV2` 시그널 핸들러)
     */
    void showPopup(const QString &targetCatId,
                   const QStringList &items,
                   const QString &topRow,
                   const QStringList &recent,
                   const QList<UnimEmojiCategoryInfo> &categories,
                   int x, int y, int cursorHeight,
                   UnimEmojiCommitCallback callback);

    /**
     * 팝업 숨기기
     */
    void hidePopup();

    /**
     * `PopupNavigate` 시그널 처리 — 같은 카테고리 내 페이지/셀 갱신.
     */
    void navigatePopup(int page, int totalPages, int selected,
                        int rows, int cols, int selRow, int selCol);

    /**
     * 'Recent' 탭 캐시 갱신 (별도 시그널 갱신용 — 보통 ShowEmojiPopupV2 재발행으로
     * 처리되므로 호출 빈도가 낮다).
     */
    void setRecent(const QStringList &emojis);

protected:
    void mousePressEvent(QMouseEvent *event) override;

private:
    void rebuildGrid();
    void updateSelection();
    void emitSelect(int col, int row);
    void adjustPosition(int x, int y, int cursorHeight);
    QString tabLabelFor(const QString &catId) const;

    /* 데이터 */
    QString m_currentCatId;
    QStringList m_items;
    QStringList m_recent;
    QList<UnimEmojiCategoryInfo> m_categories;
    char m_topRow[10];
    int m_currentPage;
    int m_totalPages;
    int m_selRow;
    int m_selCol;

    /* 위젯 */
    QVBoxLayout *m_mainLayout;
    QHBoxLayout *m_bodyLayout;
    QVBoxLayout *m_tabLayout;
    QGridLayout *m_gridLayout;
    QLabel *m_headerLabel;
    QLabel *m_footerLabel;
    QLabel *m_cells[EMOJI_MAX_COLS][EMOJI_MAX_ROWS];
    QLabel *m_colHeaders[EMOJI_MAX_COLS];
    QLabel *m_rowHeaders[EMOJI_MAX_ROWS];
    QToolButton *m_tabButtons[EMOJI_TAB_COUNT];

    /* 콜백 */
    UnimEmojiCommitCallback m_callback;

    /* 플래시 타이머 */
    QTimer *m_flashTimer;
    bool m_pendingHide;
};

#endif // UNIM_EMOJI_POPUP_HPP
