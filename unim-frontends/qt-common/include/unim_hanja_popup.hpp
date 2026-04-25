/**
 * UNIM 한자 후보 팝업 (Qt 공통)
 *
 * 한자 변환 시 후보 목록을 표시하는 팝업 윈도우입니다.
 * Qt5/Qt6 모두에서 사용할 수 있도록 설계되었습니다.
 */

#ifndef UNIM_HANJA_POPUP_HPP
#define UNIM_HANJA_POPUP_HPP

#include <QWidget>
#include <QLabel>
#include <QVBoxLayout>
#include <QKeyEvent>
#include <QScreen>
#include <QGuiApplication>
#include <functional>
#include "unim_dbus_client.hpp"
#include "unim.h"

/* 한 페이지에 표시할 최대 후보 수 */
#define MAX_VISIBLE_CANDIDATES 9

/**
 * 한자 선택 콜백 타입
 */
using UnimHanjaSelectCallback = std::function<void(const QString &hanja)>;

/**
 * 한자 후보 팝업 위젯
 */
class UnimHanjaPopup : public QWidget
{
    Q_OBJECT

public:
    explicit UnimHanjaPopup(QWidget *parent = nullptr);
    ~UnimHanjaPopup() override;

    /**
     * 팝업 표시
     */
    void showPopup(const QString &target,
                   const QList<UnimHanjaCandidate> &candidates,
                   int x, int y, int cursorHeight,
                   UnimHanjaSelectCallback callback);

    /**
     * 팝업 숨기기
     */
    void hidePopup();

    /**
     * 키 처리 (팝업이 보이는 동안)
     * @return true이면 키가 처리됨
     */
    bool handleKey(int key);

    /**
     * 즐겨찾기 상태 일괄 반영 (초기 fetch 결과)
     */
    void setBookmarkStates(const QList<bool> &states);

    /**
     * 특정 후보의 즐겨찾기 상태 갱신 (HanjaBookmarkChanged 시그널)
     */
    void setBookmark(quint32 globalIndex, bool bookmarked);

    /**
     * Space 토글 콜백 (프런트엔드 → DBus 호출 연결용)
     */
    using ToggleBookmarkCallback = std::function<void(quint32 globalIndex)>;
    void setToggleBookmarkCallback(ToggleBookmarkCallback callback);

protected:
    void mousePressEvent(QMouseEvent *event) override;

private:
    void updateList();
    void selectCandidate(int index);
    void nextPage();
    void prevPage();
    void adjustPosition(int x, int y, int cursorHeight);

    QList<UnimHanjaCandidate> m_candidates;
    QList<bool> m_bookmarks;
    UnimHanjaSelectCallback m_callback;
    ToggleBookmarkCallback m_toggleBookmarkCallback;

    QVBoxLayout *m_layout;
    QLabel *m_labels[MAX_VISIBLE_CANDIDATES];
    QLabel *m_pageLabel;

    int m_currentPage;
    int m_selectedIndex;

    UnimPopupState *m_popupState = nullptr;
};

#endif // UNIM_HANJA_POPUP_HPP
